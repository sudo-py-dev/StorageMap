//! Application state and the action dispatcher.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Ui;
use storagemap_core::export::{self, Delimited, ReportLabels, ReportWriter};
use storagemap_core::formats::{self, Format};
use storagemap_core::snapshot::{self, Comparison, DEFAULT_MIN_FILE_SIZE, Snapshot};
use storagemap_core::{NodeId, ScanOptions, volumes};

use crate::fmt;
use crate::i18n::{I18n, Lang};
use crate::platform;
use crate::scan_job::{PROGRESS_REFRESH, Poll, ScanJob};
use crate::settings::{STORAGE_KEY, Settings};
use crate::tabs::Tabs;
use crate::views::{self, ScanView, Tab};

const TOAST_TIME: Duration = Duration::from_secs(4);
const TOAST_FADE: f32 = 0.25;
/// Share of the monitor the window takes on first launch.
const FIRST_LAUNCH_SCREEN_SHARE: f32 = 0.82;
const FIRST_LAUNCH_MAX: egui::Vec2 = egui::vec2(1680.0, 1050.0);
const VOLUME_REFRESH: Duration = Duration::from_secs(5);
/// Snapshots kept per scanned folder; older ones are pruned.
const MAX_SNAPSHOTS_PER_FOLDER: usize = 30;
/// The clipboard gets the focused folder and its direct contents, like the
/// details table: whole-drive reports are too large to paste.
const CLIPBOARD_DEPTH: usize = 1;

/// A bulk action the user asked for; destinations are picked first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkRequest {
    MoveTo,
    CopyTo,
    Zip,
    Rename,
    Trash,
}

/// What a report covers.
#[derive(Debug, Clone, Copy)]
enum Scope {
    /// The focused folder, this many levels deep.
    Folder(usize),
    /// The Search tab's results.
    SearchResults,
}

/// Something the user asked for; applied after the frame is drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    StartScan(PathBuf),
    /// Marks or unmarks an entry for bulk actions.
    ToggleMark(NodeId),
    MarkAll(Vec<NodeId>),
    ClearMarks,
    /// Shows the tab that scanned `root` and reveals `path` in it.
    RevealInTab {
        root: PathBuf,
        path: PathBuf,
    },
    /// Shows the scan tab at this position.
    SwitchTab(usize),
    CloseTab(usize),
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
    SetDuplicateSha256(bool),
    /// Saves the found duplicate groups as CSV.
    ExportDuplicates,
    /// Shows a message; the text is already in visual order.
    Notify(String, bool),
    /// A bulk action on the marked entries, from the selection bar.
    Bulk(BulkRequest),
    RunBulkTrash,
    RunBulkRename(Vec<storagemap_core::operations::PlannedRename>),
    /// Show or hide a column of a table: (table id, column title id).
    ToggleColumn(&'static str, &'static str),
    /// A table's full column order, as title ids.
    SetColumnOrder(&'static str, Vec<&'static str>),
    ResetColumns(&'static str),
    SetChartStyle(crate::views::chart::Style),
    /// Saves the current search results as CSV, or copies them.
    ExportSearch,
    CopySearch,
    /// Saves a search by name, replacing one of the same name.
    SaveSearch(crate::views::search::SavedSearch),
    DeleteSavedSearch(usize),
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
    /// The scan tab being shown, if any.
    pub scan: Option<ScanView>,
    /// The other open scan tabs, in tab order without the shown one.
    pub parked: Vec<ScanView>,
    /// Position of `scan` among all tabs.
    pub active_tab: usize,
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
    /// A bulk action running on the marked entries, and the scan it acts on.
    pub bulk: Option<(crate::bulk::Job, PathBuf)>,
    pub confirm_bulk_trash: bool,
    pub rename: Option<views::selection::RenameDialog>,
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
            parked: Vec::new(),
            active_tab: 0,
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
            bulk: None,
            confirm_bulk_trash: false,
            rename: None,
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
            Poll::Running => {
                if let Some(tree) = job.progress.take_partial() {
                    self.show_partial(tree, job.started.elapsed());
                }
                ctx.request_repaint_after(PROGRESS_REFRESH);
            }
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
                        let mut view = match self.take_live_tab() {
                            Some(mut live) => {
                                live.finish(result);
                                live
                            }
                            None => ScanView::new(result),
                        };
                        self.attach_history(&mut view);
                        if self.settings.auto_snapshot && !view.cancelled {
                            self.writers.retain(|w| !w.is_finished());
                            self.writers.extend(save_snapshot_in_background(&view));
                        }
                        if let Some(tab) = self.devshot.as_ref().and_then(|d| d.tab) {
                            view.tab = tab;
                            if tab == Tab::Duplicates {
                                let min = self.settings.duplicate_min_size;
                                let sha = self.settings.duplicate_sha256;
                                if let Err(err) =
                                    views::duplicates::start_search(&mut view, min, sha, ctx)
                                {
                                    tracing::warn!(%err, "cannot start duplicate search");
                                }
                            }
                        }
                        self.open_tab(view);
                    }
                    Err(error) => {
                        // Keep whatever the live view had found.
                        if let Some(mut live) = self.take_live_tab() {
                            live.scanning = false;
                            live.cancelled = true;
                            self.open_tab(live);
                        }
                        let text = self.i18n.ta("scan-failed", &[("error", error.into())]);
                        self.notify(text, true);
                    }
                }
            }
        }
    }

    /// Shows a partial tree from the running scan: in its own tab, opened
    /// on the first one and updated in place after that.
    fn show_partial(&mut self, tree: storagemap_core::Tree, elapsed: std::time::Duration) {
        if let Some(view) = self.tabs_mut().find(|v| v.scanning) {
            view.replace_tree(tree);
            view.elapsed = elapsed;
            return;
        }
        let mut view = ScanView::new(storagemap_core::ScanResult {
            tree,
            backend: storagemap_core::Backend::Portable,
            elapsed,
            errors: 0,
            cancelled: false,
        });
        view.scanning = true;
        self.open_tab(view);
    }

    /// Every open scan tab, shown or not.
    fn tabs_mut(&mut self) -> impl Iterator<Item = &mut ScanView> {
        self.scan.iter_mut().chain(self.parked.iter_mut())
    }

    fn tabs(&mut self) -> Tabs<'_, ScanView> {
        Tabs {
            shown: &mut self.scan,
            parked: &mut self.parked,
            active: &mut self.active_tab,
        }
    }

    /// Number of open scan tabs.
    pub fn tab_count(&self) -> usize {
        self.parked.len() + usize::from(self.scan.is_some())
    }

    /// The tab at position `index`, and whether it is the one shown.
    pub fn tab(&self, index: usize) -> Option<(&ScanView, bool)> {
        crate::tabs::get(self.scan.as_ref(), &self.parked, self.active_tab, index)
    }

    fn close_tab(&mut self, index: usize) {
        if let Some((view, _)) = self.tab(index)
            && view.scanning
            && let Some(job) = &self.job
        {
            job.cancel();
        }
        self.tabs().close(index);
    }

    /// Shows `view` as a tab: replacing the tab of the same folder, or
    /// added after the others.
    fn open_tab(&mut self, view: ScanView) {
        self.tabs()
            .open(view, |a, b| a.tree.root_path() == b.tree.root_path());
    }

    /// Removes and returns the tab showing the running scan, if any.
    fn take_live_tab(&mut self) -> Option<ScanView> {
        self.tabs().take(|v| v.scanning)
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
                // Open tabs stay; the start screen picks the next folder.
                self.tabs().park();
                self.volumes = volumes::list();
            }
            Action::SwitchTab(index) => self.tabs().switch(index),
            Action::ToggleMark(id) => self.with_scan(|s| {
                if !s.marked.remove(&id) {
                    s.marked.insert(id);
                }
            }),
            Action::MarkAll(ids) => self.with_scan(|s| s.marked.extend(ids)),
            Action::ClearMarks => self.with_scan(|s| s.marked.clear()),
            Action::RevealInTab { root, path } => {
                let index = (0..self.tab_count())
                    .find(|&i| self.tab(i).is_some_and(|(v, _)| v.tree.root_path() == root));
                if let Some(index) = index {
                    self.tabs().switch(index);
                    if let Some(scan) = &mut self.scan
                        && let Some(id) = scan.tree.find(&path)
                    {
                        scan.set_focus(id);
                        scan.tab = Tab::Details;
                        scan.tab_changed = Some(Instant::now());
                    }
                }
            }
            Action::CloseTab(index) => self.close_tab(index),
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
                if self.scan.as_ref().is_some_and(|s| s.scanning) {
                    self.notify(self.i18n.t("wait-for-scan"), true);
                    return;
                }
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
                let depth = self.settings.export_depth.map_or(usize::MAX, usize::from);
                self.export_report(Scope::Folder(depth));
            }
            Action::CopyReport => {
                self.show_export = false;
                self.copy_report(Scope::Folder(CLIPBOARD_DEPTH), ctx);
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
            Action::SetDuplicateSha256(on) => self.settings.duplicate_sha256 = on,
            Action::ExportDuplicates => self.export_duplicates(),
            Action::Notify(text, is_error) => self.notify(text, is_error),
            Action::Bulk(request) => self.bulk_request(request, ctx),
            Action::RunBulkTrash => self.start_bulk(crate::bulk::Kind::Trash, ctx),
            Action::RunBulkRename(plans) => self.start_bulk(crate::bulk::Kind::Rename(plans), ctx),
            Action::ToggleColumn(table, column) => self.settings.toggle_column(table, column),
            Action::SetColumnOrder(table, order) => {
                let order = order.into_iter().map(str::to_owned).collect();
                self.settings.column_order.insert(table.to_owned(), order);
            }
            Action::ResetColumns(table) => self.settings.reset_columns(table),
            Action::SetChartStyle(style) => self.settings.chart_style = style,
            Action::ExportSearch => self.export_report(Scope::SearchResults),
            Action::CopySearch => self.copy_report(Scope::SearchResults, ctx),
            Action::SaveSearch(search) => {
                let saved = &mut self.settings.saved_searches;
                saved.retain(|s| s.name != search.name);
                saved.push(search);
            }
            Action::DeleteSavedSearch(index) => {
                if index < self.settings.saved_searches.len() {
                    self.settings.saved_searches.remove(index);
                }
            }
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
        // A replaced scan's tab keeps what it found so far.
        for view in self.tabs_mut().filter(|v| v.scanning) {
            view.scanning = false;
            view.cancelled = true;
        }
        // Rescanning an open folder updates its tab in place, keeping the
        // user's place, instead of opening a fresh one.
        let root = std::path::absolute(&path).unwrap_or_else(|_| path.clone());
        if let Some(view) = self.tabs_mut().find(|v| v.tree.root_path() == root) {
            view.scanning = true;
        }
        let opts = ScanOptions {
            cross_filesystems: self.settings.cross_filesystems,
            prefer_mft: self.settings.prefer_mft,
            threads: 0,
            filter: self.settings.scan_filter.clone(),
        };
        match ScanJob::start(path, opts, ctx.clone()) {
            Ok(job) => self.job = Some(job),
            Err(e) => {
                for view in self.tabs_mut() {
                    view.scanning = false;
                }
                self.notify(self.i18n.v(&e.to_string()), true);
            }
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
            columns: if self.settings.export_columns.is_empty() {
                (0..export::COLUMN_KEYS.len()).collect()
            } else {
                self.settings.export_columns.clone()
            },
        }
    }

    /// Writes a report in `format`, or as tab-separated text for the
    /// clipboard when `format` is `None`.
    fn write_report<W: std::io::Write + std::io::Seek>(
        &self,
        scope: Scope,
        out: W,
        format: Option<Format>,
    ) -> std::io::Result<()> {
        let Some(scan) = &self.scan else {
            return Ok(());
        };
        let (i, unit) = (&self.i18n, self.settings.unit);
        let readable_fmt = |bytes: u64| fmt::size(bytes, unit, 1, |k| i.logical(k));
        let readable = self
            .settings
            .export_readable
            .then_some(&readable_fmt as &dyn Fn(u64) -> String);
        let labels = self.report_labels();
        let rtl = i.is_rtl();
        let title = format!("StorageMap · {}", scan.tree.path(scan.focus).display());
        let lang = self.settings.effective_lang().code();
        let mut writer: Box<dyn ReportWriter + '_> = match format {
            None => {
                let w = Delimited::tsv(out);
                Box::new(match readable {
                    Some(r) => w.readable_sizes(r),
                    None => w,
                })
            }
            Some(Format::Csv) => {
                let w = Delimited::csv(out).with_bom();
                Box::new(match readable {
                    Some(r) => w.readable_sizes(r),
                    None => w,
                })
            }
            Some(Format::Excel) => {
                let w = formats::Excel::new(out, rtl);
                Box::new(match readable {
                    Some(r) => w.readable_sizes(r),
                    None => w,
                })
            }
            Some(Format::Html) => {
                let w = formats::Html::new(out, &title, lang, rtl);
                Box::new(match readable {
                    Some(r) => w.readable_sizes(r),
                    None => w,
                })
            }
            Some(Format::Json) => Box::new(formats::Json::new(out, labels.keys())),
            Some(Format::Text) => {
                let w = formats::Text::new(out);
                Box::new(match readable {
                    Some(r) => w.readable_sizes(r),
                    None => w,
                })
            }
        };
        match scope {
            Scope::Folder(depth) => {
                export::write_tree(&scan.tree, scan.focus, depth, &labels, writer.as_mut())
            }
            Scope::SearchResults => {
                let entries = scan
                    .search_results()
                    .iter()
                    .map(|h| (h.tree(&scan.tree, &self.parked), h.id));
                export::write_entries(entries, &labels, writer.as_mut())
            }
        }
    }

    fn export_report(&mut self, scope: Scope) {
        if self.scan.is_none() {
            return;
        }
        let format = self.settings.export_format;
        let ext = format.extension();
        let file_name = match scope {
            Scope::Folder(_) => format!("storagemap.{ext}"),
            Scope::SearchResults => format!("storagemap-search.{ext}"),
        };
        let Some(target) = rfd::FileDialog::new()
            .set_title(self.i18n.logical("export-title"))
            .set_file_name(file_name)
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        else {
            return;
        };
        let result =
            std::fs::File::create(&target).and_then(|f| self.write_report(scope, f, Some(format)));
        if result.is_err() {
            // Don't leave half a report behind.
            let _ = std::fs::remove_file(&target);
        }
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

    /// Copies the report as tab-separated text, which pastes into
    /// spreadsheets as a table.
    fn copy_report(&mut self, scope: Scope, ctx: &egui::Context) {
        let mut buf = std::io::Cursor::new(Vec::new());
        let result = self.write_report(scope, &mut buf, None);
        match result {
            Ok(()) => {
                ctx.copy_text(String::from_utf8_lossy(buf.get_ref()).into_owned());
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

    /// Asks where (for move, copy and zip) or confirms, then starts.
    fn bulk_request(&mut self, request: BulkRequest, ctx: &egui::Context) {
        let Some(scan) = &self.scan else { return };
        let top = scan.marked_top();
        if top.is_empty() || self.bulk.is_some() {
            return;
        }
        let pick_folder = |title: &str| {
            rfd::FileDialog::new()
                .set_title(self.i18n.logical(title))
                .pick_folder()
        };
        match request {
            BulkRequest::MoveTo => {
                if let Some(dest) = pick_folder("bulk-move-title") {
                    self.start_bulk(crate::bulk::Kind::MoveTo(dest), ctx);
                }
            }
            BulkRequest::CopyTo => {
                if let Some(dest) = pick_folder("bulk-copy-title") {
                    self.start_bulk(crate::bulk::Kind::CopyTo(dest), ctx);
                }
            }
            BulkRequest::Zip => {
                let name = match top.as_slice() {
                    [one] => format!("{}.zip", scan.tree.name(*one)),
                    _ => "archive.zip".to_owned(),
                };
                if let Some(archive) = rfd::FileDialog::new()
                    .set_title(self.i18n.logical("bulk-zip-title"))
                    .set_file_name(name)
                    .add_filter("ZIP", &["zip"])
                    .save_file()
                {
                    self.start_bulk(crate::bulk::Kind::Zip(archive), ctx);
                }
            }
            BulkRequest::Rename => {
                let paths = top.iter().map(|&id| scan.tree.path(id)).collect();
                self.rename = Some(views::selection::RenameDialog::new(paths));
            }
            BulkRequest::Trash => self.confirm_bulk_trash = true,
        }
    }

    /// Runs `kind` on the marked entries of the shown scan.
    fn start_bulk(&mut self, kind: crate::bulk::Kind, ctx: &egui::Context) {
        let Some(scan) = &self.scan else { return };
        let items: Vec<(NodeId, PathBuf)> = match &kind {
            crate::bulk::Kind::Rename(plans) => plans
                .iter()
                .map(|p| {
                    (
                        scan.tree.find(&p.from).unwrap_or(scan.tree.root()),
                        p.from.clone(),
                    )
                })
                .collect(),
            _ => scan
                .marked_top()
                .into_iter()
                .map(|id| (id, scan.tree.path(id)))
                .collect(),
        };
        let bytes = items.iter().map(|(id, _)| scan.tree.node(*id).size).sum();
        let root = scan.tree.root_path().to_path_buf();
        match crate::bulk::Job::start(kind, items, bytes, ctx) {
            Ok(job) => self.bulk = Some((job, root)),
            Err(e) => self.notify(self.i18n.v(&e.to_string()), true),
        }
    }

    /// Reports a finished bulk action and brings the scan up to date.
    fn poll_bulk(&mut self) {
        let Some((job, root)) = &self.bulk else {
            return;
        };
        let Some(outcome) = job.poll() else { return };
        let (kind, root) = (job.kind.clone(), root.clone());
        self.bulk = None;
        let ok: Vec<NodeId> = outcome
            .iter()
            .filter(|(_, r)| r.is_ok())
            .map(|(id, _)| *id)
            .collect();
        let errors: Vec<&String> = outcome
            .iter()
            .filter_map(|(_, r)| r.as_ref().err())
            .collect();
        let i = &self.i18n;
        let mut text = i.count(kind.done_id(), ok.len() as u64);
        if let Some(first) = errors.first() {
            text = format!(
                "{text} · {}",
                i.la(
                    "dup-failed",
                    &[
                        ("count", (errors.len() as u64).into()),
                        ("error", (*first).clone().into())
                    ]
                )
            );
        }
        self.notify(i.v(&text), !errors.is_empty());
        let shown = self.scan.as_mut().filter(|s| s.tree.root_path() == root);
        // What changed on disk inside the scanned folder.
        let changed_inside = match &kind {
            crate::bulk::Kind::Trash => false,
            crate::bulk::Kind::MoveTo(_) | crate::bulk::Kind::Rename(_) => !ok.is_empty(),
            crate::bulk::Kind::CopyTo(dest) | crate::bulk::Kind::Zip(dest) => {
                !ok.is_empty() && dest.starts_with(&root)
            }
        };
        if let Some(scan) = shown {
            for &id in &ok {
                scan.marked.remove(&id);
                if matches!(kind, crate::bulk::Kind::Trash) {
                    scan.remove(id);
                }
            }
        }
        if changed_inside {
            self.actions.push(Action::StartScan(root));
        }
    }

    fn export_duplicates(&mut self) {
        let Some(scan) = &self.scan else { return };
        let Some(target) = rfd::FileDialog::new()
            .set_title(self.i18n.logical("action-export-csv"))
            .set_file_name("storagemap-duplicates.csv")
            .add_filter("CSV", &["csv"])
            .save_file()
        else {
            return;
        };
        let (i, unit) = (&self.i18n, self.settings.unit);
        let readable_fmt = |bytes: u64| fmt::size(bytes, unit, 1, |k| i.logical(k));
        let readable = self
            .settings
            .export_readable
            .then_some(&readable_fmt as &dyn Fn(u64) -> String);
        let headers = [
            i.logical("dup-col-group"),
            "SHA-256".to_owned(),
            i.logical("col-path"),
            i.logical(if readable.is_some() {
                "col-size"
            } else {
                "csv-bytes"
            }),
            i.logical("csv-modified-utc"),
        ];
        let result = std::fs::File::create(&target)
            .and_then(|f| views::duplicates::export(scan, headers, readable, f));
        let text = match &result {
            Ok(()) => i.ta("exported", &[("path", target.display().to_string().into())]),
            Err(e) => i.ta("export-failed", &[("error", e.to_string().into())]),
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
        self.poll_bulk();
        // Ctrl +/- zoom is built into egui; remember whatever the user picked.
        self.settings.ui_scale = ctx.zoom_factor();

        views::chrome::top_bar(self, ui);
        views::chrome::update_banner(self, ui);
        views::chrome::scan_tabs(self, ui);
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
            // Idle: no scan running, and either results or the start screen
            // shown; or, for a live capture, the running scan's view shown.
            let idle = if shot.live {
                self.scan.as_ref().is_some_and(|s| s.scanning)
            } else {
                self.job.is_none()
                    && self.actions.is_empty()
                    && self
                        .scan
                        .as_ref()
                        .is_none_or(|s| !views::duplicates::is_running(s))
            };
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
