//! Duplicates tab: finds identical files on a background thread, then lets
//! the user mark copies (by hand or by a keep rule) and move them to the
//! trash or replace them with hard links to the copy kept.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use egui::{Id, Modal, Response, RichText, Ui};
use storagemap_core::duplicates::{self, DuplicateGroup, DuplicateProgress, Keep};
use storagemap_core::export::{Delimited, ReportWriter, Value};
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
/// Planned changes listed in a confirmation before "and N more".
const PREVIEW_LINES: usize = 200;

pub struct Job {
    progress: Arc<DuplicateProgress>,
    rx: Receiver<Vec<DuplicateGroup>>,
}

/// What to do with the marked copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Trash,
    HardLink,
}

/// One planned change: `file` goes; `keep` (same content) stays.
#[derive(Debug, Clone)]
struct Change {
    file: NodeId,
    path: PathBuf,
    keep: PathBuf,
    size: u64,
}

/// A trash or hard-link run on a background thread.
struct OpJob {
    op: Op,
    total: usize,
    done: Arc<AtomicU64>,
    rx: Receiver<Vec<(NodeId, Result<(), String>)>>,
}

pub struct Found {
    root: NodeId,
    groups: Vec<DuplicateGroup>,
    errors: u64,
    /// Copies marked for removal.
    marked: HashSet<NodeId>,
    rule: Keep,
    /// A confirmation being shown, with its planned changes.
    confirm: Option<(Op, Vec<Change>)>,
    running: Option<OpJob>,
}

#[derive(Default)]
pub enum State {
    #[default]
    Idle,
    Running {
        job: Job,
        root: NodeId,
    },
    Done(Box<Found>),
}

#[derive(Debug, Clone, Copy)]
enum Row {
    Group(usize),
    File(NodeId),
}

/// Live files of a group: deleted ones and ones already linked are gone.
fn live(tree: &Tree, g: &DuplicateGroup) -> Vec<NodeId> {
    g.files
        .iter()
        .copied()
        .filter(|&f| {
            let flags = tree.node(f).flags;
            !flags.contains(Flags::DELETED) && !flags.contains(Flags::HARDLINK_DUP)
        })
        .collect()
}

/// Groups flattened into rows, skipping groups with fewer than two copies left.
fn rows(tree: &Tree, groups: &[DuplicateGroup]) -> (Vec<Row>, u64) {
    let mut out = Vec::new();
    let mut wasted = 0;
    for (index, g) in groups.iter().enumerate() {
        let files = live(tree, g);
        if files.len() < 2 {
            continue;
        }
        wasted += g.size * (files.len() as u64 - 1);
        out.push(Row::Group(index));
        out.extend(files.into_iter().map(Row::File));
    }
    (out, wasted)
}

/// The marked copies as changes, each paired with an unmarked copy of the
/// same group to keep. Groups where every copy is marked are skipped, so
/// one copy always survives.
fn plan(tree: &Tree, groups: &[DuplicateGroup], marked: &HashSet<NodeId>) -> Vec<Change> {
    let mut out = Vec::new();
    for g in groups {
        let files = live(tree, g);
        let Some(&keep) = files.iter().find(|f| !marked.contains(f)) else {
            continue;
        };
        let keep_path = tree.path(keep);
        out.extend(
            files
                .iter()
                .filter(|f| marked.contains(f))
                .map(|&file| Change {
                    file,
                    path: tree.path(file),
                    keep: keep_path.clone(),
                    size: g.size,
                }),
        );
    }
    out
}

const COLUMNS: [Col; 4] = [
    Col::fixed("dup-col-remove", 64.0, 1),
    Col::flex("col-path"),
    Col::fixed("col-size", 92.0, 2),
    Col::fixed("col-modified", 96.0, 3),
];

struct Source<'a> {
    tree: &'a Tree,
    groups: &'a [DuplicateGroup],
    rows: &'a [Row],
    selected: Option<NodeId>,
    marked: &'a mut HashSet<NodeId>,
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
            (Row::Group(index), 1) => {
                let g = &self.groups[index];
                let copies = live(self.tree, g).len() as u64;
                let mut text = format!(
                    "{} · {} · {}",
                    env.i18n.count("dup-copies", copies),
                    env.i18n
                        .la("dup-each", &[("size", env.size(g.size).into())]),
                    env.i18n.la(
                        "dup-reclaimable",
                        &[("size", env.size(g.size * (copies - 1)).into())]
                    ),
                );
                if let Some(digest) = &g.sha256 {
                    text.push_str(&format!(" · SHA-256 {}…", &duplicates::hex(digest)[..12]));
                }
                let strong = ui.visuals().strong_text_color();
                table::colored(ui, env, &text, false, strong);
                if let Some(digest) = &g.sha256 {
                    ui.response()
                        .on_hover_text(format!("SHA-256 {}", duplicates::hex(digest)));
                }
            }
            (Row::Group(_), _) => {}
            (Row::File(id), 0) => {
                let mut on = self.marked.contains(&id);
                if ui
                    .checkbox(&mut on, "")
                    .on_hover_text(env.t("dup-mark-hint"))
                    .changed()
                {
                    if on {
                        self.marked.insert(id);
                    } else {
                        self.marked.remove(&id);
                    }
                }
            }
            (Row::File(id), 1) => {
                let path = format!("    {}", relative_path(self.tree, id));
                if self.marked.contains(&id) {
                    let weak = ui.visuals().weak_text_color();
                    table::colored(ui, env, &path, false, weak);
                } else {
                    table::text(ui, env, &path, false);
                }
            }
            (Row::File(id), 2) => table::text(ui, env, &env.size(self.tree.node(id).size), true),
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
    sha256: bool,
    ctx: &egui::Context,
) -> std::io::Result<()> {
    scan.duplicates = start(scan, min_size, sha256, ctx)?;
    Ok(())
}

pub fn is_running(scan: &ScanView) -> bool {
    match &scan.duplicates {
        State::Running { .. } => true,
        State::Done(found) => found.running.is_some(),
        State::Idle => false,
    }
}

/// Path of `id` below the scanned folder, e.g. `Photos/beach.jpg`.
fn relative_path(tree: &Tree, id: NodeId) -> String {
    let full = tree.path(id);
    full.strip_prefix(tree.root_path())
        .unwrap_or(&full)
        .to_string_lossy()
        .into_owned()
}

fn start(
    scan: &ScanView,
    min_size: u64,
    sha256: bool,
    ctx: &egui::Context,
) -> std::io::Result<State> {
    let candidates = duplicates::candidates(&scan.tree, scan.focus, min_size);
    let progress = Arc::new(DuplicateProgress::default());
    let (tx, rx) = mpsc::channel();
    let (thread_progress, ctx) = (Arc::clone(&progress), ctx.clone());
    std::thread::Builder::new()
        .name("storagemap-duplicates".into())
        .spawn(move || {
            let mut groups = duplicates::find(&candidates, &thread_progress);
            if sha256 {
                duplicates::add_sha256(&mut groups, &candidates, &thread_progress);
            }
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
            State::Done(Box::new(Found {
                root: *root,
                groups,
                errors: job.progress.errors.load(Relaxed),
                marked: HashSet::new(),
                rule: Keep::Oldest,
                confirm: None,
                running: None,
            }))
        };
    }
}

/// Runs `changes` on a background thread.
fn run(op: Op, changes: Vec<Change>, ctx: &egui::Context) -> std::io::Result<OpJob> {
    let (tx, rx) = mpsc::channel();
    let done = Arc::new(AtomicU64::new(0));
    let (thread_done, ctx) = (Arc::clone(&done), ctx.clone());
    let total = changes.len();
    std::thread::Builder::new()
        .name("storagemap-dedupe".into())
        .spawn(move || {
            let results = changes
                .into_iter()
                .map(|c| {
                    let result = match op {
                        Op::Trash => trash::delete(&c.path).map_err(|e| e.to_string()),
                        Op::HardLink => duplicates::replace_with_hard_link(&c.keep, &c.path)
                            .map_err(|e| e.to_string()),
                    };
                    thread_done.fetch_add(1, Relaxed);
                    (c.file, result)
                })
                .collect();
            let _ = tx.send(results);
            ctx.request_repaint();
        })?;
    Ok(OpJob {
        op,
        total,
        done,
        rx,
    })
}

/// Applies a finished run to the tree and reports how it went.
fn poll_run(scan: &mut ScanView, env: &mut Env<'_>) {
    let State::Done(found) = &mut scan.duplicates else {
        return;
    };
    let Some(job) = &found.running else { return };
    let results = match job.rx.try_recv() {
        Ok(results) => results,
        Err(TryRecvError::Empty) => return,
        Err(TryRecvError::Disconnected) => Vec::new(),
    };
    let op = job.op;
    found.running = None;
    let mut failed = Vec::new();
    let mut ok = Vec::new();
    for (id, result) in results {
        found.marked.remove(&id);
        match result {
            Ok(()) => ok.push(id),
            Err(e) => failed.push(e),
        }
    }
    for &id in &ok {
        match op {
            Op::Trash => scan.remove(id),
            Op::HardLink => scan.mark_linked(id),
        }
    }
    let done_id = match op {
        Op::Trash => "dup-trashed",
        Op::HardLink => "dup-linked",
    };
    let mut text = env.i18n.count(done_id, ok.len() as u64);
    if let Some(first) = failed.first() {
        text = format!(
            "{text} · {}",
            env.i18n.la(
                "dup-failed",
                &[
                    ("count", (failed.len() as u64).into()),
                    ("error", first.clone().into())
                ]
            )
        );
    }
    env.push(Action::Notify(env.i18n.v(&text), !failed.is_empty()));
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    poll(&mut scan.duplicates);
    poll_run(scan, env);
    let dir = env.i18n.dir();
    // A saved scan's files may be gone or changed: compare only real files.
    let live = scan.saved_at.is_none();
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
            let find = egui::Button::new(format!("{} {}", crate::icons::SEARCH, env.t("dup-find")));
            restart = ui
                .add_enabled(live, find)
                .on_disabled_hover_text(env.t("saved-read-only"))
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
            let mut sha = env.settings.duplicate_sha256;
            if ui
                .checkbox(&mut sha, env.t("dup-sha256"))
                .on_hover_text(env.t("dup-sha256-hint"))
                .changed()
            {
                env.push(Action::SetDuplicateSha256(sha));
            }
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
        let (min, sha) = (
            env.settings.duplicate_min_size,
            env.settings.duplicate_sha256,
        );
        match start(scan, min, sha, ui.ctx()) {
            Ok(state) => scan.duplicates = state,
            Err(e) => {
                ui.colored_label(ui.visuals().error_fg_color, env.i18n.v(&e.to_string()));
            }
        }
    }
    ui.add_space(4.0);
    let scanning = scan.scanning;
    let ScanView {
        tree,
        duplicates: state,
        selected,
        ..
    } = scan;
    let found = match state {
        State::Idle | State::Running { .. } => {
            empty_state(
                ui,
                env.i18n,
                crate::icons::DUPLICATES,
                "dup-empty-title",
                "dup-hint",
                |_| {},
            );
            return;
        }
        State::Done(found) => found,
    };
    let (rows, wasted) = rows(tree, &found.groups);
    row(ui, dir, |ui| {
        let scope = tree.path(found.root).to_string_lossy().into_owned();
        let groups_text = env.i18n.count(
            "dup-groups",
            rows.iter().filter(|r| matches!(r, Row::Group(_))).count() as u64,
        );
        let reclaim = env
            .i18n
            .la("dup-reclaimable", &[("size", env.size(wasted).into())]);
        ui.strong(env.i18n.v(&format!("{groups_text} · {reclaim}")));
        if found.errors > 0 {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                env.i18n.v(&env.i18n.count("scan-errors", found.errors)),
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
        return;
    }
    actions_row(ui, env, tree, found, scanning);
    let mut source = Source {
        tree,
        groups: &found.groups,
        rows: &rows,
        selected: *selected,
        marked: &mut found.marked,
    };
    table::show(ui, env, "duplicates", &COLUMNS, &mut source);
    confirm_dialog(ui, env, found);
}

/// Keep rule, marking, and what to do with the marked copies.
fn actions_row(ui: &mut Ui, env: &mut Env<'_>, tree: &Tree, found: &mut Found, scanning: bool) {
    let dir = env.i18n.dir();
    if let Some(job) = &found.running {
        row(ui, dir, |ui| {
            ui.spinner();
            ui.label(env.i18n.ta(
                "dup-working",
                &[
                    ("done", fmt::count(job.done.load(Relaxed)).into()),
                    ("total", fmt::count(job.total as u64).into()),
                ],
            ));
        });
        ui.ctx().request_repaint_after(PROGRESS_REFRESH);
        return;
    }
    let layout = dir.row().with_main_wrap(true);
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 0.0), layout, |ui| {
        ui.label(env.t("dup-keep"));
        egui::ComboBox::from_id_salt("dup-rule")
            .selected_text(env.t(rule_label(found.rule)))
            .show_ui(ui, |ui| {
                for rule in Keep::ALL {
                    ui.selectable_value(&mut found.rule, rule, env.t(rule_label(rule)));
                }
            });
        if ui
            .button(env.t("dup-mark-others"))
            .on_hover_text(env.t("dup-mark-others-hint"))
            .clicked()
        {
            found.marked.clear();
            for g in &found.groups {
                let files = live(tree, g);
                if let Some(keep) = duplicates::keeper(tree, &files, found.rule) {
                    found
                        .marked
                        .extend(files.into_iter().filter(|&f| f != keep));
                }
            }
        }
        if !found.marked.is_empty() && ui.button(env.t("dup-clear-marks")).clicked() {
            found.marked.clear();
        }
        ui.separator();
        let changes = plan(tree, &found.groups, &found.marked);
        let size: u64 = changes.iter().map(|c| c.size).sum();
        ui.label(env.i18n.v(&env.i18n.la(
            "dup-marked",
            &[
                ("count", (changes.len() as u64).into()),
                ("size", env.size(size).into()),
            ],
        )));
        let can_act = !changes.is_empty() && !scanning;
        let trash = format!("{}  {}", crate::icons::TRASH, env.t("dup-trash"));
        if ui.add_enabled(can_act, egui::Button::new(trash)).clicked() {
            found.confirm = Some((Op::Trash, changes.clone()));
        }
        let link = format!("{}  {}", crate::icons::LINK, env.t("dup-link"));
        if ui
            .add_enabled(can_act, egui::Button::new(link))
            .on_hover_text(env.t("dup-link-hint"))
            .clicked()
        {
            found.confirm = Some((Op::HardLink, changes));
        }
        if ui
            .button(format!(
                "{}  {}",
                crate::icons::EXPORT,
                env.t("search-export")
            ))
            .clicked()
        {
            env.push(Action::ExportDuplicates);
        }
    });
}

fn rule_label(rule: Keep) -> &'static str {
    match rule {
        Keep::Newest => "dup-keep-newest",
        Keep::Oldest => "dup-keep-oldest",
        Keep::ShortestPath => "dup-keep-shortest",
    }
}

/// Lists exactly what will happen (a dry run) and asks before doing it.
fn confirm_dialog(ui: &Ui, env: &mut Env<'_>, found: &mut Found) {
    let Some((op, changes)) = &found.confirm else {
        return;
    };
    let (op, i) = (*op, env.i18n);
    let size: u64 = changes.iter().map(|c| c.size).sum();
    let mut decision = None;
    let modal = Modal::new(Id::unique("dup-confirm")).show(ui.ctx(), |ui| {
        ui.set_width(560.0);
        ui.with_layout(egui::Layout::top_down(i.dir().start()), |ui| {
            let (title, body) = match op {
                Op::Trash => ("dup-confirm-trash", "dup-confirm-trash-body"),
                Op::HardLink => ("dup-confirm-link", "dup-confirm-link-body"),
            };
            ui.heading(i.t(title));
            ui.add_space(4.0);
            crate::widgets::paragraph(
                ui,
                i,
                &i.la(
                    body,
                    &[
                        ("count", (changes.len() as u64).into()),
                        ("size", env.size(size).into()),
                    ],
                ),
            );
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for c in changes.iter().take(PREVIEW_LINES) {
                        let line = match op {
                            Op::Trash => c.path.display().to_string(),
                            Op::HardLink => format!("{} → {}", c.path.display(), c.keep.display()),
                        };
                        ui.label(RichText::new(i.v(&line)).small());
                    }
                    if changes.len() > PREVIEW_LINES {
                        ui.weak(i.count("dup-more", (changes.len() - PREVIEW_LINES) as u64));
                    }
                });
            ui.add_space(12.0);
            row(ui, i.dir(), |ui| {
                let go = match op {
                    Op::Trash => "dup-trash",
                    Op::HardLink => "dup-link",
                };
                let button = egui::Button::new(RichText::new(i.t(go)).color(egui::Color32::WHITE))
                    .fill(ui.visuals().error_fg_color);
                if ui.add(button).clicked() {
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
            if let Some((op, changes)) = found.confirm.take() {
                match run(op, changes, ui.ctx()) {
                    Ok(job) => found.running = Some(job),
                    Err(e) => env.push(Action::Notify(i.v(&e.to_string()), true)),
                }
            }
        }
        Some(false) => found.confirm = None,
        None => {}
    }
}

/// Writes the found groups as CSV: group number, SHA-256, path, size and
/// modification date of every copy.
pub fn export(
    scan: &ScanView,
    headers: [String; 5],
    readable: Option<&dyn Fn(u64) -> String>,
    out: impl std::io::Write,
) -> std::io::Result<()> {
    let State::Done(found) = &scan.duplicates else {
        return Ok(());
    };
    let mut writer = Delimited::csv(out).with_bom();
    if let Some(r) = readable {
        writer = writer.readable_sizes(r);
    }
    writer.header(&headers)?;
    let tree = &scan.tree;
    for (n, g) in found.groups.iter().enumerate() {
        let sha = g.sha256.as_ref().map(duplicates::hex).unwrap_or_default();
        for id in live(tree, g) {
            writer.row(&[
                Value::Count(n as u64 + 1),
                Value::Text(sha.as_str().into()),
                Value::Text(tree.path(id).to_string_lossy()),
                Value::Bytes(g.size),
                Value::Date(tree.node(id).modified),
            ])?;
        }
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use storagemap_core::{EntryInfo, Kind, TreeBuilder};

    fn tree() -> (Tree, [NodeId; 4]) {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            owner: storagemap_core::NO_OWNER,
            size: 0,
            allocated: 0,
        };
        let file = EntryInfo {
            kind: Kind::File,
            size: 10,
            ..dir
        };
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir);
        let ids = [b"a", b"b", b"c", b"d"].map(|n| b.push(NodeId::ROOT, n, file));
        (b.finish(), ids)
    }

    #[test]
    fn plan_keeps_one_copy_per_group() {
        let (t, [a, b, c, d]) = tree();
        let groups = vec![
            DuplicateGroup {
                size: 10,
                files: vec![a, b],
                sha256: None,
            },
            DuplicateGroup {
                size: 10,
                files: vec![c, d],
                sha256: None,
            },
        ];
        // Every copy of the second group is marked: it is left alone.
        let marked = HashSet::from([b, c, d]);
        let changes = plan(&t, &groups, &marked);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file, b);
        assert_eq!(changes[0].keep, PathBuf::from("/r/a"));
    }
}
