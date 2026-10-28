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

/// Totals for one owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OwnerStat {
    /// Owner id, or [`crate::NO_OWNER`].
    pub owner: u32,
    pub size: u64,
    pub allocated: u64,
    pub files: u64,
}

impl OwnerStat {
    pub fn metric(&self, metric: Metric) -> u64 {
        match metric {
            Metric::Size => self.size,
            Metric::Allocated => self.allocated,
            Metric::FileCount => self.files,
        }
    }
}

/// File totals per owner below `root`, largest first. Extra hard links are
/// not counted again, so the totals add up to the folder's.
pub fn owners(tree: &Tree, root: NodeId, metric: Metric) -> Vec<OwnerStat> {
    let mut map: HashMap<u32, OwnerStat> = HashMap::new();
    for id in tree.walk(root) {
        let node = tree.node(id);
        if node.kind == Kind::Dir || node.flags.contains(crate::Flags::HARDLINK_DUP) {
            continue;
        }
        let stat = map.entry(node.owner).or_default();
        stat.size += node.size;
        stat.allocated += node.allocated;
        stat.files += 1;
    }
    let mut stats: Vec<_> = map
        .into_iter()
        .map(|(owner, s)| OwnerStat { owner, ..s })
        .collect();
    stats.sort_unstable_by(|a, b| {
        b.metric(metric)
            .cmp(&a.metric(metric))
            .then_with(|| a.owner.cmp(&b.owner))
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

    /// The bucket for something last modified `age_secs` ago.
    pub fn of(age_secs: i64) -> AgeBucket {
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
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
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
    fn owners_add_up_and_skip_extra_hard_links() {
        let mut b = TreeBuilder::new(PathBuf::from("/x"), info(Kind::Dir, 0, 0));
        let owned = |owner, size| EntryInfo {
            owner,
            ..info(Kind::File, size, 0)
        };
        b.push(NodeId::ROOT, b"a", owned(1000, 30));
        b.push(NodeId::ROOT, b"b", owned(0, 50));
        b.push(NodeId::ROOT, b"c", owned(1000, 40));
        let mut link = owned(1000, 40);
        link.flags.insert(Flags::HARDLINK_DUP);
        b.push(NodeId::ROOT, b"c-link", link);
        let t = b.finish();
        let stats = owners(&t, t.root(), Metric::Size);
        assert_eq!(
            stats
                .iter()
                .map(|s| (s.owner, s.size, s.files))
                .collect::<Vec<_>>(),
            [(1000, 70, 2), (0, 50, 1)]
        );
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
}
