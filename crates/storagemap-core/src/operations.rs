//! Acting on many files: moving, copying, zipping and renaming.
//!
//! Every operation refuses to overwrite: an existing destination is an
//! error for that item, and the others go ahead.

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use zip::write::SimpleFileOptions;

fn exists_error(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("{} already exists", path.display()),
    )
}

/// `dest_dir/<name of src>`, refusing a folder into itself and existing names.
fn target(src: &Path, dest_dir: &Path) -> io::Result<PathBuf> {
    let name = src
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    if dest_dir.starts_with(src) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot put a folder inside itself",
        ));
    }
    let to = dest_dir.join(name);
    if fs::symlink_metadata(&to).is_ok() {
        return Err(exists_error(&to));
    }
    Ok(to)
}

/// Moves `src` into the folder `dest_dir`; returns its new path. Across
/// drives it is copied, then the original is deleted.
pub fn move_into(src: &Path, dest_dir: &Path) -> io::Result<PathBuf> {
    let to = target(src, dest_dir)?;
    match fs::rename(src, &to) {
        Ok(()) => Ok(to),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            copy_tree(src, &to)?;
            if fs::symlink_metadata(src)?.is_dir() {
                fs::remove_dir_all(src)?;
            } else {
                fs::remove_file(src)?;
            }
            Ok(to)
        }
        Err(e) => Err(e),
    }
}

/// Copies `src` (a file or a whole folder) into `dest_dir`; returns the copy's path.
pub fn copy_into(src: &Path, dest_dir: &Path) -> io::Result<PathBuf> {
    let to = target(src, dest_dir)?;
    copy_tree(src, &to)?;
    Ok(to)
}

fn copy_tree(src: &Path, to: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.is_symlink() {
        copy_link(src, to)
    } else if meta.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if fs::symlink_metadata(to).is_ok() {
            return Err(exists_error(to));
        }
        fs::copy(src, to).map(drop)
    }
}

/// Links are recreated as links, not followed.
#[cfg(unix)]
fn copy_link(src: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, to)
}

#[cfg(not(unix))]
fn copy_link(src: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!("{} is a link; links are not copied", src.display()),
    ))
}

/// Writes `items` (files and whole folders) into a new zip archive. Each
/// item sits at the top of the archive under its own name. `done` counts
/// the bytes read, for progress.
pub fn zip_items(archive: &Path, items: &[PathBuf], done: &AtomicU64) -> io::Result<()> {
    let file = File::create_new(archive)?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    // Large files need ZIP64 headers; deciding up front keeps it simple.
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    let result = (|| {
        for item in items {
            let name = item
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?
                .to_string_lossy()
                .into_owned();
            add_to_zip(&mut zip, archive, item, &name, options, done)?;
        }
        zip.finish().map_err(io::Error::other)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(archive);
    }
    result
}

fn add_to_zip<W: io::Write + io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    archive: &Path,
    path: &Path,
    name: &str,
    options: SimpleFileOptions,
    done: &AtomicU64,
) -> io::Result<()> {
    if path == archive {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        zip.add_directory(format!("{name}/"), options)
            .map_err(io::Error::other)?;
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let child = format!("{name}/{}", entry.file_name().to_string_lossy());
            add_to_zip(zip, archive, &entry.path(), &child, options, done)?;
        }
    } else if meta.is_file() {
        zip.start_file(name, options).map_err(io::Error::other)?;
        let copied = io::copy(&mut File::open(path)?, zip)?;
        done.fetch_add(copied, Relaxed);
    }
    // Links and special files are left out.
    Ok(())
}

/// Find-and-replace in file names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenameRule {
    pub find: String,
    pub replace: String,
    /// When off, letters A–Z match either case (other scripts have no case
    /// or are matched exactly).
    pub match_case: bool,
}

impl RenameRule {
    /// `name` with every occurrence of `find` replaced.
    pub fn apply(&self, name: &str) -> String {
        if self.find.is_empty() {
            return name.to_owned();
        }
        if self.match_case {
            return name.replace(&self.find, &self.replace);
        }
        // ASCII lower-casing keeps byte offsets, so matches map back exactly.
        let (hay, needle) = (name.to_ascii_lowercase(), self.find.to_ascii_lowercase());
        let mut out = String::with_capacity(name.len());
        let mut last = 0;
        for (at, _) in hay.match_indices(&needle) {
            out.push_str(&name[last..at]);
            out.push_str(&self.replace);
            last = at + needle.len();
        }
        out.push_str(&name[last..]);
        out
    }
}

/// Why a planned rename will not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameProblem {
    /// The name does not change.
    Unchanged,
    /// The new name is empty, `.`/`..`, or contains a path separator.
    Invalid,
    /// Something else already has the new name.
    Exists,
    /// Another file in the batch would get the same new name.
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRename {
    pub from: PathBuf,
    pub to: PathBuf,
    pub problem: Option<RenameProblem>,
}

/// What `rule` would do to each of `paths`, checking every conflict up front
/// so the user sees them before anything is renamed.
pub fn plan_renames(paths: &[PathBuf], rule: &RenameRule) -> Vec<PlannedRename> {
    let mut seen = std::collections::HashSet::new();
    paths
        .iter()
        .map(|from| {
            let old = from
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let new = rule.apply(&old);
            let to = from.with_file_name(&new);
            let problem = if new == old {
                Some(RenameProblem::Unchanged)
            } else if new.is_empty() || new == "." || new == ".." || new.contains(['/', '\\']) {
                Some(RenameProblem::Invalid)
            } else if !seen.insert(to.clone()) {
                Some(RenameProblem::Duplicate)
            } else if fs::symlink_metadata(&to).is_ok() && !same_name_but_case(&old, &new) {
                Some(RenameProblem::Exists)
            } else {
                None
            };
            PlannedRename {
                from: from.clone(),
                to,
                problem,
            }
        })
        .collect()
}

/// On case-insensitive filesystems `a.TXT` "exists" when renaming `a.txt`;
/// that is the file itself, not a conflict.
fn same_name_but_case(old: &str, new: &str) -> bool {
    old.to_lowercase() == new.to_lowercase()
}

/// Renames one planned entry, refusing to overwrite.
pub fn rename(plan: &PlannedRename) -> io::Result<()> {
    if let Some(problem) = plan.problem {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{problem:?}"),
        ));
    }
    let old = plan.from.file_name().unwrap_or_default().to_string_lossy();
    let new = plan.to.file_name().unwrap_or_default().to_string_lossy();
    if fs::symlink_metadata(&plan.to).is_ok() && !same_name_but_case(&old, &new) {
        return Err(exists_error(&plan.to));
    }
    fs::rename(&plan.from, &plan.to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, data: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, data).expect("write");
    }

    #[test]
    fn moves_and_copies_without_overwriting() {
        let dir = tempfile::tempdir().expect("tempdir");
        let d = dir.path();
        write(&d.join("src/a.txt"), "a");
        write(&d.join("src/sub/b.txt"), "b");
        write(&d.join("dest/a.txt"), "existing");
        fs::create_dir_all(d.join("dest2")).expect("mkdir");

        let copy = copy_into(&d.join("src"), &d.join("dest2")).expect("copy folder");
        assert_eq!(
            fs::read_to_string(copy.join("sub/b.txt")).expect("read"),
            "b"
        );
        assert!(d.join("src/a.txt").exists());

        let err = move_into(&d.join("src/a.txt"), &d.join("dest")).expect_err("exists");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(d.join("dest/a.txt")).expect("read"),
            "existing"
        );

        let moved = move_into(&d.join("src/sub"), &d.join("dest")).expect("move");
        assert!(moved.join("b.txt").exists());
        assert!(!d.join("src/sub").exists());

        assert!(move_into(&d.join("dest"), &d.join("dest/sub")).is_err());
    }

    #[test]
    fn zips_files_and_folders() {
        let dir = tempfile::tempdir().expect("tempdir");
        let d = dir.path();
        write(&d.join("photos/x.jpg"), "xx");
        write(&d.join("notes.txt"), "hello");
        let archive = d.join("out.zip");
        let done = AtomicU64::new(0);
        zip_items(&archive, &[d.join("photos"), d.join("notes.txt")], &done).expect("zip");
        assert_eq!(done.load(Relaxed), 7);
        let zip = zip::ZipArchive::new(File::open(&archive).expect("open")).expect("read");
        let mut names: Vec<String> = zip
            .file_names()
            .map(|n| n.expect("name").into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["notes.txt", "photos/", "photos/x.jpg"]);
        // Never overwrites an archive.
        assert!(zip_items(&archive, &[d.join("notes.txt")], &done).is_err());
    }

    #[test]
    fn replaces_text_in_names() {
        let rule = RenameRule {
            find: "IMG_".into(),
            replace: "holiday-".into(),
            match_case: false,
        };
        assert_eq!(rule.apply("img_001.jpg"), "holiday-001.jpg");
        assert_eq!(rule.apply("IMG_IMG_.jpg"), "holiday-holiday-.jpg");
        let exact = RenameRule {
            match_case: true,
            ..rule
        };
        assert_eq!(exact.apply("img_001.jpg"), "img_001.jpg");
    }

    #[test]
    fn plans_flag_every_conflict_before_renaming() {
        let dir = tempfile::tempdir().expect("tempdir");
        let d = dir.path();
        // " " removed: "x 1" and "x1 " both become "x1"; "b 2" hits an
        // existing "b2"; "c" has no space.
        for name in ["x 1.txt", "x1 .txt", "b 2.txt", "b2.txt", "c.txt"] {
            write(&d.join(name), name);
        }
        let paths: Vec<PathBuf> = ["x 1.txt", "x1 .txt", "b 2.txt", "c.txt"]
            .iter()
            .map(|n| d.join(n))
            .collect();
        let rule = RenameRule {
            find: " ".into(),
            replace: String::new(),
            match_case: true,
        };
        let plan = plan_renames(&paths, &rule);
        let problems: Vec<_> = plan.iter().map(|p| p.problem).collect();
        assert_eq!(
            problems,
            [
                None,
                Some(RenameProblem::Duplicate),
                Some(RenameProblem::Exists),
                Some(RenameProblem::Unchanged),
            ]
        );
        rename(&plan[0]).expect("rename");
        assert!(d.join("x1.txt").exists() && !d.join("x 1.txt").exists());
        assert!(rename(&plan[2]).is_err());
        assert_eq!(
            fs::read_to_string(d.join("b2.txt")).expect("read"),
            "b2.txt"
        );
    }
}
