//! Runs a scan on a background thread and hands the result to the UI.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use storagemap_core::{ScanError, ScanOptions, ScanProgress, ScanResult};

/// UI refresh period while a scan runs.
pub const PROGRESS_REFRESH: std::time::Duration = std::time::Duration::from_millis(100);

pub struct ScanJob {
    pub root: PathBuf,
    pub progress: Arc<ScanProgress>,
    pub started: Instant,
    rx: Receiver<Result<ScanResult, String>>,
}

pub enum Poll {
    Running,
    Done(Result<ScanResult, String>),
}

impl ScanJob {
    pub fn start(root: PathBuf, opts: ScanOptions, ctx: egui::Context) -> std::io::Result<ScanJob> {
        let progress = Arc::new(ScanProgress::new());
        let (tx, rx) = mpsc::channel();
        let (thread_root, thread_progress) = (root.clone(), Arc::clone(&progress));
        std::thread::Builder::new()
            .name("storagemap-scan".into())
            .spawn(move || {
                let result = storagemap_core::scan(&thread_root, &opts, &thread_progress)
                    .map_err(|e: ScanError| e.to_string());
                // The receiver may be gone if the user started another scan.
                let _ = tx.send(result);
                ctx.request_repaint();
            })?;
        Ok(ScanJob {
            root,
            progress,
            started: Instant::now(),
            rx,
        })
    }

    pub fn cancel(&self) {
        self.progress.cancel();
    }

    pub fn poll(&self) -> Poll {
        match self.rx.try_recv() {
            Ok(result) => Poll::Done(result),
            Err(TryRecvError::Empty) => Poll::Running,
            Err(TryRecvError::Disconnected) => {
                Poll::Done(Err("scanner thread stopped unexpectedly".into()))
            }
        }
    }
}
