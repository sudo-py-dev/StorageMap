//! Which entries a scan skips or counts.
//!
//! Patterns are globs: `*` matches any run of characters (including `/`) and
//! `?` matches one character. A pattern without `/` matches an entry's name
//! wherever it is (`node_modules`, `*.iso`); a pattern with `/` matches its
//! path relative to the scan root, written with `/` (`build/cache`,
//! `*/target`). Matching ignores case on Windows.

/// Exclude and include rules for a scan. Empty means "scan everything".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ScanFilter {
    /// Files and folders to leave out entirely; excluded folders are never
    /// opened.
    pub exclude: Vec<String>,
    /// When not empty, only files matching one of these are counted.
    /// Folders are still walked.
    pub include_files: Vec<String>,
}

impl ScanFilter {
    pub fn is_empty(&self) -> bool {
        self.exclude.is_empty() && self.include_files.is_empty()
    }

    pub(crate) fn compile(&self) -> Matcher {
        let split = |patterns: &[String]| {
            let mut names = Vec::new();
            let mut paths = Vec::new();
            for p in patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
                let p = normalise(p);
                match p.strip_prefix('/') {
                    Some(anchored) => paths.push(anchored.to_owned()),
                    None if p.contains('/') => paths.push(p),
                    None => names.push(p),
                }
            }
            (names, paths)
        };
        let (exclude_names, exclude_paths) = split(&self.exclude);
        let (include_names, include_paths) = split(&self.include_files);
        Matcher {
            exclude_names,
            exclude_paths,
            include_names,
            include_paths,
        }
    }
}

/// A [`ScanFilter`] prepared for matching.
#[derive(Debug, Default)]
pub(crate) struct Matcher {
    exclude_names: Vec<String>,
    exclude_paths: Vec<String>,
    include_names: Vec<String>,
    include_paths: Vec<String>,
}

impl Matcher {
    pub fn is_empty(&self) -> bool {
        self.exclude_names.is_empty()
            && self.exclude_paths.is_empty()
            && self.include_names.is_empty()
            && self.include_paths.is_empty()
    }

    /// Whether [`Matcher::skips`] needs the entry's relative directory.
    pub fn needs_path(&self) -> bool {
        !self.exclude_paths.is_empty() || !self.include_paths.is_empty()
    }

    /// Whether to leave out the entry `name` inside `rel_dir` (its folder,
    /// relative to the scan root, `/`-separated; empty for the root).
    pub fn skips(&self, rel_dir: &str, name: &[u8], is_dir: bool) -> bool {
        if self.is_empty() {
            return false;
        }
        let name = normalise(&String::from_utf8_lossy(name));
        let rel = || {
            if rel_dir.is_empty() {
                name.clone()
            } else {
                format!("{}/{name}", normalise(rel_dir))
            }
        };
        let any = |names: &[String], paths: &[String]| {
            names.iter().any(|p| glob(p, &name)) || {
                !paths.is_empty() && {
                    let rel = rel();
                    paths.iter().any(|p| glob(p, &rel))
                }
            }
        };
        if any(&self.exclude_names, &self.exclude_paths) {
            return true;
        }
        let includes = !self.include_names.is_empty() || !self.include_paths.is_empty();
        !is_dir && includes && !any(&self.include_names, &self.include_paths)
    }
}

/// `/` separators, and lower case where the filesystem ignores case.
fn normalise(s: &str) -> String {
    let s = s.replace('\\', "/");
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// Wildcard match of `text` against `pattern` (`*` and `?`), iterative with
/// single backtracking, so it is linear for typical patterns.
pub(crate) fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(exclude: &[&str], include: &[&str]) -> Matcher {
        ScanFilter {
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            include_files: include.iter().map(|s| s.to_string()).collect(),
        }
        .compile()
    }

    #[test]
    fn globs() {
        assert!(glob("*.iso", "disk.iso"));
        assert!(!glob("*.iso", "disk.iso.part"));
        assert!(glob("a?c", "abc"));
        assert!(glob("*/target", "x/y/target"));
        assert!(glob("*", ""));
        assert!(!glob("a", ""));
        assert!(glob("*a*b*", "xxaxxbxx"));
    }

    #[test]
    fn names_match_anywhere_and_paths_from_the_root() {
        let m = filter(&["node_modules", "build/cache", "/top"], &[]);
        assert!(m.skips("deep/inside", b"node_modules", true));
        assert!(m.skips("build", b"cache", true));
        assert!(!m.skips("x/build", b"cache", true));
        assert!(m.skips("", b"top", true));
        assert!(!m.skips("a", b"top", true));
        assert!(!m.skips("", b"src", true));
    }

    #[test]
    fn includes_limit_files_but_not_folders() {
        let m = filter(&["*.tmp"], &["*.mp4", "*.mkv"]);
        assert!(!m.skips("", b"video.mp4", false));
        assert!(m.skips("", b"notes.txt", false));
        assert!(!m.skips("", b"folder", true));
        assert!(m.skips("", b"x.tmp", false));
    }

    #[test]
    fn empty_filter_skips_nothing() {
        let m = ScanFilter::default().compile();
        assert!(m.is_empty());
        assert!(!m.skips("a", b"b", false));
    }
}
