//! StorageMap core: fast, parallel disk-usage scanning with OS-specific
//! backends, plus the analyses (top files, file types, ages, treemap layout)
//! the UI and CLI are built on.

pub mod analysis;
mod backend;
pub mod cleanup;
pub mod duplicates;
pub mod export;
mod filter;
pub mod ntfs;
pub mod operations;
mod progress;
pub mod search;
pub mod snapshot;
pub mod source;
mod tree;
pub mod treemap;
pub mod volumes;
mod walk;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub use backend::Backend;
pub use filter::ScanFilter;
pub use progress::{ProgressSnapshot, ScanProgress};
pub use tree::{EntryInfo, Flags, Kind, Metric, NO_OWNER, Node, NodeId, Tree, TreeBuilder, ratio};

/// Stack per scanner thread; walks recurse once per directory level.
const SCAN_THREAD_STACK: usize = 32 * 1024 * 1024;

/// What to scan and how.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Descend into other mounted filesystems / volumes.
    pub cross_filesystems: bool,
    /// Use the raw NTFS MFT reader when possible (Windows, admin).
    pub prefer_mft: bool,
    /// Worker threads; 0 picks a default suited to the storage.
    pub threads: usize,
    /// Entries to leave out of the scan.
    pub filter: ScanFilter,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            cross_filesystems: false,
            prefer_mft: true,
            threads: 0,
            filter: ScanFilter::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot start scanner threads: {0}")]
    ThreadPool(String),
}

impl ScanError {
    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        ScanError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// A finished (possibly cancelled) scan.
#[derive(Debug)]
pub struct ScanResult {
    pub tree: Tree,
    pub backend: Backend,
    pub elapsed: Duration,
    pub errors: u64,
    pub cancelled: bool,
}

/// Scans `root` on a dedicated thread pool. Blocking; call from a worker
/// thread and observe `progress` from the UI.
pub fn scan(
    root: &Path,
    opts: &ScanOptions,
    progress: &ScanProgress,
) -> Result<ScanResult, ScanError> {
    let root = std::path::absolute(root).map_err(|e| ScanError::io(root, e))?;
    run_scan(opts, progress, || backend::scan(&root, opts, progress))
}

/// Runs `scan` on a dedicated thread pool and wraps up its result.
pub(crate) fn run_scan(
    opts: &ScanOptions,
    progress: &ScanProgress,
    scan: impl FnOnce() -> Result<(Tree, Backend), ScanError> + Send,
) -> Result<ScanResult, ScanError> {
    let start = Instant::now();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count(opts.threads))
        .stack_size(SCAN_THREAD_STACK)
        .thread_name(|i| format!("storagemap-scan-{i}"))
        .build()
        .map_err(|e| ScanError::ThreadPool(e.to_string()))?;
    let (tree, backend) = pool.install(scan)?;
    let snap = progress.snapshot();
    tracing::info!(?backend, entries = tree.len(), elapsed = ?start.elapsed(), "scan finished");
    Ok(ScanResult {
        tree,
        backend,
        elapsed: start.elapsed(),
        errors: snap.errors,
        cancelled: progress.is_cancelled(),
    })
}

/// Metadata scans are latency-bound, not CPU-bound: oversubscribing the
/// cores keeps more requests in flight for SSDs and network shares.
fn thread_count(requested: usize) -> usize {
    if requested > 0 {
        return requested;
    }
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    (cores * 2).clamp(4, 64)
}
