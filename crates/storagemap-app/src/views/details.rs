//! Details tab: the focused folder's entries, sortable by any column.

use std::cmp::Ordering;

use egui::{Response, Ui};
use storagemap_core::analysis::extension_of;
use storagemap_core::{Kind, NodeId, Tree};

use super::table::{self, Col, TableSource};
use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    Name,
    #[default]
    Size,
    Allocated,
    Files,
    Folders,
    Modified,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub by: SortBy,
    pub descending: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Sort {
            by: SortBy::Size,
            descending: true,
        }
    }
}

/// Columns in logical order and what each sorts by.
const COLUMNS: [(Col, Option<SortBy>); 8] = [
    (Col::flex("col-name"), Some(SortBy::Name)),
    (Col::fixed("col-size", 82.0, 1), Some(SortBy::Size)),
    (
        Col::fixed("col-allocated", 82.0, 4),
        Some(SortBy::Allocated),
    ),
    (Col::fixed("col-percent", 110.0, 2), None),
    (Col::fixed("col-files", 70.0, 5), Some(SortBy::Files)),
    (Col::fixed("col-folders", 62.0, 6), Some(SortBy::Folders)),
    (Col::fixed("col-modified", 88.0, 3), Some(SortBy::Modified)),
    (Col::fixed("col-type", 76.0, 7), Some(SortBy::Type)),
];

pub fn sorted_children(tree: &Tree, focus: NodeId, sort: Sort) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = tree.children(focus).collect();
    let key = |a: &NodeId, b: &NodeId| -> Ordering {
        let (x, y) = (tree.node(*a), tree.node(*b));
        match sort.by {
            SortBy::Name => tree
                .name(*a)
                .to_lowercase()
                .cmp(&tree.name(*b).to_lowercase()),
            SortBy::Size => x.size.cmp(&y.size),
            SortBy::Allocated => x.allocated.cmp(&y.allocated),
            SortBy::Files => x.files.cmp(&y.files),
            SortBy::Folders => x.dirs.cmp(&y.dirs),
            SortBy::Modified => x.modified.cmp(&y.modified),
            SortBy::Type => (x.kind != Kind::Dir, extension_of(tree.name_bytes(*a)))
                .cmp(&(y.kind != Kind::Dir, extension_of(tree.name_bytes(*b)))),
        }
    };
    ids.sort_by(|a, b| {
        if sort.descending {
            key(b, a)
        } else {
            key(a, b)
        }
    });
    ids
}

pub fn kind_label_id(kind: Kind) -> &'static str {
    match kind {
        Kind::Dir => "type-folder",
        Kind::File => "type-file",
        Kind::Symlink => "type-link",
        Kind::Other => "type-other",
    }
}

struct Source<'a> {
    tree: &'a Tree,
    rows: &'a [NodeId],
    selected: Option<NodeId>,
    sort: &'a mut Sort,
}

impl TableSource for Source<'_> {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn is_selected(&self, row: usize) -> bool {
        self.selected == Some(self.rows[row])
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        let id = self.rows[row];
        let node = self.tree.node(id);
        match col {
            0 => {
                let icon = if node.is_dir() {
                    "📁"
                } else if node.kind == Kind::Symlink {
                    "🔗"
                } else {
                    "📄"
                };
                table::text(ui, env, &format!("{icon} {}", self.tree.name(id)), false);
            }
            1 => table::text(ui, env, &env.size(node.size), true),
            2 => table::text(ui, env, &env.size(node.allocated), true),
            3 => table::share(ui, env, self.tree.share_of_parent(id, env.settings.metric)),
            4 => table::text(ui, env, &fmt::count(node.files), true),
            5 => table::text(ui, env, &fmt::count(node.dirs), true),
            6 => table::text(ui, env, &fmt::date(node.modified), true),
            _ => {
                let ext = extension_of(self.tree.name_bytes(id));
                let kind = if node.kind == Kind::File && !ext.is_empty() {
                    format!(".{ext}")
                } else {
                    env.i18n.logical(kind_label_id(node.kind))
                };
                table::text(ui, env, &kind, false);
            }
        }
    }

    fn row_response(&mut self, env: &mut Env<'_>, row: usize, response: &Response) {
        let id = self.rows[row];
        if response.double_clicked() && self.tree.node(id).is_dir() {
            env.push(Action::Focus(id));
        } else if response.clicked() {
            env.push(Action::Select(id));
        }
        response.context_menu(|ui| node_menu::show(ui, env, self.tree, id));
    }

    fn header_clicked(&mut self, col: usize) {
        if let Some(by) = COLUMNS[col].1 {
            *self.sort = if self.sort.by == by {
                Sort {
                    by,
                    descending: !self.sort.descending,
                }
            } else {
                Sort {
                    by,
                    descending: by != SortBy::Name && by != SortBy::Type,
                }
            };
        }
    }

    fn sorted(&self, col: usize) -> Option<bool> {
        (COLUMNS[col].1 == Some(self.sort.by)).then_some(self.sort.descending)
    }
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let (tree, focus, sort) = (&scan.tree, scan.focus, scan.details_sort);
    let rows = scan.details_rows.get((focus, sort, scan.generation), || {
        sorted_children(tree, focus, sort)
    });
    if rows.is_empty() {
        crate::widgets::empty_state(
            ui,
            env.i18n,
            "📭",
            "empty-folder",
            "empty-folder-hint",
            |_| {},
        );
        return;
    }
    let cols: Vec<Col> = COLUMNS.iter().map(|(c, _)| *c).collect();
    let mut source = Source {
        tree,
        rows,
        selected: scan.selected,
        sort: &mut scan.details_sort,
    };
    table::show(ui, env, "details", &cols, &mut source);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use storagemap_core::{EntryInfo, Flags, TreeBuilder};

    #[test]
    fn sorts_by_name_and_size() {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            size: 0,
            allocated: 0,
        };
        let file = |size| EntryInfo {
            kind: Kind::File,
            size,
            allocated: size,
            ..dir
        };
        let mut b = TreeBuilder::new(PathBuf::from("/"), dir);
        b.push(NodeId(0), b"b", file(1));
        b.push(NodeId(0), b"A", file(3));
        b.push(NodeId(0), b"c", file(2));
        let t = b.finish();
        let names = |s| {
            sorted_children(&t, t.root(), s)
                .iter()
                .map(|&i| t.name(i).into_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(Sort {
                by: SortBy::Name,
                descending: false
            }),
            ["A", "b", "c"]
        );
        assert_eq!(
            names(Sort {
                by: SortBy::Size,
                descending: true
            }),
            ["A", "c", "b"]
        );
    }
}
