//! Changes tab: what grew, shrank, appeared or vanished since a snapshot.

use egui::{Color32, Response, RichText, Ui};
use storagemap_core::snapshot::{Change, Comparison};
use storagemap_core::{NodeId, Tree};

use super::table::{self, Col, TableSource};
use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::fmt;
use crate::widgets::{empty_state, row};

/// Shrinking is good news for a disk; colour it calmly.
const SHRINK_COLOR: Color32 = Color32::from_rgb(70, 170, 100);

/// One line of the table: a current entry or one that is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Present(NodeId, i128),
    Removed(usize),
}

/// Direct children of `focus` that changed, biggest change first.
pub fn build_rows(tree: &Tree, cmp: &Comparison, focus: NodeId) -> Vec<Row> {
    let mut rows: Vec<(u128, Row)> = tree
        .children(focus)
        .filter_map(|id| {
            cmp.size_delta(tree, id)
                .map(|d| (d.unsigned_abs(), Row::Present(id, d)))
        })
        .collect();
    let focus_path = tree.path(focus);
    rows.extend(
        cmp.removed
            .iter()
            .enumerate()
            .filter(|(_, r)| r.path.parent() == Some(focus_path.as_path()))
            .map(|(i, r)| (u128::from(r.stats.size), Row::Removed(i))),
    );
    rows.sort_by_key(|(magnitude, _)| std::cmp::Reverse(*magnitude));
    rows.into_iter().map(|(_, r)| r).collect()
}

const COLUMNS: [Col; 4] = [
    Col::flex("col-name"),
    Col::fixed("col-before", 92.0, 2),
    Col::fixed("col-now", 92.0, 3),
    Col::fixed("col-change", 100.0, 1),
];

struct Source<'a> {
    tree: &'a Tree,
    cmp: &'a Comparison,
    rows: &'a [Row],
    selected: Option<NodeId>,
}

impl Source<'_> {
    fn delta_cell(ui: &mut Ui, env: &Env<'_>, delta: i128) {
        let color = match delta.signum() {
            1 => ui.visuals().error_fg_color,
            -1 => SHRINK_COLOR,
            _ => ui.visuals().weak_text_color(),
        };
        table::colored(ui, env, &signed(env, delta), true, color);
    }
}

impl TableSource for Source<'_> {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn is_selected(&self, row: usize) -> bool {
        matches!(self.rows[row], Row::Present(id, _) if self.selected == Some(id))
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        match self.rows[row] {
            Row::Present(id, delta) => {
                let node = self.tree.node(id);
                match col {
                    0 => {
                        let icon = if node.is_dir() {
                            crate::icons::FOLDER
                        } else {
                            crate::icons::FILE
                        };
                        table::text(ui, env, &format!("{icon} {}", self.tree.name(id)), false);
                    }
                    1 => match self.cmp.change(self.tree, id) {
                        Change::Known(old) => table::text(ui, env, &env.size(old.size), true),
                        _ => {
                            let weak = ui.visuals().weak_text_color();
                            table::colored(ui, env, &env.i18n.logical("change-new"), true, weak);
                        }
                    },
                    2 => table::text(ui, env, &env.size(node.size), true),
                    _ => Self::delta_cell(ui, env, delta),
                }
            }
            Row::Removed(i) => {
                let removed = &self.cmp.removed[i];
                let weak = ui.visuals().weak_text_color();
                match col {
                    0 => {
                        let icon = if removed.is_dir {
                            crate::icons::FOLDER
                        } else {
                            crate::icons::FILE
                        };
                        let name = removed
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        table::colored(ui, env, &format!("{icon} {name}"), false, weak);
                    }
                    1 => table::text(ui, env, &env.size(removed.stats.size), true),
                    2 => table::colored(ui, env, &env.i18n.logical("change-removed"), true, weak),
                    _ => Self::delta_cell(ui, env, -i128::from(removed.stats.size)),
                }
            }
        }
    }

    fn row_response(&mut self, env: &mut Env<'_>, row: usize, response: &Response) {
        let Row::Present(id, _) = self.rows[row] else {
            return;
        };
        if response.double_clicked() && self.tree.node(id).is_dir() {
            env.push(Action::Focus(id));
        } else if response.clicked() {
            env.push(Action::Select(id));
        }
        response.context_menu(|ui| node_menu::show(ui, env, self.tree, id));
    }
}

/// Signed size text in the user's units (logical order).
pub fn signed(env: &Env<'_>, delta: i128) -> String {
    fmt::signed_size(
        delta,
        env.settings.unit,
        usize::from(env.settings.decimals),
        |id| env.i18n.logical(id),
    )
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let Some(cmp) = &scan.comparison else {
        picker(ui, env, scan);
        return;
    };
    let dir = env.i18n.dir();
    let date = fmt::date_time(cmp.taken_at);
    row(ui, dir, |ui| {
        if let Some(delta) = cmp.size_delta(&scan.tree, scan.focus) {
            let summary = env.i18n.ta(
                "compare-summary",
                &[
                    ("change", signed(env, delta).into()),
                    ("date", date.clone().into()),
                ],
            );
            ui.label(RichText::new(summary).strong());
        }
        ui.separator();
        ui.menu_button(env.t("compare-other"), |ui| snapshot_menu(ui, env, scan));
        if ui.button(env.t("compare-clear")).clicked() {
            env.push(Action::ClearComparison);
        }
    });
    ui.add_space(4.0);
    let (tree, focus) = (&scan.tree, scan.focus);
    let rows = scan
        .change_rows
        .get((focus, scan.generation, cmp.taken_at), || {
            build_rows(tree, cmp, focus)
        });
    if rows.is_empty() {
        ui.weak(env.t("compare-unchanged"));
        return;
    }
    table::show(
        ui,
        env,
        "changes",
        &COLUMNS,
        &mut Source {
            tree,
            cmp,
            rows,
            selected: scan.selected,
        },
    );
}

fn picker(ui: &mut Ui, env: &mut Env<'_>, scan: &ScanView) {
    ui.with_layout(egui::Layout::top_down(env.i18n.dir().start()), |ui| {
        if scan.snapshots.is_empty() {
            empty_state(
                ui,
                env.i18n,
                crate::icons::CHANGES,
                "compare-title",
                "compare-none",
                |_| {},
            );
        } else {
            ui.heading(env.t("compare-title"));
            snapshot_menu(ui, env, scan);
        }
        ui.add_space(8.0);
        if ui
            .button(format!(
                "{} {}",
                crate::icons::FOLDER_OPEN,
                env.t("compare-browse")
            ))
            .clicked()
        {
            env.push(Action::BrowseSnapshot);
        }
    });
}

/// Buttons for each earlier snapshot of this folder.
fn snapshot_menu(ui: &mut Ui, env: &mut Env<'_>, scan: &ScanView) {
    for info in &scan.snapshots {
        let label = env.i18n.ta(
            "snapshot-entry",
            &[
                ("date", fmt::date_time(info.taken_at).into()),
                ("size", env.size(info.total.size).into()),
            ],
        );
        if ui.button(label).clicked() {
            env.push(Action::CompareWith(info.file.clone()));
            ui.close();
        }
    }
    if scan.comparison.is_some() {
        ui.separator();
        if ui.button(env.t("compare-browse")).clicked() {
            env.push(Action::BrowseSnapshot);
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use storagemap_core::snapshot::Snapshot;
    use storagemap_core::{EntryInfo, Flags, Kind, TreeBuilder};

    fn info(kind: Kind, size: u64) -> EntryInfo {
        EntryInfo {
            kind,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            size,
            allocated: size,
        }
    }

    #[test]
    fn rows_sorted_by_magnitude_with_removed_entries() {
        const MB: u64 = 1 << 20;
        let mut b = TreeBuilder::new(PathBuf::from("/r"), info(Kind::Dir, 0));
        b.push(NodeId::ROOT, b"grow", info(Kind::File, 2 * MB));
        b.push(NodeId::ROOT, b"gone", info(Kind::File, 5 * MB));
        let old = b.finish();
        let mut b = TreeBuilder::new(PathBuf::from("/r"), info(Kind::Dir, 0));
        b.push(NodeId::ROOT, b"grow", info(Kind::File, 3 * MB));
        let now = b.finish();
        let cmp = Comparison::new(&now, &Snapshot::capture(&old, 0, MB)).expect("compare");
        let rows = build_rows(&now, &cmp, now.root());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], Row::Removed(0));
        assert!(matches!(rows[1], Row::Present(_, d) if d == i128::from(MB)));
    }
}
