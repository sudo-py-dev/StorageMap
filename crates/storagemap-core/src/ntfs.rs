//! NTFS Master File Table record parsing.
//!
//! Platform independent so it can be unit-tested anywhere; only reading the
//! raw volume (in `backend::windows::mft`) needs Windows. All offsets come
//! from untrusted on-disk data and are bounds-checked.

/// Record number of the volume's root directory.
pub const ROOT_RECORD: u64 = 5;
/// `$FILE_NAME` namespace for 8.3 short names, which duplicate a long name.
pub const NAMESPACE_DOS: u8 = 2;
/// Lower 48 bits of a file reference are the record number.
pub const RECORD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

const SIGNATURE: &[u8; 4] = b"FILE";
/// Update sequence arrays protect the last two bytes of every 512-byte stride.
const FIXUP_STRIDE: usize = 512;
const FLAG_IN_USE: u16 = 0x0001;
const FLAG_DIRECTORY: u16 = 0x0002;

const ATTR_STANDARD_INFORMATION: u32 = 0x10;
const ATTR_ATTRIBUTE_LIST: u32 = 0x20;
const ATTR_FILE_NAME: u32 = 0x30;
const ATTR_DATA: u32 = 0x80;
const ATTR_INDEX_ALLOCATION: u32 = 0xA0;
const ATTR_END: u32 = 0xFFFF_FFFF;

const ATTR_FLAG_COMPRESSED: u16 = 0x0001;
const ATTR_FLAG_SPARSE: u16 = 0x8000;

/// Seconds between 1601-01-01 and 1970-01-01.
const FILETIME_EPOCH_DELTA: i64 = 11_644_473_600;
const FILETIME_TICKS_PER_SEC: u64 = 10_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NtfsError {
    #[error("record has no FILE signature")]
    BadSignature,
    #[error("update sequence mismatch (torn write)")]
    BadFixup,
    #[error("record structure out of bounds")]
    Truncated,
    #[error("malformed data run list")]
    BadRunlist,
}

/// One `$FILE_NAME` (a hard link has several).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName {
    pub parent: u64,
    pub name: String,
    pub namespace: u8,
}

/// A contiguous run of clusters; `lcn == None` is a sparse hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub lcn: Option<u64>,
    pub clusters: u64,
}

/// An `$ATTRIBUTE_LIST` entry pointing at an extension record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListEntry {
    pub attr_type: u32,
    pub lowest_vcn: u64,
    pub record: u64,
}

/// Everything StorageMap needs from one MFT record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub in_use: bool,
    pub is_dir: bool,
    /// Base record for extension records, 0 for base records.
    pub base: u64,
    pub names: Vec<FileName>,
    /// Logical size of the unnamed data stream.
    pub size: u64,
    /// Bytes allocated on disk for all streams and directory indexes.
    pub allocated: u64,
    /// Unix seconds.
    pub modified: i64,
    pub accessed: i64,
    pub created: i64,
    pub attributes: u32,
    pub has_std_info: bool,
    /// Unnamed `$DATA` runs as `(lowest_vcn, runs)`; only when requested.
    pub data_runs: Vec<(u64, Vec<Run>)>,
    /// Resident attribute list; only when requested.
    pub attribute_list: Vec<ListEntry>,
    pub has_nonresident_attribute_list: bool,
}

impl Record {
    /// Folds an extension record's attributes into this base record.
    pub fn merge_extension(&mut self, ext: Record) {
        self.names.extend(ext.names);
        self.size = self.size.max(ext.size);
        self.allocated += ext.allocated;
        self.data_runs.extend(ext.data_runs);
        if !self.has_std_info && ext.has_std_info {
            self.modified = ext.modified;
            self.accessed = ext.accessed;
            self.created = ext.created;
            self.attributes = ext.attributes;
            self.has_std_info = true;
        }
    }
}

/// Converts a Windows FILETIME (100 ns ticks since 1601) to Unix seconds.
pub fn filetime_to_unix(ft: u64) -> i64 {
    (ft / FILETIME_TICKS_PER_SEC) as i64 - FILETIME_EPOCH_DELTA
}

/// Restores the sector-end bytes replaced by the update sequence array.
pub fn apply_fixups(buf: &mut [u8]) -> Result<(), NtfsError> {
    if buf.get(..4) != Some(SIGNATURE) {
        return Err(NtfsError::BadSignature);
    }
    let usa_off = usize::from(u16_at(buf, 4)?);
    let usa_count = usize::from(u16_at(buf, 6)?);
    if usa_count == 0 || (usa_count - 1) * FIXUP_STRIDE > buf.len() {
        return Err(NtfsError::Truncated);
    }
    let usn = u16_at(buf, usa_off)?;
    for i in 1..usa_count {
        let end = i * FIXUP_STRIDE - 2;
        if u16_at(buf, end)? != usn {
            return Err(NtfsError::BadFixup);
        }
        let saved = u16_at(buf, usa_off + 2 * i)?;
        buf[end..end + 2].copy_from_slice(&saved.to_le_bytes());
    }
    Ok(())
}

/// Parses one raw MFT record in place (fixups are applied to `buf`).
/// `want_layout` also collects `$DATA` runs and the attribute list, needed
/// only to locate the `$MFT` itself.
pub fn parse_record(buf: &mut [u8], want_layout: bool) -> Result<Record, NtfsError> {
    apply_fixups(buf)?;
    let buf: &[u8] = buf;
    let flags = u16_at(buf, 22)?;
    let mut rec = Record {
        in_use: flags & FLAG_IN_USE != 0,
        is_dir: flags & FLAG_DIRECTORY != 0,
        base: u64_at(buf, 32)? & RECORD_MASK,
        ..Record::default()
    };
    if !rec.in_use {
        return Ok(rec);
    }
    let used = (u32_at(buf, 24)? as usize).min(buf.len());
    let mut off = usize::from(u16_at(buf, 20)?);
    while off + 8 <= used {
        let attr_type = u32_at(buf, off)?;
        if attr_type == ATTR_END {
            break;
        }
        let len = u32_at(buf, off + 4)? as usize;
        if len < 16 || off + len > used {
            return Err(NtfsError::Truncated);
        }
        parse_attribute(&buf[off..off + len], attr_type, want_layout, &mut rec)?;
        off += len;
    }
    Ok(rec)
}

fn parse_attribute(
    attr: &[u8],
    attr_type: u32,
    want_layout: bool,
    rec: &mut Record,
) -> Result<(), NtfsError> {
    let non_resident = attr[8] != 0;
    let named = attr[9] != 0;
    let flags = u16_at(attr, 12)?;
    match (attr_type, non_resident) {
        (ATTR_STANDARD_INFORMATION, false) => {
            let v = resident_value(attr)?;
            // Creation, modification, MFT change and access times.
            rec.created = filetime_to_unix(u64_at(v, 0)?);
            rec.modified = filetime_to_unix(u64_at(v, 8)?);
            rec.accessed = filetime_to_unix(u64_at(v, 24)?);
            rec.attributes = u32_at(v, 32)?;
            rec.has_std_info = true;
        }
        (ATTR_FILE_NAME, false) => {
            let v = resident_value(attr)?;
            let len = usize::from(*v.get(64).ok_or(NtfsError::Truncated)?);
            let raw = v.get(66..66 + 2 * len).ok_or(NtfsError::Truncated)?;
            let units: Vec<u16> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            rec.names.push(FileName {
                parent: u64_at(v, 0)? & RECORD_MASK,
                name: String::from_utf16_lossy(&units),
                namespace: v[65],
            });
        }
        (ATTR_DATA, false) => {
            let len = u64::from(u32_at(attr, 16)?);
            if !named {
                rec.size = len;
            }
        }
        (ATTR_DATA | ATTR_INDEX_ALLOCATION, true) => {
            let lowest_vcn = u64_at(attr, 16)?;
            if lowest_vcn == 0 {
                let compressed = flags & (ATTR_FLAG_COMPRESSED | ATTR_FLAG_SPARSE) != 0;
                rec.allocated += if compressed {
                    u64_at(attr, 64)?
                } else {
                    u64_at(attr, 40)?
                };
                if attr_type == ATTR_DATA && !named {
                    rec.size = u64_at(attr, 48)?;
                }
            }
            if want_layout && attr_type == ATTR_DATA && !named {
                let runs_off = usize::from(u16_at(attr, 32)?);
                rec.data_runs.push((
                    lowest_vcn,
                    parse_runlist(attr.get(runs_off..).ok_or(NtfsError::Truncated)?)?,
                ));
            }
        }
        (ATTR_ATTRIBUTE_LIST, false) if want_layout => {
            rec.attribute_list = parse_attribute_list(resident_value(attr)?);
        }
        (ATTR_ATTRIBUTE_LIST, true) => rec.has_nonresident_attribute_list = true,
        _ => {}
    }
    Ok(())
}

fn resident_value(attr: &[u8]) -> Result<&[u8], NtfsError> {
    let len = u32_at(attr, 16)? as usize;
    let off = usize::from(u16_at(attr, 20)?);
    attr.get(off..off + len).ok_or(NtfsError::Truncated)
}

/// Decodes a mapping-pairs run list into absolute cluster runs.
pub fn parse_runlist(data: &[u8]) -> Result<Vec<Run>, NtfsError> {
    let mut runs = Vec::new();
    let mut pos = 0;
    let mut lcn: i64 = 0;
    while let Some(&header) = data.get(pos) {
        if header == 0 {
            return Ok(runs);
        }
        let (len_size, off_size) = (usize::from(header & 0x0F), usize::from(header >> 4));
        if len_size == 0 || len_size > 8 || off_size > 8 {
            return Err(NtfsError::BadRunlist);
        }
        let len_bytes = data
            .get(pos + 1..pos + 1 + len_size)
            .ok_or(NtfsError::BadRunlist)?;
        let clusters = le_unsigned(len_bytes);
        let off_start = pos + 1 + len_size;
        let run_lcn = if off_size == 0 {
            None
        } else {
            let delta = le_signed(
                data.get(off_start..off_start + off_size)
                    .ok_or(NtfsError::BadRunlist)?,
            );
            lcn = lcn
                .checked_add(delta)
                .filter(|v| *v >= 0)
                .ok_or(NtfsError::BadRunlist)?;
            Some(lcn as u64)
        };
        runs.push(Run {
            lcn: run_lcn,
            clusters,
        });
        pos = off_start + off_size;
    }
    Err(NtfsError::BadRunlist)
}

/// Parses a resident `$ATTRIBUTE_LIST` value.
pub fn parse_attribute_list(value: &[u8]) -> Vec<ListEntry> {
    let mut out = Vec::new();
    let mut off = 0;
    while off + 26 <= value.len() {
        let (Ok(attr_type), Ok(len), Ok(vcn), Ok(reference)) = (
            u32_at(value, off),
            u16_at(value, off + 4),
            u64_at(value, off + 8),
            u64_at(value, off + 16),
        ) else {
            break;
        };
        if len == 0 {
            break;
        }
        out.push(ListEntry {
            attr_type,
            lowest_vcn: vcn,
            record: reference & RECORD_MASK,
        });
        off += usize::from(len);
    }
    out
}

fn le_unsigned(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0, |acc, &b| (acc << 8) | u64::from(b))
}

fn le_signed(bytes: &[u8]) -> i64 {
    let raw = le_unsigned(bytes);
    let shift = 64 - 8 * bytes.len() as u32;
    ((raw << shift) as i64) >> shift
}

fn u16_at(b: &[u8], off: usize) -> Result<u16, NtfsError> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(NtfsError::Truncated)
}

fn u32_at(b: &[u8], off: usize) -> Result<u32, NtfsError> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(NtfsError::Truncated)
}

fn u64_at(b: &[u8], off: usize) -> Result<u64, NtfsError> {
    b.get(off..off + 8)
        .and_then(|s| s.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(NtfsError::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORD_SIZE: usize = 1024;
    const USN: u16 = 0x0042;

    /// Builds a 1 KiB FILE record with the given attributes, protected by
    /// an update sequence array like a real on-disk record.
    fn record(flags: u16, base: u64, attrs: &[Vec<u8>]) -> Vec<u8> {
        let mut b = vec![0u8; RECORD_SIZE];
        b[..4].copy_from_slice(SIGNATURE);
        b[4..6].copy_from_slice(&48u16.to_le_bytes()); // usa offset
        b[6..8].copy_from_slice(&3u16.to_le_bytes()); // usn + 2 sectors
        b[20..22].copy_from_slice(&56u16.to_le_bytes()); // first attribute
        b[22..24].copy_from_slice(&flags.to_le_bytes());
        b[32..40].copy_from_slice(&base.to_le_bytes());
        let mut off = 56;
        for a in attrs {
            b[off..off + a.len()].copy_from_slice(a);
            off += a.len();
        }
        b[off..off + 4].copy_from_slice(&ATTR_END.to_le_bytes());
        b[24..28].copy_from_slice(&((off + 8) as u32).to_le_bytes());
        // Protect sector ends: save real bytes into the USA, write the USN.
        b[48..50].copy_from_slice(&USN.to_le_bytes());
        for i in 1..3 {
            let end = i * FIXUP_STRIDE - 2;
            let saved = [b[end], b[end + 1]];
            b[48 + 2 * i..50 + 2 * i].copy_from_slice(&saved);
            b[end..end + 2].copy_from_slice(&USN.to_le_bytes());
        }
        b
    }

    fn resident(attr_type: u32, name_len: u8, value: &[u8]) -> Vec<u8> {
        let len = (24 + value.len()).next_multiple_of(8);
        let mut a = vec![0u8; len];
        a[..4].copy_from_slice(&attr_type.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[9] = name_len;
        a[16..20].copy_from_slice(&(value.len() as u32).to_le_bytes());
        a[20..22].copy_from_slice(&24u16.to_le_bytes());
        a[24..24 + value.len()].copy_from_slice(value);
        a
    }

    fn non_resident_data(alloc: u64, size: u64, runs: &[u8]) -> Vec<u8> {
        let len = (64 + runs.len()).next_multiple_of(8);
        let mut a = vec![0u8; len];
        a[..4].copy_from_slice(&ATTR_DATA.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 1;
        a[32..34].copy_from_slice(&64u16.to_le_bytes());
        a[40..48].copy_from_slice(&alloc.to_le_bytes());
        a[48..56].copy_from_slice(&size.to_le_bytes());
        a[64..64 + runs.len()].copy_from_slice(runs);
        a
    }

    fn file_name(parent: u64, name: &str, namespace: u8) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut v = vec![0u8; 66 + 2 * units.len()];
        v[..8].copy_from_slice(&(parent | (7 << 48)).to_le_bytes()); // sequence bits must be masked
        v[64] = units.len() as u8;
        v[65] = namespace;
        for (i, u) in units.iter().enumerate() {
            v[66 + 2 * i..68 + 2 * i].copy_from_slice(&u.to_le_bytes());
        }
        resident(ATTR_FILE_NAME, 0, &v)
    }

    fn std_info(filetime: u64, attrs: u32) -> Vec<u8> {
        // Creation one second earlier, access one second later.
        let tick = FILETIME_TICKS_PER_SEC;
        let mut v = vec![0u8; 48];
        v[0..8].copy_from_slice(&(filetime - tick).to_le_bytes());
        v[8..16].copy_from_slice(&filetime.to_le_bytes());
        v[24..32].copy_from_slice(&(filetime + tick).to_le_bytes());
        v[32..36].copy_from_slice(&attrs.to_le_bytes());
        resident(ATTR_STANDARD_INFORMATION, 0, &v)
    }

    #[test]
    fn parses_resident_file() {
        let unix_2020 = (1_577_836_800 + FILETIME_EPOCH_DELTA) as u64 * FILETIME_TICKS_PER_SEC;
        let mut b = record(
            FLAG_IN_USE,
            0,
            &[
                std_info(unix_2020, 0x20),
                file_name(ROOT_RECORD, "שלום.txt", 1),
                resident(ATTR_DATA, 0, b"hello"),
            ],
        );
        let r = parse_record(&mut b, false).expect("parse");
        assert!(r.in_use && !r.is_dir);
        assert_eq!(
            r.names,
            vec![FileName {
                parent: ROOT_RECORD,
                name: "שלום.txt".into(),
                namespace: 1
            }]
        );
        assert_eq!(r.size, 5);
        assert_eq!(r.allocated, 0);
        assert_eq!(r.modified, 1_577_836_800);
        assert_eq!(r.created, 1_577_836_799);
        assert_eq!(r.accessed, 1_577_836_801);
        assert_eq!(r.attributes, 0x20);
    }

    #[test]
    fn parses_non_resident_data_and_runs() {
        // One run: 0x10 clusters at LCN 0x2000.
        let runs = [0x21, 0x10, 0x00, 0x20, 0x00];
        let mut b = record(
            FLAG_IN_USE,
            0,
            &[
                file_name(ROOT_RECORD, "big.bin", 1),
                non_resident_data(65_536, 60_000, &runs),
            ],
        );
        let r = parse_record(&mut b, true).expect("parse");
        assert_eq!((r.size, r.allocated), (60_000, 65_536));
        assert_eq!(
            r.data_runs,
            vec![(
                0,
                vec![Run {
                    lcn: Some(0x2000),
                    clusters: 0x10
                }]
            )]
        );
    }

    #[test]
    fn named_streams_count_as_allocated_only() {
        let mut b = record(
            FLAG_IN_USE,
            0,
            &[
                resident(ATTR_DATA, 4, b"zone-id"),
                resident(ATTR_DATA, 0, b"ab"),
            ],
        );
        let r = parse_record(&mut b, false).expect("parse");
        assert_eq!(r.size, 2);
    }

    #[test]
    fn extension_and_free_records() {
        let mut ext = record(FLAG_IN_USE, 42 | (3 << 48), &[]);
        assert_eq!(parse_record(&mut ext, false).expect("parse").base, 42);
        let mut free = record(0, 0, &[file_name(ROOT_RECORD, "gone", 1)]);
        let r = parse_record(&mut free, false).expect("parse");
        assert!(!r.in_use && r.names.is_empty());
    }

    #[test]
    fn detects_torn_records_and_bad_signatures() {
        let mut b = record(FLAG_IN_USE, 0, &[]);
        b[FIXUP_STRIDE - 2] ^= 0xFF;
        assert_eq!(parse_record(&mut b, false), Err(NtfsError::BadFixup));
        let mut z = vec![0u8; RECORD_SIZE];
        assert_eq!(parse_record(&mut z, false), Err(NtfsError::BadSignature));
    }

    #[test]
    fn rejects_attribute_overrun() {
        let mut b = record(FLAG_IN_USE, 0, &[resident(ATTR_DATA, 0, b"x")]);
        b[56 + 4..56 + 8].copy_from_slice(&5000u32.to_le_bytes());
        assert_eq!(parse_record(&mut b, false), Err(NtfsError::Truncated));
    }

    #[test]
    fn runlist_relative_negative_and_sparse() {
        // +0x100 len 8, then sparse len 4, then -0x80 len 2.
        let data = [0x21, 0x08, 0x00, 0x01, 0x01, 0x04, 0x11, 0x02, 0x80, 0x00];
        let runs = parse_runlist(&data).expect("runs");
        assert_eq!(
            runs,
            vec![
                Run {
                    lcn: Some(0x100),
                    clusters: 8
                },
                Run {
                    lcn: None,
                    clusters: 4
                },
                Run {
                    lcn: Some(0x80),
                    clusters: 2
                },
            ]
        );
        assert_eq!(parse_runlist(&[0x21, 0x08]), Err(NtfsError::BadRunlist));
        assert_eq!(
            parse_runlist(&[0x11, 0x01, 0x80, 0x00]),
            Err(NtfsError::BadRunlist)
        ); // negative LCN
    }

    #[test]
    fn attribute_list_entries() {
        let mut v = vec![0u8; 32];
        v[..4].copy_from_slice(&ATTR_DATA.to_le_bytes());
        v[4..6].copy_from_slice(&32u16.to_le_bytes());
        v[8..16].copy_from_slice(&100u64.to_le_bytes());
        v[16..24].copy_from_slice(&(17u64 | (1 << 48)).to_le_bytes());
        assert_eq!(
            parse_attribute_list(&v),
            vec![ListEntry {
                attr_type: ATTR_DATA,
                lowest_vcn: 100,
                record: 17
            }]
        );
    }
}
