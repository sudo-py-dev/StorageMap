app-title = StorageMap

## Menus
menu-file = קובץ
menu-view = תצוגה
menu-language = שפה
menu-tools = כלים
menu-help = עזרה
lang-system = ברירת המחדל של המערכת

## Actions
action-scan-folder = סריקת תיקייה…
action-rescan = סריקה מחדש
action-stop = עצירה
action-export-csv = ייצוא ל־Excel ‏(CSV)…
action-export-short = ייצוא
action-export-hint = שמירת תוכן התיקייה כדוח CSV שנפתח ב־Excel או בכל גיליון אלקטרוני
csv-bytes = גודל (בתים)
csv-allocated-bytes = בדיסק (בתים)
csv-modified-utc = תאריך שינוי (UTC)
export-title = ייצוא ל־Excel ‏(CSV)
export-include = מה לכלול
export-include-all = הכול
export-include-folders = תיקיות בלבד
export-include-files = קבצים בלבד
export-sizes = גדלים
export-sizes-readable = קריא (1.4 ג״ב)
export-sizes-bytes = בתים מדויקים
export-save = שמירה…
export-copy = העתקה
export-copy-hint = העתקת התיקייה הזו ומה שנמצא ישירות בתוכה, מוכן להדבקה בגיליון אלקטרוני.
report-copied = הדוח הועתק
action-quit = יציאה
action-up = למעלה
action-open = פתיחה
action-show-in-folder = הצגה במנהל הקבצים
action-copy-path = העתקת נתיב
action-trash = העברה לאשפה…
action-scan-this = סריקת תיקייה זו
action-collapse-all = כיווץ הכול
tree-filter-all = כל הקבצים
tree-filter-types = סוג
tree-filter-modified = שינוי אחרון
action-run-as-admin = הפעלה מחדש כמנהל מערכת
action-integrate = הוספת „פתיחה ב־StorageMap” למנהל הקבצים
action-unintegrate = הסרת השילוב ממנהל הקבצים
action-home = מסך הפתיחה

## Metrics
metric-size = גודל
metric-allocated = גודל בדיסק
metric-files = מספר קבצים
metric-label = מדידה לפי

## Tabs
tab-treemap = מפת עץ
tab-details = פרטים
tab-top-files = הקבצים הגדולים
tab-types = סוגי קבצים
tab-ages = גיל
tab-chart = תרשים
chart-donut = טבעת
chart-bars = עמודות
chart-other =
    { $count ->
        [one] פריט אחר
       *[other] { $n } פריטים אחרים
    }
tab-search = חיפוש

## Columns
columns-menu = עמודות
column-earlier = הזזה קדימה
column-later = הזזה אחורה
columns-reset = איפוס עמודות
col-name = שם
col-size = גודל
col-allocated = בדיסק
col-percent = % מההורה
col-files = קבצים
col-folders = תיקיות
col-modified = תאריך שינוי
col-accessed = גישה אחרונה
col-created = תאריך יצירה
col-attributes = מאפיינים
col-owner = בעלים
owner-unknown = בעלים לא ידועים
owner-id = משתמש { $id }
types-by-type = לפי סוג
types-by-owner = לפי בעלים
attr-read-only = לקריאה בלבד
attr-hidden = מוסתר
attr-system = מערכת
attr-compressed = דחוס
attr-encrypted = מוצפן
attr-sparse = דליל
col-type = סוג
col-path = נתיב
col-extension = סיומת
col-share = חלק

## Start screen
start-title = לאן נעלם המקום שלי?
start-subtitle = בחרו כונן או תיקייה למיפוי.
start-drives = כוננים
start-recent = תיקיות אחרונות
volume-free = { $free } פנויים מתוך { $total }
volume-fixed = דיסק מקומי
volume-removable = נשלף
volume-network = רשת
volume-optical = אופטי
volume-ram = דיסק RAM
volume-unknown = אמצעי אחסון

## Scanning
scan-progress = סורק… { $files } קבצים ב־{ $dirs } תיקיות · { $size }
tab-new = סריקת תיקייה נוספת
tab-close = סגירה
wait-for-scan = יש להמתין לסיום הסריקה
scan-live-hint = אפשר כבר לעיין בתיקיות שהסריקה שלהן הסתיימה
scan-done = { $items } פריטים ב־{ $seconds } שניות
scan-cancelled = הסריקה נעצרה – מוצגות תוצאות חלקיות
scan-errors =
    { $count ->
        [one] לא ניתן היה לקרוא פריט אחד
       *[other] לא ניתן היה לקרוא { $n } פריטים
    }
scan-failed = הסריקה נכשלה: { $error }
backend-linux = מנוע statx
backend-mft = קריאה ישירה של NTFS MFT
backend-dirinfo = מנוע התיקיות של Windows
backend-portable = מנוע רגיל
backend-source = מנוע { $name }
admin-hint = הפעילו כמנהל מערכת כדי לקרוא ישירות את טבלת הקבצים הראשית של NTFS – בדרך כלל מהיר פי כמה.

## Trash
confirm-trash-title = להעביר לאשפה?
confirm-trash-body = „{ $name }” ({ $size }) יועבר לאשפה.
confirm-trash = העברה לאשפה
cancel = ביטול
trash-failed = לא ניתן היה להעביר לאשפה: { $error }
trashed = „{ $name }” הועבר לאשפה
action-empty-trash = ריקון האשפה
confirm-empty-title = לרוקן את האשפה?
confirm-empty-body = כל מה שבאשפה ({ $size }) יימחק לצמיתות. לא ניתן לבטל פעולה זו.
emptying-trash = מרוקן את האשפה…
trash-emptied = האשפה רוקנה — פונו { $size }
empty-trash-failed = לא ניתן היה לרוקן את האשפה: { $error }
already-in-trash = הפריט כבר נמצא באשפה

## Misc
search-hint = חיפוש לפי שם…
search-clear = ניקוי החיפוש
search-filters = מסננים
search-saved = שמורים
search-saved-none = אין עדיין חיפושים שמורים
search-saved-name = שם
search-save = שמירה
search-saved-delete = מחיקת החיפוש השמור
search-export = ייצוא…
search-copy-hint = העתקת התוצאות, מוכנות להדבקה בגיליון אלקטרוני.
filter-everything = קבצים ותיקיות
filter-files = קבצים
filter-folders = תיקיות
filter-at-least = לפחות
filter-at-most = לכל היותר
filter-any-size = כל גודל
filter-any-time = שונה בכל זמן
filter-within-week = שונה ב־7 הימים האחרונים
filter-within-month = שונה ב־30 הימים האחרונים
filter-within-year = שונה בשנה האחרונה
filter-older-year = לא שונה שנה
filter-older-two-years = לא שונה שנתיים
filter-older-five-years = לא שונה 5 שנים
filter-any-owner = כל הבעלים
filter-any-attribute = כל המאפיינים
filter-clear = ניקוי המסננים
search-results =
    { $count ->
        [one] תוצאה אחת
       *[other] { $n } תוצאות
    }
empty-folder = התיקייה ריקה.
files-count =
    { $count ->
        [one] קובץ אחד
        [two] שני קבצים
       *[other] { $n } קבצים
    }
folders-count =
    { $count ->
        [one] תיקייה אחת
        [two] שתי תיקיות
       *[other] { $n } תיקיות
    }
type-folder = תיקייה
type-file = קובץ
type-link = קישור
type-other = אחר
no-extension = (ללא סיומת)
age-week = 7 הימים האחרונים
age-month = 30 הימים האחרונים
age-half-year = חצי השנה האחרונה
age-year = השנה האחרונה
age-two-years = 1–2 שנים
age-five-years = 2–5 שנים
age-older = יותר מ־5 שנים
note-hardlink = קישור קשיח – נספר פעם אחת
note-mount = מערכת קבצים אחרת – לא נסרקה
note-error = לא ניתן היה לקרוא
note-incomplete = לא נסרק (נעצר)
percent-of-parent = { $percent } מההורה
copied = הנתיב הועתק
exported = יוצא אל { $path }
export-failed = הייצוא נכשל: { $error }
integration-done = השילוב במנהל הקבצים הותקן
integration-removed = השילוב במנהל הקבצים הוסר
integration-failed = השילוב נכשל: { $error }
open-failed = לא ניתן היה לפתוח: { $error }
free-on-volume = { $free } פנויים באמצעי האחסון

## View options
view-theme = ערכת נושא
theme-system = לפי המערכת
theme-light = בהירה
theme-dark = כהה
view-units = יחידות
units-auto = אוטומטי
view-decimals = ספרות אחרי הנקודה
treemap-depth = רמת פירוט מפת העץ
treemap-files = הצגת קבצים במפת העץ
treemap-labels = הצגת תוויות
highlight-large = הדגשת פריטים גדולים
setting-highlight-large-hint = צביעת השמות של קבצים ותיקיות שתופסים חלק גדול מהתיקייה שלהם.
option-cross-fs = לסרוק גם מערכות קבצים מעוגנות אחרות
option-mft = סריקת NTFS מהירה (MFT)

## Help
help-about = אודות StorageMap
about-body = כלי מהיר לניתוח שטח דיסק עבור Windows ו־Linux.
about-version = גרסה { $version }

## Units
unit-b = ב׳
unit-kb = ק״ב
unit-mb = מ״ב
unit-gb = ג״ב
unit-tb = ט״ב

## File categories
cat-image = תמונות
cat-video = וידאו
cat-audio = שמע
cat-archive = ארכיונים וחבילות
cat-document = מסמכים
cat-code = קוד ותוצרי בנייה
cat-executable = תוכניות וספריות
cat-disk-image = תמונות דיסק
cat-other = אחר

## Snapshots and changes
tab-changes = שינויים
compare-title = השוואה לסריקה קודמת
compare-none = עדיין אין סריקות קודמות של התיקייה הזו. כל סריקה נשמרת כתמונת מצב, כך שבפעם הבאה תראו בדיוק מה גדל ומה נעלם.
compare-browse = פתיחת קובץ תמונת מצב…
compare-summary = { $change } מאז { $date }
compare-other = השוואה אל…
compare-clear = הפסקת ההשוואה
compare-unchanged = שום דבר כאן לא השתנה מאז תמונת המצב.
compare-failed = לא ניתן להשוות: { $error }
snapshot-entry = { $date } · { $size }
action-save-snapshot = שמירת תמונת מצב עכשיו
snapshot-saved = תמונת המצב נשמרה
snapshot-failed = לא ניתן היה לשמור את תמונת המצב: { $error }
option-auto-snapshot = שמירת תמונת מצב אחרי כל סריקה
col-before = לפני
col-now = עכשיו
col-change = שינוי
change-new = חדש
change-removed = נמחק

## Duplicates
tab-duplicates = כפילויות
dup-find = חיפוש כפילויות
dup-min-size = גודל מינימלי
dup-scope = ב־{ $path }
dup-progress = משווה תוכן… { $files } מתוך { $total } קבצים · נקראו { $size }
dup-copies =
    { $count ->
        [one] עותק אחד
        [two] שני עותקים
       *[other] { $n } עותקים
    }
dup-groups =
    { $count ->
        [one] קבוצת כפילויות אחת
        [two] שתי קבוצות כפילויות
       *[other] { $n } קבוצות כפילויות
    }
dup-each = { $size } כל אחד
dup-reclaimable = ניתן לפנות { $size }
dup-none = לא נמצאו קבצים כפולים.
dup-sha256 = SHA-256
dup-sha256-hint = חישוב גם של סכום ביקורת SHA-256 לכל קבוצה, להשוואה עם כלים אחרים. כל קבוצה נקראת פעם נוספת.
dup-col-remove = הסרה
dup-col-group = קבוצה
dup-mark-hint = סימון העותק הזה להעברה לאשפה או להחלפה בקישור קשיח
dup-keep = לשמור:
dup-keep-newest = את העותק החדש ביותר
dup-keep-oldest = את העותק הישן ביותר
dup-keep-shortest = את הנתיב הקצר ביותר
dup-mark-others = סימון שאר העותקים
dup-mark-others-hint = בכל קבוצה, סימון כל העותקים מלבד זה שהכלל שומר
dup-clear-marks = ניקוי הסימונים
dup-marked = { $count } מסומנים · { $size }
dup-trash = העברה לאשפה
dup-link = החלפה בקישורים קשיחים
dup-link-hint = שמירת עותק אחד בדיסק והפניית האחרים אליו. כל העותקים צריכים להיות באותו כונן.
dup-working = מתבצע… { $done } מתוך { $total }
dup-confirm-trash = להעביר את העותקים המסומנים לאשפה?
dup-confirm-trash-body = { $count } קבצים ({ $size }) יועברו לאשפה. עותק אחד מכל קבוצה תמיד נשאר.
dup-confirm-link = להחליף את העותקים המסומנים בקישורים קשיחים?
dup-confirm-link-body = { $count } קבצים ({ $size }) יהפכו לקישורים קשיחים לעותק שנשמר, וכך יפסיקו לתפוס מקום. כל זוג נבדק קודם בייט אחר בייט. לאחר מכן שני השמות חולקים עותק אחד: שינוי של אחד משנה גם את השני.
dup-more =
    { $count ->
        [one] ועוד אחד
       *[other] ועוד { $n }
    }
dup-trashed =
    { $count ->
        [one] עותק אחד הועבר לאשפה
       *[other] { $n } עותקים הועברו לאשפה
    }
dup-linked =
    { $count ->
        [one] עותק אחד הוחלף בקישור קשיח
       *[other] { $n } עותקים הוחלפו בקישורים קשיחים
    }
dup-failed = { $count } נכשלו: { $error }
dup-hint = מאתר קבצים בעלי תוכן זהה בתיקייה הנוכחית. הקבצים מושווים לפי גודל, אחר כך לפי טביעת אצבע מהירה של ההתחלה והסוף, ורק לבסוף לפי גיבוב מלא של התוכן. קישורים קשיחים אינם נכללים כי הם כבר חולקים את אותו מקום.

## Settings, updates and polish
menu-settings = הגדרות…
action-zoom-in = הגדלה
action-zoom-out = הקטנה
action-zoom-reset = גודל מקורי
action-check-updates = בדיקת עדכונים…
action-open-github = StorageMap ב־GitHub
start-drop-hint = או גררו תיקייה לכל מקום בחלון
settings-title = הגדרות
settings-general = כללי
settings-appearance = מראה
settings-scanning = סריקה
settings-integration = שילוב
settings-updates = עדכונים
settings-about = אודות
setting-language = שפה
setting-scale = גודל הממשק
setting-scale-hint = מגדיל או מקטין טקסט ופקדים. גם Ctrl + ו־Ctrl − עובדים.
setting-treemap-depth-hint = כמה רמות תיקיות מפת העץ מציירת. פחות רמות מצוירות מהר יותר.
setting-auto-snapshot-hint = מאפשר לראות מה השתנה בפעם הבאה שתסרקו את אותה תיקייה. תמונות המצב נשמרות במחשב הזה בלבד.
setting-cross-fs-hint = מעבר דרך נקודות עיגון לדיסקים אחרים ולשיתופי רשת.
scan-exclude = להשמיט מהסריקות
scan-exclude-hint = תבנית אחת בכל שורה. שם כמו node_modules או ‎*.iso‎ מתאים בכל מקום; נתיב כמו build/cache מתאים מהתיקייה הנסרקת. * מתאים לכל דבר, ? לתו אחד.
scan-include = לספור רק את הקבצים האלה
scan-include-hint = תבנית אחת בכל שורה, למשל ‎*.mp4‎. השאירו ריק כדי לספור את כל הקבצים. התיקיות נסרקות תמיד.
setting-mft-hint = קורא ישירות את טבלת הקבצים הראשית של NTFS — מהיר פי כמה. דורש הרשאות מנהל.
integration-hint = מוסיף „פתיחה ב־StorageMap” לתיקיות במנהל הקבצים. משפיע רק על חשבון המשתמש שלכם.
setting-auto-update = בדיקת עדכונים אוטומטית
setting-auto-update-hint = שואל את GitHub פעם ביום אם יש גרסה חדשה יותר. שום מידע עליכם או על הקבצים שלכם לא נשלח.
version-current = הגרסה המותקנת: { $version }
update-check-now = בדיקה עכשיו
update-checking = בודק עדכונים…
update-up-to-date = הגרסה שלכם היא העדכנית ביותר.
update-available = StorageMap { $version } זמינה
update-install = הורדה והתקנה
update-manual = פתיחת דף ההורדה
update-notes = מה חדש
update-downloading = מוריד… { $done } מתוך { $total }
update-ready = מתקין…
update-failed = העדכון נכשל: { $error }
update-restart = העדכון הותקן. הפעילו מחדש את StorageMap כדי להשתמש בגרסה החדשה.
update-handed-off = תוכנית ההתקנה נפתחה — המשיכו לפי השלבים שלה.
update-later = אחר כך
about-privacy = הסריקות, תמונות המצב וחיפושי הכפילויות שלכם לעולם לא יוצאים מהמחשב. בקשת הרשת היחידה היא בדיקת העדכונים היומית, שאפשר לכבות.
about-source = קוד מקור, גרסאות ודיווח על בעיות
update-skip = דילוג על גרסה זו
update-restart-now = הפעלה מחדש עכשיו
update-no-notes = לא פורסמו הערות שחרור לגרסה זו.
settings-shortcuts = קיצורי מקלדת
shortcut-enter = פתיחת התיקייה הנבחרת
shortcut-arrows = ניווט בעץ התיקיות
shortcut-double-click = התמקדות בתיקייה במפת העץ
search-empty = הקלידו בשדה שלמעלה חלק משם של קובץ או תיקייה כדי למצוא אותו בתיקייה הנוכחית.
dup-empty-title = מציאת קבצים שמבזבזים מקום
empty-folder-hint = בחרו תיקייה אחרת בעץ, או עלו רמה אחת.

## Cleanup
tab-cleanup = ניקוי
cleanup-total = כנראה אפשר לפנות { $size }
cleanup-hint = אלה מקומות שבדרך כלל מכילים נתונים זמניים. בדקו כל פריט לפני העברתו לאשפה — אפשר לשחזר הכול מהאשפה.
cleanup-none-title = אין מה לנקות כרגע
cleanup-none = לא נמצאו בתיקייה הזו מטמונים, תוצרי בנייה, הורדות ישנות או נתונים זמניים אחרים.
cleanup-all = הצגת כל { $count }
cleanup-fewer = הצגת פחות
cleanup-trash = אשפה
cleanup-trash-hint = קבצים שכבר מחקתם. ריקון האשפה מפנה את המקום לצמיתות.
cleanup-cache = מטמוני יישומים
cleanup-cache-hint = נתונים שיישומים שומרים כדי לעלות מהר יותר. הם נבנים מחדש לפי הצורך; סגרו את היישומים קודם.
cleanup-build = תוצרי בנייה
cleanup-build-hint = ‏node_modules, תיקיות target של Rust ודומיהן. נוצרים מחדש בבנייה או בהתקנה הבאה.
cleanup-packages = מטמוני חבילות
cleanup-packages-hint = חבילות שהורדו ונשמרו על ידי Cargo,‏ npm,‏ Gradle,‏ Maven,‏ Go ואחרים. יורדו שוב לפי הצורך.
cleanup-temp = קבצים זמניים
cleanup-temp-hint = שאריות של תוכניות התקנה ויישומים. בטוח למחוק כשהיישומים שיצרו אותן סגורים.
cleanup-logs = קובצי יומן גדולים
cleanup-logs-hint = קובצי יומן מעל 50 מ״ב. שמרו אותם רק אם אתם מאבחנים תקלה.
cleanup-dumps = קובצי קריסה
cleanup-dumps-hint = תמונות זיכרון שנכתבו כשתוכנית קרסה. שימושיות רק למפתחים.
cleanup-downloads = הורדות ישנות
cleanup-downloads-hint = קבצים גדולים בתיקיית ההורדות שלא נגעתם בהם חצי שנה.
