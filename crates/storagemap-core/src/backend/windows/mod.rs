//! Windows backends: raw NTFS MFT reading when elevated, otherwise batched
//! `GetFileInformationByHandleEx` directory listings.

mod dirinfo;
mod mft;

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetVolumeNameForVolumeMountPointW, GetVolumePathNameW, OPEN_EXISTING,
};

use crate::backend::Backend;
use crate::progress::ScanProgress;
use crate::tree::Tree;
use crate::{ScanError, ScanOptions};

/// Room for `\\?\Volume{GUID}\` and long volume mount paths.
const VOLUME_NAME_CAP: usize = 1024;

pub(crate) fn scan(
    root: &Path,
    opts: &ScanOptions,
    progress: &ScanProgress,
) -> Result<(Tree, Backend), ScanError> {
    if opts.prefer_mft {
        match mft::scan(root, progress) {
            Ok(tree) => return Ok((tree, Backend::NtfsMft)),
            Err(_) if progress.is_cancelled() => {
                return Ok((cancelled_tree(root), Backend::NtfsMft));
            }
            Err(err) => tracing::info!(%err, "MFT scan unavailable, using directory listings"),
        }
    }
    dirinfo::scan(root, progress).map(|tree| (tree, Backend::WindowsDirInfo))
}

fn cancelled_tree(root: &Path) -> Tree {
    let mut info = crate::walk::dir_info(0, 0);
    info.flags.insert(crate::tree::Flags::INCOMPLETE);
    crate::tree::TreeBuilder::new(root.to_path_buf(), info).finish()
}

/// NUL-terminated UTF-16 for Win32 calls.
pub(crate) fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

/// `\\?\` form that lifts the 260-character path limit.
pub(crate) fn extended_path(path: &Path) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if s.starts_with(r"\\?\") {
        path.to_path_buf()
    } else if let Some(unc) = s.strip_prefix(r"\\") {
        PathBuf::from(format!(r"\\?\UNC\{unc}"))
    } else {
        PathBuf::from(format!(r"\\?\{s}"))
    }
}

/// Opens a file or device; `INVALID_HANDLE_VALUE` becomes an `io::Error`.
pub(crate) fn open(path: &OsStr, access: u32, flags: u32) -> io::Result<OwnedHandle> {
    let name = wide(path);
    // SAFETY: `name` is NUL-terminated and outlives the call; null security
    // attributes and template handle are documented as valid.
    let h = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `h` is a fresh, valid handle we exclusively own.
    Ok(unsafe { OwnedHandle::from_raw_handle(h) })
}

/// Volume mount point containing `path` (e.g. `C:\`) and its device name
/// (e.g. `\\?\Volume{…}\`).
pub(crate) fn volume_of(path: &Path) -> io::Result<(PathBuf, OsString)> {
    let name = wide(path);
    let mut mount = vec![0u16; VOLUME_NAME_CAP];
    let mut device = vec![0u16; VOLUME_NAME_CAP];
    // SAFETY: buffers are VOLUME_NAME_CAP long and the lengths passed match.
    unsafe {
        if GetVolumePathNameW(name.as_ptr(), mount.as_mut_ptr(), VOLUME_NAME_CAP as u32) == 0 {
            return Err(io::Error::last_os_error());
        }
        if GetVolumeNameForVolumeMountPointW(
            mount.as_ptr(),
            device.as_mut_ptr(),
            VOLUME_NAME_CAP as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok((PathBuf::from(from_wide_nul(&mount)), from_wide_nul(&device)))
}

fn from_wide_nul(buf: &[u16]) -> OsString {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..len])
}
