//! Portable `std::fs` backend for platforms without a specialised one.

use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::backend::Backend;
use crate::progress::ScanProgress;
use crate::tree::{EntryInfo, Flags, Kind, Tree};
use crate::walk::{self, Platform, RawEntry};
use crate::{ScanError, ScanOptions};

struct Portable;

pub(crate) fn scan(
    root: &Path,
    opts: &ScanOptions,
    progress: &ScanProgress,
) -> Result<(Tree, Backend), ScanError> {
    let meta = fs::metadata(root).map_err(|e| ScanError::io(root, e))?;
    if !meta.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let root_info = walk::dir_info(modified(&meta), 0);
    let tree = walk::walk(&Portable, root, root_info, &opts.filter.compile(), progress)
        .map_err(|e| ScanError::io(root, e))?;
    Ok((tree, Backend::Portable))
}

impl Platform for Portable {
    type Dir = PathBuf;

    fn open_root(&self, path: &Path) -> io::Result<PathBuf> {
        Ok(path.to_path_buf())
    }

    fn open_child(&self, _: &PathBuf, parent_path: &Path, name: &[u8]) -> io::Result<PathBuf> {
        Ok(parent_path.join(walk::name_to_os(name)))
    }

    fn list(&self, dir: &PathBuf, out: &mut Vec<RawEntry>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let Ok(entry) = entry else { continue };
            let name = entry
                .file_name()
                .to_string_lossy()
                .into_owned()
                .into_bytes();
            let Ok(meta) = entry.metadata() else { continue };
            let kind = if meta.is_dir() {
                Kind::Dir
            } else if meta.is_symlink() {
                Kind::Symlink
            } else if meta.is_file() {
                Kind::File
            } else {
                Kind::Other
            };
            let size = if kind == Kind::File { meta.len() } else { 0 };
            let info = EntryInfo {
                kind,
                flags: walk::dot_hidden(&name),
                modified: modified(&meta),
                accessed: unix_secs(meta.accessed()),
                created: unix_secs(meta.created()),
                owner: owner(&meta),
                size,
                allocated: size,
            };
            out.push(RawEntry {
                name,
                info,
                descend: kind == Kind::Dir,
            });
        }
        Ok(())
    }
}

#[cfg(unix)]
fn owner(meta: &Metadata) -> u32 {
    std::os::unix::fs::MetadataExt::uid(meta)
}

#[cfg(not(unix))]
fn owner(_meta: &Metadata) -> u32 {
    crate::NO_OWNER
}

fn modified(meta: &Metadata) -> i64 {
    unix_secs(meta.modified())
}

fn unix_secs(time: io::Result<std::time::SystemTime>) -> i64 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}
