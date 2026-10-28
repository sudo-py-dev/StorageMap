//! The marked entries: a bar with bulk actions, and the dialogs those
//! actions open (confirming the trash, previewing a bulk rename).

use egui::{Color32, Id, Layout, Modal, RichText, Ui};
use storagemap_core::operations::{self, RenameProblem, RenameRule};

use super::{Env, ScanView};
use crate::app::{Action, App, BulkRequest};
use crate::widgets::{paragraph, row};

/// Shown above the views while anything is marked.
pub fn bar(ui: &mut Ui, env: &mut Env<'_>, scan: &ScanView, busy: bool) {
    if scan.marked.is_empty() {
        return;
    }
    let top = scan.marked_top();
    let size: u64 = top.iter().map(|&id| scan.tree.node(id).size).sum();
    let frame = egui::Frame::new()
        .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.35))
        .corner_radius(egui::CornerRadius::same(crate::theme::RADIUS))
        .inner_margin(egui::Margin::symmetric(8, 4));
    frame.show(ui, |ui| {
        let layout = env.i18n.dir().row().with_main_wrap(true);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 0.0), layout, |ui| {
            ui.strong(env.i18n.v(&env.i18n.la(
                "marked-summary",
                &[
                    ("count", (top.len() as u64).into()),
                    ("size", env.size(size).into()),
                ],
            )));
            ui.separator();
            let enabled = !busy && !scan.scanning;
            for (request, icon, id) in [
                (BulkRequest::MoveTo, crate::icons::MOVE, "bulk-move"),
                (BulkRequest::CopyTo, crate::icons::COPY, "bulk-copy"),
                (BulkRequest::Zip, crate::icons::ARCHIVE, "bulk-zip"),
                (BulkRequest::Rename, crate::icons::RENAME, "bulk-rename"),
                (BulkRequest::Trash, crate::icons::TRASH, "bulk-trash"),
            ] {
                let button = egui::Button::new(format!("{icon}  {}", env.t(id)));
                if ui.add_enabled(enabled, button).clicked() {
                    env.push(Action::Bulk(request));
                }
            }
            if ui.button(env.t("marks-clear")).clicked() {
                env.push(Action::ClearMarks);
            }
        });
    });
    ui.add_space(4.0);
}

/// The rename dialog's state.
pub struct RenameDialog {
    pub rule: RenameRule,
    /// The rule the preview was computed for.
    previewed: Option<RenameRule>,
    paths: Vec<std::path::PathBuf>,
    plan: Vec<operations::PlannedRename>,
}

impl RenameDialog {
    pub fn new(paths: Vec<std::path::PathBuf>) -> Self {
        RenameDialog {
            rule: RenameRule::default(),
            previewed: None,
            paths,
            plan: Vec::new(),
        }
    }
}

fn problem_id(problem: RenameProblem) -> &'static str {
    match problem {
        RenameProblem::Unchanged => "rename-unchanged",
        RenameProblem::Invalid => "rename-invalid",
        RenameProblem::Exists => "rename-exists",
        RenameProblem::Duplicate => "rename-duplicate",
    }
}

/// The bulk-trash confirmation and the rename dialog, when open.
pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    confirm_trash(app, ctx);
    rename(app, ctx);
}

fn confirm_trash(app: &mut App, ctx: &egui::Context) {
    let (true, Some(scan)) = (app.confirm_bulk_trash, &app.scan) else {
        return;
    };
    let i = &app.i18n;
    let top = scan.marked_top();
    let size: u64 = top.iter().map(|&id| scan.tree.node(id).size).sum();
    let size = crate::fmt::size(size, app.settings.unit, 1, |k| i.logical(k));
    let mut decision = None;
    let modal = Modal::new(Id::unique("confirm-bulk-trash")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
            ui.heading(format!(
                "{}  {}",
                crate::icons::TRASH,
                i.t("bulk-trash-title")
            ));
            ui.add_space(4.0);
            paragraph(
                ui,
                i,
                &i.la(
                    "bulk-trash-body",
                    &[("count", (top.len() as u64).into()), ("size", size.into())],
                ),
            );
            ui.add_space(12.0);
            row(ui, i.dir(), |ui| {
                let danger =
                    egui::Button::new(RichText::new(i.t("bulk-trash")).color(Color32::WHITE))
                        .fill(ui.visuals().error_fg_color);
                if ui.add(danger).clicked() {
                    decision = Some(true);
                }
                if ui.button(i.t("cancel")).clicked() {
                    decision = Some(false);
                }
            });
        });
    });
    if modal.should_close() {
        decision = Some(false);
    }
    if let Some(go) = decision {
        app.confirm_bulk_trash = false;
        if go {
            app.actions.push(Action::RunBulkTrash);
        }
    }
}

fn rename(app: &mut App, ctx: &egui::Context) {
    let i = &app.i18n;
    let Some(dialog) = &mut app.rename else {
        return;
    };
    if dialog.previewed.as_ref() != Some(&dialog.rule) {
        dialog.plan = operations::plan_renames(&dialog.paths, &dialog.rule);
        dialog.previewed = Some(dialog.rule.clone());
    }
    let ready = dialog.plan.iter().filter(|p| p.problem.is_none()).count();
    let mut decision = None;
    let modal = Modal::new(Id::unique("bulk-rename")).show(ctx, |ui| {
        ui.set_width(620.0);
        ui.with_layout(Layout::top_down(i.dir().start()), |ui| {
            ui.heading(format!("{}  {}", crate::icons::RENAME, i.t("rename-title")));
            ui.add_space(6.0);
            egui::Grid::new("rename-fields")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label(i.t("rename-find"));
                    ui.text_edit_singleline(&mut dialog.rule.find);
                    ui.end_row();
                    ui.label(i.t("rename-replace"));
                    ui.text_edit_singleline(&mut dialog.rule.replace);
                    ui.end_row();
                });
            ui.checkbox(&mut dialog.rule.match_case, i.t("rename-match-case"));
            ui.add_space(8.0);
            ui.weak(i.ta("rename-preview", &[("count", (ready as u64).into())]));
            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    for p in &dialog.plan {
                        let from = p.from.file_name().unwrap_or_default().to_string_lossy();
                        let to = p.to.file_name().unwrap_or_default().to_string_lossy();
                        match p.problem {
                            None => {
                                ui.label(RichText::new(i.v(&format!("{from} → {to}"))).small());
                            }
                            Some(problem) => {
                                let text = format!("{from} · {}", i.logical(problem_id(problem)));
                                let color = if problem == RenameProblem::Unchanged {
                                    ui.visuals().weak_text_color()
                                } else {
                                    ui.visuals().warn_fg_color
                                };
                                ui.label(RichText::new(i.v(&text)).small().color(color));
                            }
                        }
                    }
                });
            ui.add_space(12.0);
            row(ui, i.dir(), |ui| {
                let apply = crate::theme::primary_button(i.t("rename-apply"));
                if ui.add_enabled(ready > 0, apply).clicked() {
                    decision = Some(true);
                }
                if ui.button(i.t("cancel")).clicked() {
                    decision = Some(false);
                }
            });
        });
    });
    if modal.should_close() {
        decision = Some(false);
    }
    match decision {
        Some(true) => {
            let plans = dialog
                .plan
                .iter()
                .filter(|p| p.problem.is_none())
                .cloned()
                .collect();
            app.rename = None;
            app.actions.push(Action::RunBulkRename(plans));
        }
        Some(false) => app.rename = None,
        None => {}
    }
}
