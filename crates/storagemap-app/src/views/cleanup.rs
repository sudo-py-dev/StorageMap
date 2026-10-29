//! Cleanup tab: grouped suggestions of space that is usually safe to free.

use egui::{CornerRadius, Frame, Margin, RichText, Sides, Ui};
use storagemap_core::cleanup::{self, CleanupKind, Suggestion};
use storagemap_core::{NodeId, Tree};

use super::{Env, ScanView};
use crate::app::Action;
use crate::fmt;
use crate::i18n::Direction;
use crate::theme;
use crate::widgets::{empty_state, paint_text, paragraph_small};

/// Most items collected per category.
const PER_KIND: usize = 200;
/// Items shown per category before "Show all".
const PREVIEW: usize = 5;

fn label(kind: CleanupKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        CleanupKind::Trash => (crate::icons::TRASH, "cleanup-trash", "cleanup-trash-hint"),
        CleanupKind::Cache => (
            crate::icons::DATABASE,
            "cleanup-cache",
            "cleanup-cache-hint",
        ),
        CleanupKind::BuildOutput => (crate::icons::WRENCH, "cleanup-build", "cleanup-build-hint"),
        CleanupKind::PackageCache => (
            crate::icons::PACKAGE,
            "cleanup-packages",
            "cleanup-packages-hint",
        ),
        CleanupKind::Temp => (crate::icons::HOURGLASS, "cleanup-temp", "cleanup-temp-hint"),
        CleanupKind::Logs => (crate::icons::SCROLL, "cleanup-logs", "cleanup-logs-hint"),
        CleanupKind::CrashDumps => (crate::icons::BUG, "cleanup-dumps", "cleanup-dumps-hint"),
        CleanupKind::OldDownloads => (
            crate::icons::DOWNLOAD,
            "cleanup-downloads",
            "cleanup-downloads-hint",
        ),
    }
}

/// Total suggested for the focused folder (cached).
pub fn reclaimable(scan: &mut ScanView) -> u64 {
    let (tree, focus) = (&scan.tree, scan.focus);
    scan.cleanup
        .get((focus, scan.generation), || {
            cleanup::suggestions(tree, focus, fmt::now_unix(), PER_KIND)
        })
        .iter()
        .map(|s| s.size)
        .sum()
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let (tree, focus) = (&scan.tree, scan.focus);
    let list = scan.cleanup.get((focus, scan.generation), || {
        cleanup::suggestions(tree, focus, fmt::now_unix(), PER_KIND)
    });
    if list.is_empty() {
        empty_state(
            ui,
            env.i18n,
            crate::icons::SPARKLE,
            "cleanup-none-title",
            "cleanup-none",
            |_| {},
        );
        return;
    }
    let total: u64 = list.iter().map(|s| s.size).sum();
    let expanded = &mut scan.cleanup_expanded;
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(env.i18n.dir().start()), |ui| {
                summary(ui, env, total);
                ui.add_space(10.0);
                for (kind, size, count) in cleanup::totals(list) {
                    let open = expanded.contains(&kind);
                    if category(ui, env, tree, list, kind, size, count, open) {
                        if open {
                            expanded.remove(&kind);
                        } else {
                            expanded.insert(kind);
                        }
                    }
                    ui.add_space(8.0);
                }
            });
        });
}

fn summary(ui: &mut Ui, env: &Env<'_>, total: u64) {
    Frame::new()
        .fill(theme::ACCENT.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.6)))
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down(env.i18n.dir().start()), |ui| {
                let headline = env
                    .i18n
                    .ta("cleanup-total", &[("size", env.size(total).into())]);
                ui.label(
                    RichText::new(format!("{} {headline}", crate::icons::SPARKLE))
                        .size(20.0)
                        .strong(),
                );
                paragraph_small(ui, env.i18n, &env.i18n.logical("cleanup-hint"));
            });
        });
}

/// One category card; returns true when "show all / fewer" was clicked.
#[allow(clippy::too_many_arguments)]
fn category(
    ui: &mut Ui,
    env: &mut Env<'_>,
    tree: &Tree,
    list: &[Suggestion],
    kind: CleanupKind,
    size: u64,
    count: usize,
    open: bool,
) -> bool {
    let (icon, title, hint) = label(kind);
    let dir = env.i18n.dir();
    let mut toggled = false;
    Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down(dir.start()), |ui| {
                let heading = |ui: &mut Ui| {
                    ui.label(
                        RichText::new(format!("{icon}  {}", env.t(title)))
                            .size(16.0)
                            .strong(),
                    );
                };
                let total = |ui: &mut Ui| {
                    ui.label(
                        RichText::new(env.i18n.v(&env.size(size)))
                            .size(16.0)
                            .strong()
                            .color(theme::ACCENT_BRIGHT),
                    );
                };
                sides(ui, dir, heading, total);
                paragraph_small(ui, env.i18n, &env.i18n.logical(hint));
                ui.add_space(4.0);
                let shown = if open { count } else { count.min(PREVIEW) };
                for s in list.iter().filter(|s| s.kind == kind).take(shown) {
                    item(ui, env, tree, s.node, s.size);
                }
                if count > PREVIEW {
                    let text = if open {
                        env.t("cleanup-fewer")
                    } else {
                        env.i18n
                            .ta("cleanup-all", &[("count", (count as u64).into())])
                    };
                    toggled = ui.link(text).clicked();
                }
            });
        });
    toggled
}

fn item(ui: &mut Ui, env: &mut Env<'_>, tree: &Tree, id: NodeId, size: u64) {
    const ROW_HEIGHT: f32 = 28.0;
    const CONTROLS_WIDTH: f32 = 290.0;
    let dir = env.i18n.dir();
    let full = tree.path(id);
    let shown = super::relative_path(tree, id);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    let controls_width = CONTROLS_WIDTH.min(rect.width() * 0.6);
    // Path fills the start side; buttons get a fixed span at the end side.
    let path_rect = dir.span(rect, 0.0, rect.width() - controls_width - 8.0);
    let color = ui.visuals().text_color();
    paint_text(
        ui.painter(),
        env.i18n,
        path_rect,
        &shown,
        egui::TextStyle::Body.resolve(ui.style()),
        color,
        false,
    );
    let controls_rect = dir.span(rect, rect.width() - controls_width, controls_width);
    let mut actions = Vec::new();
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(controls_rect)
            .layout(dir.end_row()),
        |ui| {
            let is_trash =
                crate::platform::trash_location(&full) == crate::platform::TrashLocation::Root;
            let (label, action) = if is_trash {
                (env.t("action-empty-trash"), Action::AskEmptyTrash(id))
            } else {
                (env.t("confirm-trash"), Action::AskTrash(id))
            };
            if ui
                .small_button(format!("{} {label}", crate::icons::TRASH))
                .clicked()
            {
                actions.push(action);
            }
            if ui
                .small_button(format!(
                    "{} {}",
                    crate::icons::REVEAL,
                    env.t("action-open-folder")
                ))
                .on_hover_text(env.t("action-show-in-folder"))
                .clicked()
            {
                actions.push(Action::Reveal(id));
            }
            ui.label(RichText::new(env.i18n.v(&env.size(size))).monospace());
        },
    );
    for a in actions {
        env.push(a);
    }
}

/// Start content at the reading-start edge, end content at the other edge.
fn sides(ui: &mut Ui, dir: Direction, start: impl FnOnce(&mut Ui), end: impl FnOnce(&mut Ui)) {
    match dir {
        Direction::Ltr => {
            Sides::new().show(ui, start, end);
        }
        Direction::Rtl => {
            Sides::new().show(ui, end, start);
        }
    }
}
