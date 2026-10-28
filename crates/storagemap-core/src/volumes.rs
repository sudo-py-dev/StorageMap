//! Lists mounted volumes / drives with capacity, for the start screen.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VolumeKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Ram,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub mount: PathBuf,
    pub label: String,
    pub fs_type: String,
    pub kind: VolumeKind,
    pub total: u64,
    pub free: u64,
}

impl Volume {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }
}

/// Mounted, user-relevant volumes. Errors on individual volumes are skipped.
pub fn list() -> Vec<Volume> {
    imp::list()
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{Volume, VolumeKind};
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// Kernel and container filesystems nobody wants to analyse.
    const PSEUDO_FS: &[&str] = &[
        "proc",
        "sysfs",
        "devtmpfs",
        "devpts",
        "securityfs",
        "cgroup",
        "cgroup2",
        "pstore",
        "bpf",
        "debugfs",
        "tracefs",
        "mqueue",
        "hugetlbfs",
        "configfs",
        "fusectl",
        "autofs",
        "binfmt_misc",
        "efivarfs",
        "squashfs",
        "overlay",
        "nsfs",
        "ramfs",
        "rpc_pipefs",
        "fuse.portal",
        "fuse.gvfsd-fuse",
        "fuse.snapfuse",
        "selinuxfs",
        "tmpfs",
    ];
    const NETWORK_FS: &[&str] = &[
        "nfs",
        "nfs4",
        "cifs",
        "smb3",
        "smbfs",
        "sshfs",
        "fuse.sshfs",
        "9p",
        "ceph",
        "glusterfs",
    ];

    pub(super) fn list() -> Vec<Volume> {
        let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") else {
            return Vec::new();
        };
        let mut seen_sources = HashSet::new();
        let mut out = Vec::new();
        for line in info.lines() {
            let Some(m) = parse_mountinfo_line(line) else {
                continue;
            };
            if PSEUDO_FS.contains(&m.fs_type.as_str())
                || !seen_sources.insert((m.device.clone(), m.root.clone()))
            {
                continue;
            }
            let Ok(vfs) = rustix::fs::statvfs(&m.mount) else {
                continue;
            };
            let total = vfs.f_blocks * vfs.f_frsize;
            if total == 0 {
                continue;
            }
            let kind = if NETWORK_FS.contains(&m.fs_type.as_str()) {
                VolumeKind::Network
            } else if m.fs_type == "iso9660" || m.fs_type == "udf" {
                VolumeKind::Optical
            } else if m.mount.starts_with("/media") || m.mount.starts_with("/run/media") {
                VolumeKind::Removable
            } else {
                VolumeKind::Fixed
            };
            let label = m
                .mount
                .file_name()
                .map_or_else(|| "/".to_owned(), |n| n.to_string_lossy().into_owned());
            out.push(Volume {
                mount: m.mount,
                label,
                fs_type: m.fs_type,
                kind,
                total,
                free: vfs.f_bavail * vfs.f_frsize,
            });
        }
        out.sort_by(|a, b| a.mount.cmp(&b.mount));
        out
    }

    struct MountLine {
        root: String,
        mount: PathBuf,
        fs_type: String,
        device: String,
    }

    /// `36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw`
    fn parse_mountinfo_line(line: &str) -> Option<MountLine> {
        let (left, right) = line.split_once(" - ")?;
        let mut l = left.split(' ');
        let root = unescape(l.nth(3)?);
        let mount = PathBuf::from(unescape(l.next()?));
        let mut r = right.split(' ');
        let fs_type = r.next()?.to_owned();
        let device = r.next()?.to_owned();
        Some(MountLine {
            root,
            mount,
            fs_type,
            device,
        })
    }

    /// mountinfo escapes space, tab, newline and backslash as `\ooo`.
    fn unescape(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\'
                && i + 3 < b.len()
                && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c))
            {
                out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
                i += 4;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_escaped_mountinfo() {
            let m = parse_mountinfo_line(
                "36 35 98:0 / /media/My\\040Disk rw,noatime shared:1 - ext4 /dev/sdb1 rw",
            )
            .expect("parse");
            assert_eq!(m.mount, PathBuf::from("/media/My Disk"));
            assert_eq!(m.fs_type, "ext4");
            assert_eq!(m.device, "/dev/sdb1");
        }

        #[test]
        fn rejects_garbage() {
            assert!(parse_mountinfo_line("nonsense").is_none());
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::{Volume, VolumeKind};
    use std::path::PathBuf;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;
    const DRIVE_RAMDISK: u32 = 6;
    const MAX_LABEL: usize = 261;

    pub(super) fn list() -> Vec<Volume> {
        // SAFETY: no arguments; returns a bitmask of present drive letters.
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .filter_map(|i| volume(char::from(b'A' + i)))
            .collect()
    }

    fn volume(letter: char) -> Option<Volume> {
        let root = format!("{letter}:\\");
        let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `wide` is NUL-terminated and outlives each call; all out
        // pointers reference live stack/heap buffers of the stated length.
        unsafe {
            let kind = match GetDriveTypeW(wide.as_ptr()) {
                DRIVE_FIXED => VolumeKind::Fixed,
                DRIVE_REMOVABLE => VolumeKind::Removable,
                DRIVE_REMOTE => VolumeKind::Network,
                DRIVE_CDROM => VolumeKind::Optical,
                DRIVE_RAMDISK => VolumeKind::Ram,
                _ => VolumeKind::Unknown,
            };
            let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
            if GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free) == 0 {
                return None;
            }
            let mut label = vec![0u16; MAX_LABEL];
            let mut fs = vec![0u16; MAX_LABEL];
            let ok = GetVolumeInformationW(
                wide.as_ptr(),
                label.as_mut_ptr(),
                MAX_LABEL as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs.as_mut_ptr(),
                MAX_LABEL as u32,
            ) != 0;
            let text = |buf: &[u16]| {
                String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)])
            };
            Some(Volume {
                mount: PathBuf::from(&root),
                label: if ok { text(&label) } else { String::new() },
                fs_type: if ok { text(&fs) } else { String::new() },
                kind,
                total,
                free,
            })
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    pub(super) fn list() -> Vec<super::Volume> {
        Vec::new()
    }
}
