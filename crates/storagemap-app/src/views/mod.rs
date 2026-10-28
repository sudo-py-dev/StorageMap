//! Screens and panels. Views draw from state and report intents as
//! [`Action`]s; only the app applies them.

mod changes;
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

use storagemap_core::analysis::{AgeStat, ExtensionStat};
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

impl Env<'_> {
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
}

impl Tab {
    pub const ALL: [Tab; 9] = [
        Tab::Treemap,
        Tab::Details,
        Tab::Cleanup,
        Tab::TopFiles,
        Tab::Types,
        Tab::Ages,
        Tab::Changes,
        Tab::Duplicates,
        Tab::Search,
    ];

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
    pub search_edited: Option<std::time::Instant>,
    pub details_sort: details::Sort,
    pub context_target: Option<NodeId>,
    pub treemap: treemap::Cache,
    pub(super) tree_rows: Cached<(Metric, u64, u64), Vec<tree_panel::Row>>,
    pub(super) expansion_version: u64,
    pub(super) top_files: Cached<AnalysisKey, Vec<NodeId>>,
    pub(super) types: Cached<AnalysisKey, Vec<ExtensionStat>>,
    pub(super) ages: Cached<(NodeId, u64), [AgeStat; 7]>,
    pub(super) search: Cached<(NodeId, String, u64), Vec<NodeId>>,
    pub(super) details_rows: Cached<(NodeId, details::Sort, u64), Vec<NodeId>>,
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
            search_edited: None,
            details_sort: details::Sort::default(),
            context_target: None,
            treemap: treemap::Cache::default(),
            tree_rows: Cached::default(),
            expansion_version: 0,
            top_files: Cached::default(),
            types: Cached::default(),
            ages: Cached::default(),
            search: Cached::default(),
            details_rows: Cached::default(),
            comparison: None,
            snapshots: Vec::new(),
            change_rows: Cached::default(),
            duplicates: duplicates::State::default(),
            cleanup: Cached::default(),
            cleanup_expanded: HashSet::new(),
        }
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
