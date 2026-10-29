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
action-export-csv = Export report…
action-export-short = Export
action-export-hint = Save this folder's contents as a report: Excel, CSV, web page, JSON or text
csv-bytes = Size (bytes)
csv-allocated-bytes = On disk (bytes)
csv-modified-utc = Modified (UTC)
export-title = Export report
export-format = Format
export-format-csv = CSV
export-format-excel = Excel
export-format-html = Web page
export-format-json = JSON
export-format-text = Text
export-depth = Folder levels
export-depth-levels =
    { $count ->
        [one] 1 level
       *[other] { $count } levels
    }
export-depth-all = All levels
export-columns = Columns
export-include = Show only
export-include-all = Everything
export-include-folders = Folders
export-include-files = Files
export-sizes = Sizes
export-sizes-readable = Readable (1.4 GB)
export-sizes-bytes = Exact bytes
export-save = Save…
export-copy = Copy
export-copy-hint = Copy this folder and what it directly contains, ready to paste into a spreadsheet.
report-copied = Report copied
action-quit = Quit
action-up = Up
action-open = Open
action-show-in-folder = Show in file manager
action-open-folder = Open folder
action-copy-path = Copy path
action-trash = Move to trash…
action-scan-this = Scan this folder
action-collapse-all = Collapse all
tree-filter-all = All files
tree-filter-types = Type
tree-filter-modified = Modified
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
tab-chart = Chart
chart-donut = Donut
chart-bars = Bars
chart-other =
    { $count ->
        [one] 1 other item
       *[other] { $n } other items
    }
tab-search = Search

## Columns
columns-menu = Columns
col-mark = ✔
mark-hint = Mark for moving, copying, zipping, renaming or the trash (Ctrl-click a row also marks it)
marked-summary = { $count } marked · { $size }
marks-clear = Clear marks
search-mark-all = Mark all
search-mark-all-hint = Mark every result in this scan, to act on them together
bulk-move = Move to…
bulk-copy = Copy to…
bulk-zip = Zip…
bulk-rename = Rename…
bulk-trash = Move to trash
bulk-move-title = Move the marked items to
bulk-copy-title = Copy the marked items to
bulk-zip-title = Save the archive as
bulk-trash-title = Move the marked items to the trash?
bulk-trash-body = { $count } items ({ $size }) go to the trash. You can restore them from there.
bulk-moving = Moving… { $done } of { $total }
bulk-copying = Copying… { $done } of { $total }
bulk-zipping = Zipping… { $done } of { $total }
bulk-trashing = Moving to the trash… { $done } of { $total }
bulk-renaming = Renaming… { $done } of { $total }
bulk-moved =
    { $count ->
        [one] 1 item moved
       *[other] { $n } items moved
    }
bulk-copied =
    { $count ->
        [one] 1 item copied
       *[other] { $n } items copied
    }
bulk-zipped =
    { $count ->
        [one] 1 item zipped
       *[other] { $n } items zipped
    }
bulk-trashed =
    { $count ->
        [one] 1 item moved to the trash
       *[other] { $n } items moved to the trash
    }
bulk-renamed =
    { $count ->
        [one] 1 item renamed
       *[other] { $n } items renamed
    }
rename-title = Rename the marked items
rename-find = Find
rename-replace = Replace with
rename-match-case = Match case
rename-preview = { $count } will be renamed:
rename-apply = Rename
rename-unchanged = no change
rename-invalid = not a valid name
rename-exists = that name is taken
rename-duplicate = another item would get the same name
column-earlier = Move earlier
column-later = Move later
columns-reset = Reset columns
col-name = Name
col-size = Size
col-allocated = On disk
col-percent = % of parent
col-files = Files
col-folders = Folders
col-modified = Modified
col-accessed = Last opened
col-created = Created
col-attributes = Attributes
col-owner = Owner
owner-unknown = Unknown owner
owner-id = user { $id }
types-by-type = By type
types-by-owner = By owner
attr-read-only = Read-only
attr-hidden = Hidden
attr-system = System
attr-compressed = Compressed
attr-encrypted = Encrypted
attr-sparse = Sparse
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
tab-new = Scan another folder
tab-close = Close
wait-for-scan = Wait for the scan to finish first
scan-live-hint = Finished folders can be browsed now
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
backend-source = { $name } engine
admin-hint = Run as administrator to read the NTFS master file table directly – usually many times faster.

## Trash
confirm-trash-title = Move to trash?
confirm-trash-body = “{ $name }” ({ $size }) will be moved to the trash.
confirm-trash = Move to trash
cancel = Cancel
trash-failed = Could not move to trash: { $error }
trashed = Moved “{ $name }” to the trash
action-empty-trash = Empty trash
confirm-empty-title = Empty the trash?
confirm-empty-body = Everything in the trash ({ $size }) will be deleted permanently. This cannot be undone.
emptying-trash = Emptying the trash…
trash-emptied = Trash emptied — { $size } freed
empty-trash-failed = Could not empty the trash: { $error }
already-in-trash = This item is already in the trash

## Misc
search-hint = Search names…
search-clear = Clear search
search-filters = Filters
search-all-tabs = All open scans
search-all-tabs-hint = Also search the folders and drives open in the other tabs
search-other-tab-hint = In another tab: click to go there
search-saved = Saved
search-saved-none = No saved searches yet
search-saved-name = Name
search-save = Save
search-saved-delete = Delete this saved search
search-export = Export…
search-copy-hint = Copy the results, ready to paste into a spreadsheet.
filter-everything = Files and folders
filter-files = Files
filter-folders = Folders
filter-at-least = At least
filter-at-most = At most
filter-any-size = any size
filter-any-time = Modified any time
filter-within-week = Modified in the last 7 days
filter-within-month = Modified in the last 30 days
filter-within-year = Modified in the last year
filter-older-year = Not modified for a year
filter-older-two-years = Not modified for 2 years
filter-older-five-years = Not modified for 5 years
filter-any-owner = Any owner
filter-any-attribute = Any attributes
filter-clear = Clear filters
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
highlight-large = Highlight large items
setting-highlight-large-hint = Colour the names of files and folders that take up a large share of their folder.
option-cross-fs = Also scan other mounted filesystems
option-mft = Fast NTFS scan (MFT)

## Help
help-about = About StorageMap
about-body = A fast disk space analyzer for Windows and Linux.
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
action-open-snapshot = Open saved scan…
snapshot-open = Open
snapshot-open-hint = Browse this saved scan in its own tab, as the folder was then
saved-scan-from = Saved scan from { $date }
saved-read-only = A saved scan shows the folder as it was; scan the folder again to act on its files.
saved-small-files =
    { $count ->
        [one] 1 smaller file
       *[other] { $n } smaller files
    }
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
dup-sha256 = SHA-256
dup-sha256-hint = Also compute a SHA-256 checksum of each group, to compare with other tools. Reads each group once more.
dup-col-remove = Remove
dup-col-group = Group
dup-mark-hint = Mark this copy to move to the trash or replace with a hard link
dup-keep = Keep:
dup-keep-newest = the newest copy
dup-keep-oldest = the oldest copy
dup-keep-shortest = the shortest path
dup-mark-others = Mark the other copies
dup-mark-others-hint = In every group, mark all copies except the one the rule keeps
dup-clear-marks = Clear marks
dup-marked = { $count } marked · { $size }
dup-trash = Move to trash
dup-link = Replace with hard links
dup-link-hint = Keep one copy on disk and make the others point to it. Needs every copy on the same drive.
dup-working = Working… { $done } of { $total }
dup-confirm-trash = Move the marked copies to the trash?
dup-confirm-trash-body = { $count } files ({ $size }) go to the trash. One copy of each group always stays.
dup-confirm-link = Replace the marked copies with hard links?
dup-confirm-link-body = { $count } files ({ $size }) become hard links to the copy kept, so they stop taking space. Each pair is checked byte by byte first. Afterwards both names share one copy: changing one changes the other.
dup-more =
    { $count ->
        [one] and 1 more
       *[other] and { $n } more
    }
dup-trashed =
    { $count ->
        [one] 1 copy moved to the trash
       *[other] { $n } copies moved to the trash
    }
dup-linked =
    { $count ->
        [one] 1 copy replaced with a hard link
       *[other] { $n } copies replaced with hard links
    }
dup-failed = { $count } failed: { $error }
dup-hint = Finds files with identical content in the current folder. Files are matched by size, then by a quick fingerprint of their start and end, and only then by a full content hash. Hard links are ignored because they already share space.

## Settings, updates and polish
menu-settings = Settings…
action-zoom-in = Zoom in
action-zoom-out = Zoom out
action-zoom-reset = Actual size
action-check-updates = Check for updates…
action-open-github = StorageMap on GitHub
start-drop-hint = or drop a folder anywhere on this window
settings-title = Settings
settings-general = General
settings-appearance = Appearance
settings-scanning = Scanning
settings-integration = Integration
settings-updates = Updates
settings-about = About
setting-language = Language
setting-scale = Interface size
setting-scale-hint = Makes text and controls larger or smaller. Ctrl + and Ctrl − work too.
setting-treemap-depth-hint = How many folder levels the treemap draws. Fewer levels draw faster.
setting-auto-snapshot-hint = Lets you see what changed the next time you scan the same folder. Snapshots stay on this computer.
setting-cross-fs-hint = Follow mount points into other disks and network shares.
scan-exclude = Leave out of scans
scan-exclude-hint = One pattern per line. A name such as node_modules or *.iso matches anywhere; a path such as build/cache matches from the scanned folder. * matches anything, ? one character.
scan-include = Count only these files
scan-include-hint = One pattern per line, such as *.mp4. Leave empty to count every file. Folders are always scanned.
setting-mft-hint = Reads the NTFS master file table directly — many times faster. Needs administrator rights.
integration-hint = Adds “Open with StorageMap” for folders in your file manager. Only your user account is affected.
setting-auto-update = Check for updates automatically
setting-auto-update-hint = Asks GitHub once a day whether a newer version exists. Nothing about you or your files is sent.
version-current = Installed version: { $version }
update-check-now = Check now
update-checking = Checking for updates…
update-up-to-date = You have the latest version.
update-available = StorageMap { $version } is available
update-install = Download and install
update-manual = Open download page
update-notes = What’s new
update-downloading = Downloading… { $done } of { $total }
update-ready = Installing…
update-failed = Update failed: { $error }
update-restart = Update installed. Restart StorageMap to use the new version.
update-handed-off = The installer has opened — follow its steps to finish.
update-later = Later
about-privacy = Your scans, snapshots and duplicate searches never leave this computer. The only network request is the optional daily update check.
about-source = Source code, releases and issues
update-skip = Skip this version
update-restart-now = Restart now
update-no-notes = No release notes were published for this version.
settings-shortcuts = Keyboard shortcuts
shortcut-enter = Open the selected folder
shortcut-arrows = Move through the folder tree
shortcut-double-click = Zoom into a folder in the treemap
search-empty = Type part of a file or folder name above to find it in the current folder.
dup-empty-title = Find files that waste space
empty-folder-hint = Pick another folder in the tree, or go up a level.

## Cleanup
tab-cleanup = Cleanup
cleanup-total = { $size } can probably be freed
cleanup-hint = These are places that usually hold disposable data. Review each item before moving it to the trash — you can restore anything from the trash.
cleanup-none-title = Nothing obvious to clean up
cleanup-none = No caches, build output, old downloads or other disposable data were found in this folder.
cleanup-all = Show all { $count }
cleanup-fewer = Show fewer
cleanup-trash = Trash
cleanup-trash-hint = Files you already deleted. Emptying the trash frees this space for good.
cleanup-cache = Application caches
cleanup-cache-hint = Data apps keep to start faster. Apps rebuild it when needed; close them first.
cleanup-build = Build output
cleanup-build-hint = node_modules, Rust target folders and similar. Regenerated by the next build or install.
cleanup-packages = Package caches
cleanup-packages-hint = Downloaded packages kept by Cargo, npm, Gradle, Maven, Go and others. Downloaded again when needed.
cleanup-temp = Temporary files
cleanup-temp-hint = Leftovers from installers and apps. Safe to remove when the apps that made them are closed.
cleanup-logs = Large log files
cleanup-logs-hint = Log files over 50 MB. Keep them only if you are troubleshooting something.
cleanup-dumps = Crash dumps
cleanup-dumps-hint = Memory dumps written when a program crashed. Useful only to developers.
cleanup-downloads = Old downloads
cleanup-downloads-hint = Large files in Downloads you have not touched for six months.
