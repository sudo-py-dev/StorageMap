//! Raw NTFS Master File Table scanner (needs administrator rights).
//!
//! Instead of asking the filesystem about every directory, read the `$MFT`
//! sequentially in large chunks and reconstruct the hierarchy from parent
//! references. A reader thread keeps the disk busy while rayon parses the
//! previous chunk.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use std::path::{Component, Path};
use std::sync::mpsc;

use rayon::prelude::*;
use windows_sys::Win32::Foundation::GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_BEGIN, FILE_FLAG_SEQUENTIAL_SCAN, ReadFile, SetFilePointerEx,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{FSCTL_GET_NTFS_VOLUME_DATA, NTFS_VOLUME_DATA_BUFFER};

use super::{open, volume_of};
use crate::filter::Matcher;
use crate::ntfs::{self, NAMESPACE_DOS, ROOT_RECORD, Run};
use crate::progress::ScanProgress;
use crate::tree::{EntryInfo, Flags, Kind, NodeId, Tree, TreeBuilder};

/// Bytes per sequential read of the MFT.
const CHUNK_BYTES: u64 = 8 * 1024 * 1024;
/// Chunks buffered between the reader thread and the parser.
const CHUNKS_IN_FLIGHT: usize = 4;
const ATTR_DATA: u32 = 0x80;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[derive(Debug, Clone, Copy)]
struct Geometry {
    record_size: u64,
    cluster: u64,
    mft_start_lcn: u64,
    valid_len: u64,
}

/// Per-record facts kept for every MFT record (compact on purpose).
#[derive(Debug, Clone, Copy, Default)]
struct Meta {
    size: u64,
    allocated: u64,
    modified: i64,
    attributes: u32,
    in_use: bool,
    is_dir: bool,
}

/// A name linking `child` into `parent`; names live in a shared pool.
#[derive(Debug, Clone, Copy)]
struct Link {
    parent: u32,
    child: u32,
    name_start: u32,
    name_len: u32,
}

#[derive(Default)]
struct Table {
    meta: Vec<Meta>,
    links: Vec<Link>,
    names: Vec<u8>,
}

/// Parsed output of one chunk, merged into the [`Table`] in order.
struct ChunkOut {
    records: Vec<(u32, ntfs::Record)>,
}

pub(super) fn scan(root: &Path, filter: &Matcher, progress: &ScanProgress) -> io::Result<Tree> {
    let root = std::path::absolute(root)?;
    let (mount, device) = volume_of(&root)?;
    let device = device.to_string_lossy().trim_end_matches('\\').to_owned();
    let volume = open(OsStr::new(&device), GENERIC_READ, FILE_FLAG_SEQUENTIAL_SCAN)?;
    let geo = geometry(&volume)?;
    progress.set_current(Path::new("$MFT"));

    let runs = mft_runs(&volume, &geo)?;
    let table = read_table(&volume, &geo, &runs, progress)?;
    if progress.is_cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let rel = root.strip_prefix(&mount).unwrap_or(Path::new(""));
    let children = index_children(&table);
    let start = locate(&table, &children, rel).ok_or(io::ErrorKind::NotFound)?;
    Ok(build(&root, &table, &children, start, filter))
}

fn geometry(volume: &OwnedHandle) -> io::Result<Geometry> {
    // SAFETY: plain-old-data struct, zero is a valid bit pattern.
    let mut data: NTFS_VOLUME_DATA_BUFFER = unsafe { std::mem::zeroed() };
    let mut returned = 0u32;
    // SAFETY: output buffer is a live NTFS_VOLUME_DATA_BUFFER of the given size.
    let ok = unsafe {
        DeviceIoControl(
            volume.as_raw_handle(),
            FSCTL_GET_NTFS_VOLUME_DATA,
            std::ptr::null(),
            0,
            (&raw mut data).cast(),
            size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error()); // not NTFS
    }
    let geo = Geometry {
        record_size: u64::from(data.BytesPerFileRecordSegment),
        cluster: u64::from(data.BytesPerCluster),
        mft_start_lcn: data.MftStartLcn.max(0) as u64,
        valid_len: data.MftValidDataLength.max(0) as u64,
    };
    if geo.record_size == 0 || geo.cluster == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid NTFS geometry",
        ));
    }
    Ok(geo)
}

/// Run list of the `$MFT` itself, following its attribute list if fragmented.
fn mft_runs(volume: &OwnedHandle, geo: &Geometry) -> io::Result<Vec<Run>> {
    let mut buf = vec![0u8; geo.record_size as usize];
    read_at(volume, geo.mft_start_lcn * geo.cluster, &mut buf)?;
    let rec0 = ntfs::parse_record(&mut buf, true).map_err(invalid)?;
    if rec0.has_nonresident_attribute_list {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "MFT attribute list is non-resident",
        ));
    }
    let mut pieces = rec0.data_runs.clone();
    let mut flat = flatten(&pieces);
    let mut extensions: Vec<u64> = rec0
        .attribute_list
        .iter()
        .filter(|e| e.attr_type == ATTR_DATA && e.record != 0)
        .map(|e| e.record)
        .collect();
    extensions.dedup();
    for rec in extensions {
        let offset = stream_to_volume(&flat, geo, rec * geo.record_size)
            .ok_or_else(|| invalid(ntfs::NtfsError::BadRunlist))?;
        read_at(volume, offset, &mut buf)?;
        let ext = ntfs::parse_record(&mut buf, true).map_err(invalid)?;
        pieces.extend(ext.data_runs);
        pieces.sort_by_key(|(vcn, _)| *vcn);
        flat = flatten(&pieces);
    }
    Ok(flat)
}

fn flatten(pieces: &[(u64, Vec<Run>)]) -> Vec<Run> {
    let mut sorted: Vec<_> = pieces.iter().collect();
    sorted.sort_by_key(|(vcn, _)| *vcn);
    sorted
        .into_iter()
        .flat_map(|(_, runs)| runs.iter().copied())
        .collect()
}

/// Maps a byte offset inside the MFT stream to a byte offset on the volume.
fn stream_to_volume(runs: &[Run], geo: &Geometry, stream_off: u64) -> Option<u64> {
    let mut vcn_start = 0u64;
    let target = stream_off / geo.cluster;
    for run in runs {
        if target < vcn_start + run.clusters {
            return Some((run.lcn? + target - vcn_start) * geo.cluster + stream_off % geo.cluster);
        }
        vcn_start += run.clusters;
    }
    None
}

/// Reads the whole MFT (reader thread) and parses it (rayon) into a table.
fn read_table(
    volume: &OwnedHandle,
    geo: &Geometry,
    runs: &[Run],
    progress: &ScanProgress,
) -> io::Result<Table> {
    let total_records = geo.valid_len / geo.record_size;
    let mut table = Table {
        meta: vec![Meta::default(); total_records as usize],
        ..Table::default()
    };
    let rs = geo.record_size as usize;

    std::thread::scope(|scope| -> io::Result<()> {
        let (tx, rx) = mpsc::sync_channel::<(u64, Vec<u8>)>(CHUNKS_IN_FLIGHT);
        let reader = scope.spawn(move || read_stream(volume, geo, runs, progress, &tx));
        for (first_record, mut chunk) in rx {
            let records: Vec<(u32, ntfs::Record)> = chunk
                .par_chunks_mut(rs)
                .enumerate()
                .filter_map(|(i, raw)| {
                    let rec = ntfs::parse_record(raw, false).ok()?;
                    rec.in_use.then(|| ((first_record + i as u64) as u32, rec))
                })
                .collect();
            let bytes = records.iter().map(|(_, r)| r.size).sum();
            progress.add_files(records.len() as u64, bytes);
            merge(&mut table, ChunkOut { records });
        }
        reader
            .join()
            .map_err(|_| io::Error::other("MFT reader thread panicked"))?
    })?;
    Ok(table)
}

/// Streams whole records, in order, as `(first_record_number, bytes)`.
fn read_stream(
    volume: &OwnedHandle,
    geo: &Geometry,
    runs: &[Run],
    progress: &ScanProgress,
    tx: &mpsc::SyncSender<(u64, Vec<u8>)>,
) -> io::Result<()> {
    let rs = geo.record_size;
    let mut remaining = geo.valid_len;
    let mut carry: Vec<u8> = Vec::new();
    let mut next_record = 0u64;
    for run in runs {
        let Some(lcn) = run.lcn else { continue };
        let mut offset = lcn * geo.cluster;
        let mut left = (run.clusters * geo.cluster).min(remaining);
        while left > 0 {
            if progress.is_cancelled() {
                return Ok(());
            }
            let len = left.min(CHUNK_BYTES);
            let mut buf = std::mem::take(&mut carry);
            let start = buf.len();
            buf.resize(start + len as usize, 0);
            read_at(volume, offset, &mut buf[start..])?;
            offset += len;
            left -= len;
            remaining -= len;
            // Records can straddle runs when clusters are smaller than records.
            let whole = buf.len() as u64 / rs * rs;
            carry = buf.split_off(whole as usize);
            let count = whole / rs;
            if count > 0 && tx.send((next_record, buf)).is_err() {
                return Ok(()); // parser stopped
            }
            next_record += count;
        }
    }
    Ok(())
}

fn merge(table: &mut Table, chunk: ChunkOut) {
    for (number, rec) in chunk.records {
        let target = if rec.base != 0 {
            rec.base as u32
        } else {
            number
        };
        let Some(meta) = table.meta.get_mut(target as usize) else {
            continue;
        };
        if rec.base == 0 {
            meta.in_use = true;
            meta.is_dir = rec.is_dir;
            meta.modified = rec.modified;
            meta.attributes = rec.attributes;
        }
        meta.size = meta.size.max(rec.size);
        meta.allocated += rec.allocated;
        add_links(table, target, &rec.names);
    }
}

/// Adds one link per distinct parent; 8.3 short names duplicate long names.
fn add_links(table: &mut Table, child: u32, names: &[ntfs::FileName]) {
    for (i, n) in names.iter().enumerate() {
        let dup_parent = names[..i]
            .iter()
            .any(|m| m.parent == n.parent && m.namespace != NAMESPACE_DOS);
        if n.namespace == NAMESPACE_DOS
            && names
                .iter()
                .any(|m| m.parent == n.parent && m.namespace != NAMESPACE_DOS)
        {
            continue;
        }
        if dup_parent || (child as u64 == ROOT_RECORD) {
            continue;
        }
        let start = table.names.len() as u32;
        table.names.extend_from_slice(n.name.as_bytes());
        table.links.push(Link {
            parent: n.parent as u32,
            child,
            name_start: start,
            name_len: n.name.len() as u32,
        });
    }
}

/// CSR index: links grouped by parent record.
struct Children {
    offsets: Vec<u32>,
    links: Vec<Link>,
}

impl Children {
    fn of(&self, record: u32) -> &[Link] {
        let i = record as usize;
        match (self.offsets.get(i), self.offsets.get(i + 1)) {
            (Some(&a), Some(&b)) => &self.links[a as usize..b as usize],
            _ => &[],
        }
    }
}

fn index_children(table: &Table) -> Children {
    let mut links = table.links.clone();
    links.sort_unstable_by_key(|l| l.parent);
    let mut offsets = vec![0u32; table.meta.len() + 1];
    for l in &links {
        if let Some(o) = offsets.get_mut(l.parent as usize + 1) {
            *o += 1;
        }
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    Children { offsets, links }
}

fn link_name<'a>(table: &'a Table, l: &Link) -> &'a [u8] {
    &table.names[l.name_start as usize..(l.name_start + l.name_len) as usize]
}

/// Finds the record for `rel` (relative to the volume root), case-insensitively.
fn locate(table: &Table, children: &Children, rel: &Path) -> Option<u32> {
    let mut cur = ROOT_RECORD as u32;
    for comp in rel.components() {
        let Component::Normal(part) = comp else {
            continue;
        };
        let want = part.to_string_lossy().to_lowercase();
        cur = children
            .of(cur)
            .iter()
            .find(|l| String::from_utf8_lossy(link_name(table, l)).to_lowercase() == want)?
            .child;
    }
    Some(cur)
}

/// `filter` is applied while linking records into the tree, so excluded
/// folders and everything below them never enter it.
fn build(root: &Path, table: &Table, children: &Children, start: u32, filter: &Matcher) -> Tree {
    let info = |m: &Meta| {
        let reparse = m.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0;
        let kind = match (m.is_dir, reparse) {
            (true, true) => Kind::Symlink,
            (true, false) => Kind::Dir,
            (false, _) => Kind::File,
        };
        let mut flags = Flags::empty();
        if m.attributes & FILE_ATTRIBUTE_HIDDEN != 0 {
            flags.insert(Flags::HIDDEN);
        }
        EntryInfo {
            kind,
            flags,
            modified: m.modified,
            size: if m.is_dir { 0 } else { m.size },
            allocated: m.allocated,
        }
    };
    let root_meta = table.meta[start as usize];
    let mut builder = TreeBuilder::new(
        root.to_path_buf(),
        EntryInfo {
            kind: Kind::Dir,
            ..info(&root_meta)
        },
    )
    .with_capacity(table.links.len(), table.names.len());
    let mut seen = vec![false; table.meta.len()];
    seen[start as usize] = true;
    // Each queued folder carries its path relative to `root` when the filter
    // has path patterns.
    let mut queue = VecDeque::from([(start, NodeId::ROOT, String::new())]);
    while let Some((record, node, rel)) = queue.pop_front() {
        for link in children.of(record) {
            let Some(meta) = table.meta.get(link.child as usize).filter(|m| m.in_use) else {
                continue;
            };
            let mut entry = info(meta);
            let name = link_name(table, link);
            if filter.skips(&rel, name, entry.kind == Kind::Dir) {
                continue;
            }
            let first = !std::mem::replace(&mut seen[link.child as usize], true);
            if !first {
                if meta.is_dir {
                    continue; // directories cannot be hard-linked; guard against cycles
                }
                entry.flags.insert(Flags::HARDLINK_DUP);
            }
            let id = builder.push(node, name, entry);
            if entry.kind == Kind::Dir {
                let child_rel = if !filter.needs_path() {
                    String::new()
                } else if rel.is_empty() {
                    String::from_utf8_lossy(name).into_owned()
                } else {
                    format!("{rel}/{}", String::from_utf8_lossy(name))
                };
                queue.push_back((link.child, id, child_rel));
            }
        }
    }
    builder.finish()
}

fn read_at(volume: &OwnedHandle, offset: u64, buf: &mut [u8]) -> io::Result<()> {
    // SAFETY: valid handle; null "new position" pointer is allowed.
    if unsafe {
        SetFilePointerEx(
            volume.as_raw_handle(),
            offset as i64,
            std::ptr::null_mut(),
            FILE_BEGIN,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut filled = 0usize;
    while filled < buf.len() {
        let want = u32::try_from(buf.len() - filled).unwrap_or(u32::MAX);
        let mut got = 0u32;
        // SAFETY: the destination range lies within `buf`.
        let ok = unsafe {
            ReadFile(
                volume.as_raw_handle(),
                buf[filled..].as_mut_ptr(),
                want,
                &mut got,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        if got == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        filled += got as usize;
    }
    Ok(())
}

fn invalid(err: ntfs::NtfsError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err)
}
