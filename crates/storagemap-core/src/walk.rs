//! Backend-independent parallel directory walker.
//!
//! A [`Platform`] knows how to open and list one directory as fast as the OS
//! allows; this module fans directories out over a rayon pool, then flattens
//! the result into a [`Tree`] without deep recursion.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::filter::Matcher;
use crate::progress::ScanProgress;
use crate::tree::{EntryInfo, Flags, Kind, NodeId, Tree, TreeBuilder};

/// One entry of a directory listing.
#[derive(Debug)]
pub(crate) struct RawEntry {
    pub name: Vec<u8>,
    pub info: EntryInfo,
    /// Whether the walker should descend (a directory on the same volume).
    pub descend: bool,
}

/// OS-specific directory access.
pub(crate) trait Platform: Sync {
    /// An open directory, used to list it and to open its children.
    type Dir: Send + Sync;

    fn open_root(&self, path: &Path) -> io::Result<Self::Dir>;
    fn open_child(
        &self,
        parent: &Self::Dir,
        parent_path: &Path,
        name: &[u8],
    ) -> io::Result<Self::Dir>;
    /// Appends every entry except `.` and `..` to `out`.
    fn list(&self, dir: &Self::Dir, out: &mut Vec<RawEntry>) -> io::Result<()>;
}

/// How often a partial tree is published while the scan runs.
const LIVE_INTERVAL: Duration = Duration::from_millis(1000);

/// A scanned directory before flattening.
struct RawDir {
    name: Vec<u8>,
    info: EntryInfo,
    files: Vec<RawEntry>,
    subdirs: Vec<RawDir>,
}

impl RawDir {
    fn leaf(entry: RawEntry, extra: Flags) -> Self {
        let mut info = entry.info;
        info.flags.insert(extra);
        RawDir {
            name: entry.name,
            info,
            files: Vec::new(),
            subdirs: Vec::new(),
        }
    }
}

pub(crate) fn walk<P: Platform>(
    platform: &P,
    root: &Path,
    root_info: EntryInfo,
    filter: &Matcher,
    progress: &ScanProgress,
) -> io::Result<Tree> {
    let dir = platform.open_root(root)?;
    let root_entry = RawEntry {
        name: Vec::new(),
        info: root_info,
        descend: true,
    };
    let ctx = Ctx {
        platform,
        filter,
        progress,
    };
    let raw = walk_dir(
        &ctx,
        dir,
        root.to_path_buf(),
        String::new(),
        root_entry,
        true,
    );
    Ok(flatten(root.to_path_buf(), raw))
}

/// What every level of the walk shares.
struct Ctx<'a, P> {
    platform: &'a P,
    filter: &'a Matcher,
    progress: &'a ScanProgress,
}

/// `rel` is `path` relative to the scan root (`/`-separated), kept only when
/// the filter has path patterns.
fn walk_dir<P: Platform>(
    ctx: &Ctx<'_, P>,
    dir: P::Dir,
    path: PathBuf,
    rel: String,
    me: RawEntry,
    is_root: bool,
) -> RawDir {
    let (platform, filter, progress) = (ctx.platform, ctx.filter, ctx.progress);
    if progress.is_cancelled() {
        return RawDir::leaf(me, Flags::INCOMPLETE);
    }
    let mut entries = Vec::new();
    let mut flags = Flags::empty();
    if let Err(err) = platform.list(&dir, &mut entries) {
        tracing::debug!(path = %path.display(), %err, "cannot list directory");
        progress.add_error();
        flags.insert(Flags::ERROR);
    }
    entries.retain(|e| !filter.skips(&rel, &e.name, e.info.kind == Kind::Dir));

    let (to_descend, files): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| e.descend);
    let file_bytes = files.iter().map(|f| f.info.size).sum();
    progress.add_dir(&path, files.len() as u64, file_bytes);

    let mut info = me.info;
    info.flags.insert(flags);
    // At the root, finished folders are kept aside so partial trees can be
    // published for browsing while the others are still being walked.
    let live = (is_root && progress.wants_live())
        .then(|| Live::new(path.clone(), info, &files, &to_descend));
    if let Some(live) = &live {
        live.publish(progress);
    }

    let subdirs: Vec<Option<RawDir>> = to_descend
        .into_par_iter()
        .enumerate()
        .map(|(index, entry)| {
            let child_path = path.join(name_to_os(&entry.name));
            let child_rel = if filter.needs_path() {
                let name = String::from_utf8_lossy(&entry.name);
                if rel.is_empty() {
                    name.into_owned()
                } else {
                    format!("{rel}/{name}")
                }
            } else {
                String::new()
            };
            let walked = match platform.open_child(&dir, &path, &entry.name) {
                Ok(child) => walk_dir(ctx, child, child_path, child_rel, entry, false),
                Err(err) => {
                    tracing::debug!(path = %child_path.display(), %err, "cannot open directory");
                    progress.add_error();
                    RawDir::leaf(entry, Flags::ERROR)
                }
            };
            match &live {
                Some(live) => {
                    live.finished(index, walked, progress);
                    None
                }
                None => Some(walked),
            }
        })
        .collect();
    drop(dir);

    let subdirs = match live {
        Some(live) => live.into_dirs(),
        None => subdirs.into_iter().flatten().collect(),
    };
    RawDir {
        name: me.name,
        info,
        files,
        subdirs,
    }
}

/// A top-level folder: still being walked, or finished.
enum Slot {
    Pending(Vec<u8>, EntryInfo),
    Done(RawDir),
}

/// The scan root's contents while its folders are walked in parallel.
struct Live {
    root_path: PathBuf,
    info: EntryInfo,
    files: Vec<(Vec<u8>, EntryInfo)>,
    slots: Mutex<Vec<Slot>>,
    last_publish: Mutex<Instant>,
}

impl Live {
    fn new(root_path: PathBuf, info: EntryInfo, files: &[RawEntry], dirs: &[RawEntry]) -> Self {
        Live {
            root_path,
            info,
            files: files.iter().map(|f| (f.name.clone(), f.info)).collect(),
            slots: Mutex::new(
                dirs.iter()
                    .map(|d| Slot::Pending(d.name.clone(), d.info))
                    .collect(),
            ),
            last_publish: Mutex::new(Instant::now()),
        }
    }

    /// Stores a finished folder and publishes a partial tree when the last
    /// one is old enough. Only one thread builds at a time.
    fn finished(&self, index: usize, dir: RawDir, progress: &ScanProgress) {
        if let Ok(mut slots) = self.slots.lock() {
            slots[index] = Slot::Done(dir);
        }
        let Ok(mut last) = self.last_publish.try_lock() else {
            return;
        };
        if last.elapsed() >= LIVE_INTERVAL {
            self.publish(progress);
            *last = Instant::now();
        }
    }

    /// Builds the tree as far as it is known: finished folders in full,
    /// the rest as empty folders marked incomplete.
    fn publish(&self, progress: &ScanProgress) {
        let Ok(slots) = self.slots.lock() else { return };
        let mut builder = TreeBuilder::new(self.root_path.clone(), self.info);
        for (name, info) in &self.files {
            builder.push(NodeId::ROOT, name, *info);
        }
        for slot in slots.iter() {
            match slot {
                Slot::Pending(name, info) => {
                    let mut info = *info;
                    info.flags.insert(Flags::INCOMPLETE);
                    builder.push(NodeId::ROOT, name, info);
                }
                Slot::Done(dir) => push_ref(&mut builder, NodeId::ROOT, dir),
            }
        }
        drop(slots);
        progress.publish_partial(builder.finish());
    }

    fn into_dirs(self) -> Vec<RawDir> {
        self.slots
            .into_inner()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Done(dir) => Some(dir),
                // Every folder is finished before the walk returns.
                Slot::Pending(..) => None,
            })
            .collect()
    }
}

/// Copies `dir` (and everything below it) under `parent`, without consuming it.
fn push_ref(builder: &mut TreeBuilder, parent: NodeId, dir: &RawDir) {
    let mut stack = vec![(parent, dir)];
    while let Some((parent, dir)) = stack.pop() {
        let id = builder.push(parent, &dir.name, dir.info);
        for f in &dir.files {
            builder.push(id, &f.name, f.info);
        }
        stack.extend(dir.subdirs.iter().map(|d| (id, d)));
    }
}

/// Converts the nested result into the arena in pre-order, iteratively, so
/// even pathological depths cannot overflow the stack.
fn flatten(root_path: PathBuf, root: RawDir) -> Tree {
    let mut builder = TreeBuilder::new(root_path, root.info);
    let mut stack: Vec<(NodeId, RawDir)> = Vec::new();
    push_contents(&mut builder, &mut stack, NodeId::ROOT, root);
    while let Some((parent, dir)) = stack.pop() {
        let id = builder.push(parent, &dir.name, dir.info);
        push_contents(&mut builder, &mut stack, id, dir);
    }
    builder.finish()
}

fn push_contents(
    builder: &mut TreeBuilder,
    stack: &mut Vec<(NodeId, RawDir)>,
    id: NodeId,
    dir: RawDir,
) {
    for f in dir.files {
        builder.push(id, &f.name, f.info);
    }
    stack.extend(dir.subdirs.into_iter().map(|d| (id, d)));
}

#[cfg(unix)]
pub(crate) fn name_to_os(name: &[u8]) -> &std::ffi::OsStr {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(name)
}

#[cfg(not(unix))]
pub(crate) fn name_to_os(name: &[u8]) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(std::str::from_utf8(name).unwrap_or_default())
}

/// A directory entry with no data, used for roots and placeholders.
pub(crate) fn dir_info(modified: i64, allocated: u64) -> EntryInfo {
    EntryInfo {
        kind: Kind::Dir,
        flags: Flags::empty(),
        modified,
        accessed: 0,
        created: 0,
        owner: crate::NO_OWNER,
        size: 0,
        allocated,
    }
}

/// Classifies a name as hidden by Unix convention.
#[cfg(not(windows))]
pub(crate) fn dot_hidden(name: &[u8]) -> Flags {
    if name.first() == Some(&b'.') {
        Flags::HIDDEN
    } else {
        Flags::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// In-memory platform: path -> entries.
    struct Fake(HashMap<PathBuf, Vec<(&'static str, bool, u64)>>);

    impl Platform for Fake {
        type Dir = PathBuf;
        fn open_root(&self, path: &Path) -> io::Result<PathBuf> {
            Ok(path.to_path_buf())
        }
        fn open_child(&self, _: &PathBuf, parent_path: &Path, name: &[u8]) -> io::Result<PathBuf> {
            let p = parent_path.join(name_to_os(name));
            if p.ends_with("locked") {
                Err(io::ErrorKind::PermissionDenied.into())
            } else {
                Ok(p)
            }
        }
        fn list(&self, dir: &PathBuf, out: &mut Vec<RawEntry>) -> io::Result<()> {
            for &(name, is_dir, size) in self.0.get(dir).into_iter().flatten() {
                let kind = if is_dir { Kind::Dir } else { Kind::File };
                let info = EntryInfo {
                    kind,
                    flags: Flags::empty(),
                    modified: 1,
                    accessed: 0,
                    created: 0,
                    owner: crate::NO_OWNER,
                    size,
                    allocated: size,
                };
                out.push(RawEntry {
                    name: name.as_bytes().to_vec(),
                    info,
                    descend: is_dir,
                });
            }
            Ok(())
        }
    }

    #[test]
    fn walks_nested_dirs_and_records_errors() {
        let mut fs = HashMap::new();
        fs.insert(
            PathBuf::from("/"),
            vec![("a", true, 0), ("f", false, 5), ("locked", true, 0)],
        );
        fs.insert(PathBuf::from("/a"), vec![("b", true, 0), ("g", false, 7)]);
        fs.insert(PathBuf::from("/a/b"), vec![("h", false, 11)]);
        let progress = ScanProgress::new();
        let tree = walk(
            &Fake(fs),
            Path::new("/"),
            dir_info(0, 0),
            &Matcher::default(),
            &progress,
        )
        .expect("walk");
        assert_eq!(tree.node(tree.root()).size, 23);
        assert_eq!(tree.node(tree.root()).files, 3);
        assert_eq!(progress.snapshot().errors, 1);
        let locked = tree.find(Path::new("/locked")).expect("locked node");
        assert!(tree.node(locked).flags.contains(Flags::ERROR));
    }

    #[test]
    fn filter_leaves_out_excluded_entries() {
        let mut fs = HashMap::new();
        fs.insert(
            PathBuf::from("/"),
            vec![("keep", true, 0), ("skip", true, 0), ("a.tmp", false, 5)],
        );
        fs.insert(
            PathBuf::from("/keep"),
            vec![("f", false, 7), ("skip", false, 3)],
        );
        fs.insert(PathBuf::from("/skip"), vec![("big", false, 100)]);
        let filter = crate::ScanFilter {
            exclude: vec!["/skip".into(), "*.tmp".into()],
            include_files: Vec::new(),
        }
        .compile();
        let progress = ScanProgress::new();
        let tree = walk(
            &Fake(fs),
            Path::new("/"),
            dir_info(0, 0),
            &filter,
            &progress,
        )
        .expect("walk");
        assert_eq!(tree.node(tree.root()).size, 10);
        assert!(tree.find(Path::new("/skip")).is_none());
        assert!(tree.find(Path::new("/keep/skip")).is_some());
    }

    #[test]
    fn live_scan_publishes_partial_trees_and_the_same_result() {
        let mut fs = HashMap::new();
        fs.insert(
            PathBuf::from("/"),
            vec![("a", true, 0), ("b", true, 0), ("f", false, 5)],
        );
        fs.insert(PathBuf::from("/a"), vec![("x", false, 7)]);
        fs.insert(PathBuf::from("/b"), vec![("y", false, 11)]);
        let progress = ScanProgress::new();
        progress.enable_live();
        let tree = walk(
            &Fake(fs),
            Path::new("/"),
            dir_info(0, 0),
            &Matcher::default(),
            &progress,
        )
        .expect("walk");
        assert_eq!(tree.node(tree.root()).size, 23);
        assert!(tree.find(Path::new("/b/y")).is_some());
        // At least the first partial (top level only) was published.
        let partial = progress.take_partial().expect("partial tree");
        assert!(partial.find(Path::new("/a")).is_some());
        assert!(partial.find(Path::new("/f")).is_some());
        assert!(progress.take_partial().is_none());
    }

    #[test]
    fn cancelled_scan_marks_incomplete() {
        let progress = ScanProgress::new();
        progress.cancel();
        let tree = walk(
            &Fake(HashMap::new()),
            Path::new("/"),
            dir_info(0, 0),
            &Matcher::default(),
            &progress,
        )
        .expect("walk");
        assert!(tree.node(tree.root()).flags.contains(Flags::INCOMPLETE));
    }
}
