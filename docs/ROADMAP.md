# Roadmap

Where StorageMap is heading after 0.1. Work lands one phase at a time; each
item ships on its own once it is usable. Items marked **groundwork** are the
shared foundations the later items build on.

## Phase 0 — Groundwork (done)

- **Scan filters** (groundwork). Exclude folders or files by name or path
  pattern, and optionally count only files that match include patterns.
  Applied by every backend, including the NTFS table reader. Set in
  Settings → Scanning, or with `--exclude` and `--include`.
- **Configurable columns** (groundwork). Every table column has a stable id;
  which columns are shown is a per-table preference, chosen by
  right-clicking a column header.
- **Report formats** (groundwork). Exports build one report model (rows of
  typed values) and hand it to a format writer. CSV and tab-separated text
  (copied from the export dialog) first; other formats only add a
  `ReportWriter`.
- **Scan sources** (groundwork). A scan reads from a source: the local
  filesystem today, and anything that can list a directory later (SSH, S3,
  WebDAV, Azure Blob, SharePoint, disk images, phones), reusing the parallel
  walker, filters and tree: a new place implements `storagemap_core::source::Source`.

## Phase 1 — Everyday analysis

- Browse results while a scan is still running (live, partial tree).
- Several scans open at once, as tabs.
- Last-access date, creation date and attributes in the details table.
- Pie and bar charts of the selected folder next to the treemap.
- Owner statistics (who uses the space), per folder.
- Filter the folder tree by file type, owner or age.
- Bold or highlight unusually large items.
- Choose and reorder columns per table, and per export.

## Phase 2 — Search

- Advanced search: name patterns, size range, dates, type, owner,
  attributes, combined freely, with saved searches.
- Search several drives or folders at once, including network shares.
- Export search results in every report format.
- Duplicate finder: optional SHA-256 output, keep/remove rules per group.
- Replace duplicates with hard links (NTFS and Unix filesystems), after
  confirmation, with a dry run first.

## Phase 3 — Acting on files

- Select many items and move them to the trash, move, copy or archive them
  (zip), with a summary and confirmation before anything happens.
- Rule-based selection, such as "files older than two years over 100 MB".
- Bulk rename: replace characters or text in many names at once, with a
  preview.

## Phase 4 — Reports

- Formats: Excel workbook, HTML report, PDF report, plain text, JSON,
  clipboard.
- Options per report: depth, columns and their order, which rows, and the
  units used for sizes.
- Full file list export for use in a database.

## Phase 5 — Growth over time

- Load a saved scan and browse it without rescanning.
- Compare any two saved scans, not only the latest.
- Windows: read Volume Shadow Copies to compare with an earlier state of the
  disk without having saved a scan.

## Phase 6 — Windows and NTFS depth

- Alternate data streams: count them and list them per file.
- NTFS permissions: show and export who can access a folder.
- Active Directory: find folders a user or group can access.
- The Explorer context menu for items inside StorageMap.
- Drag items out of StorageMap into other applications.

## Phase 7 — More places to scan

Each is a scan source (see Phase 0).

- Linux and Unix machines over SSH.
- Amazon S3 and compatible storage, Azure Blob Storage.
- WebDAV and SharePoint document libraries.
- Disk images (VHD/VHDX) without mounting them.
- Outlook mailboxes (PST files and Exchange).
- Phones and other MTP devices.

## Performance

- Pick the number of scanner threads automatically from the storage type
  (SSD, HDD, network).
- Rescan only folders that changed since the last scan, when the filesystem
  can tell.
