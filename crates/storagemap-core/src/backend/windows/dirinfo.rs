//! Directory walker using `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`.
//!
//! One call returns a 64 KiB batch of entries *with* logical size, allocation
//! size, attributes and times, so no per-file `stat` is needed — far fewer
//! kernel transitions than `FindFirstFile`/`GetFileAttributesEx`.

use std::ffi::OsString;
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::ERROR_NO_MORE_FILES;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_BOTH_DIR_INFO, FILE_LIST_DIRECTORY,
    FileIdBothDirectoryInfo, GetFileInformationByHandleEx,
};

use super::{extended_path, open};
use crate::ScanError;
use crate::filter::Matcher;
use crate::ntfs::filetime_to_unix;
use crate::progress::ScanProgress;
use crate::tree::{EntryInfo, Flags, Kind, Tree};
use crate::walk::{self, Platform, RawEntry};

/// Bytes per listing batch; u64 elements keep the buffer 8-byte aligned.
const BATCH_BYTES: usize = 64 * 1024;

struct DirInfo;

/// An open directory and its extended-length path.
pub(super) struct DirHandle {
    handle: OwnedHandle,
    path: PathBuf,
}

pub(super) fn scan(
    root: &Path,
    filter: &Matcher,
    progress: &ScanProgress,
) -> Result<Tree, ScanError> {
    let meta = std::fs::metadata(root).map_err(|e| ScanError::io(root, e))?;
    if !meta.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    walk::walk(
        &DirInfo,
        root,
        walk::dir_info(modified, 0),
        filter,
        progress,
    )
    .map_err(|e| ScanError::io(root, e))
}

impl Platform for DirInfo {
    type Dir = DirHandle;

    fn open_root(&self, path: &Path) -> io::Result<DirHandle> {
        open_dir(extended_path(path))
    }

    fn open_child(&self, parent: &DirHandle, _: &Path, name: &[u8]) -> io::Result<DirHandle> {
        open_dir(parent.path.join(walk::name_to_os(name)))
    }

    fn list(&self, dir: &DirHandle, out: &mut Vec<RawEntry>) -> io::Result<()> {
        let mut buf = vec![0u64; BATCH_BYTES / size_of::<u64>()];
        loop {
            // SAFETY: the handle is open for FILE_LIST_DIRECTORY and `buf`
            // is BATCH_BYTES long and 8-byte aligned.
            let ok = unsafe {
                GetFileInformationByHandleEx(
                    dir.handle.as_raw_handle(),
                    FileIdBothDirectoryInfo,
                    buf.as_mut_ptr().cast(),
                    BATCH_BYTES as u32,
                )
            };
            if ok == 0 {
                let err = io::Error::last_os_error();
                return if err.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                    Ok(())
                } else {
                    Err(err)
                };
            }
            parse_batch(bytes_of(&buf), out);
        }
    }
}

fn open_dir(path: PathBuf) -> io::Result<DirHandle> {
    let handle = open(
        path.as_os_str(),
        FILE_LIST_DIRECTORY,
        FILE_FLAG_BACKUP_SEMANTICS,
    )?;
    Ok(DirHandle { handle, path })
}

fn bytes_of(buf: &[u64]) -> &[u8] {
    // SAFETY: any u64 slice is a valid, initialised byte slice of 8x length.
    unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), std::mem::size_of_val(buf)) }
}

/// Walks the `NextEntryOffset` chain of one batch, bounds-checking every step.
fn parse_batch(batch: &[u8], out: &mut Vec<RawEntry>) {
    let header = offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
    let mut off = 0usize;
    while off + header <= batch.len() {
        // SAFETY: `off + header` is in bounds; read_unaligned has no alignment needs.
        let info: FILE_ID_BOTH_DIR_INFO =
            unsafe { std::ptr::read_unaligned(batch.as_ptr().add(off).cast()) };
        let name_len = info.FileNameLength as usize;
        let Some(raw) = batch.get(off + header..off + header + name_len) else {
            break;
        };
        let units: Vec<u16> = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        if units != [u16::from(b'.')] && units != [u16::from(b'.'); 2] {
            out.push(entry(&info, &units));
        }
        if info.NextEntryOffset == 0 {
            break;
        }
        off += info.NextEntryOffset as usize;
    }
}

fn entry(info: &FILE_ID_BOTH_DIR_INFO, name: &[u16]) -> RawEntry {
    let attrs = info.FileAttributes;
    let is_reparse = attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    let kind = match (attrs & FILE_ATTRIBUTE_DIRECTORY != 0, is_reparse) {
        (true, true) => Kind::Symlink,
        (true, false) => Kind::Dir,
        (false, _) => Kind::File,
    };
    let mut flags = Flags::empty();
    if attrs & FILE_ATTRIBUTE_HIDDEN != 0 {
        flags.insert(Flags::HIDDEN);
    }
    let name = OsString::from_wide(name)
        .to_string_lossy()
        .into_owned()
        .into_bytes();
    RawEntry {
        name,
        info: EntryInfo {
            kind,
            flags,
            modified: filetime_to_unix(info.LastWriteTime.max(0) as u64),
            size: if kind == Kind::File {
                info.EndOfFile.max(0) as u64
            } else {
                0
            },
            allocated: info.AllocationSize.max(0) as u64,
        },
        // Junctions and mount points are reparse points: never followed,
        // which also keeps the scan on one volume.
        descend: kind == Kind::Dir,
    }
}
