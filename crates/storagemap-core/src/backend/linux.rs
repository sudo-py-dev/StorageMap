//! Linux backend: `openat` + `getdents64` (rustix `Dir`) + `statx`.
//!
//! Every directory is opened relative to its parent's descriptor, so the
//! kernel never re-resolves full paths. `statx` is asked only for the fields
//! we use and with `AT_STATX_DONT_SYNC`, which keeps network filesystems from
//! round-tripping to the server for attributes.

use std::collections::HashSet;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;
use std::sync::Mutex;

use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, Statx, StatxFlags, openat, statx};

use crate::backend::Backend;
use crate::progress::ScanProgress;
use crate::tree::{EntryInfo, Flags, Kind, Tree};
use crate::walk::{self, Platform, RawEntry};
use crate::{ScanError, ScanOptions};

const STATX_MASK: StatxFlags = StatxFlags::TYPE
    .union(StatxFlags::SIZE)
    .union(StatxFlags::BLOCKS)
    .union(StatxFlags::INO)
    .union(StatxFlags::NLINK)
    .union(StatxFlags::MTIME)
    .union(StatxFlags::ATIME)
    .union(StatxFlags::BTIME);
const STAT_FLAGS: AtFlags = AtFlags::SYMLINK_NOFOLLOW.union(AtFlags::STATX_DONT_SYNC);
const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOATIME);
/// `st_blocks` is always in 512-byte units, whatever the filesystem block size.
const BLOCK_UNIT: u64 = 512;
/// Shards for the hard-link set, to keep lock contention negligible.
const LINK_SHARDS: usize = 64;

struct Linux {
    root_dev: (u32, u32),
    cross_filesystems: bool,
    seen_links: Vec<Mutex<HashSet<(u32, u32, u64)>>>,
}

pub(crate) fn scan(
    root: &Path,
    opts: &ScanOptions,
    progress: &ScanProgress,
) -> Result<(Tree, Backend), ScanError> {
    raise_fd_limit();
    let st = statx(CWD, root, AtFlags::STATX_DONT_SYNC, STATX_MASK)
        .map_err(|e| ScanError::io(root, e.into()))?;
    if FileType::from_raw_mode(st.stx_mode.into()) != FileType::Directory {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let platform = Linux {
        root_dev: (st.stx_dev_major, st.stx_dev_minor),
        cross_filesystems: opts.cross_filesystems,
        seen_links: (0..LINK_SHARDS).map(|_| Mutex::default()).collect(),
    };
    let root_info = walk::dir_info(st.stx_mtime.tv_sec, st.stx_blocks * BLOCK_UNIT);
    let tree = walk::walk(&platform, root, root_info, &opts.filter.compile(), progress)
        .map_err(|e| ScanError::io(root, e))?;
    Ok((tree, Backend::LinuxStatx))
}

impl Platform for Linux {
    type Dir = OwnedFd;

    fn open_root(&self, path: &Path) -> io::Result<OwnedFd> {
        open_dir(CWD, path)
    }

    fn open_child(
        &self,
        parent: &OwnedFd,
        _parent_path: &Path,
        name: &[u8],
    ) -> io::Result<OwnedFd> {
        let name = CString::new(name).map_err(|_| io::ErrorKind::InvalidInput)?;
        open_dir(parent, name.as_c_str())
    }

    fn list(&self, dir: &OwnedFd, out: &mut Vec<RawEntry>) -> io::Result<()> {
        // The getdents64 reader takes ownership of a descriptor; a dup is the
        // cheapest way to keep `dir` usable for statx and opening children.
        let mut reader = Dir::new(rustix::io::dup(dir)?)?;
        let fd = dir.as_fd();
        while let Some(entry) = reader.read() {
            let entry = entry?;
            let name = entry.file_name();
            let bytes = name.to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            match statx(fd, name, STAT_FLAGS, STATX_MASK) {
                Ok(st) => out.push(self.entry(bytes, &st)),
                // Vanished between listing and stat: not worth reporting.
                Err(rustix::io::Errno::NOENT) => {}
                Err(_) => out.push(RawEntry {
                    name: bytes.to_vec(),
                    info: EntryInfo {
                        kind: kind_of(entry.file_type()),
                        flags: Flags::ERROR,
                        modified: 0,
                        accessed: 0,
                        created: 0,
                        size: 0,
                        allocated: 0,
                    },
                    descend: false,
                }),
            }
        }
        Ok(())
    }
}

impl Linux {
    fn entry(&self, name: &[u8], st: &Statx) -> RawEntry {
        let kind = kind_of(FileType::from_raw_mode(st.stx_mode.into()));
        let mut flags = walk::dot_hidden(name);
        let on_root_dev = (st.stx_dev_major, st.stx_dev_minor) == self.root_dev;
        if kind == Kind::Dir && !on_root_dev {
            flags.insert(Flags::MOUNT_POINT);
        }
        if kind == Kind::File && st.stx_nlink > 1 && !self.first_link(st) {
            flags.insert(Flags::HARDLINK_DUP);
        }
        RawEntry {
            name: name.to_vec(),
            info: EntryInfo {
                kind,
                flags,
                modified: st.stx_mtime.tv_sec,
                accessed: st.stx_atime.tv_sec,
                // Not every filesystem records creation (birth) time.
                created: if StatxFlags::from_bits_retain(st.stx_mask).contains(StatxFlags::BTIME) {
                    st.stx_btime.tv_sec
                } else {
                    0
                },
                size: if kind == Kind::Dir { 0 } else { st.stx_size },
                allocated: st.stx_blocks * BLOCK_UNIT,
            },
            descend: kind == Kind::Dir && (on_root_dev || self.cross_filesystems),
        }
    }

    /// True the first time an inode is seen.
    fn first_link(&self, st: &Statx) -> bool {
        let key = (st.stx_dev_major, st.stx_dev_minor, st.stx_ino);
        let shard = (st.stx_ino as usize) % LINK_SHARDS;
        self.seen_links[shard]
            .lock()
            .map(|mut s| s.insert(key))
            .unwrap_or(true)
    }
}

fn open_dir<P: rustix::path::Arg + Copy>(at: impl AsFd, path: P) -> io::Result<OwnedFd> {
    match openat(&at, path, DIR_FLAGS, Mode::empty()) {
        // O_NOATIME is only permitted on directories we own.
        Err(rustix::io::Errno::PERM) => openat(
            &at,
            path,
            DIR_FLAGS.difference(OFlags::NOATIME),
            Mode::empty(),
        ),
        other => other,
    }
    .map_err(Into::into)
}

/// Lifts the soft open-files limit to the hard limit: a parallel walk keeps
/// one descriptor per directory level per worker.
fn raise_fd_limit() {
    use rustix::process::{Resource, getrlimit, setrlimit};
    let mut lim = getrlimit(Resource::Nofile);
    if lim.current < lim.maximum {
        lim.current = lim.maximum;
        if let Err(err) = setrlimit(Resource::Nofile, lim) {
            tracing::debug!(%err, "could not raise RLIMIT_NOFILE");
        }
    }
}

fn kind_of(ft: FileType) -> Kind {
    match ft {
        FileType::RegularFile => Kind::File,
        FileType::Directory => Kind::Dir,
        FileType::Symlink => Kind::Symlink,
        _ => Kind::Other,
    }
}
