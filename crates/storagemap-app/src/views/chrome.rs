//! Window chrome: menus, toolbar, breadcrumbs, tabs, status bar, dialogs and
//! keyboard shortcuts.

use egui::{
    CentralPanel, Id, Key, KeyboardShortcut, Layout, Modal, Modifiers, Panel, RichText, Ui,
};
use storagemap_core::{Backend, Metric};

use super::{Env, ScanView, Tab, changes, details, duplicates, lists, start, tree_panel, treemap};
use crate::app::{Action, App, seconds};
use crate::fmt::{self, Unit};
use crate::i18n::{Direction, Lang};
use crate::platform;
use crate::settings::{MAX_TREEMAP_DEPTH, Theme};
use crate::widgets::{paragraph, row};

const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const EXPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
const FIND: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);

const METRICS: [(Metric, &str); 3] = [
    (Metric::Size, "metric-size"),
    (Metric::Allocated, "metric-allocated"),
    (Metric::FileCount, "metric-files"),
];

pub fn top_bar(app: &mut App, ui: &mut Ui) {
    Panel::top("menu").show(ui, |ui| {
        row(ui, app.i18n.dir(), |ui| {
            file_menu(app, ui);
            view_menu(app, ui);
            language_menu(app, ui);
            tools_menu(app, ui);
            help_menu(app, ui);
        });
    });
    if app.scan.is_some() {
        Panel::top("toolbar").show(ui, |ui| toolbar(app, ui));
    }
}

/// Menu contents aligned to the reading-start edge.
fn menu_column<R>(dir: Direction, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(Layout::top_down_justified(dir.start()), add)
        .inner
}

fn file_menu(app: &mut App, ui: &mut Ui) {
    let (has_scan, scanning) = (app.scan.is_some(), app.job.is_some());
    ui.menu_button(app.i18n.t("menu-file"), |ui| {
        menu_column(app.i18n.dir(), ui, |ui| {
            let i = &app.i18n;
            if ui
                .add(
                    egui::Button::new(i.t("action-scan-folder"))
                        .shortcut_text(ui.ctx().format_shortcut(&OPEN)),
                )
                .clicked()
            {
                app.actions.push(Action::PickFolder);
            }
            if ui
                .add_enabled(
                    has_scan,
                    egui::Button::new(i.t("action-rescan")).shortcut_text("F5"),
                )
                .clicked()
            {
                app.actions.push(Action::Rescan);
            }
            if ui
                .add_enabled(
                    scanning,
                    egui::Button::new(i.t("action-stop")).shortcut_text("Esc"),
                )
                .clicked()
            {
                app.actions.push(Action::Stop);
            }
            let export = egui::Button::new(i.t("action-export-csv"))
                .shortcut_text(ui.ctx().format_shortcut(&EXPORT));
            if ui.add_enabled(has_scan, export).clicked() {
                app.actions.push(Action::ExportCsv);
            }
            if ui
                .add_enabled(!scanning, egui::Button::new(i.t("action-home")))
                .clicked()
            {
                app.actions.push(Action::Home);
            }
            ui.separator();
            if ui.button(i.t("action-quit")).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    });
}

fn view_menu(app: &mut App, ui: &mut Ui) {
    let App { i18n, settings, .. } = app;
    let dir = i18n.dir();
    ui.menu_button(i18n.t("menu-view"), |ui| {
        ui.with_layout(Layout::top_down(dir.start()), |ui| {
            ui.label(RichText::new(i18n.t("metric-label")).weak());
            for (metric, id) in METRICS {
                ui.radio_value(&mut settings.metric, metric, i18n.t(id));
            }
            ui.separator();
            ui.menu_button(i18n.t("view-units"), |ui| {
                for unit in Unit::ALL {
                    ui.radio_value(&mut settings.unit, unit, i18n.t(unit.label_id()));
                }
            });
            ui.add(egui::Slider::new(&mut settings.decimals, 0..=3).text(i18n.t("view-decimals")));
            ui.separator();
            ui.menu_button(i18n.t("view-theme"), |ui| {
                for theme in Theme::ALL {
                    if ui
                        .radio_value(&mut settings.theme, theme, i18n.t(theme.label_id()))
                        .clicked()
                    {
                        ui.ctx().set_theme(theme.preference());
                    }
                }
            });
            ui.separator();
            ui.add(
                egui::Slider::new(&mut settings.treemap_depth, 1..=MAX_TREEMAP_DEPTH)
                    .text(i18n.t("treemap-depth")),
            );
            ui.checkbox(&mut settings.treemap_files, i18n.t("treemap-files"));
            ui.checkbox(&mut settings.treemap_labels, i18n.t("treemap-labels"));
        });
    });
}

fn language_menu(app: &mut App, ui: &mut Ui) {
    let current = app.settings.lang;
    ui.menu_button(format!("🌐 {}", app.i18n.t("menu-language")), |ui| {
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
    });
}

fn tools_menu(app: &mut App, ui: &mut Ui) {
    let (elevated, has_scan) = (app.elevated, app.scan.is_some());
    let App {
        i18n,
        settings,
        actions,
        ..
    } = app;
    let dir = i18n.dir();
    ui.menu_button(i18n.t("menu-tools"), |ui| {
        ui.with_layout(Layout::top_down_justified(dir.start()), |ui| {
            if ui.button(i18n.t("action-integrate")).clicked() {
                actions.push(Action::Integrate(true));
            }
            if ui.button(i18n.t("action-unintegrate")).clicked() {
                actions.push(Action::Integrate(false));
            }
            ui.separator();
            ui.checkbox(&mut settings.auto_snapshot, i18n.t("option-auto-snapshot"));
            if ui
                .add_enabled(has_scan, egui::Button::new(i18n.t("action-save-snapshot")))
                .clicked()
            {
                actions.push(Action::SaveSnapshot);
            }
            ui.separator();
            ui.checkbox(&mut settings.cross_filesystems, i18n.t("option-cross-fs"));
            if platform::ELEVATION_HELPS {
                ui.checkbox(&mut settings.prefer_mft, i18n.t("option-mft"));
                if !elevated
                    && ui
                        .button(format!("🛡 {}", i18n.t("action-run-as-admin")))
                        .clicked()
                {
                    actions.push(Action::RunAsAdmin);
                }
            }
        });
    });
}

fn help_menu(app: &mut App, ui: &mut Ui) {
    ui.menu_button(app.i18n.t("menu-help"), |ui| {
        if ui.button(app.i18n.t("help-about")).clicked() {
            app.actions.push(Action::About(true));
        }
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
    row(ui, i18n.dir(), |ui| {
        if ui
            .button(format!("📂 {}", i18n.t("action-scan-folder")))
            .clicked()
        {
            actions.push(Action::PickFolder);
        }
        if ui
            .add_enabled(
                job.is_none(),
                egui::Button::new(format!("⟳ {}", i18n.t("action-rescan"))),
            )
            .clicked()
        {
            actions.push(Action::Rescan);
        }
        let can_go_up = scan
            .as_ref()
            .is_some_and(|s| s.tree.parent(s.focus).is_some());
        let up = if i18n.is_rtl() { "⮕" } else { "⬅" };
        if ui
            .add_enabled(
                can_go_up,
                egui::Button::new(format!("{up} {}", i18n.t("action-up"))),
            )
            .clicked()
        {
            actions.push(Action::Up);
        }
        ui.separator();
        ui.label(i18n.t("metric-label"));
        let selected = METRICS
            .iter()
            .find(|(m, _)| *m == settings.metric)
            .map_or("metric-size", |(_, id)| id);
        egui::ComboBox::from_id_salt("metric")
            .selected_text(i18n.t(selected))
            .show_ui(ui, |ui| {
                for (metric, id) in METRICS {
                    ui.selectable_value(&mut settings.metric, metric, i18n.t(id));
                }
            });
        if let Some(job) = job {
            ui.separator();
            ui.spinner();
            let snap = job.progress.snapshot();
            ui.label(i18n.ta(
                "scan-progress",
                &[
                    ("files", fmt::count(snap.files).into()),
                    ("dirs", fmt::count(snap.dirs).into()),
                    (
                        "size",
                        fmt::size(snap.bytes, settings.unit, 1, |id| i18n.logical(id)).into(),
                    ),
                ],
            ));
            if ui.button(format!("⏹ {}", i18n.t("action-stop"))).clicked() {
                actions.push(Action::Stop);
            }
        }
    });
}

pub fn status_bar(app: &mut App, ui: &mut Ui) {
    Panel::bottom("status").show(ui, |ui| {
        row(ui, app.i18n.dir(), |ui| {
            if let Some(toast) = &app.toast {
                let color = if toast.is_error {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().strong_text_color()
                };
                ui.colored_label(color, &toast.text);
                return;
            }
            let Some(scan) = &app.scan else {
                ui.weak(format!("StorageMap {}", env!("CARGO_PKG_VERSION")));
                return;
            };
            let i = &app.i18n;
            ui.label(format!("⚡ {}", i.t(backend_label_id(scan.backend))));
            ui.separator();
            ui.label(i.ta(
                "scan-done",
                &[
                    ("items", fmt::count(scan.tree.len() as u64).into()),
                    ("seconds", seconds(scan.elapsed).into()),
                ],
            ));
            if scan.errors > 0 {
                ui.separator();
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    i.v(&i.count("scan-errors", scan.errors)),
                );
            }
            if scan.cancelled {
                ui.separator();
                ui.colored_label(ui.visuals().warn_fg_color, i.t("scan-cancelled"));
            }
            if let Some(cmp) = &scan.comparison
                && let Some(delta) = cmp.size_delta(&scan.tree, scan.tree.root())
            {
                ui.separator();
                let change = fmt::signed_size(delta, app.settings.unit, 1, |id| i.logical(id));
                let date = fmt::date_time(cmp.taken_at);
                ui.label(i.ta(
                    "compare-summary",
                    &[("change", change.into()), ("date", date.into())],
                ));
            }
            if let Some(v) = volume_of(&app.volumes, scan.tree.root_path()) {
                ui.separator();
                let free = fmt::size(v.free, app.settings.unit, 1, |id| i.logical(id));
                ui.label(i.ta("free-on-volume", &[("free", free.into())]));
            }
        });
    });
}

fn backend_label_id(backend: Backend) -> &'static str {
    match backend {
        Backend::LinuxStatx => "backend-linux",
        Backend::NtfsMft => "backend-mft",
        Backend::WindowsDirInfo => "backend-dirinfo",
        Backend::Portable => "backend-portable",
    }
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
    let App {
        i18n,
        settings,
        actions,
        scan,
        job,
        volumes,
        elevated,
        ..
    } = app;
    let mut env = Env {
        i18n,
        settings,
        actions,
    };
    match (scan.as_mut(), job.as_ref()) {
        (Some(scan), _) => {
            let panel = if env.i18n.is_rtl() {
                Panel::right("folders")
            } else {
                Panel::left("folders")
            };
            panel
                .resizable(true)
                .default_size(340.0)
                .min_size(200.0)
                .show(ui, |ui| {
                    tree_panel::show(ui, &mut env, scan);
                });
            CentralPanel::default().show(ui, |ui| central(ui, &mut env, scan));
        }
        (None, Some(job)) => {
            let snap = job.progress.snapshot();
            let secs = seconds(job.started.elapsed());
            CentralPanel::default().show(ui, |ui| {
                start::progress(ui, &mut env, &snap, &job.root, secs)
            });
        }
        (None, None) => {
            CentralPanel::default().show(ui, |ui| start::show(ui, &mut env, volumes, *elevated));
        }
    }
}

fn central(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    breadcrumbs(ui, env, scan);
    row(ui, env.i18n.dir(), |ui| {
        for tab in Tab::ALL {
            ui.selectable_value(&mut scan.tab, tab, env.t(tab.label_id()));
        }
    });
    ui.separator();
    match scan.tab {
        Tab::Treemap => treemap::show(ui, env, scan),
        Tab::Details => details::show(ui, env, scan),
        Tab::TopFiles => lists::top_files(ui, env, scan),
        Tab::Types => lists::types(ui, env, scan),
        Tab::Ages => lists::ages(ui, env, scan),
        Tab::Search => lists::search(ui, env, scan),
        Tab::Changes => changes::show(ui, env, scan),
        Tab::Duplicates => duplicates::show(ui, env, scan),
    }
}

fn breadcrumbs(ui: &mut Ui, env: &mut Env<'_>, scan: &ScanView) {
    let separator = if env.i18n.is_rtl() { "‹" } else { "›" };
    egui::ScrollArea::horizontal()
        .id_salt("crumbs")
        .show(ui, |ui| {
            row(ui, env.i18n.dir(), |ui| {
                for (i, id) in scan.tree.ancestry(scan.focus).into_iter().enumerate() {
                    if i > 0 {
                        ui.weak(separator);
                    }
                    if ui
                        .selectable_label(id == scan.focus, env.i18n.v(&scan.tree.name(id)))
                        .clicked()
                    {
                        env.push(Action::Focus(id));
                    }
                }
                ui.separator();
                let node = scan.tree.node(scan.focus);
                ui.strong(env.i18n.v(&env.metric_text(node)));
            });
        });
}

pub fn modals(app: &mut App, ctx: &egui::Context) {
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
            ui.set_width(420.0);
            ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
                ui.heading(i.t("confirm-trash-title"));
                paragraph(ui, i, &body);
                ui.add_space(8.0);
                row(ui, i.dir(), |ui| {
                    let confirm = egui::Button::new(
                        RichText::new(i.t("confirm-trash")).color(ui.visuals().error_fg_color),
                    );
                    if ui.add(confirm).clicked() {
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
    if app.show_about {
        let i = &app.i18n;
        let modal = Modal::new(Id::unique("about")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
                ui.heading("StorageMap");
                ui.label(i.ta(
                    "about-version",
                    &[("version", env!("CARGO_PKG_VERSION").into())],
                ));
                paragraph(ui, i, &i.logical("about-body"));
            });
        });
        if modal.should_close() {
            app.actions.push(Action::About(false));
        }
    }
}

pub fn keyboard(app: &mut App, ctx: &egui::Context) {
    let shortcut = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
    if shortcut(OPEN) {
        app.actions.push(Action::PickFolder);
    }
    if shortcut(EXPORT) && app.scan.is_some() {
        app.actions.push(Action::ExportCsv);
    }
    if shortcut(FIND)
        && let Some(scan) = &mut app.scan
    {
        scan.tab = Tab::Search;
    }
    if ctx.egui_wants_keyboard_input() || app.confirm_trash.is_some() {
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
    let key = (metric, scan.generation, scan.expansion_version);
    let rows = scan.tree_rows.get(key, || {
        tree_panel::build_rows(&scan.tree, &scan.expanded, metric)
    });
    let Some(index) = rows.iter().position(|r| r.id == selected) else {
        return;
    };
    let row = rows[index];
    if down && index + 1 < rows.len() {
        app.actions.push(Action::Focus(rows[index + 1].id));
    } else if up && index > 0 {
        app.actions.push(Action::Focus(rows[index - 1].id));
    } else if open && row.has_children {
        scan.set_expanded(row.id, true);
    } else if close {
        if row.expanded && row.has_children {
            scan.set_expanded(row.id, false);
        } else if let Some(parent) = scan.tree.parent(row.id) {
            app.actions.push(Action::Focus(parent));
        }
    }
}
