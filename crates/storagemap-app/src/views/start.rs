//! Start screen (hero, drive cards, recent folders) and the scan progress view.

use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    pos2, vec2,
};
use storagemap_core::ProgressSnapshot;
use storagemap_core::volumes::{Volume, VolumeKind};

use super::Env;
use crate::app::Action;
use crate::fmt;
use crate::platform;
use crate::theme;
use crate::widgets::{paint_text, paragraph_small, row, share_bar};

const CONTENT_MAX_WIDTH: f32 = 1040.0;
const CARD_SIZE: egui::Vec2 = vec2(244.0, 104.0);
const CARD_GAP: f32 = 12.0;
const HERO_LOGO: f32 = 64.0;
/// Fill ratio at which a drive bar turns amber, then red.
const WARN_AT: f64 = 0.80;
const FULL_AT: f64 = 0.92;
const AMBER: Color32 = Color32::from_rgb(234, 160, 40);

fn kind_label_id(kind: VolumeKind) -> &'static str {
    match kind {
        VolumeKind::Fixed => "volume-fixed",
        VolumeKind::Removable => "volume-removable",
        VolumeKind::Network => "volume-network",
        VolumeKind::Optical => "volume-optical",
        VolumeKind::Ram => "volume-ram",
        VolumeKind::Unknown => "volume-unknown",
    }
}

fn kind_icon(kind: VolumeKind) -> &'static str {
    match kind {
        VolumeKind::Fixed => crate::icons::DRIVE,
        VolumeKind::Removable => crate::icons::USB,
        VolumeKind::Network => crate::icons::NETWORK,
        VolumeKind::Optical => crate::icons::DISC,
        VolumeKind::Ram | VolumeKind::Unknown => crate::icons::MEMORY,
    }
}

/// Centres a column of at most `CONTENT_MAX_WIDTH`.
fn centered(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let width = ui.available_width().min(CONTENT_MAX_WIDTH);
    let margin = ((ui.available_width() - width) / 2.0).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_width(width);
            add(ui);
        });
    });
}

pub fn show(
    ui: &mut Ui,
    env: &mut Env<'_>,
    volumes: &[Volume],
    elevated: bool,
    logo: &egui::TextureHandle,
) {
    let dir = env.i18n.dir();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            centered(ui, |ui| {
                ui.with_layout(Layout::top_down(dir.start()), |ui| {
                    ui.add_space(28.0);
                    let hero = vec2(ui.available_width(), HERO_LOGO);
                    ui.allocate_ui_with_layout(hero, dir.row(), |ui| {
                        ui.add(
                            egui::Image::new(logo).fit_to_exact_size(vec2(HERO_LOGO, HERO_LOGO)),
                        );
                        ui.add_space(8.0);
                        ui.with_layout(Layout::top_down(dir.start()), |ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new(env.t("start-title")).size(30.0).strong());
                            ui.label(RichText::new(env.t("start-subtitle")).size(15.5).weak());
                        });
                    });
                    ui.add_space(18.0);
                    row(ui, dir, |ui| {
                        let primary = theme::primary_button(
                            RichText::new(format!(
                                "{}  {}",
                                crate::icons::FOLDER_OPEN,
                                env.t("action-scan-folder")
                            ))
                            .size(16.0),
                        )
                        .min_size(vec2(0.0, 36.0));
                        if ui.add(primary).clicked() {
                            env.push(Action::PickFolder);
                        }
                        ui.label(RichText::new(env.t("start-drop-hint")).weak().small());
                    });
                    if platform::ELEVATION_HELPS && !elevated {
                        ui.add_space(14.0);
                        admin_card(ui, env);
                    }
                    ui.add_space(26.0);
                    ui.label(RichText::new(env.t("start-drives")).size(18.0).strong());
                    ui.add_space(6.0);
                    drive_grid(ui, env, volumes);
                    if !env.settings.recent.is_empty() {
                        ui.add_space(26.0);
                        ui.label(RichText::new(env.t("start-recent")).size(18.0).strong());
                        ui.add_space(4.0);
                        for path in env.settings.recent.clone() {
                            recent_row(ui, env, &path);
                        }
                    }
                    ui.add_space(24.0);
                });
            });
        });
}

fn admin_card(ui: &mut Ui, env: &mut Env<'_>) {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_max_width(620.0);
            ui.with_layout(Layout::top_down(env.i18n.dir().start()), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        crate::icons::LIGHTNING,
                        env.t("backend-mft")
                    ))
                    .strong(),
                );
                paragraph_small(ui, env.i18n, &env.i18n.logical("admin-hint"));
                ui.add_space(4.0);
                if ui
                    .button(format!(
                        "{}  {}",
                        crate::icons::SHIELD,
                        env.t("action-run-as-admin")
                    ))
                    .clicked()
                {
                    env.push(Action::RunAsAdmin);
                }
            });
        });
}

/// Cards laid out in as many columns as fit, flowing from the start edge.
fn drive_grid(ui: &mut Ui, env: &mut Env<'_>, volumes: &[Volume]) {
    let per_row = ((ui.available_width() + CARD_GAP) / (CARD_SIZE.x + CARD_GAP))
        .floor()
        .max(1.0) as usize;
    for chunk in volumes.chunks(per_row) {
        row(ui, env.i18n.dir(), |ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            for v in chunk {
                volume_card(ui, env, v);
            }
        });
        ui.add_space(CARD_GAP - ui.spacing().item_spacing.y);
    }
}

fn volume_card(ui: &mut Ui, env: &mut Env<'_>, v: &Volume) {
    let (slot, response) = ui.allocate_exact_size(CARD_SIZE, Sense::click());
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id, response.hovered(), theme::ANIMATION_TIME);
    // Lift slightly on hover.
    let rect = slot.translate(vec2(0.0, -2.0 * hover));
    let visuals = ui.visuals();
    let painter = ui.painter();
    let shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: (6.0 + 10.0 * hover) as u8,
        spread: 0,
        color: Color32::from_black_alpha((30.0 + 40.0 * hover) as u8),
    };
    painter.add(shadow.as_shape(rect, CornerRadius::same(theme::CARD_RADIUS)));
    painter.rect_filled(rect, theme::CARD_RADIUS, visuals.faint_bg_color);
    let border = visuals
        .widgets
        .noninteractive
        .bg_stroke
        .color
        .lerp_to_gamma(theme::ACCENT, hover);
    painter.rect_stroke(
        rect,
        theme::CARD_RADIUS,
        Stroke::new(1.0 + hover, border),
        StrokeKind::Inside,
    );

    let dir = env.i18n.dir();
    let inner = rect.shrink2(vec2(14.0, 12.0));
    let line = |i: f32, h: f32| {
        Rect::from_min_size(pos2(inner.left(), inner.top() + i), vec2(inner.width(), h))
    };
    let title = if v.label.is_empty() {
        v.mount.to_string_lossy().into_owned()
    } else {
        format!("{} ({})", v.label, v.mount.display())
    };
    paint_text(
        painter,
        env.i18n,
        line(0.0, 20.0),
        &format!("{} {title}", kind_icon(v.kind)),
        FontId::proportional(15.5),
        visuals.strong_text_color(),
        false,
    );
    let sub = format!(
        "{} · {}",
        env.i18n.logical(kind_label_id(v.kind)),
        v.fs_type
    );
    paint_text(
        painter,
        env.i18n,
        line(22.0, 16.0),
        &sub,
        FontId::proportional(12.0),
        visuals.weak_text_color(),
        false,
    );

    let used = storagemap_core::ratio(v.used(), v.total);
    let fill = if used >= FULL_AT {
        visuals.error_fg_color
    } else if used >= WARN_AT {
        AMBER
    } else {
        theme::ACCENT
    };
    share_bar(
        painter,
        dir,
        line(46.0, 8.0),
        used,
        fill,
        visuals.extreme_bg_color,
    );
    let free = env.i18n.la(
        "volume-free",
        &[
            ("free", env.size(v.free).into()),
            ("total", env.size(v.total).into()),
        ],
    );
    paint_text(
        painter,
        env.i18n,
        line(60.0, 18.0),
        &free,
        FontId::proportional(12.5),
        visuals.text_color(),
        false,
    );
    paint_text(
        painter,
        env.i18n,
        line(60.0, 18.0),
        &fmt::percent(used),
        FontId::proportional(12.5),
        fill,
        true,
    );

    let response = response.on_hover_text(env.i18n.v(&v.mount.to_string_lossy()));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &title));
    if response.clicked() {
        env.push(Action::StartScan(v.mount.clone()));
    }
}

fn recent_row(ui: &mut Ui, env: &mut Env<'_>, path: &std::path::Path) {
    let height = 30.0;
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(720.0), height),
        Sense::click(),
    );
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id, response.hovered(), theme::ANIMATION_TIME);
    let visuals = ui.visuals();
    let fill = Color32::TRANSPARENT.lerp_to_gamma(visuals.widgets.hovered.bg_fill, hover);
    ui.painter().rect_filled(rect, theme::RADIUS, fill);
    let text = rect.shrink2(vec2(10.0, 0.0));
    let label = format!("{}  {}", crate::icons::FOLDER, path.display());
    paint_text(
        ui.painter(),
        env.i18n,
        text,
        &label,
        FontId::proportional(14.0),
        visuals.text_color(),
        false,
    );
    let chevron = if env.i18n.is_rtl() { "‹" } else { "›" };
    paint_text(
        ui.painter(),
        env.i18n,
        text,
        chevron,
        FontId::proportional(16.0),
        visuals.weak_text_color(),
        true,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, path.to_string_lossy()));
    if response.clicked() {
        env.push(Action::StartScan(path.to_path_buf()));
    }
}

/// Full-screen progress while the first scan runs.
pub fn progress(
    ui: &mut Ui,
    env: &mut Env<'_>,
    snap: &ProgressSnapshot,
    root: &std::path::Path,
    seconds: String,
) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(ui.available_height() * 0.22);
        ui.label(
            RichText::new(env.i18n.v(&root.to_string_lossy()))
                .size(20.0)
                .strong(),
        );
        ui.add_space(14.0);
        indeterminate_bar(ui, 420.0);
        ui.add_space(14.0);
        let line = env.i18n.ta(
            "scan-progress",
            &[
                ("files", fmt::count(snap.files).into()),
                ("dirs", fmt::count(snap.dirs).into()),
                ("size", env.size(snap.bytes).into()),
            ],
        );
        ui.label(RichText::new(line).size(15.0));
        ui.label(RichText::new(format!("{seconds} s")).weak());
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 20.0), Sense::hover());
        let color = ui.visuals().weak_text_color();
        paint_text(
            ui.painter(),
            env.i18n,
            rect,
            &snap.current.to_string_lossy(),
            FontId::proportional(12.0),
            color,
            false,
        );
        ui.add_space(16.0);
        if ui
            .button(format!("{}  {}", crate::icons::STOP, env.t("action-stop")))
            .clicked()
        {
            env.push(Action::Stop);
        }
    });
}

/// A track with a highlight sweeping across it (unknown total).
fn indeterminate_bar(ui: &mut Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let t = (ui.input(|i| i.time) * 0.7).fract() as f32;
    let seg = width * 0.3;
    let x = rect.left() - seg + (width + seg) * theme::ease_out(t);
    let bar = Rect::from_min_max(
        pos2(x.max(rect.left()), rect.top()),
        pos2((x + seg).min(rect.right()), rect.bottom()),
    );
    if bar.width() > 0.0 {
        painter.rect_filled(bar, 3.0, theme::ACCENT);
    }
    ui.ctx().request_repaint();
}
