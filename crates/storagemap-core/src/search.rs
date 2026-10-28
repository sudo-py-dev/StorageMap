//! Searching a scanned tree by name, size, dates, owner and attributes.

use std::cmp::Reverse;

use crate::tree::{Flags, Kind, NodeId, Tree};

/// Which kinds of entries a search returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Entries {
    #[default]
    All,
    Files,
    Folders,
}

/// What a search looks for. Every criterion that is set must match; an
/// empty query matches nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    /// Part of the name, or a pattern with `*` and `?` matched against the
    /// whole name. Case is ignored.
    pub name: String,
    pub entries: Entries,
    /// Size bounds in bytes, inclusive.
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    /// Last modification bounds, Unix seconds, inclusive.
    pub modified_after: Option<i64>,
    pub modified_before: Option<i64>,
    pub owner: Option<u32>,
    /// Attributes the entry must all have.
    pub attributes: Flags,
}

impl SearchQuery {
    /// Whether no criterion is set.
    pub fn is_empty(&self) -> bool {
        self.name.trim().is_empty()
            && self.entries == Entries::All
            && self.min_size.is_none()
            && self.max_size.is_none()
            && self.modified_after.is_none()
            && self.modified_before.is_none()
            && self.owner.is_none()
            && self.attributes == Flags::empty()
    }
}

/// Name matching prepared once per search.
enum NameMatch {
    Any,
    Contains(String),
    Pattern(String),
}

impl NameMatch {
    fn new(name: &str) -> Self {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            NameMatch::Any
        } else if name.contains(['*', '?']) {
            NameMatch::Pattern(name)
        } else {
            NameMatch::Contains(name)
        }
    }

    fn matches(&self, name: &str) -> bool {
        match self {
            NameMatch::Any => true,
            NameMatch::Contains(part) => name.to_lowercase().contains(part.as_str()),
            NameMatch::Pattern(p) => crate::filter::glob(p, &name.to_lowercase()),
        }
    }
}

/// Entries below `root` matching `query` and `also` (for criteria the
/// caller adds, such as file categories), largest first, at most `limit`.
pub fn search(
    tree: &Tree,
    root: NodeId,
    query: &SearchQuery,
    also: impl Fn(&Tree, NodeId) -> bool,
    limit: usize,
) -> Vec<NodeId> {
    if query.is_empty() {
        return Vec::new();
    }
    let name = NameMatch::new(&query.name);
    let mut hits: Vec<NodeId> = tree
        .walk(root)
        .filter(|&id| {
            let node = tree.node(id);
            id != root
                && match query.entries {
                    Entries::All => true,
                    Entries::Files => node.kind != Kind::Dir,
                    Entries::Folders => node.kind == Kind::Dir,
                }
                && query.min_size.is_none_or(|min| node.size >= min)
                && query.max_size.is_none_or(|max| node.size <= max)
                && query.modified_after.is_none_or(|t| node.modified >= t)
                && query.modified_before.is_none_or(|t| node.modified <= t)
                && query.owner.is_none_or(|o| node.owner == o)
                && node.flags.contains(query.attributes)
                && name.matches(&tree.name(id))
                && also(tree, id)
        })
        .collect();
    hits.sort_unstable_by_key(|&id| Reverse(tree.node(id).size));
    hits.truncate(limit);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryInfo, TreeBuilder};
    use std::path::PathBuf;

    fn sample() -> Tree {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
            size: 0,
            allocated: 0,
        };
        let file = |size, modified, owner| EntryInfo {
            kind: Kind::File,
            size,
            modified,
            owner,
            ..dir
        };
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir);
        let docs = b.push(NodeId::ROOT, b"Docs", dir);
        b.push(docs, b"report.PDF", file(300, 100, 1000));
        b.push(docs, b"notes.txt", file(10, 900, 1000));
        let mut hidden = file(5_000, 500, 0);
        hidden.flags.insert(Flags::HIDDEN);
        b.push(NodeId::ROOT, b".cache.bin", hidden);
        b.push(NodeId::ROOT, b"movie.mkv", file(90_000, 50, 0));
        b.finish()
    }

    fn names(t: &Tree, q: &SearchQuery) -> Vec<String> {
        search(t, t.root(), q, |_, _| true, 100)
            .into_iter()
            .map(|id| t.name(id).into_owned())
            .collect()
    }

    #[test]
    fn text_and_patterns_ignore_case() {
        let t = sample();
        let q = |name: &str| SearchQuery {
            name: name.into(),
            ..SearchQuery::default()
        };
        assert_eq!(names(&t, &q("PDF")), ["report.PDF"]);
        assert_eq!(names(&t, &q("*.txt")), ["notes.txt"]);
        assert_eq!(
            names(&t, &q("*o*")),
            ["movie.mkv", "Docs", "report.PDF", "notes.txt"]
        );
        assert!(names(&t, &q("  ")).is_empty());
    }

    #[test]
    fn criteria_combine() {
        let t = sample();
        let big_files = SearchQuery {
            entries: Entries::Files,
            min_size: Some(200),
            ..SearchQuery::default()
        };
        assert_eq!(
            names(&t, &big_files),
            ["movie.mkv", ".cache.bin", "report.PDF"]
        );
        let old_and_owned = SearchQuery {
            modified_before: Some(200),
            owner: Some(1000),
            ..SearchQuery::default()
        };
        assert_eq!(names(&t, &old_and_owned), ["report.PDF"]);
        let hidden = SearchQuery {
            attributes: Flags::HIDDEN,
            ..SearchQuery::default()
        };
        assert_eq!(names(&t, &hidden), [".cache.bin"]);
        let folders = SearchQuery {
            entries: Entries::Folders,
            ..SearchQuery::default()
        };
        assert_eq!(names(&t, &folders), ["Docs"]);
    }

    #[test]
    fn caller_criteria_apply_too() {
        let t = sample();
        let q = SearchQuery {
            entries: Entries::Files,
            ..SearchQuery::default()
        };
        let mkv = search(&t, t.root(), &q, |t, id| t.name(id).ends_with(".mkv"), 10);
        assert_eq!(mkv.len(), 1);
    }
}
