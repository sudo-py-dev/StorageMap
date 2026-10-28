app-title = StorageMap

## Menus
menu-file = File
menu-view = View
menu-language = Language
menu-tools = Tools
menu-help = Help
lang-system = System default

## Actions
action-scan-folder = Scan folder…
action-rescan = Rescan
action-stop = Stop
action-export-csv = Export CSV…
action-quit = Quit
action-up = Up
action-open = Open
action-show-in-folder = Show in file manager
action-copy-path = Copy path
action-trash = Move to trash…
action-scan-this = Scan this folder
action-collapse-all = Collapse all
action-run-as-admin = Restart as administrator
action-integrate = Add “Open with StorageMap” to the file manager
action-unintegrate = Remove file manager integration
action-home = Start screen

## Metrics
metric-size = Size
metric-allocated = Size on disk
metric-files = File count
metric-label = Measure by

## Tabs
tab-treemap = Treemap
tab-details = Details
tab-top-files = Largest files
tab-types = File types
tab-ages = Age
tab-search = Search

## Columns
col-name = Name
col-size = Size
col-allocated = On disk
col-percent = % of parent
col-files = Files
col-folders = Folders
col-modified = Modified
col-type = Type
col-path = Path
col-extension = Extension
col-share = Share

## Start screen
start-title = Where did my space go?
start-subtitle = Pick a drive or a folder to map.
start-drives = Drives
start-recent = Recent folders
volume-free = { $free } free of { $total }
volume-fixed = Local disk
volume-removable = Removable
volume-network = Network
volume-optical = Optical
volume-ram = RAM disk
volume-unknown = Volume

## Scanning
scan-progress = Scanning… { $files } files in { $dirs } folders · { $size }
scan-done = { $items } items in { $seconds } s
scan-cancelled = Scan stopped – showing partial results
scan-errors =
    { $count ->
        [one] 1 item could not be read
       *[other] { $n } items could not be read
    }
scan-failed = Scan failed: { $error }
backend-linux = statx engine
backend-mft = NTFS MFT direct read
backend-dirinfo = Windows directory engine
backend-portable = standard engine
admin-hint = Run as administrator to read the NTFS master file table directly – usually many times faster.

## Trash
confirm-trash-title = Move to trash?
confirm-trash-body = “{ $name }” ({ $size }) will be moved to the trash.
confirm-trash = Move to trash
cancel = Cancel
trash-failed = Could not move to trash: { $error }
trashed = Moved “{ $name }” to the trash

## Misc
search-hint = Search names…
search-results =
    { $count ->
        [one] 1 match
       *[other] { $n } matches
    }
empty-folder = This folder is empty.
files-count =
    { $count ->
        [one] 1 file
       *[other] { $n } files
    }
folders-count =
    { $count ->
        [one] 1 folder
       *[other] { $n } folders
    }
type-folder = Folder
type-file = File
type-link = Link
type-other = Other
no-extension = (no extension)
age-week = Last 7 days
age-month = Last 30 days
age-half-year = Last 6 months
age-year = Last year
age-two-years = 1–2 years
age-five-years = 2–5 years
age-older = Older than 5 years
note-hardlink = Hard link – counted once
note-mount = Another filesystem – not scanned
note-error = Could not be read
note-incomplete = Not scanned (stopped)
percent-of-parent = { $percent } of parent
copied = Path copied
exported = Exported to { $path }
export-failed = Export failed: { $error }
integration-done = File manager integration installed
integration-removed = File manager integration removed
integration-failed = Integration failed: { $error }
open-failed = Could not open: { $error }
free-on-volume = { $free } free on this volume

## View options
view-theme = Theme
theme-system = Follow system
theme-light = Light
theme-dark = Dark
view-units = Units
units-auto = Automatic
view-decimals = Decimals
treemap-depth = Treemap detail
treemap-files = Show files in treemap
treemap-labels = Show labels
option-cross-fs = Also scan other mounted filesystems
option-mft = Fast NTFS scan (MFT)

## Help
help-about = About StorageMap
about-body = A fast disk space analyzer for Windows and Linux. Scans run locally; nothing leaves your computer.
about-version = Version { $version }

## Units
unit-b = B
unit-kb = KB
unit-mb = MB
unit-gb = GB
unit-tb = TB

## File categories
cat-image = Images
cat-video = Video
cat-audio = Audio
cat-archive = Archives & packages
cat-document = Documents
cat-code = Code & build output
cat-executable = Programs & libraries
cat-disk-image = Disk images
cat-other = Other

## Snapshots and changes
tab-changes = Changes
compare-title = Compare with an earlier scan
compare-none = No earlier scans of this folder yet. Every scan is saved as a snapshot, so next time you will see exactly what grew and what disappeared.
compare-browse = Open snapshot file…
compare-summary = { $change } since { $date }
compare-other = Compare with…
compare-clear = Stop comparing
compare-unchanged = Nothing changed here since the snapshot.
compare-failed = Cannot compare: { $error }
snapshot-entry = { $date } · { $size }
action-save-snapshot = Save snapshot now
snapshot-saved = Snapshot saved
snapshot-failed = Could not save the snapshot: { $error }
option-auto-snapshot = Save a snapshot after every scan
col-before = Before
col-now = Now
col-change = Change
change-new = new
change-removed = removed

## Duplicates
tab-duplicates = Duplicates
dup-find = Find duplicates
dup-min-size = Minimum size
dup-scope = in { $path }
dup-progress = Comparing contents… { $files } of { $total } files · { $size } read
dup-copies =
    { $count ->
        [one] 1 copy
       *[other] { $n } copies
    }
dup-groups =
    { $count ->
        [one] 1 set of duplicates
       *[other] { $n } sets of duplicates
    }
dup-each = { $size } each
dup-reclaimable = { $size } reclaimable
dup-none = No duplicate files found.
dup-hint = Finds files with identical content in the current folder. Files are matched by size, then by a quick fingerprint of their start and end, and only then by a full content hash. Hard links are ignored because they already share space.
