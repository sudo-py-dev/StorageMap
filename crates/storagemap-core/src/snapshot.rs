//! Saved scans ("snapshots") and comparison of a scan against one.
//!
//! A snapshot keeps every folder plus files of at least `min_file_size`
//! bytes: folder totals stay exact, files below the threshold are only
//! counted. Files are untrusted input on load, so every length and index is
//! validated.

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::tree::{Flags, Kind, NodeId, Tree};

const MAGIC: &[u8; 8] = b"SMAPSNP1";
const VERSION: u32 = 1;
const EXTENSION: &str = "smap";
const NO_PARENT: u32 = u32::MAX;
/// Upper bounds that reject corrupt or hostile files before allocating.
const MAX_ENTRIES: u32 = 100_000_000;
const MAX_PATH_BYTES: u32 = 64 * 1024;
/// Files below this are folded into their folder's totals by default.
pub const DEFAULT_MIN_FILE_SIZE: u64 = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("not a StorageMap snapshot")]
    BadMagic,
    #[error("snapshot version {0} is not supported")]
    UnsupportedVersion(u32),
    #[error("snapshot is corrupt: {0}")]
    Corrupt(&'static str),
    #[error("snapshot is of {snapshot}, not {scan}")]
    DifferentRoot { snapshot: PathBuf, scan: PathBuf },
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Totals of one entry when the snapshot was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub size: u64,
    pub allocated: u64,
    pub files: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    parent: u32,
    is_dir: bool,
    stats: Stats,
    name: Box<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub root: PathBuf,
    /// Unix seconds.
    pub taken_at: i64,
    pub min_file_size: u64,
    entries: Vec<Entry>,
}

/// Header of a snapshot file, for listing without loading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub file: PathBuf,
    pub root: PathBuf,
    pub taken_at: i64,
    pub entries: u32,
    /// Totals of the scanned folder.
    pub total: Stats,
}

impl Snapshot {
    /// Captures `tree` (folders plus files ≥ `min_file_size`).
    pub fn capture(tree: &Tree, taken_at: i64, min_file_size: u64) -> Snapshot {
        let mut entries = Vec::new();
        let mut index_of: HashMap<NodeId, u32> = HashMap::new();
        let mut stack = vec![tree.root()];
        while let Some(id) = stack.pop() {
            let node = tree.node(id);
            let keep = id == tree.root()
                || node.is_dir()
                || (node.kind == Kind::File && node.size >= min_file_size);
            if !keep {
                continue;
            }
            let parent = tree
                .parent(id)
                .and_then(|p| index_of.get(&p).copied())
                .unwrap_or(NO_PARENT);
            index_of.insert(id, entries.len() as u32);
            entries.push(Entry {
                parent,
                is_dir: node.is_dir(),
                stats: Stats {
                    size: node.size,
                    allocated: node.allocated,
                    files: node.files,
                },
                name: if id == tree.root() {
                    Box::default()
                } else {
                    tree.name_bytes(id).into()
                },
            });
            if node.is_dir() {
                stack.extend(tree.children(id).rev());
            }
        }
        Snapshot {
            root: tree.root_path().to_path_buf(),
            taken_at,
            min_file_size,
            entries,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total(&self) -> Stats {
        self.entries.first().map(|e| e.stats).unwrap_or_default()
    }

    pub fn write_to<W: Write>(&self, out: W) -> io::Result<()> {
        let mut w = BufWriter::new(out);
        w.write_all(MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&self.taken_at.to_le_bytes())?;
        let root = path_bytes(&self.root);
        w.write_all(&(root.len() as u32).to_le_bytes())?;
        w.write_all(&root)?;
        w.write_all(&self.min_file_size.to_le_bytes())?;
        w.write_all(&(self.entries.len() as u32).to_le_bytes())?;
        for e in &self.entries {
            w.write_all(&e.parent.to_le_bytes())?;
            w.write_all(&[u8::from(e.is_dir)])?;
            for v in [e.stats.size, e.stats.allocated, e.stats.files] {
                w.write_all(&v.to_le_bytes())?;
            }
            let name_len =
                u16::try_from(e.name.len()).map_err(|_| io::Error::other("file name too long"))?;
            w.write_all(&name_len.to_le_bytes())?;
            w.write_all(&e.name)?;
        }
        w.flush()
    }

    pub fn read_from<R: Read>(input: R) -> Result<Snapshot, SnapshotError> {
        let mut r = BufReader::new(input);
        let (header, count) = read_header(&mut r)?;
        // Cap the up-front allocation; a lying count must not exhaust memory.
        let mut entries = Vec::with_capacity(count.min(1 << 20) as usize);
        for i in 0..count {
            let parent = u32::from_le_bytes(array(&mut r)?);
            if (i == 0) != (parent == NO_PARENT) || (parent != NO_PARENT && parent >= i) {
                return Err(SnapshotError::Corrupt("parent index out of order"));
            }
            let is_dir = match array::<1>(&mut r)?[0] {
                0 => false,
                1 => true,
                _ => return Err(SnapshotError::Corrupt("bad entry kind")),
            };
            if parent != NO_PARENT
                && !entries
                    .get(parent as usize)
                    .is_some_and(|p: &Entry| p.is_dir)
            {
                return Err(SnapshotError::Corrupt("parent is not a folder"));
            }
            let stats = Stats {
                size: u64::from_le_bytes(array(&mut r)?),
                allocated: u64::from_le_bytes(array(&mut r)?),
                files: u64::from_le_bytes(array(&mut r)?),
            };
            let name_len = u16::from_le_bytes(array(&mut r)?) as usize;
            let mut name = vec![0u8; name_len];
            r.read_exact(&mut name)?;
            entries.push(Entry {
                parent,
                is_dir,
                stats,
                name: name.into(),
            });
        }
        Ok(Snapshot {
            root: header.root,
            taken_at: header.taken_at,
            min_file_size: header.min_file_size,
            entries,
        })
    }

    /// Writes into `dir` atomically and returns the file path.
    pub fn save(&self, dir: &Path) -> io::Result<PathBuf> {
        fs::create_dir_all(dir)?;
        let name = format!(
            "{}-{:016x}.{EXTENSION}",
            self.taken_at,
            fnv1a(&path_bytes(&self.root))
        );
        let target = dir.join(name);
        let tmp = target.with_extension("tmp");
        self.write_to(fs::File::create(&tmp)?)?;
        fs::rename(&tmp, &target)?;
        Ok(target)
    }

    pub fn load(file: &Path) -> Result<Snapshot, SnapshotError> {
        Snapshot::read_from(fs::File::open(file)?)
    }
}

struct Header {
    root: PathBuf,
    taken_at: i64,
    min_file_size: u64,
}

fn read_header<R: Read>(r: &mut R) -> Result<(Header, u32), SnapshotError> {
    if &array::<8>(r)? != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    let version = u32::from_le_bytes(array(r)?);
    if version != VERSION {
        return Err(SnapshotError::UnsupportedVersion(version));
    }
    let taken_at = i64::from_le_bytes(array(r)?);
    let root_len = u32::from_le_bytes(array(r)?);
    if root_len > MAX_PATH_BYTES {
        return Err(SnapshotError::Corrupt("root path too long"));
    }
    let mut root = vec![0u8; root_len as usize];
    r.read_exact(&mut root)?;
    let min_file_size = u64::from_le_bytes(array(r)?);
    let count = u32::from_le_bytes(array(r)?);
    if count == 0 || count > MAX_ENTRIES {
        return Err(SnapshotError::Corrupt("entry count out of range"));
    }
    Ok((
        Header {
            root: bytes_to_path(root),
            taken_at,
            min_file_size,
        },
        count,
    ))
}

/// Reads the first (root) entry's totals right after the header.
fn read_root_stats<R: Read>(r: &mut R) -> io::Result<Stats> {
    let _parent_and_kind: [u8; 5] = array(r)?;
    Ok(Stats {
        size: u64::from_le_bytes(array(r)?),
        allocated: u64::from_le_bytes(array(r)?),
        files: u64::from_le_bytes(array(r)?),
    })
}

/// Snapshots in `dir`, newest first; only those of `root` when given.
/// Unreadable files are skipped.
pub fn list(dir: &Path, root: Option<&Path>) -> Vec<SnapshotInfo> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<SnapshotInfo> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == EXTENSION))
        .filter_map(|file| {
            let mut r = BufReader::new(fs::File::open(&file).ok()?);
            let (h, entries) = read_header(&mut r).ok()?;
            let total = read_root_stats(&mut r).ok()?;
            Some(SnapshotInfo {
                file,
                root: h.root,
                taken_at: h.taken_at,
                entries,
                total,
            })
        })
        .filter(|info| root.is_none_or(|r| info.root == r))
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(i.taken_at));
    out
}

/// How an entry of the current scan relates to the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Present in both; holds the old totals.
    Known(Stats),
    /// Not in the snapshot, though it would have been recorded.
    New,
    /// A file below the snapshot's size threshold: no record either way.
    Untracked,
}

/// An entry that existed in the snapshot but is gone now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub path: PathBuf,
    pub is_dir: bool,
    pub stats: Stats,
}

/// A scan matched against a snapshot of the same folder.
#[derive(Debug, Clone)]
pub struct Comparison {
    pub taken_at: i64,
    pub min_file_size: u64,
    old: Vec<Option<Stats>>,
    /// Topmost removed entries, largest first.
    pub removed: Vec<Removed>,
}

impl Comparison {
    pub fn new(tree: &Tree, snap: &Snapshot) -> Result<Comparison, SnapshotError> {
        if snap.root != tree.root_path() {
            return Err(SnapshotError::DifferentRoot {
                snapshot: snap.root.clone(),
                scan: tree.root_path().to_path_buf(),
            });
        }
        let mut old = vec![None; tree.len()];
        let mut mapped: Vec<Option<NodeId>> = Vec::with_capacity(snap.entries.len());
        let mut children: HashMap<NodeId, HashMap<&[u8], NodeId>> = HashMap::new();
        let mut removed = Vec::new();
        for (i, e) in snap.entries.iter().enumerate() {
            let node = if i == 0 {
                Some(tree.root())
            } else {
                match mapped[e.parent as usize] {
                    Some(parent) => children
                        .entry(parent)
                        .or_insert_with(|| {
                            tree.children(parent)
                                .map(|c| (tree.name_bytes(c), c))
                                .collect()
                        })
                        .get(&*e.name)
                        .copied(),
                    None => None, // an ancestor is already reported as removed
                }
            };
            match node {
                Some(id) => old[id.index()] = Some(e.stats),
                None if mapped.get(e.parent as usize).is_some_and(Option::is_some) => {
                    removed.push(Removed {
                        path: snap_path(snap, i),
                        is_dir: e.is_dir,
                        stats: e.stats,
                    });
                }
                None => {}
            }
            mapped.push(node);
        }
        removed.sort_by_key(|r| std::cmp::Reverse(r.stats.size));
        Ok(Comparison {
            taken_at: snap.taken_at,
            min_file_size: snap.min_file_size,
            old,
            removed,
        })
    }

    pub fn change(&self, tree: &Tree, id: NodeId) -> Change {
        match self.old.get(id.index()).copied().flatten() {
            Some(stats) => Change::Known(stats),
            None => {
                let node = tree.node(id);
                let tracked = node.is_dir() || node.size >= self.min_file_size;
                if tracked && !node.flags.contains(Flags::DELETED) {
                    Change::New
                } else {
                    Change::Untracked
                }
            }
        }
    }

    /// Signed change of logical size, when known.
    pub fn size_delta(&self, tree: &Tree, id: NodeId) -> Option<i128> {
        let now = i128::from(tree.node(id).size);
        match self.change(tree, id) {
            Change::Known(s) => Some(now - i128::from(s.size)),
            Change::New => Some(now),
            Change::Untracked => None,
        }
    }
}

fn snap_path(snap: &Snapshot, mut i: usize) -> PathBuf {
    let mut parts = Vec::new();
    while i != 0 {
        let e = &snap.entries[i];
        parts.push(bytes_to_path(e.name.to_vec()));
        i = e.parent as usize;
    }
    let mut path = snap.root.clone();
    path.extend(parts.into_iter().rev());
    path
}

/// Stable 64-bit FNV-1a, used only to make file names distinct per folder.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn array<const N: usize>(r: &mut impl Read) -> io::Result<[u8; N]> {
    let mut buf = [0u8; N];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(unix)]
fn path_bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(p: &Path) -> Vec<u8> {
    p.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn bytes_to_path(b: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(b))
}

#[cfg(not(unix))]
fn bytes_to_path(b: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&b).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryInfo, TreeBuilder};

    const MB: u64 = 1024 * 1024;

    fn info(kind: Kind, size: u64) -> EntryInfo {
        EntryInfo {
            kind,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
            size,
            allocated: size,
        }
    }

    /// root/{docs/{big.iso 5MB, small.txt 10B}, old/{a.bin 2MB}, video.mkv 3MB}
    fn before() -> Tree {
        let mut b = TreeBuilder::new(PathBuf::from("/data"), info(Kind::Dir, 0));
        let docs = b.push(NodeId::ROOT, b"docs", info(Kind::Dir, 0));
        b.push(docs, b"big.iso", info(Kind::File, 5 * MB));
        b.push(docs, b"small.txt", info(Kind::File, 10));
        let old = b.push(NodeId::ROOT, b"old", info(Kind::Dir, 0));
        b.push(old, b"a.bin", info(Kind::File, 2 * MB));
        b.push(NodeId::ROOT, b"video.mkv", info(Kind::File, 3 * MB));
        b.finish()
    }

    /// docs/big.iso grew to 7MB, old/ deleted, new/ added.
    fn after() -> Tree {
        let mut b = TreeBuilder::new(PathBuf::from("/data"), info(Kind::Dir, 0));
        let docs = b.push(NodeId::ROOT, b"docs", info(Kind::Dir, 0));
        b.push(docs, b"big.iso", info(Kind::File, 7 * MB));
        b.push(docs, b"small.txt", info(Kind::File, 20));
        let new = b.push(NodeId::ROOT, b"new", info(Kind::Dir, 0));
        b.push(new, b"n.bin", info(Kind::File, MB));
        b.push(NodeId::ROOT, b"video.mkv", info(Kind::File, 3 * MB));
        b.finish()
    }

    fn find(t: &Tree, name: &str) -> NodeId {
        t.walk(t.root())
            .find(|&i| t.name(i) == name)
            .expect("node exists")
    }

    #[test]
    fn capture_keeps_folders_and_large_files() {
        let s = Snapshot::capture(&before(), 100, MB);
        assert_eq!(s.len(), 6); // root, docs, big.iso, old, a.bin, video.mkv
        assert_eq!(s.total().size, 10 * MB + 10);
    }

    #[test]
    fn round_trips_through_bytes() {
        let s = Snapshot::capture(&before(), 1_700_000_000, MB);
        let mut buf = Vec::new();
        s.write_to(&mut buf).expect("write");
        assert_eq!(Snapshot::read_from(buf.as_slice()).expect("read"), s);
    }

    #[test]
    fn rejects_corrupt_input() {
        assert!(matches!(
            Snapshot::read_from(&b"NOTASNAP........"[..]),
            Err(SnapshotError::BadMagic)
        ));
        let mut buf = Vec::new();
        Snapshot::capture(&before(), 1, MB)
            .write_to(&mut buf)
            .expect("write");
        buf.truncate(buf.len() - 3);
        assert!(Snapshot::read_from(buf.as_slice()).is_err());
        // A child pointing at a later index must be rejected.
        let mut bad = Vec::new();
        Snapshot::capture(&before(), 1, MB)
            .write_to(&mut bad)
            .expect("write");
        let first_child = 8 + 4 + 8 + 4 + "/data".len() + 8 + 4 + (4 + 1 + 24 + 2);
        bad[first_child..first_child + 4].copy_from_slice(&5u32.to_le_bytes());
        assert!(matches!(
            Snapshot::read_from(bad.as_slice()),
            Err(SnapshotError::Corrupt(_))
        ));
    }

    #[test]
    fn compares_growth_new_and_removed() {
        let (old, now) = (before(), after());
        let cmp = Comparison::new(&now, &Snapshot::capture(&old, 0, MB)).expect("same root");
        assert_eq!(cmp.size_delta(&now, now.root()), Some(i128::from(MB) + 10));
        assert_eq!(
            cmp.size_delta(&now, find(&now, "big.iso")),
            Some(i128::from(2 * MB))
        );
        assert_eq!(cmp.change(&now, find(&now, "new")), Change::New);
        assert_eq!(cmp.change(&now, find(&now, "small.txt")), Change::Untracked);
        let removed: Vec<_> = cmp.removed.iter().map(|r| r.path.clone()).collect();
        assert_eq!(removed, [PathBuf::from("/data/old")]); // a.bin is implied by its folder
    }

    #[test]
    fn refuses_a_different_folder() {
        let s = Snapshot::capture(&before(), 0, MB);
        let mut b = TreeBuilder::new(PathBuf::from("/other"), info(Kind::Dir, 0));
        b.push(NodeId::ROOT, b"x", info(Kind::File, 1));
        assert!(matches!(
            Comparison::new(&b.finish(), &s),
            Err(SnapshotError::DifferentRoot { .. })
        ));
    }

    #[test]
    fn save_and_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tree = before();
        Snapshot::capture(&tree, 10, MB)
            .save(dir.path())
            .expect("save");
        let newest = Snapshot::capture(&tree, 20, MB)
            .save(dir.path())
            .expect("save");
        std::fs::write(dir.path().join("junk.smap"), b"garbage").expect("write junk");
        let listed = list(dir.path(), Some(Path::new("/data")));
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].file, newest);
        assert_eq!(listed[0].total.size, 10 * MB + 10);
        assert!(list(dir.path(), Some(Path::new("/elsewhere"))).is_empty());
        assert_eq!(Snapshot::load(&newest).expect("load").taken_at, 20);
    }
}
