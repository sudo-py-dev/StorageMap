//! Largest files, file types and age distribution.

use egui::{Response, Ui, vec2};
use storagemap_core::analysis::{self, AgeBucket, AgeStat, ExtensionStat, OwnerStat};
use storagemap_core::{Metric, NodeId, Tree, ratio};

use super::table::{self, Col, TableSource};
use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::filetype::Category;
use crate::fmt;

const TOP_FILES: usize = 500;

/// A list of entries with name, value and containing folder.
struct EntryList<'a> {
    tree: &'a Tree,
    ids: &'a [NodeId],
    selected: Option<NodeId>,
}

pub(super) const ENTRY_COLUMNS: [Col; 4] = [
    Col::fixed("col-name", 240.0, 1),
    Col::fixed("col-size", 92.0, 2),
    Col::flex("col-path"),
    Col::fixed("col-modified", 88.0, 3),
];

impl TableSource for EntryList<'_> {
    fn rows(&self) -> usize {
        self.ids.len()
    }

    fn is_selected(&self, row: usize) -> bool {
        self.selected == Some(self.ids[row])
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        let id = self.ids[row];
        match col {
            0 => table::text(ui, env, &self.tree.name(id), false),
            1 => table::text(ui, env, &env.metric_text(self.tree.node(id)), true),
            3 => table::text(ui, env, &fmt::date(self.tree.node(id).modified), true),
            _ => {
                let parent = self
                    .tree
                    .parent(id)
                    .map(|p| self.tree.path(p).to_string_lossy().into_owned())
                    .unwrap_or_default();
                table::text(ui, env, &parent, false);
            }
        }
    }

    fn row_response(&mut self, env: &mut Env<'_>, row: usize, response: &Response) {
        let id = self.ids[row];
        if response.double_clicked() {
            env.push(Action::Focus(id));
        } else if response.clicked() {
            env.push(Action::Select(id));
        }
        response.context_menu(|ui| node_menu::show(ui, env, self.tree, id));
    }
}

pub fn top_files(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let metric = match env.settings.metric {
        Metric::FileCount => Metric::Size,
        m => m,
    };
    let (tree, focus) = (&scan.tree, scan.focus);
    let ids = scan.top_files.get((focus, metric, scan.generation), || {
        analysis::top_files(tree, focus, metric, TOP_FILES)
    });
    table::show(
        ui,
        env,
        "top-files",
        &ENTRY_COLUMNS,
        &mut EntryList {
            tree,
            ids,
            selected: scan.selected,
        },
    );
}

struct TypeList<'a> {
    stats: &'a [ExtensionStat],
    total: u64,
    metric: Metric,
}

const TYPE_COLUMNS: [Col; 5] = [
    Col::fixed("col-extension", 110.0, 1),
    Col::fixed("col-type", 160.0, 3),
    Col::fixed("col-size", 92.0, 2),
    Col::flex("col-share"),
    Col::fixed("col-files", 80.0, 4),
];

impl TableSource for TypeList<'_> {
    fn rows(&self) -> usize {
        self.stats.len()
    }

    fn is_selected(&self, _: usize) -> bool {
        false
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        let s = &self.stats[row];
        let category = Category::of_extension(&s.extension);
        match col {
            0 => {
                let (swatch, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch, 2.0, category.color());
                let ext = if s.extension.is_empty() {
                    env.i18n.logical("no-extension")
                } else {
                    format!(".{}", s.extension)
                };
                table::text(ui, env, &ext, false);
            }
            1 => table::text(ui, env, &env.i18n.logical(category.label_id()), false),
            2 => table::text(ui, env, &env.metric_value_text(s.metric(self.metric)), true),
            3 => table::share(ui, env, ratio(s.metric(self.metric), self.total)),
            _ => table::text(ui, env, &fmt::count(s.files), true),
        }
    }

    fn row_response(&mut self, _: &mut Env<'_>, _: usize, _: &Response) {}
}

struct OwnerList<'a> {
    stats: &'a [OwnerStat],
    total: u64,
    metric: Metric,
}

const OWNER_COLUMNS: [Col; 4] = [
    Col::fixed("col-owner", 200.0, 1),
    Col::fixed("col-size", 92.0, 2),
    Col::flex("col-share"),
    Col::fixed("col-files", 80.0, 3),
];

impl TableSource for OwnerList<'_> {
    fn rows(&self) -> usize {
        self.stats.len()
    }

    fn is_selected(&self, _: usize) -> bool {
        false
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        let s = &self.stats[row];
        match col {
            0 => table::text(ui, env, &env.owner_name(s.owner), false),
            1 => table::text(ui, env, &env.metric_value_text(s.metric(self.metric)), true),
            2 => table::share(ui, env, ratio(s.metric(self.metric), self.total)),
            _ => table::text(ui, env, &fmt::count(s.files), true),
        }
    }

    fn row_response(&mut self, _: &mut Env<'_>, _: usize, _: &Response) {}
}

/// File types, or owners when the user switched to them.
pub fn types(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let metric = env.settings.metric;
    let (tree, focus) = (&scan.tree, scan.focus);
    let owners = scan.owners.get((focus, metric, scan.generation), || {
        analysis::owners(tree, focus, metric)
    });
    // Only worth offering when the engine knows who owns what.
    if owners.iter().any(|o| o.owner != storagemap_core::NO_OWNER) {
        crate::widgets::row(ui, env.i18n.dir(), |ui| {
            ui.selectable_value(&mut scan.by_owner, false, env.t("types-by-type"));
            ui.selectable_value(&mut scan.by_owner, true, env.t("types-by-owner"));
        });
        ui.add_space(6.0);
        if scan.by_owner {
            let total = owners.iter().map(|s| s.metric(metric)).sum();
            table::show(
                ui,
                env,
                "owners",
                &OWNER_COLUMNS,
                &mut OwnerList {
                    stats: owners,
                    total,
                    metric,
                },
            );
            return;
        }
    }
    let stats = scan.types.get((focus, metric, scan.generation), || {
        analysis::extensions(tree, focus, metric)
    });
    let total = stats.iter().map(|s| s.metric(metric)).sum();
    table::show(
        ui,
        env,
        "types",
        &TYPE_COLUMNS,
        &mut TypeList {
            stats,
            total,
            metric,
        },
    );
}

struct AgeList<'a> {
    stats: &'a [AgeStat; 7],
    total: u64,
    metric: Metric,
}

const AGE_COLUMNS: [Col; 4] = [
    Col::fixed("col-modified", 170.0, 1),
    Col::fixed("col-size", 92.0, 2),
    Col::flex("col-share"),
    Col::fixed("col-files", 80.0, 3),
];

pub fn age_label_id(bucket: AgeBucket) -> &'static str {
    match bucket {
        AgeBucket::Week => "age-week",
        AgeBucket::Month => "age-month",
        AgeBucket::HalfYear => "age-half-year",
        AgeBucket::Year => "age-year",
        AgeBucket::TwoYears => "age-two-years",
        AgeBucket::FiveYears => "age-five-years",
        AgeBucket::Older => "age-older",
    }
}

fn age_metric(s: &AgeStat, metric: Metric) -> u64 {
    match metric {
        Metric::Size => s.size,
        Metric::Allocated => s.allocated,
        Metric::FileCount => s.files,
    }
}

impl TableSource for AgeList<'_> {
    fn rows(&self) -> usize {
        self.stats.len()
    }

    fn is_selected(&self, _: usize) -> bool {
        false
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        let s = &self.stats[row];
        match col {
            0 => table::text(
                ui,
                env,
                &env.i18n.logical(age_label_id(AgeBucket::ALL[row])),
                false,
            ),
            1 => table::text(
                ui,
                env,
                &env.metric_value_text(age_metric(s, self.metric)),
                true,
            ),
            2 => table::share(ui, env, ratio(age_metric(s, self.metric), self.total)),
            _ => table::text(ui, env, &fmt::count(s.files), true),
        }
    }

    fn row_response(&mut self, _: &mut Env<'_>, _: usize, _: &Response) {}
}

pub fn ages(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let metric = env.settings.metric;
    let (tree, focus) = (&scan.tree, scan.focus);
    let stats = scan.ages.get((focus, scan.generation), || {
        analysis::ages(tree, focus, fmt::now_unix())
    });
    let total = stats.iter().map(|s| age_metric(s, metric)).sum();
    table::show(
        ui,
        env,
        "ages",
        &AGE_COLUMNS,
        &mut AgeList {
            stats,
            total,
            metric,
        },
    );
}
