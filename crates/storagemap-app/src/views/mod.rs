//! Screens and panels. Views draw from state and report intents as
//! [`Action`]s; only the app applies them.

mod changes;
pub mod chart;
pub mod chrome;
mod cleanup;
mod details;
pub mod duplicates;
mod lists;
mod node_menu;
pub mod settings;
mod start;
mod table;
mod tree_panel;
mod treemap;

use std::collections::HashSet;
use std::time::Duration;

use storagemap_core::analysis::{AgeStat, ExtensionStat, OwnerStat};
use storagemap_core::snapshot::{Comparison, SnapshotInfo};
use storagemap_core::{Backend, Metric, Node, NodeId, ScanResult, Tree};

use crate::app::Action;
use crate::fmt;
use crate::i18n::I18n;
use crate::settings::Settings;

/// Read-only context plus the action sink, handed to every view.
pub struct Env<'a> {
    pub i18n: &'a I18n,
    pub settings: &'a Settings,
    pub actions: &'a mut Vec<Action>,
}

/// Share of its folder above which an item stands out.
const LARGE_SHARE_OF_PARENT: f64 = 0.25;
/// ...as long as it is also this much of the whole scan, so a big file in a
/// tiny folder does not.
const LARGE_SHARE_OF_SCAN: f64 = 0.02;

impl Env<'_> {
    /// Whether `id` should be highlighted as unusually large.
    pub fn is_large(&self, tree: &Tree, id: NodeId) -> bool {
        let metric = self.settings.metric;
        self.settings.highlight_large
            && id != tree.root()
            && tree.share_of_parent(id, metric) >= LARGE_SHARE_OF_PARENT
            && storagemap_core::ratio(
                tree.node(id).metric(metric),
                tree.node(tree.root()).metric(metric),
            ) >= LARGE_SHARE_OF_SCAN
    }

    /// Translated, display-ready text.
    pub fn t(&self, id: &str) -> String {
        self.i18n.t(id)
    }

    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// Logical-order size text in the user's units.
    pub fn size(&self, bytes: u64) -> String {
        fmt::size(
            bytes,
            self.settings.unit,
            usize::from(self.settings.decimals),
            |id| self.i18n.logical(id),
        )
    }

    /// Logical-order value of `node` for the active metric.
    pub fn metric_text(&self, node: &Node) -> String {
        match self.settings.metric {
            Metric::Size => self.size(node.size),
            Metric::Allocated => self.size(node.allocated),
            Metric::FileCount => fmt::count(node.files),
        }
    }

    /// Logical-order name of an owner id.
    pub fn owner_name(&self, owner: u32) -> String {
        if owner == storagemap_core::NO_OWNER {
            return self.i18n.logical("owner-unknown");
        }
        crate::platform::user_name(owner).unwrap_or_else(|| {
            self.i18n
                .la("owner-id", &[("id", owner.to_string().into())])
        })
    }

    pub fn metric_value_text(&self, value: u64) -> String {
        match self.settings.metric {
            Metric::FileCount => fmt::count(value),
            _ => self.size(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Treemap,
    Details,
    TopFiles,
    Types,
    Ages,
    Search,
    Changes,
    Duplicates,
    Cleanup,
    Chart,
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Treemap,
        Tab::Chart,
        Tab::Details,
        Tab::Cleanup,
        Tab::TopFiles,
        Tab::Types,
        Tab::Ages,
        Tab::Changes,
        Tab::Duplicates,
        Tab::Search,
    ];

    pub fn icon(self) -> &'static str {
        use crate::icons;
        match self {
            Tab::Treemap => icons::TREEMAP,
            Tab::Details => icons::DETAILS,
            Tab::TopFiles => icons::LARGEST,
            Tab::Types => icons::TYPES,
            Tab::Ages => icons::AGE,
            Tab::Search => icons::SEARCH,
            Tab::Changes => icons::CHANGES,
            Tab::Duplicates => icons::DUPLICATES,
            Tab::Cleanup => icons::CLEANUP,
            Tab::Chart => icons::CHART_DONUT,
        }
    }

    pub fn label_id(self) -> &'static str {
        match self {
            Tab::Treemap => "tab-treemap",
            Tab::Details => "tab-details",
            Tab::TopFiles => "tab-top-files",
            Tab::Types => "tab-types",
            Tab::Ages => "tab-ages",
            Tab::Search => "tab-search",
            Tab::Changes => "tab-changes",
            Tab::Duplicates => "tab-duplicates",
            Tab::Cleanup => "tab-cleanup",
            Tab::Chart => "tab-chart",
        }
    }
}

/// A cached computation, valid while its key matches.
pub struct Cached<K, V> {
    key: Option<K>,
    value: V,
}

impl<K: PartialEq, V: Default> Default for Cached<K, V> {
    fn default() -> Self {
        Cached {
            key: None,
            value: V::default(),
        }
    }
}

impl<K: PartialEq, V> Cached<K, V> {
    /// The value as last computed by [`Cached::get`].
    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn get(&mut self, key: K, compute: impl FnOnce() -> V) -> &V {
        if self.key.as_ref() != Some(&key) {
            self.value = compute();
            self.key = Some(key);
        }
        &self.value
    }
}

/// (focus, metric, generation) — what most analyses depend on.
pub(super) type AnalysisKey = (NodeId, Metric, u64);

/// Everything about a loaded scan the UI keeps between frames.
pub struct ScanView {
    pub tree: Tree,
    pub backend: Backend,
    pub elapsed: Duration,
    pub errors: u64,
    pub cancelled: bool,
    /// The tree is a partial one from a scan still running; it is replaced
    /// as the scan progresses.
    pub scanning: bool,
    /// Folder shown by the central views.
    pub focus: NodeId,
    pub selected: Option<NodeId>,
    pub tab: Tab,
    /// When the tab last changed, for the content fade-in.
    pub tab_changed: Option<std::time::Instant>,
    /// Bumped whenever the tree changes (deletions).
    pub generation: u64,
    pub expanded: HashSet<NodeId>,
    pub scroll_tree_to: Option<NodeId>,
    /// Folder tree scroll offset and viewport height from the last frame.
    pub(super) tree_scroll: (f32, f32),
    pub search_query: String,
    /// The Search tab's field takes focus on its next frame (Ctrl+F, opening the tab).
    pub focus_search: bool,
    pub search_edited: Option<std::time::Instant>,
    pub details_sort: details::Sort,
    pub context_target: Option<NodeId>,
    pub treemap: treemap::Cache,
    pub(super) tree_rows: Cached<(Metric, u64, u64, tree_panel::TreeFilter), Vec<tree_panel::Row>>,
    /// Which files the folder tree counts.
    pub tree_filter: tree_panel::TreeFilter,
    pub(super) filtered: Cached<(tree_panel::TreeFilter, Metric, u64), Vec<u64>>,
    pub(super) expansion_version: u64,
    pub(super) top_files: Cached<AnalysisKey, Vec<NodeId>>,
    pub(super) types: Cached<AnalysisKey, Vec<ExtensionStat>>,
    pub(super) owners: Cached<AnalysisKey, Vec<OwnerStat>>,
    /// The Types tab lists owners instead of file types.
    pub(super) by_owner: bool,
    /// Owners anywhere in the scan, for the folder tree's filter.
    pub(super) all_owners: Cached<u64, Vec<u32>>,
    pub(super) ages: Cached<(NodeId, u64), [AgeStat; 7]>,
    pub(super) search: Cached<(NodeId, String, u64), Vec<NodeId>>,
    pub(super) details_rows: Cached<(NodeId, details::Sort, u64), Vec<NodeId>>,
    pub(super) chart: Cached<(NodeId, Metric, u64), Vec<chart::Slice>>,
    /// Active comparison with an earlier snapshot.
    pub comparison: Option<Comparison>,
    /// Earlier snapshots of this folder, newest first.
    pub snapshots: Vec<SnapshotInfo>,
    pub(super) change_rows: Cached<(NodeId, u64, i64), Vec<changes::Row>>,
    pub(super) duplicates: duplicates::State,
    pub(super) cleanup: Cached<(NodeId, u64), Vec<storagemap_core::cleanup::Suggestion>>,
    pub(super) cleanup_expanded: HashSet<storagemap_core::cleanup::CleanupKind>,
}

impl ScanView {
    pub fn new(result: ScanResult) -> Self {
        let root = result.tree.root();
        ScanView {
            tree: result.tree,
            backend: result.backend,
            elapsed: result.elapsed,
            errors: result.errors,
            cancelled: result.cancelled,
            scanning: false,
            focus: root,
            selected: Some(root),
            tab: Tab::default(),
            // Fade the results in when a scan finishes.
            tab_changed: Some(std::time::Instant::now()),
            generation: 0,
            expanded: HashSet::from([root]),
            scroll_tree_to: None,
            tree_scroll: (0.0, 0.0),
            search_query: String::new(),
            focus_search: false,
            search_edited: None,
            details_sort: details::Sort::default(),
            context_target: None,
            treemap: treemap::Cache::default(),
            tree_rows: Cached::default(),
            tree_filter: tree_panel::TreeFilter::default(),
            filtered: Cached::default(),
            expansion_version: 0,
            top_files: Cached::default(),
            types: Cached::default(),
            owners: Cached::default(),
            by_owner: false,
            all_owners: Cached::default(),
            ages: Cached::default(),
            search: Cached::default(),
            details_rows: Cached::default(),
            chart: Cached::default(),
            comparison: None,
            snapshots: Vec::new(),
            change_rows: Cached::default(),
            duplicates: duplicates::State::default(),
            cleanup: Cached::default(),
            cleanup_expanded: HashSet::new(),
        }
    }

    /// Swaps in a newer tree of the same folder (a fresher partial tree, or
    /// the finished one), keeping focus, selection and open folders by path.
    pub fn replace_tree(&mut self, tree: Tree) {
        let path = |id: NodeId| self.tree.path(id);
        let focus = path(self.focus);
        let selected = self.selected.map(path);
        let expanded: Vec<_> = self.expanded.iter().map(|&id| path(id)).collect();
        self.tree = tree;
        let root = self.tree.root();
        self.focus = self.tree.find(&focus).unwrap_or(root);
        self.selected = selected.and_then(|p| self.tree.find(&p));
        self.expanded = expanded.iter().filter_map(|p| self.tree.find(p)).collect();
        self.expanded.insert(root);
        self.scroll_tree_to = None;
        self.context_target = None;
        self.duplicates = duplicates::State::default();
        self.treemap.invalidate();
        self.generation += 1;
        self.expansion_changed();
    }

    /// Replaces a live view's partial tree with the finished scan.
    pub fn finish(&mut self, result: ScanResult) {
        self.replace_tree(result.tree);
        self.backend = result.backend;
        self.elapsed = result.elapsed;
        self.errors = result.errors;
        self.cancelled = result.cancelled;
        self.scanning = false;
    }

    /// Shows `id` (a folder; files show their parent) in the central views.
    pub fn set_focus(&mut self, id: NodeId) {
        let dir = if self.tree.node(id).is_dir() {
            id
        } else {
            self.tree.parent(id).unwrap_or(id)
        };
        self.focus = dir;
        self.select(id);
    }

    /// Selects `id` and reveals it in the folder tree.
    pub fn select(&mut self, id: NodeId) {
        self.selected = Some(id);
        let mut changed = false;
        for a in self.tree.ancestry(id) {
            if a != id || !self.tree.node(id).is_dir() {
                changed |= self.expanded.insert(a);
            }
        }
        if changed {
            self.expansion_changed();
        }
        self.scroll_tree_to = Some(if self.tree.node(id).is_dir() {
            id
        } else {
            self.tree.parent(id).unwrap_or(id)
        });
    }

    pub fn up(&mut self) {
        if let Some(parent) = self.tree.parent(self.focus) {
            self.set_focus(parent);
        }
    }

    pub fn toggle(&mut self, id: NodeId) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.expansion_changed();
    }

    pub fn set_expanded(&mut self, id: NodeId, open: bool) {
        let changed = if open {
            self.expanded.insert(id)
        } else {
            self.expanded.remove(&id)
        };
        if changed {
            self.expansion_changed();
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded = HashSet::from([self.tree.root()]);
        self.expansion_changed();
    }

    fn expansion_changed(&mut self) {
        self.expansion_version += 1;
    }

    /// Removes a deleted entry and moves focus/selection off it.
    pub fn remove(&mut self, id: NodeId) {
        let parent = self.tree.parent(id).unwrap_or(self.tree.root());
        let ancestry = self.tree.ancestry(self.focus);
        self.tree.remove(id);
        self.generation += 1;
        if ancestry.contains(&id) {
            self.focus = parent;
        }
        if self
            .selected
            .is_some_and(|s| self.tree.ancestry(s).contains(&id))
        {
            self.selected = Some(parent);
        }
        self.treemap.invalidate();
        self.expansion_changed();
    }
}
