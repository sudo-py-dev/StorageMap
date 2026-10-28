# StorageMap

A fast disk space analyzer for **Windows and Linux**, written in Rust with egui.
It is multilingual from the start (English, Hebrew, Arabic) with full
right-to-left support.

## Features

- **Fast scanning on each OS**
  - Linux: `openat` + `getdents64` + `statx` on a parallel work-stealing
    pool, opening each folder relative to its parent's descriptor.
    - Hard links are counted once; the walk stays on one filesystem by
      default.
    - The open-files limit is raised automatically.
  - Windows: reads the NTFS Master File Table directly when running as
    administrator.
    - Without admin rights it falls back to batched
      `GetFileInformationByHandleEx` directory listings (about 64 KiB of
      entries per call).
- **Views:** folder tree, a nested squarified treemap with cushion shading and
  category colours, details table, largest files, file types, age
  distribution, and name search.
- **Measures:** size, size on disk, or file count; all views follow the
  choice.
- **Actions:** open, show in file manager, copy path, move to trash (with
  confirmation), rescan a subfolder, export CSV, drag a folder onto the
  window to scan it.
- **OS integration:** an "Open with StorageMap" entry for folders.
  - Windows: Explorer context menu, written to HKCU; no admin needed.
  - Linux: a `.desktop` entry for file managers.
  - Windows also offers a one-click restart as administrator.
- **Internationalisation:** Project Fluent messages with CLDR plural rules
  (Hebrew dual, the six Arabic plural forms) and localised size units.
  - Direction follows the language: panels, tables, the treemap and arrow
    keys all mirror in RTL.
  - Files with Hebrew or Arabic names display correctly in any UI language.
- **Snapshots and changes over time:** every scan is saved as a compact
  snapshot (all folders plus files ≥ 1 MiB), and the next scan of the same
  folder is compared with it automatically.
  - The *Changes* tab lists what grew, shrank, appeared or disappeared,
    biggest change first.
  - You can compare with any earlier snapshot or an exported `.smap` file.
  - The last 30 snapshots per folder are kept.
- **Duplicate finder:** files are grouped by size, then by a BLAKE3
  fingerprint of their first and last 16 KiB, and only then by a full
  BLAKE3 hash.
  - It runs in the background with progress and can be stopped.
  - Hard links are ignored, since they already share space.
  - Results show reclaimable space, and every copy offers open, show and
    trash.
- **Cleanup suggestions:** finds space that is usually safe to reclaim —
  trash, application caches, build output (`node_modules`, Rust `target`
  next to a `Cargo.toml`), package-manager caches, temp folders, huge logs,
  crash dumps and large downloads untouched for six months. Every item is
  reviewed and moved to the trash, never deleted outright.
- **Installers and auto-update:** an Inno Setup installer for Windows (per
  user, no admin needed) and a `.deb` for Debian/Ubuntu. The app checks
  GitHub Releases once a day (can be turned off), shows the release notes,
  verifies the download against `SHA256SUMS`, then installs and relaunches.
- **Polished UI:** light and dark themes, adjustable interface size,
  animated tabs, treemap zoom and toasts, a settings window with keyboard
  shortcuts, and the window sized to the screen on first launch.
- **CLI:** `storagemap --cli PATH [--top N] [--csv FILE] [--allocated]` for
  scripts and benchmarks.

## Build and run

```bash
cargo build --release
```

```bash
./target/release/storagemap            # open the app
./target/release/storagemap /home      # open and scan /home
./target/release/storagemap --cli /usr --top 10
```

On Linux, egui needs the usual windowing libraries (X11/Wayland and
xkbcommon), which desktop distributions already ship. To build for Windows,
build on Windows (MSVC), or cross-compile with `cargo build --release --target
x86_64-pc-windows-gnu` with MinGW installed.

## Releasing

1. Bump `version` in the workspace `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The *Release* workflow then:
- creates a **draft** release;
- builds the Windows installer and portable zip and the Linux `.deb` and
  tarball, with build-provenance attestations when the repository is public;
- uploads `SHA256SUMS`;
- publishes the release only once every asset is in place, so the updater
  never sees a half-finished release.

The updater reads GitHub's public API, so it only finds releases once the
repository (or at least its releases) is public.

## Layout

```
crates/storagemap-core   scanner, tree model, analyses, treemap layout, NTFS parser
  backend/linux.rs       statx walker
  backend/windows/       MFT reader + directory-info walker
  ntfs.rs                platform-independent MFT record parser (unit-tested)
crates/storagemap-app    egui application and CLI
  i18n/                  Fluent bundles + bidi layer
  locales/*.ftl          translations
  views/                 tree panel, treemap, tables, start screen, chrome
```

## How RTL works with egui

egui 0.37 shapes each font run with harfrust, so a single Hebrew or Arabic
word renders correctly and Arabic letters join. It does not order runs
within a sentence. StorageMap therefore:

1. runs the Unicode Bidirectional Algorithm over each line to get the run
   order;
2. puts the characters inside each RTL run back in logical order, so the
   shaper's own reversal and Arabic joining come out right;
3. elides and wraps text in logical order *before* reordering, so RTL text
   loses its end, not its beginning.

Layouts mirror through direction-aware helpers (`widgets.rs`) instead of
hard-coded left and right.

## Adding a language

1. Copy `crates/storagemap-app/locales/en.ftl` to `<code>.ftl` and translate
   it.
2. Add the language to `Lang` in `i18n/mod.rs`: code, native name, direction
   and source file.
3. Run `cargo test`. It fails if any message is missing or extra.

## Development

```bash
cargo test && cargo clippy --all-targets
```

To check the visuals automatically, run:

```bash
STORAGEMAP_SCREENSHOT=/tmp/shot.rgba STORAGEMAP_SCREENSHOT_TAB=details ./target/release/storagemap /usr
```

This scans the folder, writes the window as raw RGBA (prefixed by a
`W H` line) and exits.
