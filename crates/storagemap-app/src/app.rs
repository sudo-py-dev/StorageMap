//! Application state and the action dispatcher.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Ui;
use storagemap_core::snapshot::{self, Comparison, DEFAULT_MIN_FILE_SIZE, Snapshot};
use storagemap_core::{NodeId, ScanOptions, volumes};

use crate::fmt;
use crate::i18n::{I18n, Lang};
use crate::platform;
use crate::scan_job::{PROGRESS_REFRESH, Poll, ScanJob};
use crate::settings::{STORAGE_KEY, Settings};
use crate::views::{self, ScanView, Tab};

const TOAST_TIME: Duration = Duration::from_secs(4);
const VOLUME_REFRESH: Duration = Duration::from_secs(5);
/// Snapshots kept per scanned folder; older ones are pruned.
const MAX_SNAPSHOTS_PER_FOLDER: usize = 30;
/// Depth exported to CSV (TreeSize-style: folders and files a few levels deep).
const CSV_EXPORT_DEPTH: usize = 64;

/// Something the user asked for; applied after the frame is drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    StartScan(PathBuf),
    PickFolder,
    Rescan,
    Stop,
    Home,
    Focus(NodeId),
    Select(NodeId),
    Up,
    Open(NodeId),
    Reveal(NodeId),
    CopyPath(NodeId),
    AskTrash(NodeId),
    ConfirmTrash,
    CancelTrash,
    ScanSubtree(NodeId),
    ExportCsv,
    SetLang(Option<Lang>),
    RunAsAdmin,
    Integrate(bool),
    About(bool),
    SaveSnapshot,
    CompareWith(PathBuf),
    BrowseSnapshot,
    ClearComparison,
    SetDuplicateMinSize(u64),
}

pub struct Toast {
    /// Display-ready (visual order) text.
    pub text: String,
    pub is_error: bool,
    pub until: Instant,
}

pub struct App {
    pub settings: Settings,
    pub i18n: I18n,
    pub volumes: Vec<volumes::Volume>,
    volumes_at: Instant,
    pub job: Option<ScanJob>,
    pub scan: Option<ScanView>,
    pub toast: Option<Toast>,
    pub confirm_trash: Option<NodeId>,
    pub show_about: bool,
    pub elevated: bool,
    pub actions: Vec<Action>,
    devshot: Option<crate::devshot::DevShot>,
    /// Background snapshot writes; joined on exit so none is cut short.
    writers: Vec<std::thread::JoinHandle<()>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();
        cc.egui_ctx.set_theme(settings.theme.preference());
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(8.0, 4.0);
        });
        let mut app = App {
            i18n: I18n::new(settings.effective_lang()),
            settings,
            volumes: volumes::list(),
            volumes_at: Instant::now(),
            job: None,
            scan: None,
            toast: None,
            confirm_trash: None,
            show_about: false,
            elevated: platform::is_elevated(),
            actions: Vec::new(),
            devshot: crate::devshot::DevShot::from_env(),
            writers: Vec::new(),
        };
        if let Some(path) = initial {
            app.actions.push(Action::StartScan(path));
        }
        app
    }

    /// Shows a transient message; `text` must already be in visual order.
    pub fn notify(&mut self, text: String, is_error: bool) {
        self.toast = Some(Toast {
            text,
            is_error,
            until: Instant::now() + TOAST_TIME,
        });
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        match job.poll() {
            Poll::Running => ctx.request_repaint_after(PROGRESS_REFRESH),
            Poll::Done(result) => {
                let root = job.root.clone();
                self.job = None;
                match result {
                    Ok(result) => {
                        self.settings.remember(root.clone());
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} — StorageMap",
                            root.display()
                        )));
                        let mut view = ScanView::new(result);
                        self.attach_history(&mut view);
                        if self.settings.auto_snapshot && !view.cancelled {
                            self.writers.retain(|w| !w.is_finished());
                            self.writers.extend(save_snapshot_in_background(&view));
                        }
                        if let Some(tab) = self.devshot.as_ref().and_then(|d| d.tab) {
                            view.tab = tab;
                            if tab == Tab::Duplicates {
                                let min = self.settings.duplicate_min_size;
                                if let Err(err) =
                                    views::duplicates::start_search(&mut view, min, ctx)
                                {
                                    tracing::warn!(%err, "cannot start duplicate search");
                                }
                            }
                        }
                        self.scan = Some(view);
                    }
                    Err(error) => {
                        let text = self.i18n.ta("scan-failed", &[("error", error.into())]);
                        self.notify(text, true);
                    }
                }
            }
        }
    }

    fn handle_dropped_folders(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if let Some(dir) = dropped.into_iter().find(|p| p.is_dir()) {
            self.actions.push(Action::StartScan(dir));
        }
    }

    fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::StartScan(path) => self.start_scan(path, ctx),
            Action::PickFolder => {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title(self.i18n.logical("action-scan-folder"))
                    .pick_folder()
                {
                    self.start_scan(dir, ctx);
                }
            }
            Action::Rescan => {
                if let Some(root) = self.scan.as_ref().map(|s| s.tree.root_path().to_path_buf()) {
                    self.start_scan(root, ctx);
                }
            }
            Action::Stop => {
                if let Some(job) = &self.job {
                    job.cancel();
                }
            }
            Action::Home => {
                if self.job.is_none() {
                    self.scan = None;
                    self.volumes = volumes::list();
                }
            }
            Action::Focus(id) => self.with_scan(|s| s.set_focus(id)),
            Action::Select(id) => self.with_scan(|s| s.select(id)),
            Action::Up => self.with_scan(ScanView::up),
            Action::Open(id) => self.on_path(id, |p| platform::open(&p)),
            Action::Reveal(id) => self.on_path(id, |p| platform::reveal(&p)),
            Action::CopyPath(id) => {
                if let Some(path) = self.path_of(id) {
                    ctx.copy_text(path.to_string_lossy().into_owned());
                    self.notify(self.i18n.t("copied"), false);
                }
            }
            Action::AskTrash(id) => {
                if self.scan.as_ref().is_some_and(|s| s.tree.root() != id) {
                    self.confirm_trash = Some(id);
                }
            }
            Action::ConfirmTrash => self.trash_confirmed(),
            Action::CancelTrash => self.confirm_trash = None,
            Action::ScanSubtree(id) => {
                if let Some(path) = self.path_of(id) {
                    self.start_scan(path, ctx);
                }
            }
            Action::ExportCsv => self.export_csv(),
            Action::SetLang(lang) => {
                self.settings.lang = lang;
                self.i18n = I18n::new(self.settings.effective_lang());
            }
            Action::RunAsAdmin => {
                let root = self.scan.as_ref().map(|s| s.tree.root_path().to_path_buf());
                match platform::relaunch_elevated(root.as_deref()) {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => self.notify(self.i18n.v(&e.to_string()), true),
                }
            }
            Action::Integrate(install) => {
                let label = self.i18n.logical("action-integrate");
                let (ok_id, result) = (
                    if install {
                        "integration-done"
                    } else {
                        "integration-removed"
                    },
                    platform::set_file_manager_integration(install, &label),
                );
                match result {
                    Ok(()) => self.notify(self.i18n.t(ok_id), false),
                    Err(e) => {
                        let text = self
                            .i18n
                            .ta("integration-failed", &[("error", e.to_string().into())]);
                        self.notify(text, true);
                    }
                }
            }
            Action::About(show) => self.show_about = show,
            Action::SaveSnapshot => self.save_snapshot_now(),
            Action::CompareWith(file) => self.compare_with(&file),
            Action::BrowseSnapshot => {
                let mut dialog = rfd::FileDialog::new()
                    .set_title(self.i18n.logical("compare-browse"))
                    .add_filter("StorageMap snapshot", &["smap"]);
                if let Some(dir) = platform::snapshot_dir() {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(file) = dialog.pick_file() {
                    self.compare_with(&file);
                }
            }
            Action::ClearComparison => self.with_scan(|s| s.comparison = None),
            Action::SetDuplicateMinSize(size) => self.settings.duplicate_min_size = size,
        }
    }

    fn start_scan(&mut self, path: PathBuf, ctx: &egui::Context) {
        if let Some(job) = &self.job {
            job.cancel();
        }
        let opts = ScanOptions {
            cross_filesystems: self.settings.cross_filesystems,
            prefer_mft: self.settings.prefer_mft,
            threads: 0,
        };
        match ScanJob::start(path, opts, ctx.clone()) {
            Ok(job) => self.job = Some(job),
            Err(e) => self.notify(self.i18n.v(&e.to_string()), true),
        }
    }

    fn with_scan(&mut self, f: impl FnOnce(&mut ScanView)) {
        if let Some(scan) = &mut self.scan {
            f(scan);
        }
    }

    fn path_of(&self, id: NodeId) -> Option<PathBuf> {
        self.scan.as_ref().map(|s| s.tree.path(id))
    }

    fn on_path(&mut self, id: NodeId, f: impl FnOnce(PathBuf) -> std::io::Result<()>) {
        if let Some(path) = self.path_of(id)
            && let Err(e) = f(path)
        {
            let text = self
                .i18n
                .ta("open-failed", &[("error", e.to_string().into())]);
            self.notify(text, true);
        }
    }

    fn trash_confirmed(&mut self) {
        let (Some(id), Some(scan)) = (self.confirm_trash.take(), self.scan.as_mut()) else {
            return;
        };
        let path = scan.tree.path(id);
        let name = scan.tree.name(id).into_owned();
        match trash::delete(&path) {
            Ok(()) => {
                scan.remove(id);
                let text = self.i18n.ta("trashed", &[("name", name.into())]);
                self.notify(text, false);
            }
            Err(e) => {
                let text = self
                    .i18n
                    .ta("trash-failed", &[("error", e.to_string().into())]);
                self.notify(text, true);
            }
        }
    }

    fn export_csv(&mut self) {
        let Some(scan) = &self.scan else { return };
        let Some(target) = rfd::FileDialog::new()
            .set_title(self.i18n.logical("action-export-csv"))
            .set_file_name("storagemap.csv")
            .add_filter("CSV", &["csv"])
            .save_file()
        else {
            return;
        };
        let result = std::fs::File::create(&target).and_then(|f| {
            storagemap_core::export::write_csv(&scan.tree, scan.focus, CSV_EXPORT_DEPTH, f)
        });
        let text = match &result {
            Ok(()) => self
                .i18n
                .ta("exported", &[("path", target.display().to_string().into())]),
            Err(e) => self
                .i18n
                .ta("export-failed", &[("error", e.to_string().into())]),
        };
        self.notify(text, result.is_err());
    }

    /// Lists earlier snapshots of the scanned folder and compares with the
    /// newest, so "what changed since last time" is ready immediately.
    fn attach_history(&self, view: &mut ScanView) {
        let Some(dir) = platform::snapshot_dir() else {
            return;
        };
        view.snapshots = snapshot::list(&dir, Some(view.tree.root_path()));
        if let Some(latest) = view.snapshots.first() {
            match Snapshot::load(&latest.file).and_then(|s| Comparison::new(&view.tree, &s)) {
                Ok(cmp) => view.comparison = Some(cmp),
                Err(err) => {
                    tracing::warn!(%err, file = %latest.file.display(), "cannot compare with snapshot")
                }
            }
        }
    }

    fn save_snapshot_now(&mut self) {
        let (Some(scan), Some(dir)) = (&self.scan, platform::snapshot_dir()) else {
            return;
        };
        let snap = Snapshot::capture(&scan.tree, fmt::now_unix(), DEFAULT_MIN_FILE_SIZE);
        match snap.save(&dir) {
            Ok(_) => {
                prune_snapshots(&dir, &snap.root);
                self.notify(self.i18n.t("snapshot-saved"), false);
            }
            Err(e) => {
                let text = self
                    .i18n
                    .ta("snapshot-failed", &[("error", e.to_string().into())]);
                self.notify(text, true);
            }
        }
    }

    fn compare_with(&mut self, file: &std::path::Path) {
        let Some(scan) = &mut self.scan else { return };
        match Snapshot::load(file).and_then(|s| Comparison::new(&scan.tree, &s)) {
            Ok(cmp) => {
                scan.comparison = Some(cmp);
                scan.tab = Tab::Changes;
            }
            Err(e) => {
                let text = self
                    .i18n
                    .ta("compare-failed", &[("error", e.to_string().into())]);
                self.notify(text, true);
            }
        }
    }

    fn refresh_volumes(&mut self) {
        if self.scan.is_none() && self.job.is_none() && self.volumes_at.elapsed() > VOLUME_REFRESH {
            self.volumes = volumes::list();
            self.volumes_at = Instant::now();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_job(&ctx);
        self.handle_dropped_folders(&ctx);
        self.refresh_volumes();
        if self
            .toast
            .as_ref()
            .is_some_and(|t| t.until < Instant::now())
        {
            self.toast = None;
        }

        views::chrome::top_bar(self, ui);
        views::chrome::status_bar(self, ui);
        views::chrome::body(self, ui);
        views::chrome::modals(self, &ctx);
        views::chrome::keyboard(self, &ctx);

        for action in std::mem::take(&mut self.actions) {
            self.apply(action, &ctx);
        }
        if self.toast.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if let Some(shot) = &mut self.devshot {
            let idle = self
                .scan
                .as_ref()
                .is_some_and(|s| !views::duplicates::is_running(s));
            shot.frame(&ctx, idle && self.job.is_none());
        }
    }

    fn on_exit(&mut self) {
        for writer in self.writers.drain(..) {
            if writer.join().is_err() {
                tracing::warn!("snapshot writer panicked");
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.settings);
    }
}

/// Captures on the UI thread (fast, read-only) and writes on a worker.
fn save_snapshot_in_background(view: &ScanView) -> Option<std::thread::JoinHandle<()>> {
    let dir = platform::snapshot_dir()?;
    let snap = Snapshot::capture(&view.tree, fmt::now_unix(), DEFAULT_MIN_FILE_SIZE);
    let spawned = std::thread::Builder::new()
        .name("storagemap-snapshot".into())
        .spawn(move || match snap.save(&dir) {
            Ok(file) => {
                tracing::info!(file = %file.display(), "snapshot saved");
                prune_snapshots(&dir, &snap.root);
            }
            Err(err) => tracing::warn!(%err, "cannot save snapshot"),
        });
    spawned
        .map_err(|err| tracing::warn!(%err, "cannot start snapshot writer"))
        .ok()
}

/// Deletes this folder's snapshots beyond the retention limit (oldest first).
fn prune_snapshots(dir: &std::path::Path, root: &std::path::Path) {
    for old in snapshot::list(dir, Some(root))
        .into_iter()
        .skip(MAX_SNAPSHOTS_PER_FOLDER)
    {
        if let Err(err) = std::fs::remove_file(&old.file) {
            tracing::warn!(%err, file = %old.file.display(), "cannot prune snapshot");
        }
    }
}

/// Seconds with one decimal, for "scanned in N s".
pub fn seconds(d: Duration) -> String {
    fmt::group(&format!("{:.1}", d.as_secs_f64()))
}
