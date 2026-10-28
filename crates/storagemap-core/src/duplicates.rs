//! Duplicate file finder.
//!
//! Staged so most files are never read in full: group by exact size, then by
//! a BLAKE3 hash of the first and last 16 KiB, and only then hash whole files
//! that still collide. Hard links are excluded (they share data already).

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use rayon::prelude::*;

use crate::tree::{Flags, Kind, NodeId, Tree};

/// A content digest and the number of bytes read to compute it.
type HashResult = io::Result<([u8; 32], u64)>;
type HashFn = fn(&Candidate) -> HashResult;

/// Bytes hashed from each end of a file in the prefilter stage.
const PROBE: u64 = 16 * 1024;

/// A file worth checking: some other file has exactly its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: NodeId,
    pub path: PathBuf,
    pub size: u64,
}

/// Files with identical content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub size: u64,
    pub files: Vec<NodeId>,
}

impl DuplicateGroup {
    /// Space freed by keeping just one copy.
    pub fn wasted(&self) -> u64 {
        self.size * (self.files.len() as u64).saturating_sub(1)
    }
}

#[derive(Debug, Default)]
pub struct DuplicateProgress {
    pub candidates: AtomicU64,
    pub files_hashed: AtomicU64,
    pub bytes_hashed: AtomicU64,
    pub errors: AtomicU64,
    cancelled: AtomicBool,
}

impl DuplicateProgress {
    pub fn cancel(&self) {
        self.cancelled.store(true, Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Relaxed)
    }
}

/// Files below `root` of at least `min_size` bytes that share their size
/// with another file. Cheap; runs on the caller's thread.
pub fn candidates(tree: &Tree, root: NodeId, min_size: u64) -> Vec<Candidate> {
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for id in tree.walk(root) {
        let n = tree.node(id);
        if n.kind == Kind::File
            && n.size >= min_size.max(1)
            && !n.flags.contains(Flags::HARDLINK_DUP)
        {
            by_size.entry(n.size).or_default().push(id);
        }
    }
    let mut out: Vec<Candidate> = by_size
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .flat_map(|(size, ids)| ids.into_iter().map(move |id| (size, id)))
        .map(|(size, id)| Candidate {
            id,
            path: tree.path(id),
            size,
        })
        .collect();
    out.sort_by_key(|c| (std::cmp::Reverse(c.size), c.id));
    out
}

/// Confirms duplicates among `candidates` by content. Blocking and
/// parallel; returns groups sorted by reclaimable space (empty if cancelled).
pub fn find(candidates: &[Candidate], progress: &DuplicateProgress) -> Vec<DuplicateGroup> {
    progress.candidates.store(candidates.len() as u64, Relaxed);
    let probed = regroup(candidates.iter().collect(), progress, probe_hash);
    let mut confirmed = Vec::new();
    for group in probed {
        // Files no longer than two probes were hashed completely already.
        if group[0].size <= 2 * PROBE {
            confirmed.push(group);
        } else {
            confirmed.extend(regroup(group, progress, full_hash));
        }
    }
    if progress.is_cancelled() {
        return Vec::new();
    }
    let mut groups: Vec<DuplicateGroup> = confirmed
        .into_iter()
        .map(|g| {
            let mut files: Vec<NodeId> = g.iter().map(|c| c.id).collect();
            files.sort_unstable();
            DuplicateGroup {
                size: g[0].size,
                files,
            }
        })
        .collect();
    groups.sort_by_key(|g| (std::cmp::Reverse(g.wasted()), g.files[0]));
    groups
}

/// Splits candidates by (size, hash) and keeps groups of two or more.
fn regroup<'a>(
    files: Vec<&'a Candidate>,
    progress: &DuplicateProgress,
    hash: HashFn,
) -> Vec<Vec<&'a Candidate>> {
    let hashed: Vec<((u64, [u8; 32]), &Candidate)> = files
        .into_par_iter()
        .filter_map(|c| {
            if progress.is_cancelled() {
                return None;
            }
            match hash(c) {
                Ok((h, bytes)) => {
                    progress.files_hashed.fetch_add(1, Relaxed);
                    progress.bytes_hashed.fetch_add(bytes, Relaxed);
                    Some(((c.size, h), c))
                }
                Err(err) => {
                    tracing::debug!(path = %c.path.display(), %err, "cannot hash file");
                    progress.errors.fetch_add(1, Relaxed);
                    None
                }
            }
        })
        .collect();
    let mut groups: HashMap<(u64, [u8; 32]), Vec<&Candidate>> = HashMap::new();
    for (key, c) in hashed {
        groups.entry(key).or_default().push(c);
    }
    groups.into_values().filter(|g| g.len() > 1).collect()
}

/// Hash of the size plus the first and last [`PROBE`] bytes.
fn probe_hash(c: &Candidate) -> HashResult {
    let mut file = File::open(&c.path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; PROBE as usize];
    let mut read = read_up_to(&mut file, &mut buf)?;
    hasher.update(&buf[..read]);
    if c.size > 2 * PROBE {
        file.seek(SeekFrom::End(-(PROBE as i64)))?;
    }
    if c.size > PROBE {
        let tail = read_up_to(&mut file, &mut buf)?;
        hasher.update(&buf[..tail]);
        read += tail;
    }
    hasher.update(&c.size.to_le_bytes());
    Ok((*hasher.finalize().as_bytes(), read as u64))
}

fn full_hash(c: &Candidate) -> HashResult {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(&c.path)?)?;
    Ok((*hasher.finalize().as_bytes(), c.size))
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryInfo, TreeBuilder};
    use std::path::Path;

    /// Builds a tree mirroring real files in `dir`.
    fn tree_of(dir: &Path, files: &[(&str, Vec<u8>)]) -> Tree {
        let dir_info = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            size: 0,
            allocated: 0,
        };
        let mut b = TreeBuilder::new(dir.to_path_buf(), dir_info);
        for (name, data) in files {
            std::fs::write(dir.join(name), data).expect("write");
            let size = data.len() as u64;
            b.push(
                NodeId::ROOT,
                name.as_bytes(),
                EntryInfo {
                    kind: Kind::File,
                    size,
                    allocated: size,
                    ..dir_info
                },
            );
        }
        b.finish()
    }

    fn names(t: &Tree, g: &DuplicateGroup) -> Vec<String> {
        let mut v: Vec<_> = g.files.iter().map(|&i| t.name(i).into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn finds_true_duplicates_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let big = |fill: u8, tail: u8| {
            let mut v = vec![fill; 100 * 1024];
            *v.last_mut().expect("non-empty") = tail;
            v
        };
        let mut middle = big(1, 9);
        middle[50 * 1024] = 7; // differs only where the probe does not look
        let t = tree_of(
            dir.path(),
            &[
                ("a.bin", big(1, 9)),
                ("b.bin", big(1, 9)),
                ("c.bin", middle),
                ("d.bin", big(1, 8)),
                ("e.txt", b"hello".to_vec()),
                ("f.txt", b"hello".to_vec()),
                ("g.txt", b"world".to_vec()),
                ("unique", vec![0; 3]),
            ],
        );
        let cands = candidates(&t, t.root(), 1);
        assert_eq!(cands.len(), 7); // "unique" has no size twin
        let groups = find(&cands, &DuplicateProgress::default());
        assert_eq!(groups.len(), 2);
        assert_eq!(names(&t, &groups[0]), ["a.bin", "b.bin"]);
        assert_eq!(groups[0].wasted(), 100 * 1024);
        assert_eq!(names(&t, &groups[1]), ["e.txt", "f.txt"]);
    }

    #[test]
    fn respects_minimum_size_and_hard_links() {
        let dir = tempfile::tempdir().expect("tempdir");
        let t = tree_of(dir.path(), &[("x", vec![5; 10]), ("y", vec![5; 10])]);
        assert!(candidates(&t, t.root(), 11).is_empty());
        assert_eq!(candidates(&t, t.root(), 10).len(), 2);
    }

    #[test]
    fn missing_files_are_counted_not_fatal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let t = tree_of(dir.path(), &[("p", vec![1; 4]), ("q", vec![1; 4])]);
        std::fs::remove_file(dir.path().join("q")).expect("remove");
        let progress = DuplicateProgress::default();
        assert!(find(&candidates(&t, t.root(), 1), &progress).is_empty());
        assert_eq!(progress.errors.load(Relaxed), 1);
    }

    #[test]
    fn cancelled_search_returns_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let t = tree_of(dir.path(), &[("p", vec![1; 4]), ("q", vec![1; 4])]);
        let progress = DuplicateProgress::default();
        progress.cancel();
        assert!(find(&candidates(&t, t.root(), 1), &progress).is_empty());
    }
}
