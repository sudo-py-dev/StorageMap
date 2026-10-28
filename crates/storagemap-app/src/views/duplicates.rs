//! Duplicates tab: finds identical files on a background thread.

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use egui::{Response, Ui};
use storagemap_core::duplicates::{self, DuplicateGroup, DuplicateProgress};
use storagemap_core::{Flags, NodeId, Tree};

use super::table::{self, Col, TableSource};
use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::fmt;
use crate::scan_job::PROGRESS_REFRESH;
use crate::widgets::{elided_label, empty_state, row};

/// Minimum-size choices offered in the UI.
const MIN_SIZES: [u64; 5] = [
    1024,
    100 * 1024,
    1024 * 1024,
    10 * 1024 * 1024,
    100 * 1024 * 1024,
];

pub struct Job {
    progress: Arc<DuplicateProgress>,
    rx: Receiver<Vec<DuplicateGroup>>,
}

#[derive(Default)]
pub enum State {
    #[default]
    Idle,
    Running {
        job: Job,
        root: NodeId,
    },
    Done {
        root: NodeId,
        groups: Vec<DuplicateGroup>,
        errors: u64,
    },
}

#[derive(Debug, Clone, Copy)]
enum Row {
    Group { size: u64, copies: usize },
    File(NodeId),
}

/// Groups flattened into rows, skipping files deleted since the search.
fn rows(tree: &Tree, groups: &[DuplicateGroup]) -> (Vec<Row>, u64) {
    let mut out = Vec::new();
    let mut wasted = 0;
    for g in groups {
        let live: Vec<NodeId> = g
            .files
            .iter()
            .copied()
            .filter(|&f| !tree.node(f).flags.contains(Flags::DELETED))
            .collect();
        if live.len() < 2 {
            continue;
        }
        wasted += g.size * (live.len() as u64 - 1);
        out.push(Row::Group {
            size: g.size,
            copies: live.len(),
        });
        out.extend(live.into_iter().map(Row::File));
    }
    (out, wasted)
}

const COLUMNS: [Col; 3] = [
    Col::flex("col-path"),
    Col::fixed("col-size", 92.0, 1),
    Col::fixed("col-modified", 96.0, 2),
];

struct Source<'a> {
    tree: &'a Tree,
    rows: &'a [Row],
    selected: Option<NodeId>,
}

impl TableSource for Source<'_> {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn is_selected(&self, row: usize) -> bool {
        matches!(self.rows[row], Row::File(id) if self.selected == Some(id))
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        match (self.rows[row], col) {
            (Row::Group { size, copies }, 0) => {
                let text = format!(
                    "{} · {} · {}",
                    env.i18n.count("dup-copies", copies as u64),
                    env.i18n.la("dup-each", &[("size", env.size(size).into())]),
                    env.i18n.la(
                        "dup-reclaimable",
                        &[("size", env.size(size * (copies as u64 - 1)).into())]
                    ),
                );
                let strong = ui.visuals().strong_text_color();
                table::colored(ui, env, &text, false, strong);
            }
            (Row::Group { .. }, _) => {}
            (Row::File(id), 0) => table::text(
                ui,
                env,
                &format!("    {}", relative_path(self.tree, id)),
                false,
            ),
            (Row::File(id), 1) => table::text(ui, env, &env.size(self.tree.node(id).size), true),
            (Row::File(id), _) => {
                table::text(ui, env, &fmt::date(self.tree.node(id).modified), true)
            }
        }
    }

    fn row_response(&mut self, env: &mut Env<'_>, row: usize, response: &Response) {
        let Row::File(id) = self.rows[row] else {
            return;
        };
        if response.double_clicked() {
            env.push(Action::Focus(id));
        } else if response.clicked() {
            env.push(Action::Select(id));
        }
        response.context_menu(|ui| node_menu::show(ui, env, self.tree, id));
    }
}

/// Starts a search in the focused folder (also used by the dev screenshot hook).
pub fn start_search(
    scan: &mut ScanView,
    min_size: u64,
    ctx: &egui::Context,
) -> std::io::Result<()> {
    scan.duplicates = start(scan, min_size, ctx)?;
    Ok(())
}

pub fn is_running(scan: &ScanView) -> bool {
    matches!(scan.duplicates, State::Running { .. })
}

/// Path of `id` below the scanned folder, e.g. `Photos/beach.jpg`.
fn relative_path(tree: &Tree, id: NodeId) -> String {
    let full = tree.path(id);
    full.strip_prefix(tree.root_path())
        .unwrap_or(&full)
        .to_string_lossy()
        .into_owned()
}

fn start(scan: &ScanView, min_size: u64, ctx: &egui::Context) -> std::io::Result<State> {
    let candidates = duplicates::candidates(&scan.tree, scan.focus, min_size);
    let progress = Arc::new(DuplicateProgress::default());
    let (tx, rx) = mpsc::channel();
    let (thread_progress, ctx) = (Arc::clone(&progress), ctx.clone());
    std::thread::Builder::new()
        .name("storagemap-duplicates".into())
        .spawn(move || {
            let groups = duplicates::find(&candidates, &thread_progress);
            // The view may have been closed (new scan); nothing to report then.
            let _ = tx.send(groups);
            ctx.request_repaint();
        })?;
    Ok(State::Running {
        job: Job { progress, rx },
        root: scan.focus,
    })
}

fn poll(state: &mut State) {
    let State::Running { job, root } = state else {
        return;
    };
    let finished = match job.rx.try_recv() {
        Ok(groups) => Some(groups),
        Err(TryRecvError::Disconnected) => Some(Vec::new()),
        Err(TryRecvError::Empty) => None,
    };
    if let Some(groups) = finished {
        *state = if job.progress.is_cancelled() {
            State::Idle
        } else {
            State::Done {
                root: *root,
                groups,
                errors: job.progress.errors.load(Relaxed),
            }
        };
    }
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    poll(&mut scan.duplicates);
    let dir = env.i18n.dir();
    let mut restart = false;
    row(ui, dir, |ui| match &scan.duplicates {
        State::Running { job, .. } => {
            ui.spinner();
            let p = &job.progress;
            ui.label(env.i18n.ta(
                "dup-progress",
                &[
                    ("files", fmt::count(p.files_hashed.load(Relaxed)).into()),
                    ("total", fmt::count(p.candidates.load(Relaxed)).into()),
                    ("size", env.size(p.bytes_hashed.load(Relaxed)).into()),
                ],
            ));
            if ui
                .button(format!("{} {}", crate::icons::STOP, env.t("action-stop")))
                .clicked()
            {
                p.cancel();
            }
            ui.ctx().request_repaint_after(PROGRESS_REFRESH);
        }
        _ => {
            restart = ui
                .button(format!("{} {}", crate::icons::SEARCH, env.t("dup-find")))
                .clicked();
            ui.label(env.t("dup-min-size"));
            let current = env.settings.duplicate_min_size;
            egui::ComboBox::from_id_salt("dup-min")
                .selected_text(env.i18n.v(&env.size(current)))
                .show_ui(ui, |ui| {
                    for size in MIN_SIZES {
                        if ui
                            .selectable_label(size == current, env.i18n.v(&env.size(size)))
                            .clicked()
                        {
                            env.push(Action::SetDuplicateMinSize(size));
                        }
                    }
                });
            let scope = scan.tree.path(scan.focus).to_string_lossy().into_owned();
            let weak = ui.visuals().weak_text_color();
            elided_label(
                ui,
                env.i18n,
                &env.i18n.la("dup-scope", &[("path", scope.into())]),
                weak,
            );
        }
    });
    if restart {
        match start(scan, env.settings.duplicate_min_size, ui.ctx()) {
            Ok(state) => scan.duplicates = state,
            Err(e) => {
                ui.colored_label(ui.visuals().error_fg_color, env.i18n.v(&e.to_string()));
            }
        }
    }
    ui.add_space(4.0);
    match &scan.duplicates {
        State::Idle | State::Running { .. } => {
            empty_state(
                ui,
                env.i18n,
                crate::icons::DUPLICATES,
                "dup-empty-title",
                "dup-hint",
                |_| {},
            );
        }
        State::Done {
            root,
            groups,
            errors,
        } => {
            let (rows, wasted) = rows(&scan.tree, groups);
            row(ui, dir, |ui| {
                let scope = scan.tree.path(*root).to_string_lossy().into_owned();
                let groups_text = env.i18n.count(
                    "dup-groups",
                    rows.iter()
                        .filter(|r| matches!(r, Row::Group { .. }))
                        .count() as u64,
                );
                let reclaim = env
                    .i18n
                    .la("dup-reclaimable", &[("size", env.size(wasted).into())]);
                ui.strong(env.i18n.v(&format!("{groups_text} · {reclaim}")));
                if *errors > 0 {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        env.i18n.v(&env.i18n.count("scan-errors", *errors)),
                    );
                }
                let weak = ui.visuals().weak_text_color();
                elided_label(
                    ui,
                    env.i18n,
                    &env.i18n.la("dup-scope", &[("path", scope.into())]),
                    weak,
                );
            });
            if rows.is_empty() {
                ui.weak(env.t("dup-none"));
            } else {
                let mut source = Source {
                    tree: &scan.tree,
                    rows: &rows,
                    selected: scan.selected,
                };
                table::show(ui, env, "duplicates", &COLUMNS, &mut source);
            }
        }
    }
}
