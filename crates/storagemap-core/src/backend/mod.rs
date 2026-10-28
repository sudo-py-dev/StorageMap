//! Picks the fastest available scanning strategy for the current OS.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(target_os = "linux", windows)))]
mod portable;
#[cfg(windows)]
mod windows;

use std::path::Path;

use crate::progress::ScanProgress;
use crate::tree::Tree;
use crate::{ScanError, ScanOptions};

/// The strategy that produced a scan, shown to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    /// Linux `getdents64` + `statx`, relative to parent descriptors.
    LinuxStatx,
    /// Raw NTFS Master File Table read (Windows, administrator).
    NtfsMft,
    /// Batched `GetFileInformationByHandleEx` directory listings (Windows).
    WindowsDirInfo,
    /// Plain `std::fs` traversal.
    Portable,
}

pub(crate) fn scan(
    root: &Path,
    opts: &ScanOptions,
    progress: &ScanProgress,
) -> Result<(Tree, Backend), ScanError> {
    #[cfg(target_os = "linux")]
    return linux::scan(root, opts, progress);
    #[cfg(windows)]
    return windows::scan(root, opts, progress);
    #[cfg(not(any(target_os = "linux", windows)))]
    return portable::scan(root, opts, progress);
}
