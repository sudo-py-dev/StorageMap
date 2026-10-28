//! Self-update from GitHub Releases.
//!
//! The check only asks GitHub for the latest release — no identifiers are
//! sent beyond a User-Agent. Downloads are verified against the release's
//! `SHA256SUMS` before anything runs; an asset without a matching checksum is
//! never installed.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const REPO: &str = "sudo-py-dev/StorageMap";
pub const RELEASES_PAGE: &str = "https://github.com/sudo-py-dev/StorageMap/releases";
const LATEST_API: &str = "https://api.github.com/repos/sudo-py-dev/StorageMap/releases/latest";
/// Assets must come from this repository's release downloads.
const DOWNLOAD_PREFIX: &str = "https://github.com/sudo-py-dev/StorageMap/releases/download/";
const CHECKSUMS_ASSET: &str = "SHA256SUMS";
/// Temp folders created for downloads start with this.
const TEMP_PREFIX: &str = "storagemap-update-";
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CHECKSUMS_BYTES: u64 = 64 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Minimum time between automatic checks.
pub const CHECK_INTERVAL_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response from GitHub: {0}")]
    BadResponse(String),
    #[error("the release has no package for this system")]
    NoAsset,
    #[error("the release has no checksum for {0}")]
    NoChecksum(String),
    #[error("download is corrupt (checksum mismatch)")]
    ChecksumMismatch,
    #[error("download refused: unexpected location {0}")]
    UntrustedUrl(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<ureq::Error> for UpdateError {
    fn from(e: ureq::Error) -> Self {
        UpdateError::Network(e.to_string())
    }
}

/// How this installation is updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Run the Inno Setup installer silently; it relaunches the app.
    WindowsInstaller,
    /// Install the `.deb` with the system package manager.
    Deb,
    /// Not a packaged install (e.g. built from source): open the release page.
    Manual,
}

impl Method {
    pub fn current() -> Method {
        if cfg!(windows) {
            Method::WindowsInstaller
        } else if std::env::current_exe().is_ok_and(|p| p.starts_with("/usr/bin")) {
            Method::Deb
        } else {
            Method::Manual
        }
    }

    fn matches(self, asset: &str) -> bool {
        match self {
            Method::WindowsInstaller => asset.ends_with("-windows-x64-setup.exe"),
            Method::Deb => asset.ends_with("_amd64.deb"),
            Method::Manual => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Asset {
    name: String,
    url: String,
}

/// A newer release than the running version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub page: String,
    pub notes: String,
    package: Option<Asset>,
    checksums: Option<Asset>,
}

impl Release {
    /// Whether this release can be installed automatically here.
    pub fn installable(&self) -> bool {
        self.package.is_some() && self.checksums.is_some()
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
}

pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

/// Interprets the GitHub "latest release" JSON. `None` when not newer.
pub fn parse_release(
    json: &str,
    current: &Version,
    method: Method,
) -> Result<Option<Release>, UpdateError> {
    let api: ApiRelease =
        serde_json::from_str(json).map_err(|e| UpdateError::BadResponse(e.to_string()))?;
    if api.draft || api.prerelease {
        return Ok(None);
    }
    let version = Version::parse(api.tag_name.trim_start_matches('v'))
        .map_err(|e| UpdateError::BadResponse(format!("tag {}: {e}", api.tag_name)))?;
    if version <= *current {
        return Ok(None);
    }
    let find = |pred: &dyn Fn(&str) -> bool| {
        api.assets.iter().find(|a| pred(&a.name)).map(|a| Asset {
            name: a.name.clone(),
            url: a.browser_download_url.clone(),
        })
    };
    Ok(Some(Release {
        version,
        page: api.html_url,
        notes: api.body.unwrap_or_default(),
        package: find(&|n| method.matches(n)),
        checksums: find(&|n| n == CHECKSUMS_ASSET),
    }))
}

/// Expected SHA-256 of `name` from a `sha256sum`-style file.
pub fn parse_checksum(text: &str, name: &str) -> Option<[u8; 32]> {
    text.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| decode_hex(hash))?
    })
}

fn decode_hex(s: &str) -> Option<[u8; 32]> {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out[i] = (hi * 16 + lo) as u8;
    }
    Some(out)
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .https_only(true)
        .user_agent(concat!("StorageMap/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Asks GitHub for the latest release. Blocking.
pub fn check(method: Method) -> Result<Option<Release>, UpdateError> {
    let mut response = agent(CHECK_TIMEOUT)
        .get(LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .call()?;
    let json = response.body_mut().read_to_string()?;
    parse_release(&json, &current_version(), method)
}

#[derive(Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: AtomicU64,
    pub total: AtomicU64,
}

/// Downloads and verifies the package. Blocking.
pub fn download(release: &Release, progress: &DownloadProgress) -> Result<PathBuf, UpdateError> {
    let package = release.package.as_ref().ok_or(UpdateError::NoAsset)?;
    let sums = release
        .checksums
        .as_ref()
        .ok_or_else(|| UpdateError::NoChecksum(package.name.clone()))?;
    for url in [&package.url, &sums.url] {
        if !url.starts_with(DOWNLOAD_PREFIX) {
            return Err(UpdateError::UntrustedUrl(url.clone()));
        }
    }
    let agent = agent(DOWNLOAD_TIMEOUT);
    let sums_text = agent
        .get(&sums.url)
        .call()?
        .into_body()
        .into_with_config()
        .limit(MAX_CHECKSUMS_BYTES)
        .read_to_string()?;
    let expected = parse_checksum(&sums_text, &package.name)
        .ok_or_else(|| UpdateError::NoChecksum(package.name.clone()))?;

    let dir = std::env::temp_dir().join(format!("{TEMP_PREFIX}{}", release.version));
    fs::create_dir_all(&dir)?;
    let target = dir.join(safe_file_name(&package.name));
    let response = agent.get(&package.url).call()?;
    if let Some(len) = response.body().content_length() {
        progress.total.store(len, Relaxed);
    }
    let mut reader = response
        .into_body()
        .into_with_config()
        .limit(MAX_INSTALLER_BYTES)
        .reader();
    let mut file = fs::File::create(&target)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        progress.downloaded.fetch_add(n as u64, Relaxed);
    }
    file.sync_all()?;
    drop(file);
    if hasher.finalize().as_slice() != expected {
        let _ = fs::remove_file(&target);
        return Err(UpdateError::ChecksumMismatch);
    }
    Ok(target)
}

/// Keeps only `[A-Za-z0-9._-]` so a hostile asset name cannot escape `dir`.
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_start_matches('.');
    if trimmed.is_empty() {
        "update".into()
    } else {
        trimmed.into()
    }
}

/// What happened after starting the install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    /// The installer runs now and relaunches the app: quit immediately.
    QuitNow,
    /// Installed; takes effect after restarting the app.
    RestartToFinish,
    /// Handed to the system's package installer for the user to confirm.
    HandedOff,
}

/// Starts a fresh copy of the app (after an update was installed in place).
pub fn relaunch() -> io::Result<()> {
    Command::new(std::env::current_exe()?).spawn().map(drop)
}

/// Removes installers left in the temp folder by earlier updates.
pub fn cleanup_downloads() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX)
            && let Err(err) = fs::remove_dir_all(entry.path())
        {
            tracing::debug!(%err, path = %entry.path().display(), "cannot remove old update download");
        }
    }
}

/// GitHub release notes (Markdown) as readable plain text.
pub fn plain_notes(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let line = line.trim_end();
            let line = line.trim_start_matches('#').trim_start();
            let line = match line.strip_prefix("* ").or_else(|| line.strip_prefix("- ")) {
                Some(item) => format!("• {item}"),
                None => line.to_owned(),
            };
            strip_links(&line.replace("**", "").replace('`', ""))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// `[text](url)` → `text`; bare URLs are kept.
fn strip_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|i| open + i) else {
            break;
        };
        let Some(end) = rest[close..].find(')').map(|i| close + i) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn install(package: &Path, method: Method) -> Result<Installed, UpdateError> {
    match method {
        Method::WindowsInstaller => {
            Command::new(package)
                .args([
                    "/SILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/CLOSEAPPLICATIONS",
                ])
                .spawn()?;
            Ok(Installed::QuitNow)
        }
        Method::Deb => {
            let status = Command::new("pkexec")
                .args(["apt-get", "install", "-y", "--allow-downgrades"])
                .arg(package)
                .status();
            match status {
                Ok(s) if s.success() => Ok(Installed::RestartToFinish),
                _ => {
                    open::that_detached(package)?;
                    Ok(Installed::HandedOff)
                }
            }
        }
        Method::Manual => {
            open::that_detached(RELEASES_PAGE)?;
            Ok(Installed::HandedOff)
        }
    }
}

/// Update state machine driven by the UI; work happens on threads.
#[derive(Default)]
pub enum Updater {
    #[default]
    Idle,
    Checking(Receiver<Result<Option<Release>, String>>),
    UpToDate,
    Available(Release),
    /// Downloading, verifying, then installing — all off the UI thread, so
    /// a password prompt from the package manager never freezes the app.
    Downloading {
        progress: Arc<DownloadProgress>,
        rx: Receiver<Result<Installed, String>>,
    },
    Failed(String),
    Finished(Installed),
}

impl Updater {
    pub fn available(&self) -> Option<&Release> {
        match self {
            Updater::Available(r) => Some(r),
            _ => None,
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, Updater::Checking(_) | Updater::Downloading { .. })
    }

    pub fn start_check(&mut self, ctx: &egui::Context) {
        if self.is_busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        spawn("storagemap-update-check", move || {
            let _ = tx.send(check(Method::current()).map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
        *self = Updater::Checking(rx);
    }

    /// Downloads, verifies and installs the available release.
    pub fn start_install(&mut self, ctx: &egui::Context) {
        let Updater::Available(release) = self else {
            return;
        };
        let release = release.clone();
        let progress = Arc::new(DownloadProgress::default());
        let (tx, rx) = mpsc::channel();
        let (thread_progress, ctx) = (Arc::clone(&progress), ctx.clone());
        spawn("storagemap-update-install", move || {
            let method = Method::current();
            let result = if release.installable() {
                download(&release, &thread_progress).and_then(|package| install(&package, method))
            } else {
                install(Path::new(""), Method::Manual)
            };
            let _ = tx.send(result.map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
        *self = Updater::Downloading { progress, rx };
    }

    /// Advances background work; call every frame.
    pub fn poll(&mut self) {
        let next = match self {
            Updater::Checking(rx) => match rx.try_recv() {
                Ok(Ok(Some(release))) => Updater::Available(release),
                Ok(Ok(None)) => Updater::UpToDate,
                Ok(Err(e)) => Updater::Failed(e),
                Err(TryRecvError::Disconnected) => Updater::Failed("update check stopped".into()),
                Err(TryRecvError::Empty) => return,
            },
            Updater::Downloading { rx, .. } => match rx.try_recv() {
                Ok(Ok(installed)) => Updater::Finished(installed),
                Ok(Err(e)) => Updater::Failed(e),
                Err(TryRecvError::Disconnected) => Updater::Failed("download stopped".into()),
                Err(TryRecvError::Empty) => return,
            },
            _ => return,
        };
        *self = next;
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    if let Err(err) = std::thread::Builder::new().name(name.into()).spawn(f) {
        tracing::warn!(%err, "cannot start update thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
        "tag_name": "v9.1.0", "html_url": "https://github.com/sudo-py-dev/StorageMap/releases/tag/v9.1.0",
        "body": "Notes", "draft": false, "prerelease": false,
        "assets": [
            {"name": "StorageMap-9.1.0-windows-x64-setup.exe", "browser_download_url": "https://github.com/sudo-py-dev/StorageMap/releases/download/v9.1.0/StorageMap-9.1.0-windows-x64-setup.exe"},
            {"name": "storagemap_9.1.0-1_amd64.deb", "browser_download_url": "https://github.com/sudo-py-dev/StorageMap/releases/download/v9.1.0/storagemap_9.1.0-1_amd64.deb"},
            {"name": "SHA256SUMS", "browser_download_url": "https://github.com/sudo-py-dev/StorageMap/releases/download/v9.1.0/SHA256SUMS"}
        ]}"#;

    #[test]
    fn newer_release_with_matching_asset() {
        let r = parse_release(JSON, &Version::new(0, 1, 0), Method::Deb)
            .expect("parse")
            .expect("newer");
        assert_eq!(r.version, Version::new(9, 1, 0));
        assert_eq!(
            r.package.as_ref().map(|a| a.name.as_str()),
            Some("storagemap_9.1.0-1_amd64.deb")
        );
        assert!(r.installable());
        let w = parse_release(JSON, &Version::new(0, 1, 0), Method::WindowsInstaller)
            .expect("parse")
            .expect("newer");
        assert!(w.package.expect("exe").name.ends_with("setup.exe"));
        assert!(
            !parse_release(JSON, &Version::new(0, 1, 0), Method::Manual)
                .expect("parse")
                .expect("newer")
                .installable()
        );
    }

    #[test]
    fn same_or_older_is_not_an_update() {
        assert_eq!(
            parse_release(JSON, &Version::new(9, 1, 0), Method::Deb).expect("parse"),
            None
        );
        assert_eq!(
            parse_release(JSON, &Version::new(10, 0, 0), Method::Deb).expect("parse"),
            None
        );
    }

    #[test]
    fn prereleases_and_garbage() {
        let pre = JSON.replace("\"prerelease\": false", "\"prerelease\": true");
        assert_eq!(
            parse_release(&pre, &Version::new(0, 1, 0), Method::Deb).expect("parse"),
            None
        );
        assert!(parse_release("{}", &Version::new(0, 1, 0), Method::Deb).is_err());
        assert!(
            parse_release(
                &JSON.replace("v9.1.0\"", "latest\""),
                &Version::new(0, 1, 0),
                Method::Deb
            )
            .is_err()
        );
    }

    #[test]
    fn checksum_lines() {
        let hash = "ab".repeat(32);
        let text = format!(
            "{hash}  storagemap_9.1.0-1_amd64.deb\n{}  *other.exe\n",
            "cd".repeat(32)
        );
        assert_eq!(
            parse_checksum(&text, "storagemap_9.1.0-1_amd64.deb"),
            Some([0xab; 32])
        );
        assert_eq!(parse_checksum(&text, "other.exe"), Some([0xcd; 32]));
        assert_eq!(parse_checksum(&text, "missing"), None);
        assert_eq!(parse_checksum("zz  x", "x"), None);
    }

    #[test]
    fn release_notes_become_plain_text() {
        let md =
            "## What's Changed\n* **Faster** scans by @dev in [#12](https://x/12)\n- `fix` crash\n";
        assert_eq!(
            plain_notes(md),
            "What's Changed\n• Faster scans by @dev in #12\n• fix crash"
        );
        assert_eq!(strip_links("see [docs](u) and [x](y)!"), "see docs and x!");
        assert_eq!(strip_links("unclosed [link"), "unclosed [link");
    }

    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(safe_file_name("../../evil.exe"), "_.._evil.exe");
        assert_eq!(
            safe_file_name("StorageMap-1.0-setup.exe"),
            "StorageMap-1.0-setup.exe"
        );
        assert_eq!(safe_file_name("..."), "update");
    }
}
