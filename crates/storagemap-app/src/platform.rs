//! OS integration: file manager hooks, elevation, consoles.

use std::io;
use std::path::Path;

/// Where snapshots are kept (per user, next to the app's settings).
pub fn snapshot_dir() -> Option<std::path::PathBuf> {
    eframe::storage_dir("storagemap").map(|d| d.join("snapshots"))
}

/// Where a path sits relative to the system trash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashLocation {
    /// Not a trash folder.
    Outside,
    /// A trash folder itself: `~/.local/share/Trash`, `.Trash-1000`, `$Recycle.Bin`.
    Root,
    /// Something already in the trash.
    Inside,
}

/// Classifies `path`. Moving a trash folder (or its contents) to the trash
/// is meaningless and fails, so the UI offers "Empty trash" instead.
pub fn trash_location(path: &Path) -> TrashLocation {
    let names: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let is_root = |i: usize| {
        let name = &names[i];
        name.eq_ignore_ascii_case("$Recycle.Bin")
            || name.starts_with(".Trash")
            || (name == "Trash" && i > 0 && names[i - 1] == "share")
    };
    match (0..names.len()).find(|&i| is_root(i)) {
        Some(i) if i + 1 == names.len() => TrashLocation::Root,
        Some(_) => TrashLocation::Inside,
        None => TrashLocation::Outside,
    }
}

/// The account name of a Unix user id, from `/etc/passwd` (read once).
#[cfg(unix)]
pub fn user_name(uid: u32) -> Option<String> {
    static NAMES: std::sync::OnceLock<std::collections::HashMap<u32, String>> =
        std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            std::fs::read_to_string("/etc/passwd")
                .map(|text| parse_passwd(&text))
                .unwrap_or_default()
        })
        .get(&uid)
        .cloned()
}

#[cfg(not(unix))]
pub fn user_name(_uid: u32) -> Option<String> {
    None
}

/// `name:password:uid:...` lines to a uid → name map.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_passwd(text: &str) -> std::collections::HashMap<u32, String> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let uid = fields.nth(1)?.parse().ok()?;
            Some((uid, name.to_owned()))
        })
        .collect()
}

/// Permanently deletes everything in the trash folder `root` (a path that
/// `trash_location` reports as `Root`); returns the number of items removed.
/// The folder itself and its `files`/`info` layout are kept.
#[cfg(not(windows))]
pub fn empty_trash(root: &Path) -> io::Result<usize> {
    let mut count = 0;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            // freedesktop layout: purge `files/` and `info/` but keep them.
            let keep = matches!(entry.file_name().to_str(), Some("files" | "info"));
            if keep {
                for item in std::fs::read_dir(&path)? {
                    remove_any(&item?.path())?;
                    count += 1;
                }
            } else {
                std::fs::remove_dir_all(&path)?;
                count += 1;
            }
        } else {
            std::fs::remove_file(&path)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(not(windows))]
fn remove_any(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path)?.is_dir() {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    }
}

/// Permanently deletes everything in the Recycle Bin; returns the number of
/// items removed.
#[cfg(windows)]
pub fn empty_trash(_root: &Path) -> io::Result<usize> {
    let items = trash::os_limited::list().map_err(io::Error::other)?;
    let count = items.len();
    trash::os_limited::purge_all(items).map_err(io::Error::other)?;
    Ok(count)
}

/// Opens a file or folder with the default application.
pub fn open(path: &Path) -> io::Result<()> {
    open::that_detached(path)
}

/// Shows `path` selected in the system file manager.
pub fn reveal(path: &Path) -> io::Result<()> {
    imp::reveal(path)
}

/// Adds (or removes) an "Open with StorageMap" entry for folders in the
/// file manager, for the current user only; no admin rights needed.
pub fn set_file_manager_integration(install: bool, label: &str) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    imp::set_integration(install, label, &exe)
}

/// Whether the process can read raw volumes (Windows administrator).
pub fn is_elevated() -> bool {
    imp::is_elevated()
}

/// Whether elevating would unlock a faster scanner on this OS.
pub const ELEVATION_HELPS: bool = cfg!(windows);

/// Restarts the app elevated, opening `scan_path` if given.
pub fn relaunch_elevated(scan_path: Option<&Path>) -> io::Result<()> {
    imp::relaunch_elevated(scan_path)
}

/// Lets a GUI-subsystem binary print to the console it was started from.
pub fn attach_console() {
    imp::attach_console();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_passwd() {
        let names = parse_passwd(
            "root:x:0:0:root:/root:/bin/bash\n# comment\nbad\nana:x:1000:1000::/home/ana:/bin/sh\n",
        );
        assert_eq!(names.get(&0).map(String::as_str), Some("root"));
        assert_eq!(names.get(&1000).map(String::as_str), Some("ana"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn recognises_trash_folders() {
        assert_eq!(
            trash_location(Path::new("/home/u/.local/share/Trash")),
            TrashLocation::Root
        );
        assert_eq!(
            trash_location(Path::new("/home/u/.local/share/Trash/files/a.txt")),
            TrashLocation::Inside
        );
        assert_eq!(
            trash_location(Path::new("/media/usb/.Trash-1000")),
            TrashLocation::Root
        );
        #[cfg(windows)]
        assert_eq!(
            trash_location(Path::new("C:\\$Recycle.Bin")),
            TrashLocation::Root
        );
        assert_eq!(
            trash_location(Path::new("/home/u/Documents/Trash")),
            TrashLocation::Outside
        );
        assert_eq!(
            trash_location(Path::new("/home/u/Documents")),
            TrashLocation::Outside
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn empties_trash_folder_in_place() {
        let root =
            std::env::temp_dir().join(format!("storagemap-trash-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("files/dir/sub")).expect("trash test");
        std::fs::create_dir_all(root.join("info")).expect("trash test");
        std::fs::create_dir_all(root.join("expunged/x")).expect("trash test");
        std::fs::write(root.join("files/a.txt"), "a").expect("trash test");
        std::fs::write(root.join("files/dir/sub/b.txt"), "b").expect("trash test");
        std::fs::write(root.join("info/a.txt.trashinfo"), "i").expect("trash test");
        std::fs::write(root.join("directorysizes"), "").expect("trash test");
        assert_eq!(empty_trash(&root).expect("trash test"), 5);
        assert_eq!(
            std::fs::read_dir(root.join("files"))
                .expect("trash test")
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_dir(root.join("info"))
                .expect("trash test")
                .count(),
            0
        );
        assert_eq!(std::fs::read_dir(&root).expect("trash test").count(), 2);
        std::fs::remove_dir_all(&root).expect("trash test");
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
    };
    use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Classes that get the context-menu verb: folders, drives, folder background.
    const SHELL_KEYS: [&str; 3] = [
        r"Software\Classes\Directory\shell\StorageMap",
        r"Software\Classes\Drive\shell\StorageMap",
        r"Software\Classes\Directory\Background\shell\StorageMap",
    ];
    /// ShellExecute returns a value > 32 on success.
    const SHELL_EXECUTE_OK: isize = 32;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub(super) fn reveal(path: &Path) -> io::Result<()> {
        Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
            .map(drop)
    }

    pub(super) fn is_elevated() -> bool {
        // SAFETY: no arguments, no side effects.
        unsafe { IsUserAnAdmin() != 0 }
    }

    pub(super) fn attach_console() {
        // SAFETY: failing (no parent console) is harmless.
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }

    pub(super) fn relaunch_elevated(scan_path: Option<&Path>) -> io::Result<()> {
        let exe = std::env::current_exe()?;
        let params = scan_path
            .map(|p| format!("\"{}\"", p.display().to_string().trim_end_matches('\\')))
            .unwrap_or_default();
        let (verb, file, params) = (wide("runas"), wide(&exe.to_string_lossy()), wide(&params));
        // SAFETY: all strings are NUL-terminated and outlive the call.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize > SHELL_EXECUTE_OK {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn set_integration(install: bool, label: &str, exe: &Path) -> io::Result<()> {
        for key in SHELL_KEYS {
            if install {
                let arg = if key.contains("Background") {
                    "%V"
                } else {
                    "%1"
                };
                write_key(
                    key,
                    &[(None, label), (Some("Icon"), &exe.to_string_lossy())],
                )?;
                write_key(
                    &format!(r"{key}\command"),
                    &[(None, &format!("\"{}\" \"{arg}\"", exe.display()))],
                )?;
            } else {
                let name = wide(key);
                // SAFETY: valid predefined root key and NUL-terminated path.
                let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, name.as_ptr()) };
                if status != ERROR_SUCCESS && status != 2 {
                    return Err(io::Error::from_raw_os_error(status as i32));
                }
            }
        }
        Ok(())
    }

    fn write_key(path: &str, values: &[(Option<&str>, &str)]) -> io::Result<()> {
        let name = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: out-pointer is a live HKEY; strings are NUL-terminated.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                name.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let result = values.iter().try_for_each(|(value_name, data)| {
            let vname = value_name.map(wide);
            let data = wide(data);
            // SAFETY: `key` is open for writing; data length is in bytes incl. NUL.
            let status = unsafe {
                RegSetValueExW(
                    key,
                    vname.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
                    0,
                    REG_SZ,
                    data.as_ptr().cast(),
                    (data.len() * 2) as u32,
                )
            };
            if status == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(status as i32))
            }
        });
        // SAFETY: closing the key we opened above.
        unsafe { RegCloseKey(key) };
        result
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    const DESKTOP_FILE: &str = "storagemap.desktop";

    pub(super) fn reveal(path: &Path) -> io::Result<()> {
        // freedesktop FileManager1 selects the item in Nautilus, Dolphin,
        // Nemo, Caja, Thunar...; fall back to opening the parent folder.
        let uri = format!("array:string:{}", file_uri(path));
        let shown = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.FileManager1",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
            ])
            .arg(uri)
            .arg("string:")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if shown {
            Ok(())
        } else {
            open::that_detached(path.parent().unwrap_or(path))
        }
    }

    /// `file://` URI with RFC 3986 percent-encoding of the raw path bytes.
    fn file_uri(path: &Path) -> String {
        use std::os::unix::ffi::OsStrExt;
        let mut uri = String::from("file://");
        for &b in path.as_os_str().as_bytes() {
            if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
                uri.push(char::from(b));
            } else {
                uri.push_str(&format!("%{b:02X}"));
            }
        }
        uri
    }

    pub(super) fn is_elevated() -> bool {
        false
    }

    pub(super) fn attach_console() {}

    pub(super) fn relaunch_elevated(_: Option<&Path>) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn applications_dir() -> io::Result<PathBuf> {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        Ok(data.join("applications"))
    }

    /// Desktop Entry spec: quote the program path and escape `" ` $ \`.
    fn exec_quote(path: &Path) -> String {
        let mut out = String::from("\"");
        for c in path.to_string_lossy().chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    }

    pub(super) fn set_integration(install: bool, label: &str, exe: &Path) -> io::Result<()> {
        let dir = applications_dir()?;
        let target = dir.join(DESKTOP_FILE);
        if !install {
            return match std::fs::remove_file(&target) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        std::fs::create_dir_all(&dir)?;
        let label = label.replace(['\n', '\r'], " ");
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=StorageMap\nGenericName={label}\nComment={label}\n\
             Exec={} %f\nIcon=storagemap\nTerminal=false\nCategories=System;Utility;Filesystem;\n\
             MimeType=inode/directory;\nKeywords=disk;space;usage;treemap;\n",
            exec_quote(exe)
        );
        // Atomic replace so a crash never leaves a half-written entry.
        let tmp = dir.join(format!(".{DESKTOP_FILE}.tmp"));
        std::fs::File::create(&tmp)?.write_all(entry.as_bytes())?;
        std::fs::rename(&tmp, &target)?;
        // Best effort: refresh the MIME cache so "Open With" sees it now.
        let _ = Command::new("update-desktop-database")
            .arg(&dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn uri_encoding() {
            assert_eq!(
                file_uri(Path::new("/home/a b/שלום")),
                "file:///home/a%20b/%D7%A9%D7%9C%D7%95%D7%9D"
            );
        }

        #[test]
        fn exec_quoting() {
            assert_eq!(
                exec_quote(Path::new("/opt/my $app/bin")),
                "\"/opt/my \\$app/bin\""
            );
        }
    }
}
