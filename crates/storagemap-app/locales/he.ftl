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
action-export-csv = ייצוא ל־CSV…
action-quit = יציאה
action-up = למעלה
action-open = פתיחה
action-show-in-folder = הצגה במנהל הקבצים
action-copy-path = העתקת נתיב
action-trash = העברה לאשפה…
action-scan-this = סריקת תיקייה זו
action-collapse-all = כיווץ הכול
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
tab-search = חיפוש

## Columns
col-name = שם
col-size = גודל
col-allocated = בדיסק
col-percent = % מההורה
col-files = קבצים
col-folders = תיקיות
col-modified = תאריך שינוי
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
admin-hint = הפעילו כמנהל מערכת כדי לקרוא ישירות את טבלת הקבצים הראשית של NTFS – בדרך כלל מהיר פי כמה.

## Trash
confirm-trash-title = להעביר לאשפה?
confirm-trash-body = „{ $name }” ({ $size }) יועבר לאשפה.
confirm-trash = העברה לאשפה
cancel = ביטול
trash-failed = לא ניתן היה להעביר לאשפה: { $error }
trashed = „{ $name }” הועבר לאשפה

## Misc
search-hint = חיפוש לפי שם…
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
option-cross-fs = לסרוק גם מערכות קבצים מעוגנות אחרות
option-mft = סריקת NTFS מהירה (MFT)

## Help
help-about = אודות StorageMap
about-body = כלי מהיר לניתוח שטח דיסק עבור Windows ו־Linux. הסריקה מתבצעת מקומית; שום מידע לא יוצא מהמחשב.
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
dup-hint = מאתר קבצים בעלי תוכן זהה בתיקייה הנוכחית. הקבצים מושווים לפי גודל, אחר כך לפי טביעת אצבע מהירה של ההתחלה והסוף, ורק לבסוף לפי גיבוב מלא של התוכן. קישורים קשיחים אינם נכללים כי הם כבר חולקים את אותו מקום.
