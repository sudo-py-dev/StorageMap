//! The settings window: grouped sections with a side navigation.

use egui::{Align, Id, Layout, Modal, RichText, ScrollArea, Sides, Ui, vec2};
use storagemap_core::Metric;

use crate::app::{Action, App};
use crate::fmt::{self, Unit};
use crate::i18n::{Direction, I18n, Lang};
use crate::platform;
use crate::settings::{MAX_TREEMAP_DEPTH, Settings, Theme};
use crate::theme;
use crate::update::{self, Updater};
use crate::widgets::paragraph_small;

const WINDOW_SIZE: egui::Vec2 = vec2(780.0, 520.0);
const NAV_WIDTH: f32 = 180.0;
const SCALES: [f32; 7] = [0.8, 0.9, 1.0, 1.1, 1.25, 1.4, 1.6];
const DUP_SIZES: [u64; 5] = [
    1024,
    100 * 1024,
    1024 * 1024,
    10 * 1024 * 1024,
    100 * 1024 * 1024,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    General,
    Appearance,
    Scanning,
    Integration,
    Updates,
    Shortcuts,
    About,
}

impl Section {
    const ALL: [Section; 7] = [
        Section::General,
        Section::Appearance,
        Section::Scanning,
        Section::Integration,
        Section::Updates,
        Section::Shortcuts,
        Section::About,
    ];

    /// Parses `general`, `updates`, ... (used by the screenshot hook).
    pub fn from_name(name: &str) -> Option<Section> {
        Section::ALL
            .into_iter()
            .find(|s| s.label().1.trim_start_matches("settings-") == name)
    }

    fn label(self) -> (&'static str, &'static str) {
        match self {
            Section::General => ("⚙", "settings-general"),
            Section::Appearance => ("🎨", "settings-appearance"),
            Section::Scanning => ("🔍", "settings-scanning"),
            Section::Integration => ("🧩", "settings-integration"),
            Section::Updates => ("⟳", "settings-updates"),
            Section::Shortcuts => ("⌨", "settings-shortcuts"),
            Section::About => ("ℹ", "settings-about"),
        }
    }
}

pub fn window(app: &mut App, ctx: &egui::Context) {
    if !app.show_settings {
        return;
    }
    let dir = app.i18n.dir();
    let modal = Modal::new(Id::unique("settings")).show(ctx, |ui| {
        ui.set_min_size(WINDOW_SIZE);
        ui.set_max_size(WINDOW_SIZE);
        let top = match dir {
            Direction::Ltr => Layout::left_to_right(Align::Min),
            Direction::Rtl => Layout::right_to_left(Align::Min),
        };
        ui.with_layout(top, |ui| {
            navigation(ui, app);
            ui.separator();
            ui.vertical(|ui| {
                ui.with_layout(Layout::top_down(dir.start()), |ui| {
                    let (_, id) = app.settings_section.label();
                    ui.heading(app.i18n.t(id));
                    ui.add_space(4.0);
                    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        ui.with_layout(Layout::top_down(dir.start()), |ui| section(ui, app, ctx));
                    });
                });
            });
        });
    });
    if modal.should_close() {
        app.show_settings = false;
    }
}

fn navigation(ui: &mut Ui, app: &mut App) {
    ui.allocate_ui_with_layout(
        vec2(NAV_WIDTH, WINDOW_SIZE.y),
        Layout::top_down_justified(app.i18n.dir().start()),
        |ui| {
            ui.label(
                RichText::new(app.i18n.t("settings-title"))
                    .strong()
                    .size(17.0),
            );
            ui.add_space(8.0);
            for s in Section::ALL {
                let (icon, id) = s.label();
                let label = format!("{icon}  {}", app.i18n.t(id));
                if ui
                    .selectable_label(app.settings_section == s, RichText::new(label).size(14.5))
                    .clicked()
                {
                    app.settings_section = s;
                }
            }
        },
    );
}

/// Title at the start edge, control at the end edge, optional hint below.
fn row(
    ui: &mut Ui,
    i18n: &I18n,
    title_id: &str,
    hint_id: Option<&str>,
    control: impl FnOnce(&mut Ui),
) {
    let title = |ui: &mut Ui| ui.label(RichText::new(i18n.t(title_id)).strong());
    match i18n.dir() {
        Direction::Ltr => {
            Sides::new().show(ui, title, control);
        }
        Direction::Rtl => {
            Sides::new().show(ui, control, title);
        }
    }
    if let Some(hint) = hint_id {
        paragraph_small(ui, i18n, &i18n.logical(hint));
    }
    ui.add_space(10.0);
}

fn section(ui: &mut Ui, app: &mut App, ctx: &egui::Context) {
    let current = app.settings_section;
    let App {
        i18n,
        settings,
        actions,
        updater,
        elevated,
        ..
    } = app;
    match current {
        Section::General => general(ui, i18n, settings, actions),
        Section::Appearance => appearance(ui, i18n, settings, ctx),
        Section::Scanning => scanning(ui, i18n, settings, actions, *elevated),
        Section::Integration => integration(ui, i18n, actions),
        Section::Updates => updates(ui, i18n, settings, updater, actions, ctx),
        Section::Shortcuts => shortcuts(ui, i18n),
        Section::About => about(ui, i18n),
    }
}

fn general(ui: &mut Ui, i18n: &I18n, settings: &mut Settings, actions: &mut Vec<Action>) {
    row(ui, i18n, "setting-language", None, |ui| {
        let current = settings.lang.map_or_else(
            || i18n.t("lang-system"),
            |l| crate::i18n::bidi::visual(l.native_name(), l.direction()),
        );
        egui::ComboBox::from_id_salt("lang")
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.lang.is_none(), i18n.t("lang-system"))
                    .clicked()
                {
                    actions.push(Action::SetLang(None));
                }
                for lang in Lang::ALL {
                    let name = crate::i18n::bidi::visual(lang.native_name(), lang.direction());
                    if ui
                        .selectable_label(settings.lang == Some(lang), name)
                        .clicked()
                    {
                        actions.push(Action::SetLang(Some(lang)));
                    }
                }
            });
    });
    row(ui, i18n, "metric-label", None, |ui| {
        for (metric, id) in [
            (Metric::Size, "metric-size"),
            (Metric::Allocated, "metric-allocated"),
            (Metric::FileCount, "metric-files"),
        ] {
            ui.selectable_value(&mut settings.metric, metric, i18n.t(id));
        }
    });
    row(ui, i18n, "view-units", None, |ui| {
        egui::ComboBox::from_id_salt("units")
            .selected_text(i18n.t(settings.unit.label_id()))
            .show_ui(ui, |ui| {
                for unit in Unit::ALL {
                    ui.selectable_value(&mut settings.unit, unit, i18n.t(unit.label_id()));
                }
            });
    });
    row(ui, i18n, "view-decimals", None, |ui| {
        ui.add(egui::Slider::new(&mut settings.decimals, 0..=3));
    });
}

fn appearance(ui: &mut Ui, i18n: &I18n, settings: &mut Settings, ctx: &egui::Context) {
    row(ui, i18n, "view-theme", None, |ui| {
        for t in Theme::ALL {
            if ui
                .selectable_value(&mut settings.theme, t, i18n.t(t.label_id()))
                .clicked()
            {
                ctx.set_theme(t.preference());
            }
        }
    });
    row(
        ui,
        i18n,
        "setting-scale",
        Some("setting-scale-hint"),
        |ui| {
            egui::ComboBox::from_id_salt("scale")
                .selected_text(format!("{:.0}%", settings.ui_scale * 100.0))
                .show_ui(ui, |ui| {
                    for s in SCALES {
                        if ui
                            .selectable_label(
                                (settings.ui_scale - s).abs() < 0.01,
                                format!("{:.0}%", s * 100.0),
                            )
                            .clicked()
                        {
                            settings.ui_scale = s;
                            ctx.set_zoom_factor(s);
                        }
                    }
                });
        },
    );
    row(
        ui,
        i18n,
        "treemap-depth",
        Some("setting-treemap-depth-hint"),
        |ui| {
            ui.add(egui::Slider::new(
                &mut settings.treemap_depth,
                1..=MAX_TREEMAP_DEPTH,
            ));
        },
    );
    row(ui, i18n, "treemap-files", None, |ui| {
        toggle(ui, &mut settings.treemap_files, i18n.is_rtl());
    });
    row(ui, i18n, "treemap-labels", None, |ui| {
        toggle(ui, &mut settings.treemap_labels, i18n.is_rtl());
    });
}

fn scanning(
    ui: &mut Ui,
    i18n: &I18n,
    settings: &mut Settings,
    actions: &mut Vec<Action>,
    elevated: bool,
) {
    row(
        ui,
        i18n,
        "option-auto-snapshot",
        Some("setting-auto-snapshot-hint"),
        |ui| {
            toggle(ui, &mut settings.auto_snapshot, i18n.is_rtl());
        },
    );
    row(
        ui,
        i18n,
        "option-cross-fs",
        Some("setting-cross-fs-hint"),
        |ui| {
            toggle(ui, &mut settings.cross_filesystems, i18n.is_rtl());
        },
    );
    row(ui, i18n, "dup-min-size", None, |ui| {
        let label = |b: u64| i18n.v(&fmt::size(b, Unit::Auto, 0, |id| i18n.logical(id)));
        egui::ComboBox::from_id_salt("dup-min-settings")
            .selected_text(label(settings.duplicate_min_size))
            .show_ui(ui, |ui| {
                for s in DUP_SIZES {
                    ui.selectable_value(&mut settings.duplicate_min_size, s, label(s));
                }
            });
    });
    if platform::ELEVATION_HELPS {
        row(ui, i18n, "option-mft", Some("setting-mft-hint"), |ui| {
            toggle(ui, &mut settings.prefer_mft, i18n.is_rtl());
        });
        if !elevated
            && ui
                .button(format!("🛡  {}", i18n.t("action-run-as-admin")))
                .clicked()
        {
            actions.push(Action::RunAsAdmin);
        }
    }
}

fn integration(ui: &mut Ui, i18n: &I18n, actions: &mut Vec<Action>) {
    paragraph_small(ui, i18n, &i18n.logical("integration-hint"));
    ui.add_space(8.0);
    ui.with_layout(i18n.dir().row(), |ui| {
        if ui
            .add(theme::primary_button(i18n.t("action-integrate")))
            .clicked()
        {
            actions.push(Action::Integrate(true));
        }
        if ui.button(i18n.t("action-unintegrate")).clicked() {
            actions.push(Action::Integrate(false));
        }
    });
}

fn updates(
    ui: &mut Ui,
    i18n: &I18n,
    settings: &mut Settings,
    updater: &mut Updater,
    actions: &mut Vec<Action>,
    ctx: &egui::Context,
) {
    row(
        ui,
        i18n,
        "setting-auto-update",
        Some("setting-auto-update-hint"),
        |ui| {
            toggle(ui, &mut settings.check_updates, i18n.is_rtl());
        },
    );
    ui.label(i18n.ta(
        "version-current",
        &[("version", env!("CARGO_PKG_VERSION").into())],
    ));
    ui.add_space(6.0);
    status(ui, i18n, updater, actions);
    ui.add_space(6.0);
    if !updater.is_busy()
        && ui
            .button(format!("⟳  {}", i18n.t("update-check-now")))
            .clicked()
    {
        updater.start_check(ctx);
    }
}

/// Current updater state as text plus the relevant action.
pub fn status(ui: &mut Ui, i18n: &I18n, updater: &Updater, actions: &mut Vec<Action>) {
    match updater {
        Updater::Idle => {}
        Updater::Checking(_) => {
            ui.with_layout(i18n.dir().row(), |ui| {
                ui.spinner();
                ui.label(i18n.t("update-checking"));
            });
        }
        Updater::UpToDate => {
            ui.colored_label(theme::SUCCESS, format!("✔ {}", i18n.t("update-up-to-date")));
        }
        Updater::Available(release) => {
            ui.label(
                RichText::new(i18n.ta(
                    "update-available",
                    &[("version", release.version.to_string().into())],
                ))
                .strong(),
            );
            ui.with_layout(i18n.dir().row(), |ui| {
                let install_id = if release.installable() {
                    "update-install"
                } else {
                    "update-manual"
                };
                if ui.add(theme::primary_button(i18n.t(install_id))).clicked() {
                    actions.push(Action::InstallUpdate);
                }
                if ui.button(i18n.t("update-notes")).clicked() {
                    actions.push(Action::ShowReleaseNotes(true));
                }
                if ui.button(i18n.t("update-skip")).clicked() {
                    actions.push(Action::SkipUpdate(release.version.to_string()));
                }
            });
        }
        Updater::Downloading { progress, .. } => {
            let (done, total) = (
                progress
                    .downloaded
                    .load(std::sync::atomic::Ordering::Relaxed),
                progress.total.load(std::sync::atomic::Ordering::Relaxed),
            );
            let size = |b| fmt::size(b, Unit::Auto, 1, |id| i18n.logical(id));
            ui.label(i18n.ta(
                "update-downloading",
                &[("done", size(done).into()), ("total", size(total).into())],
            ));
            if total > 0 && done >= total {
                ui.label(i18n.t("update-ready"));
            }
            let fraction = if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            };
            ui.add(
                egui::ProgressBar::new(fraction)
                    .animate(true)
                    .desired_width(320.0),
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        Updater::Failed(error) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                i18n.ta("update-failed", &[("error", error.clone().into())]),
            );
        }
        Updater::Finished(update::Installed::RestartToFinish) => {
            ui.colored_label(theme::SUCCESS, i18n.t("update-restart"));
            if ui
                .add(theme::primary_button(i18n.t("update-restart-now")))
                .clicked()
            {
                actions.push(Action::RestartApp);
            }
        }
        Updater::Finished(_) => {
            ui.label(i18n.t("update-handed-off"));
        }
    }
}

/// Keys and what they do; descriptions reuse the menu labels.
const SHORTCUTS: [(&str, &str); 14] = [
    ("Ctrl+O", "action-scan-folder"),
    ("F5", "action-rescan"),
    ("Esc", "action-stop"),
    ("Ctrl+E", "action-export-csv"),
    ("Ctrl+F", "tab-search"),
    ("Ctrl+,", "menu-settings"),
    ("Enter", "shortcut-enter"),
    ("Backspace", "action-up"),
    ("↑ ↓ ← →", "shortcut-arrows"),
    ("Delete", "action-trash"),
    ("Double-click", "shortcut-double-click"),
    ("Ctrl + / −", "action-zoom-in"),
    ("Ctrl+0", "action-zoom-reset"),
    ("Ctrl+Q", "action-quit"),
];

fn shortcuts(ui: &mut Ui, i18n: &I18n) {
    for (keys, id) in SHORTCUTS {
        let key = |ui: &mut Ui| {
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| ui.label(RichText::new(keys).monospace()));
        };
        let what = |ui: &mut Ui| {
            ui.label(i18n.t(id));
        };
        // Key chips sit at the end edge, like the controls on other pages.
        match i18n.dir() {
            Direction::Ltr => Sides::new().show(ui, what, key),
            Direction::Rtl => Sides::new().show(ui, key, what),
        };
        ui.add_space(2.0);
    }
}

fn about(ui: &mut Ui, i18n: &I18n) {
    ui.label(RichText::new("StorageMap").size(20.0).strong());
    ui.label(i18n.ta(
        "about-version",
        &[("version", env!("CARGO_PKG_VERSION").into())],
    ));
    ui.add_space(6.0);
    paragraph_small(ui, i18n, &i18n.logical("about-body"));
    ui.add_space(6.0);
    paragraph_small(ui, i18n, &i18n.logical("about-privacy"));
    ui.add_space(8.0);
    ui.hyperlink_to(
        format!("🔗 {}", i18n.t("about-source")),
        format!("https://github.com/{}", update::REPO),
    );
}

/// A switch-style checkbox.
pub fn toggle(ui: &mut Ui, on: &mut bool, rtl: bool) -> egui::Response {
    let size = vec2(38.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), *on, ""));
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, *on, theme::ANIMATION_TIME);
    let visuals = ui.visuals();
    let off_fill = visuals.widgets.inactive.bg_fill;
    let track = egui::Color32::from_rgb(
        lerp(off_fill.r(), theme::ACCENT.r(), t),
        lerp(off_fill.g(), theme::ACCENT.g(), t),
        lerp(off_fill.b(), theme::ACCENT.b(), t),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, rect.height() / 2.0, track);
    let radius = rect.height() / 2.0 - 3.0;
    let (off_x, on_x) = (rect.left() + radius + 3.0, rect.right() - radius - 3.0);
    let x = if rtl {
        egui::lerp(on_x..=off_x, t)
    } else {
        egui::lerp(off_x..=on_x, t)
    };
    painter.circle_filled(egui::pos2(x, rect.center().y), radius, egui::Color32::WHITE);
    response
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
}
