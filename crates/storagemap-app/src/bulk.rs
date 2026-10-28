//! Bulk actions on marked entries, run on a background thread.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use storagemap_core::NodeId;
use storagemap_core::operations::{self, PlannedRename};

/// What to do with the marked entries.
#[derive(Debug, Clone)]
pub enum Kind {
    MoveTo(PathBuf),
    CopyTo(PathBuf),
    /// Into one new archive.
    Zip(PathBuf),
    Trash,
    Rename(Vec<PlannedRename>),
}

impl Kind {
    /// Message id for the progress line.
    pub fn working_id(&self) -> &'static str {
        match self {
            Kind::MoveTo(_) => "bulk-moving",
            Kind::CopyTo(_) => "bulk-copying",
            Kind::Zip(_) => "bulk-zipping",
            Kind::Trash => "bulk-trashing",
            Kind::Rename(_) => "bulk-renaming",
        }
    }

    /// Message id for the summary.
    pub fn done_id(&self) -> &'static str {
        match self {
            Kind::MoveTo(_) => "bulk-moved",
            Kind::CopyTo(_) => "bulk-copied",
            Kind::Zip(_) => "bulk-zipped",
            Kind::Trash => "bulk-trashed",
            Kind::Rename(_) => "bulk-renamed",
        }
    }
}

pub type Outcome = Vec<(NodeId, Result<(), String>)>;

pub struct Job {
    pub kind: Kind,
    /// Progress: items, or bytes when zipping.
    pub done: Arc<AtomicU64>,
    pub total: u64,
    rx: Receiver<Outcome>,
}

impl Job {
    /// Starts `kind` on `items` (entry and full path). `bytes` is their
    /// total size, used for zip progress.
    pub fn start(
        kind: Kind,
        items: Vec<(NodeId, PathBuf)>,
        bytes: u64,
        ctx: &egui::Context,
    ) -> std::io::Result<Job> {
        let total = match &kind {
            Kind::Zip(_) => bytes,
            Kind::Rename(plans) => plans.len() as u64,
            _ => items.len() as u64,
        };
        let done = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel();
        let (thread_kind, thread_done, ctx) = (kind.clone(), Arc::clone(&done), ctx.clone());
        std::thread::Builder::new()
            .name("storagemap-bulk".into())
            .spawn(move || {
                let _ = tx.send(run(&thread_kind, items, &thread_done));
                ctx.request_repaint();
            })?;
        Ok(Job {
            kind,
            done,
            total,
            rx,
        })
    }

    /// The outcome once finished.
    pub fn poll(&self) -> Option<Outcome> {
        match self.rx.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Vec::new()),
        }
    }
}

fn run(kind: &Kind, items: Vec<(NodeId, PathBuf)>, done: &AtomicU64) -> Outcome {
    let each = |f: &dyn Fn(&PathBuf) -> std::io::Result<()>| -> Outcome {
        items
            .iter()
            .map(|(id, path)| {
                let result = f(path).map_err(|e| e.to_string());
                done.fetch_add(1, Relaxed);
                (*id, result)
            })
            .collect()
    };
    match kind {
        Kind::MoveTo(dest) => each(&|p| operations::move_into(p, dest).map(drop)),
        Kind::CopyTo(dest) => each(&|p| operations::copy_into(p, dest).map(drop)),
        Kind::Trash => each(&|p| trash::delete(p).map_err(std::io::Error::other)),
        Kind::Zip(archive) => {
            let paths: Vec<PathBuf> = items.iter().map(|(_, p)| p.clone()).collect();
            let result = operations::zip_items(archive, &paths, done).map_err(|e| e.to_string());
            items.iter().map(|(id, _)| (*id, result.clone())).collect()
        }
        Kind::Rename(plans) => items
            .iter()
            .zip(plans)
            .map(|((id, _), plan)| {
                let result = operations::rename(plan).map_err(|e| e.to_string());
                done.fetch_add(1, Relaxed);
                (*id, result)
            })
            .collect(),
    }
}
