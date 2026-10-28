//! Start screen: volumes with usage, recent folders, and the scan progress view.

use egui::{Align, FontId, Layout, RichText, Sense, Ui, vec2};
use storagemap_core::ProgressSnapshot;
use storagemap_core::volumes::{Volume, VolumeKind};

use super::Env;
use crate::app::Action;
use crate::fmt;
use crate::platform;
use crate::widgets::{paint_text, paragraph, share_bar};

const CARD_SIZE: egui::Vec2 = vec2(250.0, 92.0);

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

pub fn show(ui: &mut Ui, env: &mut Env<'_>, volumes: &[Volume], elevated: bool) {
    let dir = env.i18n.dir();
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.with_layout(Layout::top_down(dir.start()), |ui| {
                ui.add_space(24.0);
                ui.label(RichText::new(env.t("start-title")).size(28.0).strong());
                ui.label(RichText::new(env.t("start-subtitle")).size(15.0).weak());
                ui.add_space(12.0);
                if ui
                    .button(
                        RichText::new(format!("📂  {}", env.t("action-scan-folder"))).size(16.0),
                    )
                    .clicked()
                {
                    env.push(Action::PickFolder);
                }
                if platform::ELEVATION_HELPS && !elevated {
                    ui.add_space(8.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_max_width(560.0);
                        paragraph(ui, env.i18n, &env.i18n.logical("admin-hint"));
                        if ui
                            .button(format!("🛡  {}", env.t("action-run-as-admin")))
                            .clicked()
                        {
                            env.push(Action::RunAsAdmin);
                        }
                    });
                }
                ui.add_space(20.0);
                ui.heading(env.t("start-drives"));
                ui.with_layout(dir.row().with_main_wrap(true), |ui| {
                    for v in volumes {
                        volume_card(ui, env, v);
                    }
                });
                if !env.settings.recent.is_empty() {
                    ui.add_space(20.0);
                    ui.heading(env.t("start-recent"));
                    for path in env.settings.recent.clone() {
                        if ui.link(env.i18n.v(&path.to_string_lossy())).clicked() {
                            env.push(Action::StartScan(path));
                        }
                    }
                }
            });
        });
}

fn volume_card(ui: &mut Ui, env: &mut Env<'_>, v: &Volume) {
    let (rect, response) = ui.allocate_exact_size(CARD_SIZE, Sense::click());
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.faint_bg_color
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().rect_stroke(
        rect,
        8.0,
        visuals.widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(12.0);
    let dir = env.i18n.dir();
    let row = |i: f32| {
        egui::Rect::from_min_size(inner.min + vec2(0.0, i * 22.0), vec2(inner.width(), 20.0))
    };

    let title = if v.label.is_empty() {
        v.mount.to_string_lossy().into_owned()
    } else {
        format!("{} ({})", v.label, v.mount.display())
    };
    paint_text(
        ui.painter(),
        env.i18n,
        row(0.0),
        &format!("💽 {title}"),
        FontId::proportional(15.0),
        visuals.strong_text_color(),
        false,
    );
    let sub = format!(
        "{} · {}",
        env.i18n.logical(kind_label_id(v.kind)),
        v.fs_type
    );
    paint_text(
        ui.painter(),
        env.i18n,
        row(1.0),
        &sub,
        FontId::proportional(12.0),
        visuals.weak_text_color(),
        false,
    );
    let bar = row(2.0).shrink2(vec2(0.0, 5.0));
    let used = storagemap_core::ratio(v.used(), v.total);
    let fill = if used > 0.9 {
        visuals.error_fg_color
    } else {
        visuals.selection.bg_fill
    };
    share_bar(ui.painter(), dir, bar, used, fill, visuals.extreme_bg_color);
    let free = env.i18n.la(
        "volume-free",
        &[
            ("free", env.size(v.free).into()),
            ("total", env.size(v.total).into()),
        ],
    );
    paint_text(
        ui.painter(),
        env.i18n,
        row(3.0),
        &free,
        FontId::proportional(12.0),
        visuals.text_color(),
        false,
    );

    let response = response.on_hover_text(env.i18n.v(&v.mount.to_string_lossy()));
    if response.clicked() {
        env.push(Action::StartScan(v.mount.clone()));
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
        ui.add_space(ui.available_height() * 0.25);
        ui.add(egui::Spinner::new().size(36.0));
        ui.add_space(12.0);
        ui.label(
            RichText::new(env.i18n.v(&root.to_string_lossy()))
                .size(18.0)
                .strong(),
        );
        let line = env.i18n.ta(
            "scan-progress",
            &[
                ("files", fmt::count(snap.files).into()),
                ("dirs", fmt::count(snap.dirs).into()),
                ("size", env.size(snap.bytes).into()),
            ],
        );
        ui.label(line);
        ui.weak(format!("{seconds} s"));
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
        ui.add_space(12.0);
        if ui.button(format!("⏹  {}", env.t("action-stop"))).clicked() {
            env.push(Action::Stop);
        }
    });
}
