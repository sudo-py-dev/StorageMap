# Developing StorageMap

Planned work is in [ROADMAP.md](ROADMAP.md).

## Build and run

```bash
cargo build --release
```

```bash
./target/release/storagemap            # open the app
./target/release/storagemap /home      # open and scan /home
./target/release/storagemap --cli /usr --top 10
```

The minimum supported Rust version is 1.95 (set by egui). On Linux, egui
needs the usual windowing libraries (X11/Wayland and xkbcommon), which
desktop distributions already ship. Build for Windows on Windows (MSVC).

### Command line

```
storagemap --cli PATH [--top N] [--csv FILE] [--report FILE] [--depth N]
                [--allocated] [--cross-fs] [--no-mft] [--exclude PAT]... [--include PAT]...
```

It prints totals, the largest entries and the largest files. With `--csv`
or `--report` it also writes a full report; `--report` picks the format from
the extension (`.csv`, `.xlsx`, `.html`, `.json`, `.txt`).

## Layout

```
crates/storagemap-core   scanner, tree model, analyses, snapshots, duplicates, cleanup
  backend/linux.rs       getdents64 + statx walker
  backend/windows/       NTFS MFT reader + GetFileInformationByHandleEx walker
  ntfs.rs                platform-independent MFT record parser (unit-tested)
  snapshot.rs            snapshot file format and comparison
  duplicates.rs          size → partial hash → full BLAKE3 duplicate finder
  cleanup.rs             cleanup suggestion heuristics
  filter.rs              exclude/include glob patterns applied by every backend
  export.rs              report rows and format writers (CSV, tab-separated)
  source.rs              Source trait: scan anything that can list directories
crates/storagemap-app    egui application and CLI
  i18n/                  Fluent bundles and the bidi layer
  locales/*.ftl          translations
  views/                 tree panel, treemap, tables, settings, start screen, chrome
  update.rs              GitHub Releases self-update
  theme.rs               colours, spacing and type scale
installer/windows/       Inno Setup script
packaging/linux/         desktop entry for the .deb
assets/                  icon sources (assets/make_icon.py regenerates them)
```

## How scanning works

- **Linux:** every folder is opened relative to its parent's descriptor
  (`openat`), listed with `getdents64` and stat'ed with `statx` asking only
  for the fields that are needed. The work runs on a work-stealing thread
  pool. Hard links are counted once, and other filesystems are skipped unless
  you enable them.
- **Windows, as administrator:** the NTFS Master File Table is read directly
  in large sequential chunks while rayon parses the previous chunk.
- **Windows, otherwise:** directories are listed in 64 KiB batches with
  `GetFileInformationByHandleEx`, which returns sizes and times with the
  names.

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

Layouts mirror through direction-aware helpers in `widgets.rs` instead of
hard-coded left and right.

## Adding a language

1. Copy `crates/storagemap-app/locales/en.ftl` to `<code>.ftl` and translate
   it.
2. Add the language to `Lang` in `i18n/mod.rs`: code, native name, direction
   and source file.
3. Run `cargo test`. It fails if any message is missing or extra.

## Checks

```bash
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
```

CI runs these on Linux and Windows, plus a minimum-Rust-version build and
`cargo audit`.

To check the visuals automatically:

```bash
STORAGEMAP_SCREENSHOT=/tmp/shot.rgba STORAGEMAP_SCREENSHOT_TAB=details ./target/release/storagemap /usr
```

This scans the folder, writes the window as raw RGBA (prefixed by a `W H`
line) and exits. `STORAGEMAP_SCREENSHOT_SETTINGS=updates` opens a settings
page instead, and leaving out the folder captures the start screen.
`STORAGEMAP_SCREENSHOT_LIVE=1` captures the live view of a scan still
running instead of the finished result.

## Releasing

1. Bump `version` in the workspace `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The *Release* workflow then:
- creates a **draft** release;
- builds the Windows installer and portable zip and the Linux `.deb` and
  tarball, with build-provenance attestations;
- uploads `SHA256SUMS`;
- publishes the release only once every asset is in place, so the in-app
  updater never sees a half-finished release.
