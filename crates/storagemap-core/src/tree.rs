//! Compact arena tree holding a finished scan.
//!
//! Nodes live in one `Vec`, names in one byte pool and children in one
//! CSR-style index array, so a multi-million entry scan stays cache friendly
//! and costs ~64 bytes per entry plus its name.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Owner id when the engine cannot tell who owns an entry.
pub const NO_OWNER: u32 = u32::MAX;

/// Index of a node inside a [`Tree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const ROOT: NodeId = NodeId(0);
    const NONE: u32 = u32::MAX;

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    File,
    Dir,
    Symlink,
    Other,
}

/// Per-node state bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Flags(u16);

impl Flags {
    /// The entry could not be read (permission denied, vanished, ...).
    pub const ERROR: Flags = Flags(1);
    /// Another hard link to the same data was already counted.
    pub const HARDLINK_DUP: Flags = Flags(1 << 1);
    /// A mount point / other volume that was not descended into.
    pub const MOUNT_POINT: Flags = Flags(1 << 2);
    /// Removed by the user after the scan.
    pub const DELETED: Flags = Flags(1 << 3);
    /// The scan was cancelled before this directory was read.
    pub const INCOMPLETE: Flags = Flags(1 << 4);
    /// The entry is hidden (dot-file or hidden attribute).
    pub const HIDDEN: Flags = Flags(1 << 5);
    /// Read-only (Windows attribute, no write permission, or immutable).
    pub const READ_ONLY: Flags = Flags(1 << 6);
    /// Windows system file.
    pub const SYSTEM: Flags = Flags(1 << 7);
    /// Stored compressed by the filesystem.
    pub const COMPRESSED: Flags = Flags(1 << 8);
    /// Stored encrypted by the filesystem.
    pub const ENCRYPTED: Flags = Flags(1 << 9);
    /// Sparse: unwritten ranges take no space.
    pub const SPARSE: Flags = Flags(1 << 10);

    pub const fn empty() -> Self {
        Flags(0)
    }
    pub fn contains(self, other: Flags) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: Flags) {
        self.0 |= other.0;
    }
    pub fn union(self, other: Flags) -> Flags {
        Flags(self.0 | other.0)
    }
}

/// Which quantity drives sorting, percentages and the treemap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Metric {
    /// Logical file size.
    #[default]
    Size,
    /// Space actually allocated on disk.
    Allocated,
    /// Number of files.
    FileCount,
}

#[derive(Debug, Clone)]
pub struct Node {
    name_start: u32,
    name_len: u32,
    parent: u32,
    pub kind: Kind,
    pub flags: Flags,
    /// Last modification, seconds since the Unix epoch. For directories this
    /// is the newest modification anywhere below.
    pub modified: i64,
    /// Last access, Unix seconds (0 if unknown). For directories, the newest
    /// access anywhere below. Many systems update it lazily or not at all.
    pub accessed: i64,
    /// Creation, Unix seconds (0 if unknown); the entry's own, not aggregated.
    pub created: i64,
    /// Owner id (the user id on Unix), or [`NO_OWNER`].
    pub owner: u32,
    /// Logical size; aggregated for directories.
    pub size: u64,
    /// Allocated bytes; aggregated for directories (includes the directory's
    /// own metadata blocks).
    pub allocated: u64,
    /// Files below (1 for a file).
    pub files: u64,
    /// Directories below, not counting itself.
    pub dirs: u64,
    child_start: u32,
    child_len: u32,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }

    pub fn metric(&self, metric: Metric) -> u64 {
        match metric {
            Metric::Size => self.size,
            Metric::Allocated => self.allocated,
            Metric::FileCount => self.files,
        }
    }
}

/// Finished, immutable-shape scan result. Only deletion mutates it.
#[derive(Debug, Clone)]
pub struct Tree {
    root_path: PathBuf,
    nodes: Vec<Node>,
    names: Vec<u8>,
    children: Vec<NodeId>,
}

impl Tree {
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        let p = self.nodes[id.index()].parent;
        (p != NodeId::NONE).then_some(NodeId(p))
    }

    /// Raw name bytes (exact on Unix, UTF-8 on Windows).
    pub fn name_bytes(&self, id: NodeId) -> &[u8] {
        let n = &self.nodes[id.index()];
        &self.names[n.name_start as usize..(n.name_start + n.name_len) as usize]
    }

    /// Display name. The root shows its full path.
    pub fn name(&self, id: NodeId) -> Cow<'_, str> {
        if id == NodeId::ROOT {
            return self.root_path.to_string_lossy();
        }
        String::from_utf8_lossy(self.name_bytes(id))
    }

    /// Live children, sorted by size (largest first).
    pub fn children(&self, id: NodeId) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
        let n = &self.nodes[id.index()];
        self.children[n.child_start as usize..(n.child_start + n.child_len) as usize]
            .iter()
            .copied()
            .filter(|c| !self.nodes[c.index()].flags.contains(Flags::DELETED))
    }

    pub fn has_children(&self, id: NodeId) -> bool {
        self.children(id).next().is_some()
    }

    /// Ancestors from the root down to `id`, inclusive.
    pub fn ancestry(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = vec![id];
        let mut cur = id;
        while let Some(p) = self.parent(cur) {
            chain.push(p);
            cur = p;
        }
        chain.reverse();
        chain
    }

    pub fn depth(&self, id: NodeId) -> usize {
        self.ancestry(id).len() - 1
    }

    /// Full filesystem path of a node.
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut path = self.root_path.clone();
        for part in self.ancestry(id).into_iter().skip(1) {
            path.push(bytes_to_os(self.name_bytes(part)));
        }
        path
    }

    /// Finds the node for `path` if it lies inside this tree.
    pub fn find(&self, path: &Path) -> Option<NodeId> {
        let rel = path.strip_prefix(&self.root_path).ok()?;
        let mut cur = NodeId::ROOT;
        for comp in rel.components() {
            let want = comp.as_os_str();
            cur = self
                .children(cur)
                .find(|&c| bytes_to_os(self.name_bytes(c)) == want)?;
        }
        Some(cur)
    }

    /// Share of `id` within its parent for `metric`, in `0.0..=1.0`.
    pub fn share_of_parent(&self, id: NodeId, metric: Metric) -> f64 {
        let Some(parent) = self.parent(id) else {
            return 1.0;
        };
        ratio(
            self.node(id).metric(metric),
            self.node(parent).metric(metric),
        )
    }

    /// Marks a subtree as deleted and subtracts it from every ancestor.
    pub fn remove(&mut self, id: NodeId) {
        if id == NodeId::ROOT || self.nodes[id.index()].flags.contains(Flags::DELETED) {
            return;
        }
        let gone = self.nodes[id.index()].clone();
        let gone_dirs = gone.dirs + u64::from(gone.is_dir());
        self.nodes[id.index()].flags.insert(Flags::DELETED);
        let mut cur = self.parent(id);
        while let Some(p) = cur {
            let n = &mut self.nodes[p.index()];
            n.size = n.size.saturating_sub(gone.size);
            n.allocated = n.allocated.saturating_sub(gone.allocated);
            n.files = n.files.saturating_sub(gone.files);
            n.dirs = n.dirs.saturating_sub(gone_dirs);
            cur = self.parent(p);
        }
    }

    /// Marks a file that now shares its data with another file (it was
    /// replaced by a hard link), so its size no longer counts in its folders.
    pub fn mark_linked(&mut self, id: NodeId) {
        let node = &self.nodes[id.index()];
        if node.kind == Kind::Dir || node.flags.contains(Flags::HARDLINK_DUP) {
            return;
        }
        let gone = node.clone();
        self.nodes[id.index()].flags.insert(Flags::HARDLINK_DUP);
        let mut cur = self.parent(id);
        while let Some(p) = cur {
            let n = &mut self.nodes[p.index()];
            n.size = n.size.saturating_sub(gone.size);
            n.allocated = n.allocated.saturating_sub(gone.allocated);
            n.files = n.files.saturating_sub(gone.files);
            cur = self.parent(p);
        }
    }

    /// Visits `root` and every live descendant (depth first).
    pub fn walk(&self, root: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let mut stack = vec![root];
        std::iter::from_fn(move || {
            let id = stack.pop()?;
            stack.extend(self.children(id).rev());
            Some(id)
        })
    }
}

pub fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

#[cfg(unix)]
fn bytes_to_os(bytes: &[u8]) -> &std::ffi::OsStr {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(bytes)
}

#[cfg(not(unix))]
fn bytes_to_os(bytes: &[u8]) -> &std::ffi::OsStr {
    // Non-Unix backends only ever store UTF-8 names.
    std::ffi::OsStr::new(std::str::from_utf8(bytes).unwrap_or_default())
}

/// Leaf data for one entry, as reported by a backend.
#[derive(Debug, Clone, Copy)]
pub struct EntryInfo {
    pub kind: Kind,
    pub flags: Flags,
    pub modified: i64,
    pub accessed: i64,
    pub created: i64,
    pub owner: u32,
    pub size: u64,
    pub allocated: u64,
}

/// Builds a [`Tree`]. Parents must be pushed before their children.
#[derive(Debug)]
pub struct TreeBuilder {
    root_path: PathBuf,
    nodes: Vec<Node>,
    names: Vec<u8>,
}

impl TreeBuilder {
    /// Starts a tree whose root directory is `root_path`.
    pub fn new(root_path: PathBuf, root: EntryInfo) -> Self {
        let mut b = TreeBuilder {
            root_path,
            nodes: Vec::new(),
            names: Vec::new(),
        };
        b.push_raw(NodeId::NONE, b"", root);
        b
    }

    pub fn with_capacity(mut self, nodes: usize, name_bytes: usize) -> Self {
        self.nodes.reserve(nodes);
        self.names.reserve(name_bytes);
        self
    }

    pub fn push(&mut self, parent: NodeId, name: &[u8], info: EntryInfo) -> NodeId {
        debug_assert!(
            parent.index() < self.nodes.len(),
            "parent must be pushed first"
        );
        self.push_raw(parent.0, name, info)
    }

    pub fn node_info_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.index()]
    }

    fn push_raw(&mut self, parent: u32, name: &[u8], info: EntryInfo) -> NodeId {
        let id = u32::try_from(self.nodes.len()).expect("more than 4 billion entries");
        let name_start = u32::try_from(self.names.len()).expect("name pool exceeds 4 GiB");
        self.names.extend_from_slice(name);
        self.nodes.push(Node {
            name_start,
            name_len: name.len() as u32,
            parent,
            kind: info.kind,
            flags: info.flags,
            modified: info.modified,
            accessed: info.accessed,
            created: info.created,
            owner: info.owner,
            size: info.size,
            allocated: info.allocated,
            files: u64::from(info.kind != Kind::Dir),
            dirs: 0,
            child_start: 0,
            child_len: 0,
        });
        NodeId(id)
    }

    /// Aggregates totals bottom-up and indexes children sorted by size.
    pub fn finish(self) -> Tree {
        let TreeBuilder {
            root_path,
            mut nodes,
            names,
        } = self;

        // Parents precede children, so one reverse pass aggregates everything.
        for i in (1..nodes.len()).rev() {
            let child = nodes[i].clone();
            let p = &mut nodes[child.parent as usize];
            p.modified = p.modified.max(child.modified);
            p.accessed = p.accessed.max(child.accessed);
            p.dirs += child.dirs + u64::from(child.is_dir());
            if !child.flags.contains(Flags::HARDLINK_DUP) {
                p.size += child.size;
                p.allocated += child.allocated;
                p.files += child.files;
            }
        }

        // CSR children index: count, prefix-sum, fill.
        let mut counts = vec![0u32; nodes.len()];
        for n in &nodes[1..] {
            counts[n.parent as usize] += 1;
        }
        let mut next = 0u32;
        for (n, &c) in nodes.iter_mut().zip(&counts) {
            n.child_start = next;
            n.child_len = 0;
            next += c;
        }
        let mut children = vec![NodeId(0); next as usize];
        for i in 1..nodes.len() {
            let p = nodes[i].parent as usize;
            let slot = (nodes[p].child_start + nodes[p].child_len) as usize;
            children[slot] = NodeId(i as u32);
            nodes[p].child_len += 1;
        }
        for n in &nodes {
            let range = n.child_start as usize..(n.child_start + n.child_len) as usize;
            children[range].sort_unstable_by(|a, b| {
                let (x, y) = (&nodes[a.index()], &nodes[b.index()]);
                y.size.cmp(&x.size).then(y.allocated.cmp(&x.allocated))
            });
        }

        Tree {
            root_path,
            nodes,
            names,
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(size: u64) -> EntryInfo {
        EntryInfo {
            kind: Kind::File,
            flags: Flags::empty(),
            modified: 10,
            accessed: 0,
            created: 0,
            owner: NO_OWNER,
            size,
            allocated: size,
        }
    }

    fn dir() -> EntryInfo {
        EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            owner: NO_OWNER,
            size: 0,
            allocated: 0,
        }
    }

    fn sample() -> Tree {
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir());
        let a = b.push(NodeId::ROOT, b"a", dir());
        b.push(a, b"small", file(1));
        b.push(a, b"big", file(100));
        b.push(NodeId::ROOT, b"top", file(50));
        let mut dup = file(100);
        dup.flags = Flags::HARDLINK_DUP;
        b.push(NodeId::ROOT, b"link", dup);
        b.finish()
    }

    #[test]
    fn a_linked_file_stops_counting() {
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir());
        let d = b.push(NodeId::ROOT, b"d", dir());
        let f = b.push(d, b"f", file(10));
        b.push(d, b"g", file(5));
        let mut t = b.finish();
        t.mark_linked(f);
        assert_eq!(t.node(t.root()).size, 5);
        assert_eq!(t.node(d).files, 1);
        t.mark_linked(f);
        assert_eq!(t.node(t.root()).size, 5);
    }

    #[test]
    fn aggregates_and_skips_hardlink_duplicates() {
        let t = sample();
        let root = t.node(t.root());
        assert_eq!(root.size, 151);
        assert_eq!(root.files, 3);
        assert_eq!(root.dirs, 1);
    }

    #[test]
    fn children_sorted_largest_first() {
        let t = sample();
        let names: Vec<_> = t
            .children(t.root())
            .map(|c| t.name(c).into_owned())
            .collect();
        assert_eq!(names, ["a", "link", "top"]);
    }

    #[test]
    fn path_and_find_round_trip() {
        let t = sample();
        let big = t
            .walk(t.root())
            .find(|&n| t.name(n) == "big")
            .expect("big exists");
        let p = t.path(big);
        assert_eq!(p, PathBuf::from("/r/a/big"));
        assert_eq!(t.find(&p), Some(big));
        assert_eq!(t.find(Path::new("/elsewhere")), None);
    }

    #[test]
    fn remove_updates_ancestors() {
        let mut t = sample();
        let a = t.children(t.root()).next().expect("a");
        t.remove(a);
        let root = t.node(t.root());
        assert_eq!(root.size, 50);
        assert_eq!(root.files, 1);
        assert_eq!(root.dirs, 0);
        assert!(t.children(t.root()).all(|c| c != a));
    }

    #[test]
    fn share_of_parent_handles_zero() {
        let mut b = TreeBuilder::new(PathBuf::from("/e"), dir());
        let f = b.push(NodeId::ROOT, b"empty", file(0));
        let t = b.finish();
        assert_eq!(t.share_of_parent(f, Metric::Size), 0.0);
    }
}
