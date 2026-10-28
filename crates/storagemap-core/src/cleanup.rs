//! Cleanup suggestions: well-known space that is usually safe to reclaim.
//!
//! Purely name- and age-based heuristics over a finished scan; nothing is
//! read or deleted here. The UI lets the user review each item and move it
//! to the trash. A matched folder is never descended into, so suggestions
//! never overlap.

use crate::analysis::extension_of;
use crate::tree::{Kind, NodeId, Tree};

const MIB: u64 = 1024 * 1024;
const DAY: i64 = 86_400;
/// Folders and files smaller than this are not worth suggesting.
const MIN_SUGGESTION: u64 = MIB;
const MIN_LOG: u64 = 50 * MIB;
const MIN_OLD_DOWNLOAD: u64 = 50 * MIB;
const OLD_DOWNLOAD_AGE: i64 = 180 * DAY;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CleanupKind {
    Trash,
    Cache,
    BuildOutput,
    PackageCache,
    Temp,
    Logs,
    CrashDumps,
    OldDownloads,
}

impl CleanupKind {
    pub const ALL: [CleanupKind; 8] = [
        CleanupKind::Trash,
        CleanupKind::Cache,
        CleanupKind::BuildOutput,
        CleanupKind::PackageCache,
        CleanupKind::Temp,
        CleanupKind::Logs,
        CleanupKind::CrashDumps,
        CleanupKind::OldDownloads,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suggestion {
    pub kind: CleanupKind,
    pub node: NodeId,
    pub size: u64,
}

/// Folder names that are caches wherever they appear.
const CACHE_DIRS: &[&str] = &[
    ".cache",
    "cache",
    "caches",
    "cache2",
    "gpucache",
    "code cache",
    "shadercache",
    "inetcache",
    "dawncache",
];
/// Build/tooling output that is regenerated on demand.
const BUILD_DIRS: &[&str] = &[
    "node_modules",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".next",
    ".nuxt",
    ".parcel-cache",
    ".turbo",
];
/// Package-manager caches, matched by the last two path components.
const PACKAGE_CACHES: &[(&str, &str)] = &[
    (".cargo", "registry"),
    (".m2", "repository"),
    (".gradle", "caches"),
    ("pkg", "mod"),
    (".npm", "_cacache"),
    ("yarn", "cache"),
    ("pnpm", "store"),
    ("nuget", "packages"),
];
const TEMP_DIRS: &[&str] = &["temp", "tmp"];
const DUMP_EXTENSIONS: &[&str] = &["dmp", "mdmp", "hdmp"];

/// Suggestions below `root`, largest first, at most `limit` per kind.
pub fn suggestions(tree: &Tree, root: NodeId, now: i64, limit: usize) -> Vec<Suggestion> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = tree.children(root).collect();
    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        match classify(tree, id, now) {
            Some(kind) if node.size >= MIN_SUGGESTION => out.push(Suggestion {
                kind,
                node: id,
                size: node.size,
            }),
            Some(_) => {}
            None if node.is_dir() => stack.extend(tree.children(id)),
            None => {}
        }
    }
    out.sort_by_key(|s| (s.kind, std::cmp::Reverse(s.size)));
    let mut per_kind = std::collections::HashMap::new();
    out.retain(|s| {
        let n = per_kind.entry(s.kind).or_insert(0usize);
        *n += 1;
        *n <= limit
    });
    out.sort_by_key(|s| std::cmp::Reverse(s.size));
    out
}

fn classify(tree: &Tree, id: NodeId, now: i64) -> Option<CleanupKind> {
    let node = tree.node(id);
    let name = tree.name(id).to_lowercase();
    let parent_name = tree
        .parent(id)
        .map(|p| tree.name(p).to_lowercase())
        .unwrap_or_default();
    match node.kind {
        Kind::Dir => {
            if name.starts_with(".trash")
                || name == "$recycle.bin"
                || (name == "trash" && parent_name == "share")
            {
                Some(CleanupKind::Trash)
            } else if PACKAGE_CACHES
                .iter()
                .any(|(p, n)| *p == parent_name && *n == name)
            {
                Some(CleanupKind::PackageCache)
            } else if BUILD_DIRS.contains(&name.as_str()) || is_rust_target(tree, id, &name) {
                Some(CleanupKind::BuildOutput)
            } else if CACHE_DIRS.contains(&name.as_str()) {
                Some(CleanupKind::Cache)
            } else if TEMP_DIRS.contains(&name.as_str()) && tree.parent(id).is_some() {
                Some(CleanupKind::Temp)
            } else if name == "crashdumps" {
                Some(CleanupKind::CrashDumps)
            } else {
                None
            }
        }
        Kind::File => {
            let ext = extension_of(tree.name_bytes(id));
            if DUMP_EXTENSIONS.contains(&ext.as_str()) || is_core_dump(&name) {
                Some(CleanupKind::CrashDumps)
            } else if ext == "log" && node.size >= MIN_LOG {
                Some(CleanupKind::Logs)
            } else if parent_name == "downloads"
                && node.size >= MIN_OLD_DOWNLOAD
                && now - node.modified >= OLD_DOWNLOAD_AGE
            {
                Some(CleanupKind::OldDownloads)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `target` is only build output when it sits next to a `Cargo.toml`.
fn is_rust_target(tree: &Tree, id: NodeId, name: &str) -> bool {
    name == "target"
        && tree.parent(id).is_some_and(|p| {
            tree.children(p)
                .any(|c| tree.name_bytes(c) == b"Cargo.toml")
        })
}

/// `core` or `core.<pid>` as written by Linux on a crash.
fn is_core_dump(name: &str) -> bool {
    name == "core"
        || name
            .strip_prefix("core.")
            .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

/// Total size of the suggestions, per kind.
pub fn totals(suggestions: &[Suggestion]) -> Vec<(CleanupKind, u64, usize)> {
    CleanupKind::ALL
        .iter()
        .map(|&k| {
            let items = suggestions.iter().filter(|s| s.kind == k);
            (k, items.clone().map(|s| s.size).sum(), items.count())
        })
        .filter(|(_, size, _)| *size > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryInfo, Flags, TreeBuilder};
    use std::path::PathBuf;

    const NOW: i64 = 1_800_000_000;

    fn dir() -> EntryInfo {
        EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: NOW,
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
            size: 0,
            allocated: 0,
        }
    }

    fn file(size: u64, modified: i64) -> EntryInfo {
        EntryInfo {
            kind: Kind::File,
            flags: Flags::empty(),
            modified,
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
            size,
            allocated: size,
        }
    }

    fn sample() -> Tree {
        let mut b = TreeBuilder::new(PathBuf::from("/home/u"), dir());
        let r = NodeId::ROOT;
        let cache = b.push(r, b".cache", dir());
        let nested = b.push(cache, b"node_modules", dir()); // inside a cache: not reported twice
        b.push(nested, b"x", file(9 * MIB, NOW));
        b.push(cache, b"thumbs", file(20 * MIB, NOW));
        let proj = b.push(r, b"proj", dir());
        b.push(proj, b"Cargo.toml", file(100, NOW));
        let target = b.push(proj, b"target", dir());
        b.push(target, b"app", file(40 * MIB, NOW));
        let web = b.push(r, b"web", dir());
        let nm = b.push(web, b"node_modules", dir());
        b.push(nm, b"lib.js", file(5 * MIB, NOW));
        let other = b.push(r, b"photos", dir());
        let not_rust = b.push(other, b"target", dir()); // no Cargo.toml: keep
        b.push(not_rust, b"img.jpg", file(30 * MIB, NOW));
        let dl = b.push(r, b"Downloads", dir());
        b.push(dl, b"old.iso", file(700 * MIB, NOW - 400 * DAY));
        b.push(dl, b"new.iso", file(700 * MIB, NOW - DAY));
        b.push(r, b"core.1234", file(2 * MIB, NOW));
        b.push(r, b"app.log", file(60 * MIB, NOW));
        b.push(r, b"small.log", file(MIB, NOW));
        let cargo = b.push(r, b".cargo", dir());
        let reg = b.push(cargo, b"registry", dir());
        b.push(reg, b"crates", file(80 * MIB, NOW));
        b.finish()
    }

    fn kinds(t: &Tree) -> Vec<(CleanupKind, String)> {
        let mut v: Vec<_> = suggestions(t, t.root(), NOW, 50)
            .iter()
            .map(|s| (s.kind, t.name(s.node).into_owned()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn classifies_common_reclaimable_space() {
        let t = sample();
        assert_eq!(
            kinds(&t),
            vec![
                (CleanupKind::Cache, ".cache".to_owned()),
                (CleanupKind::BuildOutput, "node_modules".to_owned()),
                (CleanupKind::BuildOutput, "target".to_owned()),
                (CleanupKind::PackageCache, "registry".to_owned()),
                (CleanupKind::Logs, "app.log".to_owned()),
                (CleanupKind::CrashDumps, "core.1234".to_owned()),
                (CleanupKind::OldDownloads, "old.iso".to_owned()),
            ]
        );
    }

    #[test]
    fn largest_first_and_limited_per_kind() {
        let t = sample();
        let all = suggestions(&t, t.root(), NOW, 50);
        assert!(all.windows(2).all(|w| w[0].size >= w[1].size));
        let one = suggestions(&t, t.root(), NOW, 1);
        assert_eq!(
            one.iter()
                .filter(|s| s.kind == CleanupKind::BuildOutput)
                .count(),
            1
        );
    }

    #[test]
    fn totals_per_kind() {
        let t = sample();
        let s = suggestions(&t, t.root(), NOW, 50);
        let build = totals(&s)
            .into_iter()
            .find(|(k, _, _)| *k == CleanupKind::BuildOutput)
            .expect("build");
        assert_eq!((build.1, build.2), (45 * MIB, 2));
    }

    #[test]
    fn core_dump_names() {
        assert!(is_core_dump("core"));
        assert!(is_core_dump("core.42"));
        assert!(!is_core_dump("core.rs"));
        assert!(!is_core_dump("core."));
    }
}
