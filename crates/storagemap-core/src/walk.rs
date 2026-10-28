//! Backend-independent parallel directory walker.
//!
//! A [`Platform`] knows how to open and list one directory as fast as the OS
//! allows; this module fans directories out over a rayon pool, then flattens
//! the result into a [`Tree`] without deep recursion.

use std::io;
use std::path::{Path, PathBuf};

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
    let raw = walk_dir(&ctx, dir, root.to_path_buf(), String::new(), root_entry);
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

    let subdirs = to_descend
        .into_par_iter()
        .map(|entry| {
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
            match platform.open_child(&dir, &path, &entry.name) {
                Ok(child) => walk_dir(ctx, child, child_path, child_rel, entry),
                Err(err) => {
                    tracing::debug!(path = %child_path.display(), %err, "cannot open directory");
                    progress.add_error();
                    RawDir::leaf(entry, Flags::ERROR)
                }
            }
        })
        .collect();
    drop(dir);

    let mut info = me.info;
    info.flags.insert(flags);
    RawDir {
        name: me.name,
        info,
        files,
        subdirs,
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
