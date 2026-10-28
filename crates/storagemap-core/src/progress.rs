//! Lock-free scan progress shared between the scanner and the UI.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use crate::tree::Tree;

/// How often (in directories) the "current path" is refreshed; keeps the
/// mutex off the hot path.
const CURRENT_PATH_EVERY: u64 = 64;

#[derive(Debug, Default)]
pub struct ScanProgress {
    files: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
    errors: AtomicU64,
    cancelled: AtomicBool,
    current: Mutex<PathBuf>,
    /// Publish partial trees while scanning (see [`ScanProgress::take_partial`]).
    live: AtomicBool,
    partial: Mutex<Option<Tree>>,
}

/// A consistent-enough copy of the counters for display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgressSnapshot {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub current: PathBuf,
}

impl ScanProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the running scan to stop; partial results are kept.
    pub fn cancel(&self) {
        self.cancelled.store(true, Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Relaxed)
    }

    /// Asks the scan to publish partial trees for browsing while it runs.
    /// Engines that build the tree only at the end (the NTFS table reader)
    /// publish none.
    pub fn enable_live(&self) {
        self.live.store(true, Relaxed);
    }

    pub(crate) fn wants_live(&self) -> bool {
        self.live.load(Relaxed)
    }

    /// The newest partial tree published since the last call, if any.
    /// Folders still being scanned are marked [`crate::Flags::INCOMPLETE`].
    pub fn take_partial(&self) -> Option<Tree> {
        self.partial.lock().ok()?.take()
    }

    pub(crate) fn publish_partial(&self, tree: Tree) {
        if let Ok(mut partial) = self.partial.lock() {
            *partial = Some(tree);
        }
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files: self.files.load(Relaxed),
            dirs: self.dirs.load(Relaxed),
            bytes: self.bytes.load(Relaxed),
            errors: self.errors.load(Relaxed),
            current: self.current.lock().map(|p| p.clone()).unwrap_or_default(),
        }
    }

    pub(crate) fn add_dir(&self, path: &Path, files: u64, bytes: u64) {
        let n = self.dirs.fetch_add(1, Relaxed);
        self.files.fetch_add(files, Relaxed);
        self.bytes.fetch_add(bytes, Relaxed);
        if n.is_multiple_of(CURRENT_PATH_EVERY)
            && let Ok(mut cur) = self.current.try_lock()
        {
            path.clone_into(&mut cur);
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn add_files(&self, files: u64, bytes: u64) {
        self.files.fetch_add(files, Relaxed);
        self.bytes.fetch_add(bytes, Relaxed);
    }

    pub(crate) fn add_error(&self) {
        self.errors.fetch_add(1, Relaxed);
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn set_current(&self, path: &Path) {
        if let Ok(mut cur) = self.current.lock() {
            path.clone_into(&mut cur);
        }
    }
}
