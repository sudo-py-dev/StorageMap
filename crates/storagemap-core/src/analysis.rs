//! Whole-subtree analyses: largest files, file types, ages and name search.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::tree::{Kind, Metric, NodeId, Tree};

/// The `n` largest files below `root` by `metric`, largest first.
pub fn top_files(tree: &Tree, root: NodeId, metric: Metric, n: usize) -> Vec<NodeId> {
    if n == 0 {
        return Vec::new();
    }
    let mut heap = BinaryHeap::with_capacity(n + 1);
    for id in tree
        .walk(root)
        .filter(|&id| tree.node(id).kind == Kind::File)
    {
        heap.push(Reverse((tree.node(id).metric(metric), id)));
        if heap.len() > n {
            heap.pop();
        }
    }
    let mut out: Vec<_> = heap.into_iter().map(|Reverse(x)| x).collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out.into_iter().map(|(_, id)| id).collect()
}

/// Totals for one file extension.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionStat {
    /// Lower-case extension without the dot; empty for none.
    pub extension: String,
    pub size: u64,
    pub allocated: u64,
    pub files: u64,
}

impl ExtensionStat {
    pub fn metric(&self, metric: Metric) -> u64 {
        match metric {
            Metric::Size => self.size,
            Metric::Allocated => self.allocated,
            Metric::FileCount => self.files,
        }
    }
}

/// Per-extension totals below `root`, largest first by `metric`.
pub fn extensions(tree: &Tree, root: NodeId, metric: Metric) -> Vec<ExtensionStat> {
    let mut map: HashMap<String, ExtensionStat> = HashMap::new();
    for id in tree
        .walk(root)
        .filter(|&id| tree.node(id).kind == Kind::File)
    {
        let node = tree.node(id);
        let ext = extension_of(tree.name_bytes(id));
        let stat = map.entry(ext).or_default();
        stat.size += node.size;
        stat.allocated += node.allocated;
        stat.files += 1;
    }
    let mut stats: Vec<_> = map
        .into_iter()
        .map(|(extension, s)| ExtensionStat { extension, ..s })
        .collect();
    stats.sort_unstable_by(|a, b| {
        b.metric(metric)
            .cmp(&a.metric(metric))
            .then_with(|| a.extension.cmp(&b.extension))
    });
    stats
}

/// Lower-case extension of a file name; dot-files without another dot have none.
pub fn extension_of(name: &[u8]) -> String {
    match name.iter().rposition(|&b| b == b'.') {
        Some(0) | None => String::new(),
        Some(i) => String::from_utf8_lossy(&name[i + 1..]).to_lowercase(),
    }
}

/// Age buckets by last modification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgeBucket {
    Week,
    Month,
    HalfYear,
    Year,
    TwoYears,
    FiveYears,
    Older,
}

impl AgeBucket {
    pub const ALL: [AgeBucket; 7] = [
        AgeBucket::Week,
        AgeBucket::Month,
        AgeBucket::HalfYear,
        AgeBucket::Year,
        AgeBucket::TwoYears,
        AgeBucket::FiveYears,
        AgeBucket::Older,
    ];

    fn of(age_secs: i64) -> AgeBucket {
        const DAY: i64 = 86_400;
        match age_secs {
            a if a < 7 * DAY => AgeBucket::Week,
            a if a < 30 * DAY => AgeBucket::Month,
            a if a < 182 * DAY => AgeBucket::HalfYear,
            a if a < 365 * DAY => AgeBucket::Year,
            a if a < 2 * 365 * DAY => AgeBucket::TwoYears,
            a if a < 5 * 365 * DAY => AgeBucket::FiveYears,
            _ => AgeBucket::Older,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgeStat {
    pub size: u64,
    pub allocated: u64,
    pub files: u64,
}

/// File totals per age bucket, in [`AgeBucket::ALL`] order. `now` is Unix seconds.
pub fn ages(tree: &Tree, root: NodeId, now: i64) -> [AgeStat; 7] {
    let mut out = [AgeStat::default(); 7];
    for id in tree
        .walk(root)
        .filter(|&id| tree.node(id).kind == Kind::File)
    {
        let node = tree.node(id);
        let bucket = AgeBucket::of(now.saturating_sub(node.modified).max(0));
        let slot = &mut out[AgeBucket::ALL
            .iter()
            .position(|&b| b == bucket)
            .unwrap_or(6)];
        slot.size += node.size;
        slot.allocated += node.allocated;
        slot.files += 1;
    }
    out
}

/// Entries below `root` whose name contains `query` (case-insensitive),
/// largest first, at most `limit`.
pub fn search(tree: &Tree, root: NodeId, query: &str, limit: usize) -> Vec<NodeId> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<NodeId> = tree
        .walk(root)
        .filter(|&id| id != root && tree.name(id).to_lowercase().contains(&needle))
        .collect();
    hits.sort_unstable_by_key(|&id| Reverse(tree.node(id).size));
    hits.truncate(limit);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryInfo, Flags, TreeBuilder};
    use std::path::PathBuf;

    const RECENT: i64 = 400_000_000;

    fn info(kind: Kind, size: u64, modified: i64) -> EntryInfo {
        EntryInfo {
            kind,
            flags: Flags::empty(),
            modified,
            size,
            allocated: size,
        }
    }

    fn sample() -> Tree {
        let mut b = TreeBuilder::new(PathBuf::from("/x"), info(Kind::Dir, 0, 0));
        let d = b.push(NodeId::ROOT, b"docs", info(Kind::Dir, 0, 0));
        b.push(d, b"a.PDF", info(Kind::File, 30, 1_000));
        b.push(d, b"b.pdf", info(Kind::File, 20, 1_000));
        b.push(NodeId::ROOT, b".bashrc", info(Kind::File, 5, RECENT));
        b.push(NodeId::ROOT, b"movie.mkv", info(Kind::File, 100, RECENT));
        b.finish()
    }

    #[test]
    fn top_files_largest_first() {
        let t = sample();
        let names: Vec<_> = top_files(&t, t.root(), Metric::Size, 2)
            .iter()
            .map(|&i| t.name(i).into_owned())
            .collect();
        assert_eq!(names, ["movie.mkv", "a.PDF"]);
        assert!(top_files(&t, t.root(), Metric::Size, 0).is_empty());
    }

    #[test]
    fn extensions_grouped_case_insensitively() {
        let t = sample();
        let stats = extensions(&t, t.root(), Metric::Size);
        assert_eq!(stats[0].extension, "mkv");
        assert_eq!(
            stats[1],
            ExtensionStat {
                extension: "pdf".into(),
                size: 50,
                allocated: 50,
                files: 2
            }
        );
        assert!(stats.iter().any(|s| s.extension.is_empty() && s.files == 1));
    }

    #[test]
    fn extension_edge_cases() {
        assert_eq!(extension_of(b".bashrc"), "");
        assert_eq!(extension_of(b"archive.tar.GZ"), "gz");
        assert_eq!(extension_of(b"noext"), "");
        assert_eq!(extension_of(b"trailing."), "");
    }

    #[test]
    fn ages_bucket_by_modification() {
        let t = sample();
        let now = RECENT + 3 * 86_400;
        let a = ages(&t, t.root(), now);
        assert_eq!(a[0].files, 2); // modified 3 days ago
        assert_eq!(a[6].files, 2); // 1970
    }

    #[test]
    fn search_is_case_insensitive() {
        let t = sample();
        assert_eq!(search(&t, t.root(), "pdf", 10).len(), 2);
        assert!(search(&t, t.root(), "  ", 10).is_empty());
    }
}
