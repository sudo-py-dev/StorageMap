//! Scanning places other than the local filesystem.
//!
//! A [`Source`] only has to open and list directories; the parallel walker,
//! filters, progress, cancellation and the tree come for free. Servers,
//! buckets, disk images and devices plug in here.

use std::io;
use std::path::{Path, PathBuf};

use crate::backend::Backend;
use crate::progress::ScanProgress;
use crate::tree::{EntryInfo, Kind};
use crate::walk::{self, Platform, RawEntry};
use crate::{ScanError, ScanOptions, ScanResult};

/// One entry of a directory listing from a [`Source`].
#[derive(Debug, Clone)]
pub struct SourceEntry {
    /// The entry's name, without any separator.
    pub name: Vec<u8>,
    pub info: EntryInfo,
}

/// Something that can list directories.
pub trait Source: Sync {
    /// An open directory, used to list it and to open its children.
    type Dir: Send + Sync;

    /// Short, stable name shown as the scan's engine, such as `"ssh"`.
    fn name(&self) -> &'static str;
    /// Where the scan starts, shown as the root's path (a local path or an
    /// address such as `s3://bucket/prefix`).
    fn root(&self) -> PathBuf;
    fn open_root(&self) -> io::Result<Self::Dir>;
    fn open_child(&self, parent: &Self::Dir, name: &[u8]) -> io::Result<Self::Dir>;
    /// Appends every entry of `dir` to `out`; folders are walked into.
    fn list(&self, dir: &Self::Dir, out: &mut Vec<SourceEntry>) -> io::Result<()>;
}

/// Lets the walker drive a [`Source`].
struct Adapter<'a, S>(&'a S);

impl<S: Source> Platform for Adapter<'_, S> {
    type Dir = S::Dir;

    fn open_root(&self, _path: &Path) -> io::Result<S::Dir> {
        self.0.open_root()
    }

    fn open_child(&self, parent: &S::Dir, _parent_path: &Path, name: &[u8]) -> io::Result<S::Dir> {
        self.0.open_child(parent, name)
    }

    fn list(&self, dir: &S::Dir, out: &mut Vec<RawEntry>) -> io::Result<()> {
        let mut entries = Vec::new();
        self.0.list(dir, &mut entries)?;
        out.extend(entries.into_iter().map(|e| RawEntry {
            descend: e.info.kind == Kind::Dir,
            name: e.name,
            info: e.info,
        }));
        Ok(())
    }
}

/// Scans `source` like [`crate::scan`] scans a local folder. Options that
/// only apply to local disks (other filesystems, the NTFS table) are ignored.
pub fn scan_source<S: Source>(
    source: &S,
    opts: &ScanOptions,
    progress: &ScanProgress,
) -> Result<ScanResult, ScanError> {
    let root = source.root();
    let filter = opts.filter.compile();
    crate::run_scan(opts, progress, || {
        let tree = walk::walk(
            &Adapter(source),
            &root,
            walk::dir_info(0, 0),
            &filter,
            progress,
        )
        .map_err(|e| ScanError::io(&root, e))?;
        Ok((tree, Backend::Source(source.name())))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanFilter;
    use crate::tree::Flags;

    /// A tiny in-memory source: folder path -> (name, size or folder).
    struct Memory;

    impl Source for Memory {
        type Dir = String;

        fn name(&self) -> &'static str {
            "memory"
        }
        fn root(&self) -> PathBuf {
            PathBuf::from("mem://box")
        }
        fn open_root(&self) -> io::Result<String> {
            Ok(String::new())
        }
        fn open_child(&self, parent: &String, name: &[u8]) -> io::Result<String> {
            Ok(format!("{parent}/{}", String::from_utf8_lossy(name)))
        }
        fn list(&self, dir: &String, out: &mut Vec<SourceEntry>) -> io::Result<()> {
            let entries: &[(&str, Option<u64>)] = match dir.as_str() {
                "" => &[("photos", None), ("notes.txt", Some(10))],
                "/photos" => &[("a.jpg", Some(100)), ("b.jpg", Some(200))],
                _ => &[],
            };
            for &(name, size) in entries {
                out.push(SourceEntry {
                    name: name.as_bytes().to_vec(),
                    info: EntryInfo {
                        kind: if size.is_some() {
                            Kind::File
                        } else {
                            Kind::Dir
                        },
                        flags: Flags::empty(),
                        modified: 0,
                        accessed: 0,
                        created: 0,
                        size: size.unwrap_or(0),
                        allocated: size.unwrap_or(0),
                    },
                });
            }
            Ok(())
        }
    }

    #[test]
    fn scans_a_custom_source_with_filters() {
        let opts = ScanOptions {
            filter: ScanFilter {
                exclude: vec!["b.jpg".into()],
                include_files: Vec::new(),
            },
            ..ScanOptions::default()
        };
        let result = scan_source(&Memory, &opts, &ScanProgress::new()).expect("scan");
        assert_eq!(result.backend, Backend::Source("memory"));
        let tree = &result.tree;
        assert_eq!(tree.root_path(), Path::new("mem://box"));
        assert_eq!(tree.node(tree.root()).size, 110);
        assert_eq!(tree.node(tree.root()).files, 2);
    }
}
