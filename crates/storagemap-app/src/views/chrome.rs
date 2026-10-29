//! Window chrome: header with menus, toolbar, update banner, breadcrumbs,
//! tabs, status bar, dialogs, toasts and keyboard shortcuts.

use std::time::Instant;

use egui::{
    Align2, CentralPanel, Color32, CornerRadius, Frame, Id, Key, KeyboardShortcut, Layout, Margin,
    Modal, Modifiers, Panel, RichText, Sense, Stroke, Ui, vec2,
};
use storagemap_core::export::Include;
use storagemap_core::formats::Format;
use storagemap_core::{Backend, Metric};

use super::{
    Env, ScanView, Tab, changes, chart, cleanup, details, duplicates, lists, search, selection,
    start, tree_panel, treemap,
};
use crate::app::{Action, App, seconds};
use crate::fmt;
use crate::i18n::{Direction, Lang};
use crate::settings::Theme;
use crate::theme;
use crate::update::Updater;
use crate::widgets::{paragraph, row};

const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
/// Width of drop-down menus.
const MENU_WIDTH: f32 = 240.0;
const EXPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
const FIND: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
const SETTINGS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma);
const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
/// Seconds for the content cross-fade when switching tabs.
const TAB_FADE: f32 = 0.18;
const LOGO_SIZE: f32 = 22.0;
const ICON_SIZE: f32 = 17.0;
const SEARCH_WIDTH: f32 = 220.0;
const TAB_TEXT_PADDING: f32 = 22.0;
const TAB_GAP: f32 = 4.0;
/// Space around the divider between tab groups.
const TAB_GROUP_GAP: f32 = 12.0;

const METRICS: [(Metric, &str); 3] = [
    (Metric::Size, "metric-size"),
    (Metric::Allocated, "metric-allocated"),
    (Metric::FileCount, "metric-files"),
];

fn bar_frame(ui: &Ui) -> Frame {
    Frame::new()
        .fill(ui.visuals().panel_fill)
        .inner_margin(Margin::symmetric(10, 6))
}

pub fn top_bar(app: &mut App, ui: &mut Ui) {
    let frame = bar_frame(ui);
    Panel::top("header").frame(frame).show(ui, |ui| {
        let dir = app.i18n.dir();
        row(ui, dir, |ui| {
            ui.add(egui::Image::new(&app.logo).fit_to_exact_size(vec2(LOGO_SIZE, LOGO_SIZE)));
            ui.label(RichText::new("StorageMap").strong().size(15.5));
            ui.add_space(14.0);
            // A real menu bar: plain text that highlights on hover.
            let w = &mut ui.visuals_mut().widgets.inactive;
            (w.bg_fill, w.weak_bg_fill) = (Color32::TRANSPARENT, Color32::TRANSPARENT);
            file_menu(app, ui);
            view_menu(app, ui);
            help_menu(app, ui);
            ui.with_layout(dir.end_row(), |ui| {
                if icon_button(ui, crate::icons::SETTINGS, &app.i18n.t("menu-settings")).clicked() {
                    app.actions.push(Action::OpenSettings);
                }
                language_menu(app, ui);
            });
        });
    });
    if app.scan.is_some() {
        let frame = bar_frame(ui).inner_margin(Margin::symmetric(10, 4));
        Panel::top("toolbar")
            .frame(frame)
            .show(ui, |ui| toolbar(app, ui));
    }
}

/// A frameless icon button that highlights on hover, with a tooltip.
pub fn icon_button(ui: &mut Ui, icon: &str, tooltip: &str) -> egui::Response {
    let button = egui::Button::new(RichText::new(icon).size(ICON_SIZE)).frame_when_inactive(false);
    ui.add(button).on_hover_text(tooltip)
}

/// Menu contents aligned to the reading-start edge.
fn menu_column<R>(dir: Direction, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    // A fixed width: an end-aligned (RTL) layout otherwise stretches the
    // popup across all the space it is offered.
    ui.set_width(MENU_WIDTH);
    ui.with_layout(Layout::top_down_justified(dir.start()), add)
        .inner
}

fn shortcut_button(ui: &Ui, text: String, shortcut: &KeyboardShortcut) -> egui::Button<'static> {
    egui::Button::new(text).shortcut_text(ui.ctx().format_shortcut(shortcut))
}

fn file_menu(app: &mut App, ui: &mut Ui) {
    let (has_scan, scanning) = (app.scan.is_some(), app.job.is_some());
    ui.menu_button(app.i18n.t("menu-file"), |ui| {
        menu_column(app.i18n.dir(), ui, |ui| {
            let i = &app.i18n;
            let actions = &mut app.actions;
            let mut item =
                |ui: &mut Ui, enabled: bool, button: egui::Button<'static>, action: Action| {
                    if ui.add_enabled(enabled, button).clicked() {
                        actions.push(action);
                    }
                };
            item(
                ui,
                true,
                shortcut_button(
                    ui,
                    format!(
                        "{}  {}",
                        crate::icons::FOLDER_OPEN,
                        i.t("action-scan-folder")
                    ),
                    &OPEN,
                ),
                Action::PickFolder,
            );
            item(
                ui,
                has_scan && !scanning,
                egui::Button::new(format!(
                    "{}  {}",
                    crate::icons::RESCAN,
                    i.t("action-rescan")
                ))
                .shortcut_text("F5"),
                Action::Rescan,
            );
            item(
                ui,
                scanning,
                egui::Button::new(format!("{}  {}", crate::icons::STOP, i.t("action-stop")))
                    .shortcut_text("Esc"),
                Action::Stop,
            );
            ui.separator();
            item(
                ui,
                has_scan,
                egui::Button::new(format!(
                    "{}  {}",
                    crate::icons::SNAPSHOT,
                    i.t("action-save-snapshot")
                )),
                Action::SaveSnapshot,
            );
            item(
                ui,
                true,
                egui::Button::new(format!(
                    "{}  {}",
                    crate::icons::AGE,
                    i.t("action-open-snapshot")
                )),
                Action::BrowseOpenSnapshot,
            );
            item(
                ui,
                has_scan,
                shortcut_button(
                    ui,
                    format!("{}  {}", crate::icons::EXPORT, i.t("action-export-csv")),
                    &EXPORT,
                ),
                Action::ExportCsv,
            );
            ui.separator();
            item(
                ui,
                !scanning,
                egui::Button::new(format!("{}  {}", crate::icons::HOME, i.t("action-home"))),
                Action::Home,
            );
            item(
                ui,
                true,
                shortcut_button(
                    ui,
                    format!("{}  {}", crate::icons::SETTINGS, i.t("menu-settings")),
                    &SETTINGS,
                ),
                Action::OpenSettings,
            );
            ui.separator();
            if ui
                .add(shortcut_button(
                    ui,
                    format!("{}  {}", crate::icons::QUIT, i.t("action-quit")),
                    &QUIT,
                ))
                .clicked()
            {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    });
}

fn view_menu(app: &mut App, ui: &mut Ui) {
    let App { i18n, settings, .. } = app;
    ui.menu_button(i18n.t("menu-view"), |ui| {
        menu_column(i18n.dir(), ui, |ui| {
            ui.label(RichText::new(i18n.t("metric-label")).small().weak());
            for (metric, id) in METRICS {
                ui.radio_value(&mut settings.metric, metric, i18n.t(id));
            }
            ui.separator();
            ui.checkbox(&mut settings.treemap_files, i18n.t("treemap-files"));
            ui.checkbox(&mut settings.treemap_labels, i18n.t("treemap-labels"));
            ui.separator();
            ui.label(RichText::new(i18n.t("view-theme")).small().weak());
            for t in Theme::ALL {
                if ui
                    .radio_value(&mut settings.theme, t, i18n.t(t.label_id()))
                    .clicked()
                {
                    ui.ctx().set_theme(t.preference());
                }
            }
            ui.separator();
            let ctx = ui.ctx().clone();
            let zoom = |ui: &mut Ui, icon: &str, id: &str, shortcut: &str, factor: f32| {
                if ui
                    .add(
                        egui::Button::new(format!("{icon}  {}", i18n.t(id)))
                            .shortcut_text(shortcut),
                    )
                    .clicked()
                {
                    let z = if factor == 0.0 {
                        1.0
                    } else {
                        (ctx.zoom_factor() * factor).clamp(0.5, 3.0)
                    };
                    ctx.set_zoom_factor(z);
                }
            };
            zoom(ui, crate::icons::ZOOM_IN, "action-zoom-in", "Ctrl +", 1.1);
            zoom(
                ui,
                crate::icons::ZOOM_OUT,
                "action-zoom-out",
                "Ctrl -",
                1.0 / 1.1,
            );
            zoom(ui, crate::icons::SEARCH, "action-zoom-reset", "Ctrl 0", 0.0);
        });
    });
}

fn language_menu(app: &mut App, ui: &mut Ui) {
    let current = app.settings.lang;
    let label = current.map_or_else(
        || app.i18n.t("lang-system"),
        |l| crate::i18n::bidi::visual(l.native_name(), l.direction()),
    );
    let tooltip = format!("{} · {label}", app.i18n.t("menu-language"));
    ui.menu_button(
        RichText::new(crate::icons::LANGUAGE).size(ICON_SIZE),
        |ui| {
            menu_column(app.i18n.dir(), ui, |ui| {
                if ui
                    .radio(current.is_none(), app.i18n.t("lang-system"))
                    .clicked()
                {
                    app.actions.push(Action::SetLang(None));
                }
                for lang in Lang::ALL {
                    // Each language names itself, in its own direction.
                    let name = crate::i18n::bidi::visual(lang.native_name(), lang.direction());
                    if ui.radio(current == Some(lang), name).clicked() {
                        app.actions.push(Action::SetLang(Some(lang)));
                    }
                }
            });
        },
    )
    .response
    .on_hover_text(tooltip);
}

fn help_menu(app: &mut App, ui: &mut Ui) {
    ui.menu_button(app.i18n.t("menu-help"), |ui| {
        menu_column(app.i18n.dir(), ui, |ui| {
            let i = &app.i18n;
            if ui
                .button(format!(
                    "{}  {}",
                    crate::icons::UPDATE,
                    i.t("action-check-updates")
                ))
                .clicked()
            {
                app.actions.push(Action::CheckUpdates);
            }
            if ui
                .button(format!(
                    "{}  {}",
                    crate::icons::GITHUB,
                    i.t("action-open-github")
                ))
                .clicked()
            {
                app.actions.push(Action::OpenUrl(format!(
                    "https://github.com/{}",
                    crate::update::REPO
                )));
            }
            ui.separator();
            if ui
                .button(format!("{}  {}", crate::icons::INFO, i.t("help-about")))
                .clicked()
            {
                app.actions.push(Action::About(true));
            }
        });
    });
}

fn toolbar(app: &mut App, ui: &mut Ui) {
    let App {
        i18n,
        settings,
        actions,
        job,
        scan,
        ..
    } = app;
    let dir = i18n.dir();
    row(ui, dir, |ui| {
        let scan_button = theme::primary_button(format!(
            "{}  {}",
            crate::icons::FOLDER_OPEN,
            i18n.t("action-scan-folder")
        ));
        if ui.add(scan_button).clicked() {
            actions.push(Action::PickFolder);
        }
        match job {
            Some(job) => {
                if ui
                    .button(format!("{}  {}", crate::icons::STOP, i18n.t("action-stop")))
                    .clicked()
                {
                    actions.push(Action::Stop);
                }
                ui.spinner();
                let snap = job.progress.snapshot();
                let size = fmt::size(snap.bytes, settings.unit, 1, |id| i18n.logical(id));
                ui.label(
                    RichText::new(i18n.ta(
                        "scan-progress",
                        &[
                            ("files", fmt::count(snap.files).into()),
                            ("dirs", fmt::count(snap.dirs).into()),
                            ("size", size.into()),
                        ],
                    ))
                    .weak(),
                );
            }
            None => {
                if ui
                    .button(format!(
                        "{}  {}",
                        crate::icons::RESCAN,
                        i18n.t("action-rescan")
                    ))
                    .clicked()
                {
                    actions.push(Action::Rescan);
                }
                if ui
                    .button(format!(
                        "{}  {}",
                        crate::icons::EXPORT,
                        i18n.t("action-export-csv")
                    ))
                    .on_hover_text(i18n.t("action-export-hint"))
                    .clicked()
                {
                    actions.push(Action::ExportCsv);
                }
            }
        }
        ui.with_layout(dir.end_row(), |ui| {
            if let Some(scan) = scan.as_mut() {
                let env = Env {
                    i18n,
                    settings,
                    actions,
                };
                // The Search tab has its own, larger field; keep the space
                // so the toolbar does not shift.
                if scan.tab == Tab::Search {
                    ui.add_space(SEARCH_WIDTH);
                } else {
                    search::search_box(ui, &env, scan, SEARCH_WIDTH);
                }
            }
            ui.add_space(8.0);
            segmented(ui, dir, |ui| {
                // `horizontal` inherits the flow of the end-aligned group it
                // sits in; add items so they still read in the UI's direction.
                let flows_rtl = ui.layout().prefer_right_to_left();
                let reverse = (dir == Direction::Rtl) != flows_rtl;
                let ordered: Vec<_> = if reverse {
                    METRICS.iter().rev().collect()
                } else {
                    METRICS.iter().collect()
                };
                for (metric, id) in ordered {
                    ui.selectable_value(&mut settings.metric, *metric, i18n.t(id));
                }
            });
            ui.label(RichText::new(i18n.t("metric-label")).weak());
        });
    });
}

/// Selectable values grouped in one rounded track.
fn segmented(ui: &mut Ui, dir: Direction, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .corner_radius(CornerRadius::same(theme::RADIUS + 2))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // `horizontal` sizes to its content; a nested row layout would
            // grab all remaining width.
            let _ = dir;
            ui.horizontal(add);
        });
}

/// A slim banner when an update is available or in progress.
pub fn update_banner(app: &mut App, ui: &mut Ui) {
    let relevant = matches!(
        app.updater,
        Updater::Available(_)
            | Updater::Downloading { .. }
            | Updater::Finished(_)
            | Updater::Failed(_)
    );
    let skipped = app
        .updater
        .available()
        .is_some_and(|r| app.settings.skipped_update.as_deref() == Some(&r.version.to_string()));
    if !relevant || app.update_dismissed || skipped {
        return;
    }
    let tint = if ui.visuals().dark_mode {
        Color32::from_rgb(23, 37, 84)
    } else {
        Color32::from_rgb(219, 234, 254)
    };
    let frame = Frame::new()
        .fill(tint)
        .inner_margin(Margin::symmetric(12, 6));
    Panel::top("update-banner").frame(frame).show(ui, |ui| {
        let App {
            i18n,
            updater,
            actions,
            ..
        } = app;
        row(ui, i18n.dir(), |ui| {
            super::settings::status(ui, i18n, updater, actions);
            ui.with_layout(i18n.dir().end_row(), |ui| {
                if !updater.is_busy()
                    && ui
                        .small_button(crate::icons::CLOSE)
                        .on_hover_text(i18n.t("update-later"))
                        .clicked()
                {
                    actions.push(Action::DismissUpdate);
                }
            });
        });
    });
}

/// One tab per open scan, with a button to open another.
pub fn scan_tabs(app: &mut App, ui: &mut Ui) {
    // A single scan on screen has nothing to switch to; the toolbar opens
    // another and File › Home returns to the start screen.
    if app.tab_count() == 0 || (app.tab_count() == 1 && app.scan.is_some()) {
        return;
    }
    let frame = bar_frame(ui).inner_margin(Margin::symmetric(8, 4));
    Panel::top("scan-tabs").frame(frame).show(ui, |ui| {
        let dir = app.i18n.dir();
        let mut action = None;
        egui::ScrollArea::horizontal()
            .id_salt("scan-tabs-scroll")
            .show(ui, |ui| {
                row(ui, dir, |ui| {
                    for index in 0..app.tab_count() {
                        let Some((view, active)) = app.tab(index) else {
                            continue;
                        };
                        if let Some(a) = scan_tab(ui, &app.i18n, view, active, index) {
                            action = Some(a);
                        }
                    }
                    let new_tab = ui
                        .add(egui::Button::new(crate::icons::PLUS).frame(false))
                        .on_hover_text(app.i18n.t("tab-new"));
                    if new_tab.clicked() {
                        action = Some(Action::Home);
                    }
                });
            });
        app.actions.extend(action);
    });
}

/// A scan tab: the folder's name, a spinner while scanning, and a close button.
fn scan_tab(
    ui: &mut Ui,
    i: &crate::i18n::I18n,
    view: &ScanView,
    active: bool,
    index: usize,
) -> Option<Action> {
    let root = view.tree.root_path();
    let mut name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    if let Some(taken) = view.saved_at {
        name = format!("{} {name} · {}", crate::icons::AGE, fmt::date(taken));
    }
    let visuals = ui.visuals();
    let fill = if active {
        visuals.widgets.active.weak_bg_fill
    } else {
        visuals.widgets.inactive.weak_bg_fill
    };
    let mut action = None;
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            row(ui, i.dir(), |ui| {
                if view.scanning {
                    ui.add(egui::Spinner::new().size(12.0));
                }
                let text = RichText::new(i.v(&name));
                let label = egui::Label::new(if active { text.strong() } else { text })
                    .truncate()
                    .sense(Sense::click());
                let response = ui
                    .add_sized(vec2(ui.available_width().min(180.0), 18.0), label)
                    .on_hover_text(i.v(&root.to_string_lossy()));
                if response.clicked() {
                    action = Some(Action::SwitchTab(index));
                }
                if response.middle_clicked() {
                    action = Some(Action::CloseTab(index));
                }
                let close = ui
                    .add(egui::Button::new(RichText::new(crate::icons::CLOSE).small()).frame(false))
                    .on_hover_text(i.t("tab-close"));
                if close.clicked() {
                    action = Some(Action::CloseTab(index));
                }
            });
        });
    action
}

pub fn status_bar(app: &mut App, ui: &mut Ui) {
    // Before a scan there is no status to show.
    if app.scan.is_none() {
        return;
    }
    let frame = bar_frame(ui).inner_margin(Margin::symmetric(10, 3));
    Panel::bottom("status").frame(frame).show(ui, |ui| {
        row(ui, app.i18n.dir(), |ui| {
            let Some(scan) = &app.scan else {
                return;
            };
            let i = &app.i18n;
            if let Some((job, _)) = &app.bulk {
                ui.add(egui::Spinner::new().size(12.0));
                let done = job.done.load(std::sync::atomic::Ordering::Relaxed);
                let (done, total) = if matches!(job.kind, crate::bulk::Kind::Zip(_)) {
                    let size = |b| fmt::size(b, app.settings.unit, 1, |id| i.logical(id));
                    (size(done), size(job.total))
                } else {
                    (fmt::count(done), fmt::count(job.total))
                };
                let line = i.ta(
                    job.kind.working_id(),
                    &[("done", done.into()), ("total", total.into())],
                );
                ui.label(RichText::new(line).small());
                ui.ctx()
                    .request_repaint_after(crate::scan_job::PROGRESS_REFRESH);
                ui.separator();
            }
            if let Some(taken) = scan.saved_at {
                ui.label(
                    RichText::new(format!(
                        "{}  {}",
                        crate::icons::AGE,
                        i.ta("saved-scan-from", &[("date", fmt::date_time(taken).into())])
                    ))
                    .small()
                    .strong(),
                );
                ui.separator();
                ui.label(RichText::new(i.t("saved-read-only")).small().weak());
                return;
            }
            if scan.scanning
                && let Some(job) = &app.job
            {
                let snap = job.progress.snapshot();
                ui.add(egui::Spinner::new().size(12.0));
                let size = fmt::size(snap.bytes, app.settings.unit, 1, |id| i.logical(id));
                let line = i.ta(
                    "scan-progress",
                    &[
                        ("files", fmt::count(snap.files).into()),
                        ("dirs", fmt::count(snap.dirs).into()),
                        ("size", size.into()),
                    ],
                );
                ui.label(RichText::new(line).small());
                ui.separator();
                ui.label(RichText::new(i.t("scan-live-hint")).small().weak());
                return;
            }
            let done = i.ta(
                "scan-done",
                &[
                    ("items", fmt::count(scan.tree.len() as u64).into()),
                    ("seconds", seconds(scan.elapsed).into()),
                ],
            );
            // How it was scanned matters only to the curious.
            ui.label(RichText::new(done).small())
                .on_hover_text(backend_label(i, scan.backend));
            if scan.errors > 0 {
                ui.separator();
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    RichText::new(i.v(&i.count("scan-errors", scan.errors))).small(),
                );
            }
            if scan.cancelled {
                ui.separator();
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    RichText::new(i.t("scan-cancelled")).small(),
                );
            }
            if let Some(cmp) = &scan.comparison
                && let Some(delta) = cmp.size_delta(&scan.tree, scan.tree.root())
            {
                ui.separator();
                let change = fmt::signed_size(delta, app.settings.unit, 1, |id| i.logical(id));
                let date = fmt::date_time(cmp.taken_at);
                ui.label(
                    RichText::new(i.ta(
                        "compare-summary",
                        &[("change", change.into()), ("date", date.into())],
                    ))
                    .small(),
                );
            }
            if let Some(v) = volume_of(&app.volumes, scan.tree.root_path()) {
                ui.separator();
                let free = fmt::size(v.free, app.settings.unit, 1, |id| i.logical(id));
                ui.label(RichText::new(i.ta("free-on-volume", &[("free", free.into())])).small());
            }
        });
    });
}

fn backend_label(i: &crate::i18n::I18n, backend: Backend) -> String {
    let id = match backend {
        Backend::LinuxStatx => "backend-linux",
        Backend::NtfsMft => "backend-mft",
        Backend::WindowsDirInfo => "backend-dirinfo",
        Backend::Portable => "backend-portable",
        Backend::Source(name) => return i.ta("backend-source", &[("name", name.into())]),
    };
    i.t(id)
}

/// The volume with the longest mount point containing `path`.
fn volume_of<'a>(
    volumes: &'a [storagemap_core::volumes::Volume],
    path: &std::path::Path,
) -> Option<&'a storagemap_core::volumes::Volume> {
    volumes
        .iter()
        .filter(|v| path.starts_with(&v.mount))
        .max_by_key(|v| v.mount.as_os_str().len())
}

pub fn body(app: &mut App, ui: &mut Ui) {
    let busy = app.bulk.is_some();
    let App {
        i18n,
        settings,
        actions,
        scan,
        parked,
        job,
        volumes,
        elevated,
        logo,
        ..
    } = app;
    let mut env = Env {
        i18n,
        settings,
        actions,
    };
    let central = Frame::central_panel(ui.style()).inner_margin(Margin::same(12));
    match (scan.as_mut(), job.as_ref()) {
        (Some(scan), _) => {
            let panel = if env.i18n.is_rtl() {
                Panel::right("folders")
            } else {
                Panel::left("folders")
            };
            let side = Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(Margin::same(10));
            panel
                .resizable(true)
                .default_size(340.0)
                .min_size(220.0)
                .frame(side)
                .show(ui, |ui| {
                    tree_panel::show(ui, &mut env, scan);
                });
            CentralPanel::default()
                .frame(central)
                .show(ui, |ui| central_view(ui, &mut env, scan, parked, busy));
        }
        (None, Some(job)) => {
            let snap = job.progress.snapshot();
            let secs = seconds(job.started.elapsed());
            CentralPanel::default().frame(central).show(ui, |ui| {
                start::progress(ui, &mut env, &snap, &job.root, secs)
            });
        }
        (None, None) => {
            CentralPanel::default()
                .frame(central)
                .show(ui, |ui| start::show(ui, &mut env, volumes, *elevated, logo));
        }
    }
}

/// `others` are the other open scan tabs (for searching them all).
fn central_view(
    ui: &mut Ui,
    env: &mut Env<'_>,
    scan: &mut ScanView,
    others: &[ScanView],
    busy: bool,
) {
    breadcrumbs(ui, env, scan);
    cleanup_chip(ui, env, scan);
    selection::bar(ui, env, scan, busy);
    ui.add_space(4.0);
    let before = scan.tab;
    // Wraps onto a second line in narrow windows instead of widening the
    // panel, so every tab keeps its name.
    let tabs = env.i18n.dir().row().with_main_wrap(true);
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), tabs, |ui| {
        ui.spacing_mut().item_spacing = vec2(TAB_GAP, TAB_GAP);
        for (index, group) in Tab::GROUPS.iter().enumerate() {
            if index > 0 {
                ui.add(egui::Separator::default().vertical().spacing(TAB_GROUP_GAP));
            }
            for &tab in *group {
                tab_pill(ui, env, &mut scan.tab, tab);
            }
        }
    });
    if scan.tab != before {
        scan.tab_changed = Some(Instant::now());
        scan.focus_search |= scan.tab == Tab::Search;
    }
    ui.add_space(6.0);
    // Cross-fade the new tab in.
    if let Some(at) = scan.tab_changed {
        let t = at.elapsed().as_secs_f32() / TAB_FADE;
        if t < 1.0 {
            ui.set_opacity(theme::ease_out(t));
            ui.ctx().request_repaint();
        } else {
            scan.tab_changed = None;
        }
    }
    match scan.tab {
        Tab::Treemap => treemap::show(ui, env, scan),
        Tab::Details => details::show(ui, env, scan),
        Tab::TopFiles => lists::top_files(ui, env, scan),
        Tab::Types => lists::types(ui, env, scan),
        Tab::Ages => lists::ages(ui, env, scan),
        Tab::Search => search::show(ui, env, scan, others),
        Tab::Changes => changes::show(ui, env, scan),
        Tab::Duplicates => duplicates::show(ui, env, scan),
        Tab::Cleanup => cleanup::show(ui, env, scan),
        Tab::Chart => chart::show(ui, env, scan),
    }
}

/// Points at the Cleanup tab when there is meaningful space to reclaim.
fn cleanup_chip(ui: &mut Ui, env: &Env<'_>, scan: &mut ScanView) {
    const WORTH_MENTIONING: u64 = 100 * 1024 * 1024;
    if scan.tab == Tab::Cleanup {
        return;
    }
    let reclaimable = cleanup::reclaimable(scan);
    if reclaimable < WORTH_MENTIONING {
        return;
    }
    let text = format!(
        "{} {}",
        crate::icons::SPARKLE,
        env.i18n
            .ta("cleanup-total", &[("size", env.size(reclaimable).into())])
    );
    row(ui, env.i18n.dir(), |ui| {
        let chip = egui::Button::new(RichText::new(text).color(theme::ACCENT_BRIGHT))
            .fill(theme::ACCENT.gamma_multiply(0.15))
            .stroke(Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.5)))
            .corner_radius(CornerRadius::same(14));
        if ui.add(chip).clicked() {
            scan.tab = Tab::Cleanup;
            scan.tab_changed = Some(Instant::now());
        }
    });
}

/// A rounded tab with animated hover and selection.
fn tab_pill(ui: &mut Ui, env: &Env<'_>, current: &mut Tab, tab: Tab) {
    let label = env.t(tab.label_id());
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text = format!("{}  {label}", tab.icon());
    let galley = ui
        .painter()
        .layout_no_wrap(text, font, Color32::PLACEHOLDER);
    let size = galley.size() + vec2(TAB_TEXT_PADDING, 10.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let selected = *current == tab;
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, selected, theme::ANIMATION_TIME);
    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered(),
        theme::ANIMATION_TIME,
    );
    let visuals = ui.visuals();
    let idle = visuals
        .panel_fill
        .lerp_to_gamma(visuals.widgets.hovered.bg_fill, hover);
    let fill = idle.lerp_to_gamma(theme::ACCENT, t);
    ui.painter().rect_filled(rect, CornerRadius::same(14), fill);
    let ink = visuals.text_color().lerp_to_gamma(Color32::WHITE, t);
    ui.painter().galley(
        Align2::CENTER_CENTER
            .anchor_size(rect.center(), galley.size())
            .min,
        galley,
        ink,
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, &label));
    if response.clicked() {
        *current = tab;
    }
}

fn breadcrumbs(ui: &mut Ui, env: &mut Env<'_>, scan: &ScanView) {
    let separator = if env.i18n.is_rtl() { "‹" } else { "›" };
    row(ui, env.i18n.dir(), |ui| {
        // Up sits with the path it navigates.
        let can_go_up = scan.tree.parent(scan.focus).is_some();
        if ui
            .add_enabled_ui(can_go_up, |ui| {
                icon_button(ui, crate::icons::UP, &env.t("action-up"))
            })
            .inner
            .clicked()
        {
            env.push(Action::Up);
        }
        let node = scan.tree.node(scan.focus);
        let size = RichText::new(env.i18n.v(&env.metric_text(node)))
            .strong()
            .color(theme::ACCENT_BRIGHT);
        ui.with_layout(env.i18n.dir().end_row(), |ui| {
            // At the top the folder panel already shows the total.
            if scan.focus != scan.tree.root() {
                ui.label(size);
            }
            egui::ScrollArea::horizontal()
                .id_salt("crumbs")
                .show(ui, |ui| {
                    row(ui, env.i18n.dir(), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (i, id) in scan.tree.ancestry(scan.focus).into_iter().enumerate() {
                            if i > 0 {
                                ui.weak(separator);
                            }
                            let name = env.i18n.v(&scan.tree.name(id));
                            let text = if id == scan.focus {
                                RichText::new(name).strong()
                            } else {
                                RichText::new(name)
                            };
                            if ui
                                .add(egui::Button::new(text).frame(id == scan.focus))
                                .clicked()
                            {
                                env.push(Action::Focus(id));
                            }
                        }
                    });
                });
        });
    });
}

/// "What's new" for an available update, with install / skip / later.
fn release_notes(app: &mut App, ctx: &egui::Context) {
    let Some(release) = app.updater.available().filter(|_| app.show_release_notes) else {
        return;
    };
    let i = &app.i18n;
    let notes = crate::update::plain_notes(&release.notes);
    let (version, page, installable) = (
        release.version.to_string(),
        release.page.clone(),
        release.installable(),
    );
    let modal = Modal::new(Id::unique("release-notes")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
            ui.heading(i.ta("update-available", &[("version", version.clone().into())]));
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    if notes.is_empty() {
                        ui.weak(i.t("update-no-notes"));
                    } else {
                        paragraph(ui, i, &notes);
                    }
                });
            ui.hyperlink_to(
                format!("{} {}", crate::icons::GITHUB, i.t("action-open-github")),
                page,
            );
            ui.add_space(10.0);
            row(ui, i.dir(), |ui| {
                let install_id = if installable {
                    "update-install"
                } else {
                    "update-manual"
                };
                if ui.add(theme::primary_button(i.t(install_id))).clicked() {
                    app.actions.push(Action::InstallUpdate);
                    app.actions.push(Action::ShowReleaseNotes(false));
                }
                if ui.button(i.t("update-later")).clicked() {
                    app.actions.push(Action::ShowReleaseNotes(false));
                }
                if ui.button(i.t("update-skip")).clicked() {
                    app.actions.push(Action::SkipUpdate(version.clone()));
                }
            });
        });
    });
    if modal.should_close() {
        app.actions.push(Action::ShowReleaseNotes(false));
    }
}

pub fn modals(app: &mut App, ctx: &egui::Context) {
    release_notes(app, ctx);
    selection::dialogs(app, ctx);
    if let (Some(id), Some(scan)) = (app.confirm_trash, &app.scan) {
        let i = &app.i18n;
        let node = scan.tree.node(id);
        let size = fmt::size(node.size, app.settings.unit, 1, |k| i.logical(k));
        let body = i.la(
            "confirm-trash-body",
            &[
                ("name", scan.tree.name(id).into_owned().into()),
                ("size", size.into()),
            ],
        );
        let modal = Modal::new(Id::unique("confirm-trash")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
                ui.heading(format!(
                    "{}  {}",
                    crate::icons::TRASH,
                    i.t("confirm-trash-title")
                ));
                ui.add_space(4.0);
                paragraph(ui, i, &body);
                ui.add_space(12.0);
                row(ui, i.dir(), |ui| {
                    let danger = egui::Button::new(
                        RichText::new(i.t("confirm-trash")).color(Color32::WHITE),
                    )
                    .fill(ui.visuals().error_fg_color);
                    if ui.add(danger).clicked() {
                        app.actions.push(Action::ConfirmTrash);
                    }
                    if ui.button(i.t("cancel")).clicked() {
                        app.actions.push(Action::CancelTrash);
                    }
                });
            });
        });
        if modal.should_close() {
            app.actions.push(Action::CancelTrash);
        }
    }
    if app.show_export {
        export_options(app, ctx);
    }
    if let (Some(id), Some(scan)) = (app.confirm_empty, &app.scan) {
        let i = &app.i18n;
        let size = fmt::size(scan.tree.node(id).size, app.settings.unit, 1, |k| {
            i.logical(k)
        });
        let body = i.la("confirm-empty-body", &[("size", size.into())]);
        let modal = Modal::new(Id::unique("confirm-empty-trash")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
                ui.heading(format!(
                    "{}  {}",
                    crate::icons::TRASH,
                    i.t("confirm-empty-title")
                ));
                ui.add_space(4.0);
                paragraph(ui, i, &body);
                ui.add_space(12.0);
                row(ui, i.dir(), |ui| {
                    let danger = egui::Button::new(
                        RichText::new(i.t("action-empty-trash")).color(Color32::WHITE),
                    )
                    .fill(ui.visuals().error_fg_color);
                    if ui.add(danger).clicked() {
                        app.actions.push(Action::ConfirmEmptyTrash);
                    }
                    if ui.button(i.t("cancel")).clicked() {
                        app.actions.push(Action::CancelEmptyTrash);
                    }
                });
            });
        });
        if modal.should_close() {
            app.actions.push(Action::CancelEmptyTrash);
        }
    }
    if app.show_about {
        app.show_about = false;
        app.settings_section = super::settings::Section::About;
        app.show_settings = true;
    }
}

/// What the CSV report lists and how sizes are written, asked before saving.
fn export_options(app: &mut App, ctx: &egui::Context) {
    let App {
        i18n: i,
        settings,
        actions,
        ..
    } = app;
    let modal = Modal::new(Id::unique("export-options")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
            ui.heading(format!("{}  {}", crate::icons::EXPORT, i.t("export-title")));
            ui.add_space(8.0);
            ui.label(RichText::new(i.t("export-format")).strong());
            let layout = i.dir().row().with_main_wrap(true);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), layout, |ui| {
                for (format, id) in Format::ALL.into_iter().zip([
                    "export-format-csv",
                    "export-format-excel",
                    "export-format-html",
                    "export-format-json",
                    "export-format-text",
                ]) {
                    ui.selectable_value(&mut settings.export_format, format, i.t(id));
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new(i.t("export-depth")).strong());
            let depth_text = |d: Option<u8>| match d {
                Some(n) => i.ta("export-depth-levels", &[("count", u64::from(n).into())]),
                None => i.t("export-depth-all"),
            };
            egui::ComboBox::from_id_salt("export-depth")
                .selected_text(depth_text(settings.export_depth))
                .show_ui(ui, |ui| {
                    for d in [Some(1), Some(2), Some(3), Some(5), None] {
                        ui.selectable_value(&mut settings.export_depth, d, depth_text(d));
                    }
                });
            ui.add_space(8.0);
            ui.label(RichText::new(i.t("export-columns")).strong());
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), layout, |ui| {
                let all: Vec<usize> = (0..storagemap_core::export::COLUMN_KEYS.len()).collect();
                if settings.export_columns.is_empty() {
                    settings.export_columns = all.clone();
                }
                for (c, id) in all.into_iter().zip([
                    "col-path",
                    "col-type",
                    "col-size",
                    "col-allocated",
                    "col-files",
                    "col-folders",
                    "col-percent",
                    "col-modified",
                ]) {
                    let mut on = settings.export_columns.contains(&c);
                    // The path identifies each row, so it always stays.
                    let enabled = c != 0;
                    if ui
                        .add_enabled(enabled, egui::Checkbox::new(&mut on, i.t(id)))
                        .changed()
                    {
                        if on {
                            settings.export_columns.push(c);
                            settings.export_columns.sort_unstable();
                        } else {
                            settings.export_columns.retain(|&x| x != c);
                        }
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new(i.t("export-include")).strong());
            row(ui, i.dir(), |ui| {
                for (include, id) in Include::ALL.into_iter().zip([
                    "export-include-all",
                    "export-include-folders",
                    "export-include-files",
                ]) {
                    ui.selectable_value(&mut settings.export_include, include, i.t(id));
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new(i.t("export-sizes")).strong());
            row(ui, i.dir(), |ui| {
                ui.selectable_value(
                    &mut settings.export_readable,
                    true,
                    i.t("export-sizes-readable"),
                );
                ui.selectable_value(
                    &mut settings.export_readable,
                    false,
                    i.t("export-sizes-bytes"),
                );
            });
            ui.add_space(14.0);
            row(ui, i.dir(), |ui| {
                if ui.add(theme::primary_button(i.t("export-save"))).clicked() {
                    actions.push(Action::ConfirmExport);
                }
                if ui
                    .button(format!("{}  {}", crate::icons::COPY, i.t("export-copy")))
                    .on_hover_text(i.t("export-copy-hint"))
                    .clicked()
                {
                    actions.push(Action::CopyReport);
                }
                if ui.button(i.t("cancel")).clicked() {
                    actions.push(Action::CancelExport);
                }
            });
        });
    });
    if modal.should_close() {
        actions.push(Action::CancelExport);
    }
}

/// The floating notification: fades and slides in at the bottom centre.
pub fn toast(app: &App, ctx: &egui::Context) {
    let Some(toast) = &app.toast else { return };
    let opacity = toast.opacity();
    let lift = 12.0 * (1.0 - opacity);
    egui::Area::new(Id::unique("toast"))
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -36.0 + lift))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            let accent = if toast.is_error {
                ui.visuals().error_fg_color
            } else {
                theme::ACCENT
            };
            Frame::popup(ui.style())
                .corner_radius(CornerRadius::same(20))
                .inner_margin(Margin::symmetric(16, 8))
                .stroke(Stroke::new(1.5, accent))
                .show(ui, |ui| {
                    let icon = if toast.is_error {
                        crate::icons::WARNING
                    } else {
                        crate::icons::CHECK
                    };
                    row(ui, app.i18n.dir(), |ui| {
                        ui.colored_label(accent, icon);
                        ui.label(&toast.text);
                    });
                });
        });
}

pub fn keyboard(app: &mut App, ctx: &egui::Context) {
    let shortcut = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
    if shortcut(OPEN) {
        app.actions.push(Action::PickFolder);
    }
    if shortcut(SETTINGS) {
        app.actions.push(Action::OpenSettings);
    }
    if shortcut(QUIT) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    if shortcut(EXPORT) && app.scan.is_some() {
        app.actions.push(Action::ExportCsv);
    }
    if shortcut(FIND)
        && let Some(scan) = &mut app.scan
    {
        if scan.tab != Tab::Search {
            scan.tab = Tab::Search;
            scan.tab_changed = Some(Instant::now());
        }
        scan.focus_search = true;
    }
    if ctx.egui_wants_keyboard_input()
        || app.confirm_trash.is_some()
        || app.confirm_bulk_trash
        || app.rename.is_some()
        || app.confirm_empty.is_some()
        || app.show_export
        || app.show_settings
    {
        return;
    }
    let key = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
    if key(Key::F5) {
        app.actions.push(Action::Rescan);
    }
    if key(Key::Escape) && app.job.is_some() {
        app.actions.push(Action::Stop);
    }
    let rtl = app.i18n.is_rtl();
    let metric = app.settings.metric;
    let Some(scan) = &mut app.scan else { return };
    if key(Key::Backspace) {
        app.actions.push(Action::Up);
    }
    let selected = scan.selected.unwrap_or(scan.focus);
    if key(Key::Delete) {
        app.actions.push(Action::AskTrash(selected));
    }
    if key(Key::Enter) {
        app.actions.push(Action::Focus(selected));
    }
    // Folder tree navigation; Left/Right swap meaning in RTL.
    let (open_key, close_key) = if rtl {
        (Key::ArrowLeft, Key::ArrowRight)
    } else {
        (Key::ArrowRight, Key::ArrowLeft)
    };
    let (down, up, open, close) = (
        key(Key::ArrowDown),
        key(Key::ArrowUp),
        key(open_key),
        key(close_key),
    );
    if !(down || up || open || close) {
        return;
    }
    tree_panel::refresh(scan, metric);
    let rows = tree_panel::rows(scan);
    let Some(index) = rows.iter().position(|r| r.id == selected) else {
        return;
    };
    let r = rows[index];
    if down && index + 1 < rows.len() {
        app.actions.push(Action::Focus(rows[index + 1].id));
    } else if up && index > 0 {
        app.actions.push(Action::Focus(rows[index - 1].id));
    } else if open && r.has_children {
        scan.set_expanded(r.id, true);
    } else if close {
        if r.expanded && r.has_children {
            scan.set_expanded(r.id, false);
        } else if let Some(parent) = scan.tree.parent(r.id) {
            app.actions.push(Action::Focus(parent));
        }
    }
}
