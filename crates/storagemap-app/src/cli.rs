//! Command-line interface: headless scans for scripts and benchmarks.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use storagemap_core::{Metric, ScanOptions, ScanProgress, analysis, export};

use crate::fmt;

const DEFAULT_TOP: usize = 20;
const DEFAULT_CSV_DEPTH: usize = 64;

pub const USAGE: &str = "\
StorageMap — see where your disk space goes

USAGE:
    storagemap [PATH]                 open the app (and scan PATH)
    storagemap --cli PATH [OPTIONS]   scan from the command line

OPTIONS:
    --top N        list the N largest folders and files (default 20)
    --csv FILE     also write a CSV report to FILE
    --depth N      CSV depth (default 64)
    --allocated    rank by size on disk instead of logical size
    --cross-fs     descend into other mounted filesystems
    --no-mft       Windows: never read the NTFS MFT directly
    -h, --help     show this help
    -V, --version  show the version";

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub cli: bool,
    pub help: bool,
    pub version: bool,
    pub path: Option<PathBuf>,
    pub top: Option<usize>,
    pub csv: Option<PathBuf>,
    pub depth: Option<usize>,
    pub allocated: bool,
    pub cross_fs: bool,
    pub no_mft: bool,
}

impl Args {
    /// Parses arguments (without the program name). Unknown flags are errors.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
        let mut out = Args::default();
        let mut it = args.into_iter();
        while let Some(arg) = it.next() {
            let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.to_str() {
                Some("--cli") => out.cli = true,
                Some("-h" | "--help") => out.help = true,
                Some("-V" | "--version") => out.version = true,
                Some("--allocated") => out.allocated = true,
                Some("--cross-fs") => out.cross_fs = true,
                Some("--no-mft") => out.no_mft = true,
                Some("--top") => out.top = Some(number(&value("--top")?)?),
                Some("--depth") => out.depth = Some(number(&value("--depth")?)?),
                Some("--csv") => out.csv = Some(PathBuf::from(value("--csv")?)),
                Some(flag) if flag.starts_with('-') && flag.len() > 1 => {
                    return Err(format!("unknown option {flag}"));
                }
                _ => out.path = Some(clean_shell_path(arg)),
            }
        }
        Ok(out)
    }
}

fn number(s: &OsString) -> Result<usize, String> {
    s.to_str()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| format!("not a number: {}", s.to_string_lossy()))
}

/// Explorer passes drive roots as `"C:\"`, whose trailing `\"` arrives as a
/// literal quote; repair it.
fn clean_shell_path(arg: OsString) -> PathBuf {
    match arg.to_str() {
        Some(s) if s.ends_with('"') => PathBuf::from(format!("{}\\", s.trim_end_matches('"'))),
        _ => PathBuf::from(arg),
    }
}

pub fn run(args: &Args) -> ExitCode {
    let Some(path) = &args.path else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let opts = ScanOptions {
        cross_filesystems: args.cross_fs,
        prefer_mft: !args.no_mft,
        threads: 0,
    };
    let progress = ScanProgress::new();
    let result = match storagemap_core::scan(path, &opts, &progress) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let metric = if args.allocated {
        Metric::Allocated
    } else {
        Metric::Size
    };
    let tree = &result.tree;
    let root = tree.node(tree.root());
    let top = args.top.unwrap_or(DEFAULT_TOP);
    let mut out = std::io::stdout().lock();
    let report = (|| -> std::io::Result<()> {
        writeln!(out, "{}", tree.root_path().display())?;
        writeln!(
            out,
            "  {} logical, {} on disk, {} files, {} folders",
            fmt::size_en(root.size),
            fmt::size_en(root.allocated),
            fmt::count(root.files),
            fmt::count(root.dirs)
        )?;
        writeln!(
            out,
            "  scanned {} entries in {:.2}s with {:?} ({} errors{})",
            fmt::count(tree.len() as u64),
            result.elapsed.as_secs_f64(),
            result.backend,
            result.errors,
            if result.cancelled { ", cancelled" } else { "" }
        )?;
        writeln!(out, "\nLargest entries:")?;
        let mut kids: Vec<_> = tree.children(tree.root()).collect();
        kids.sort_by_key(|&k| std::cmp::Reverse(tree.node(k).metric(metric)));
        for k in kids.into_iter().take(top) {
            let share = fmt::percent(tree.share_of_parent(k, metric));
            let marker = if tree.node(k).is_dir() { "/" } else { "" };
            writeln!(
                out,
                "  {:>12}  {:>6}  {}{marker}",
                fmt::size_en(tree.node(k).metric(metric)),
                share,
                tree.name(k)
            )?;
        }
        writeln!(out, "\nLargest files:")?;
        for f in analysis::top_files(tree, tree.root(), metric, top) {
            writeln!(
                out,
                "  {:>12}  {}",
                fmt::size_en(tree.node(f).metric(metric)),
                tree.path(f).display()
            )?;
        }
        Ok(())
    })();
    if let Err(e) = report {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    if let Some(csv) = &args.csv {
        let written = std::fs::File::create(csv).and_then(|f| {
            export::write_csv(
                tree,
                tree.root(),
                args.depth.unwrap_or(DEFAULT_CSV_DEPTH),
                f,
            )
        });
        if let Err(e) = written {
            eprintln!("error: cannot write {}: {e}", csv.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        Args::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_cli_options() {
        let a = parse(&[
            "--cli",
            "/data",
            "--top",
            "5",
            "--csv",
            "out.csv",
            "--allocated",
        ])
        .expect("parse");
        assert!(a.cli && a.allocated);
        assert_eq!(a.path, Some(PathBuf::from("/data")));
        assert_eq!(a.top, Some(5));
        assert_eq!(a.csv, Some(PathBuf::from("out.csv")));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&["--top"]).is_err());
        assert!(parse(&["--top", "x"]).is_err());
        assert!(parse(&["--wat"]).is_err());
    }

    #[test]
    fn repairs_explorer_drive_quote() {
        assert_eq!(
            parse(&["C:\""]).expect("parse").path,
            Some(PathBuf::from("C:\\"))
        );
    }
}
