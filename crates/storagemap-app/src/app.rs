//! Application state and the action dispatcher.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Ui;
use storagemap_core::export::{self, Delimited, ReportLabels};
use storagemap_core::snapshot::{self, Comparison, DEFAULT_MIN_FILE_SIZE, Snapshot};
use storagemap_core::{NodeId, ScanOptions, volumes};

use crate::fmt;
use crate::i18n::{I18n, Lang};
use crate::platform;
use crate::scan_job::{PROGRESS_REFRESH, Poll, ScanJob};
use crate::settings::{STORAGE_KEY, Settings};
use crate::views::{self, ScanView, Tab};

const TOAST_TIME: Duration = Duration::from_secs(4);
const TOAST_FADE: f32 = 0.25;
/// Share of the monitor the window takes on first launch.
const FIRST_LAUNCH_SCREEN_SHARE: f32 = 0.82;
const FIRST_LAUNCH_MAX: egui::Vec2 = egui::vec2(1680.0, 1050.0);
const VOLUME_REFRESH: Duration = Duration::from_secs(5);
/// Snapshots kept per scanned folder; older ones are pruned.
const MAX_SNAPSHOTS_PER_FOLDER: usize = 30;
/// Folder depth exported to CSV.
const CSV_EXPORT_DEPTH: usize = 64;
/// The clipboard gets the focused folder and its direct contents, like the
/// details table: whole-drive reports are too large to paste.
const CLIPBOARD_DEPTH: usize = 1;

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
    /// Opens the export options; `ConfirmExport` then asks where to save.
    ExportCsv,
    ConfirmExport,
    /// Copies the export as tab-separated text instead of saving it.
    CopyReport,
    CancelExport,
    SetLang(Option<Lang>),
    RunAsAdmin,
    Integrate(bool),
    About(bool),
    SaveSnapshot,
    CompareWith(PathBuf),
    BrowseSnapshot,
    ClearComparison,
    SetDuplicateMinSize(u64),
    /// Show or hide a column of a table: (table id, column title id).
    ToggleColumn(&'static str, &'static str),
    SetChartStyle(crate::views::chart::Style),
    OpenSettings,
    CheckUpdates,
    InstallUpdate,
    DismissUpdate,
    OpenUrl(String),
    ShowReleaseNotes(bool),
    /// Don't announce this version again (manual checks still show it).
    SkipUpdate(String),
    RestartApp,
    AskEmptyTrash(NodeId),
    ConfirmEmptyTrash,
    CancelEmptyTrash,
}

pub struct Toast {
    /// Display-ready (visual order) text.
    pub text: String,
    pub is_error: bool,
    pub shown_at: Instant,
}

impl Toast {
    pub fn expired(&self) -> bool {
        self.shown_at.elapsed() > TOAST_TIME
    }

    /// Opacity for a fade in and out at either end of its lifetime.
    pub fn opacity(&self) -> f32 {
        let t = self.shown_at.elapsed().as_secs_f32();
        let fade_in = crate::theme::ease_out(t / TOAST_FADE);
        let fade_out = crate::theme::ease_out((TOAST_TIME.as_secs_f32() - t) / TOAST_FADE);
        fade_in.min(fade_out)
    }
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
    /// A trash folder the user may empty, waiting for confirmation.
    pub confirm_empty: Option<NodeId>,
    /// Emptying in progress: the trash folder, its size, and the result.
    emptying: Option<(
        NodeId,
        u64,
        std::sync::mpsc::Receiver<std::io::Result<usize>>,
    )>,
    pub show_about: bool,
    /// The CSV export options dialog is open.
    pub show_export: bool,
    pub elevated: bool,
    pub actions: Vec<Action>,
    devshot: Option<crate::devshot::DevShot>,
    /// Background snapshot writes; joined on exit so none is cut short.
    writers: Vec<std::thread::JoinHandle<()>>,
    pub show_settings: bool,
    pub settings_section: views::settings::Section,
    pub updater: crate::update::Updater,
    pub update_dismissed: bool,
    pub show_release_notes: bool,
    pub logo: egui::TextureHandle,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();
        cc.egui_ctx.set_theme(settings.theme.preference());
        crate::theme::install(&cc.egui_ctx);
        crate::icons::install(&cc.egui_ctx);
        cc.egui_ctx.set_zoom_factor(settings.ui_scale);
        let mut app = App {
            i18n: I18n::new(settings.effective_lang()),
            settings,
            volumes: volumes::list(),
            volumes_at: Instant::now(),
            job: None,
            scan: None,
            toast: None,
            confirm_trash: None,
            confirm_empty: None,
            emptying: None,
            show_about: false,
            show_export: false,
            elevated: platform::is_elevated(),
            actions: Vec::new(),
            devshot: crate::devshot::DevShot::from_env(),
            writers: Vec::new(),
            show_settings: false,
            settings_section: views::settings::Section::default(),
            updater: crate::update::Updater::default(),
            update_dismissed: false,
            show_release_notes: false,
            logo: load_logo(&cc.egui_ctx),
        };
        app.maybe_check_updates(&cc.egui_ctx);
        let page = app.devshot.as_ref().and_then(|d| d.settings.as_deref());
        if page == Some("export") {
            app.show_export = true;
        } else if let Some(section) = page.and_then(views::settings::Section::from_name) {
            app.settings_section = section;
            app.show_settings = true;
        }
        if let Err(err) = std::thread::Builder::new()
            .name("storagemap-cleanup".into())
            .spawn(crate::update::cleanup_downloads)
        {
            tracing::debug!(%err, "cannot start cleanup");
        }
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
            shown_at: Instant::now(),
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
                let Some(path) = self.path_of(id) else { return };
                match platform::trash_location(&path) {
                    platform::TrashLocation::Root => self.confirm_empty = Some(id),
                    platform::TrashLocation::Inside => {
                        self.notify(self.i18n.t("already-in-trash"), true)
                    }
                    platform::TrashLocation::Outside => {
                        if self.scan.as_ref().is_some_and(|s| s.tree.root() != id) {
                            self.confirm_trash = Some(id);
                        }
                    }
                }
            }
            Action::AskEmptyTrash(id) => self.confirm_empty = Some(id),
            Action::CancelEmptyTrash => self.confirm_empty = None,
            Action::ConfirmEmptyTrash => self.start_emptying(ctx),
            Action::ConfirmTrash => self.trash_confirmed(),
            Action::CancelTrash => self.confirm_trash = None,
            Action::ScanSubtree(id) => {
                if let Some(path) = self.path_of(id) {
                    self.start_scan(path, ctx);
                }
            }
            Action::ExportCsv => self.show_export = self.scan.is_some(),
            Action::CancelExport => self.show_export = false,
            Action::ConfirmExport => {
                self.show_export = false;
                self.export_csv();
            }
            Action::CopyReport => {
                self.show_export = false;
                self.copy_report(ctx);
            }
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
            Action::ToggleColumn(table, column) => self.settings.toggle_column(table, column),
            Action::SetChartStyle(style) => self.settings.chart_style = style,
            Action::OpenSettings => self.show_settings = true,
            Action::CheckUpdates => {
                self.update_dismissed = false;
                self.settings.skipped_update = None;
                self.updater.start_check(ctx);
                self.settings.last_update_check = fmt::now_unix();
            }
            Action::InstallUpdate => self.updater.start_install(ctx),
            Action::DismissUpdate => self.update_dismissed = true,
            Action::ShowReleaseNotes(show) => self.show_release_notes = show,
            Action::SkipUpdate(version) => {
                self.settings.skipped_update = Some(version);
                self.update_dismissed = true;
                self.show_release_notes = false;
            }
            Action::RestartApp => match crate::update::relaunch() {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => self.notify(self.i18n.v(&e.to_string()), true),
            },
            Action::OpenUrl(url) => {
                if let Err(e) = open::that_detached(&url) {
                    let text = self
                        .i18n
                        .ta("open-failed", &[("error", e.to_string().into())]);
                    self.notify(text, true);
                }
            }
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
            filter: self.settings.scan_filter.clone(),
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

    /// The tree report's titles and type names in the user's language.
    fn report_labels(&self) -> ReportLabels {
        let i = &self.i18n;
        let (size_id, allocated_id) = if self.settings.export_readable {
            ("col-size", "col-allocated")
        } else {
            ("csv-bytes", "csv-allocated-bytes")
        };
        ReportLabels {
            headers: [
                "col-path",
                "col-type",
                size_id,
                allocated_id,
                "col-files",
                "col-folders",
                "col-percent",
                "csv-modified-utc",
            ]
            .map(|id| i.logical(id)),
            kinds: ["type-folder", "type-file", "type-link", "type-other"].map(|id| i.logical(id)),
            include: self.settings.export_include,
        }
    }

    /// Writes the focused folder's report as CSV, or as tab-separated text
    /// when `tsv` is set.
    fn write_report(
        &self,
        depth: usize,
        out: impl std::io::Write,
        tsv: bool,
    ) -> std::io::Result<()> {
        let Some(scan) = &self.scan else {
            return Ok(());
        };
        let (i, unit) = (&self.i18n, self.settings.unit);
        let readable_fmt = |bytes: u64| fmt::size(bytes, unit, 1, |k| i.logical(k));
        let mut writer = if tsv {
            Delimited::tsv(out)
        } else {
            Delimited::csv(out).with_bom()
        };
        if self.settings.export_readable {
            writer = writer.readable_sizes(&readable_fmt);
        }
        export::write_tree(
            &scan.tree,
            scan.focus,
            depth,
            &self.report_labels(),
            &mut writer,
        )
    }

    fn export_csv(&mut self) {
        if self.scan.is_none() {
            return;
        }
        let Some(target) = rfd::FileDialog::new()
            .set_title(self.i18n.logical("action-export-csv"))
            .set_file_name("storagemap.csv")
            .add_filter("CSV", &["csv"])
            .save_file()
        else {
            return;
        };
        let result = std::fs::File::create(&target)
            .and_then(|f| self.write_report(CSV_EXPORT_DEPTH, f, false));
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

    /// Copies the focused folder and its direct contents as tab-separated
    /// text, which pastes into spreadsheets as a table.
    fn copy_report(&mut self, ctx: &egui::Context) {
        let mut buf = Vec::new();
        let result = self.write_report(CLIPBOARD_DEPTH, &mut buf, true);
        match result {
            Ok(()) => {
                ctx.copy_text(String::from_utf8_lossy(&buf).into_owned());
                self.notify(self.i18n.t("report-copied"), false);
            }
            Err(e) => {
                let text = self
                    .i18n
                    .ta("export-failed", &[("error", e.to_string().into())]);
                self.notify(text, true);
            }
        }
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

    /// Daily background update check, if enabled.
    fn maybe_check_updates(&mut self, ctx: &egui::Context) {
        let now = fmt::now_unix();
        if self.settings.check_updates
            && self.devshot.is_none()
            && now - self.settings.last_update_check >= crate::update::CHECK_INTERVAL_SECS
        {
            self.settings.last_update_check = now;
            self.updater.start_check(ctx);
        }
    }

    /// Empties the trash on a worker thread: it can hold many gigabytes.
    fn start_emptying(&mut self, ctx: &egui::Context) {
        let (Some(id), Some(scan)) = (self.confirm_empty.take(), &self.scan) else {
            return;
        };
        let size = scan.tree.node(id).size;
        let path = scan.tree.path(id);
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("storagemap-empty-trash".into())
            .spawn(move || {
                let _ = tx.send(platform::empty_trash(&path));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => {
                self.emptying = Some((id, size, rx));
                self.notify(self.i18n.t("emptying-trash"), false);
            }
            Err(e) => self.notify(self.i18n.v(&e.to_string()), true),
        }
    }

    fn poll_emptying(&mut self, ctx: &egui::Context) {
        let Some((id, size, rx)) = &self.emptying else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(PROGRESS_REFRESH);
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err(std::io::Error::other("stopped"))
            }
        };
        let (id, size) = (*id, *size);
        self.emptying = None;
        match result {
            Ok(_) => {
                // The folder stays; everything inside it is gone.
                if let Some(scan) = &mut self.scan {
                    let children: Vec<NodeId> = scan.tree.children(id).collect();
                    for child in children {
                        scan.remove(child);
                    }
                }
                let freed = fmt::size(size, self.settings.unit, 1, |k| self.i18n.logical(k));
                let text = self.i18n.ta("trash-emptied", &[("size", freed.into())]);
                self.notify(text, false);
            }
            Err(e) => {
                let text = self
                    .i18n
                    .ta("empty-trash-failed", &[("error", e.to_string().into())]);
                self.notify(text, true);
            }
        }
    }

    fn poll_updater(&mut self, ctx: &egui::Context) {
        self.updater.poll();
        if matches!(
            self.updater,
            crate::update::Updater::Finished(crate::update::Installed::QuitNow)
        ) {
            // The installer replaces the app and relaunches it.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// First launch: take a comfortable share of the monitor, centred.
    /// Later launches restore the size the user left it at.
    fn size_window_on_first_launch(&mut self, ctx: &egui::Context) {
        if self.settings.window_sized {
            return;
        }
        let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else {
            return;
        };
        let size = (monitor * FIRST_LAUNCH_SCREEN_SHARE)
            .min(FIRST_LAUNCH_MAX)
            .max(egui::vec2(760.0, 480.0));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
            ((monitor - size) / 2.0).to_pos2(),
        ));
        self.settings.window_sized = true;
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
        if self.toast.as_ref().is_some_and(Toast::expired) {
            self.toast = None;
        }
        self.size_window_on_first_launch(&ctx);
        self.poll_updater(&ctx);
        self.poll_emptying(&ctx);
        // Ctrl +/- zoom is built into egui; remember whatever the user picked.
        self.settings.ui_scale = ctx.zoom_factor();

        views::chrome::top_bar(self, ui);
        views::chrome::update_banner(self, ui);
        views::chrome::status_bar(self, ui);
        views::chrome::body(self, ui);
        views::chrome::modals(self, &ctx);
        views::settings::window(self, &ctx);
        views::chrome::toast(self, &ctx);
        views::chrome::keyboard(self, &ctx);

        for action in std::mem::take(&mut self.actions) {
            self.apply(action, &ctx);
        }
        if self.toast.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if let Some(shot) = &mut self.devshot {
            // Idle: no scan running, and either results or the start screen shown.
            let idle = self.job.is_none()
                && self.actions.is_empty()
                && self
                    .scan
                    .as_ref()
                    .is_none_or(|s| !views::duplicates::is_running(s));
            shot.frame(&ctx, idle);
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

/// The app icon as a texture for the header and start screen.
fn load_logo(ctx: &egui::Context) -> egui::TextureHandle {
    let image = eframe::icon_data::from_png_bytes(LOGO_PNG)
        .map(|icon| {
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            )
        })
        .unwrap_or_else(|_| egui::ColorImage::filled([1, 1], crate::theme::ACCENT));
    ctx.load_texture("storagemap-logo", image, egui::TextureOptions::LINEAR)
}

pub const LOGO_PNG: &[u8] = include_bytes!("../../../assets/icon/storagemap-128.png");

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
