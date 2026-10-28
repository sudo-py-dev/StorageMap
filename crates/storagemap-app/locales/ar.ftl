app-title = StorageMap

## Menus
menu-file = ملف
menu-view = عرض
menu-language = اللغة
menu-tools = أدوات
menu-help = مساعدة
lang-system = لغة النظام

## Actions
action-scan-folder = فحص مجلد…
action-rescan = إعادة الفحص
action-stop = إيقاف
action-export-csv = تصدير CSV…
action-quit = خروج
action-up = لأعلى
action-open = فتح
action-show-in-folder = إظهار في مدير الملفات
action-copy-path = نسخ المسار
action-trash = نقل إلى سلة المحذوفات…
action-scan-this = فحص هذا المجلد
action-collapse-all = طي الكل
action-run-as-admin = إعادة التشغيل كمسؤول
action-integrate = إضافة «فتح باستخدام StorageMap» إلى مدير الملفات
action-unintegrate = إزالة التكامل مع مدير الملفات
action-home = شاشة البداية

## Metrics
metric-size = الحجم
metric-allocated = الحجم على القرص
metric-files = عدد الملفات
metric-label = القياس حسب

## Tabs
tab-treemap = خريطة شجرية
tab-details = التفاصيل
tab-top-files = أكبر الملفات
tab-types = أنواع الملفات
tab-ages = العمر
tab-search = بحث

## Columns
col-name = الاسم
col-size = الحجم
col-allocated = على القرص
col-percent = ٪ من الأصل
col-files = الملفات
col-folders = المجلدات
col-modified = آخر تعديل
col-type = النوع
col-path = المسار
col-extension = الامتداد
col-share = الحصة

## Start screen
start-title = أين ذهبت مساحتي؟
start-subtitle = اختر قرصًا أو مجلدًا لرسم خريطته.
start-drives = الأقراص
start-recent = المجلدات الأخيرة
volume-free = { $free } متاحة من { $total }
volume-fixed = قرص محلي
volume-removable = قابل للإزالة
volume-network = شبكة
volume-optical = قرص ضوئي
volume-ram = قرص RAM
volume-unknown = وحدة تخزين

## Scanning
scan-progress = جارٍ الفحص… { $files } ملف في { $dirs } مجلد · { $size }
scan-done = { $items } عنصر في { $seconds } ث
scan-cancelled = تم إيقاف الفحص – تُعرض نتائج جزئية
scan-errors =
    { $count ->
        [one] تعذّرت قراءة عنصر واحد
        [two] تعذّرت قراءة عنصرين
        [few] تعذّرت قراءة { $n } عناصر
       *[other] تعذّرت قراءة { $n } عنصر
    }
scan-failed = فشل الفحص: { $error }
backend-linux = محرك statx
backend-mft = قراءة مباشرة لجدول NTFS MFT
backend-dirinfo = محرك مجلدات Windows
backend-portable = المحرك القياسي
admin-hint = شغّل البرنامج كمسؤول لقراءة جدول الملفات الرئيسي في NTFS مباشرة – عادةً أسرع بعدة مرات.

## Trash
confirm-trash-title = نقل إلى سلة المحذوفات؟
confirm-trash-body = سيتم نقل «{ $name }» ({ $size }) إلى سلة المحذوفات.
confirm-trash = نقل إلى السلة
cancel = إلغاء
trash-failed = تعذّر النقل إلى سلة المحذوفات: { $error }
trashed = تم نقل «{ $name }» إلى سلة المحذوفات

## Misc
search-hint = البحث بالاسم…
search-results =
    { $count ->
        [zero] لا توجد نتائج
        [one] نتيجة واحدة
        [two] نتيجتان
        [few] { $n } نتائج
       *[other] { $n } نتيجة
    }
empty-folder = هذا المجلد فارغ.
files-count =
    { $count ->
        [zero] لا ملفات
        [one] ملف واحد
        [two] ملفان
        [few] { $n } ملفات
       *[other] { $n } ملف
    }
folders-count =
    { $count ->
        [zero] لا مجلدات
        [one] مجلد واحد
        [two] مجلدان
        [few] { $n } مجلدات
       *[other] { $n } مجلد
    }
type-folder = مجلد
type-file = ملف
type-link = رابط
type-other = أخرى
no-extension = (بدون امتداد)
age-week = آخر 7 أيام
age-month = آخر 30 يومًا
age-half-year = آخر 6 أشهر
age-year = آخر سنة
age-two-years = 1–2 سنة
age-five-years = 2–5 سنوات
age-older = أقدم من 5 سنوات
note-hardlink = رابط ثابت – يُحسب مرة واحدة
note-mount = نظام ملفات آخر – لم يُفحص
note-error = تعذّرت القراءة
note-incomplete = لم يُفحص (تم الإيقاف)
percent-of-parent = { $percent } من الأصل
copied = تم نسخ المسار
exported = تم التصدير إلى { $path }
export-failed = فشل التصدير: { $error }
integration-done = تم تثبيت التكامل مع مدير الملفات
integration-removed = تمت إزالة التكامل مع مدير الملفات
integration-failed = فشل التكامل: { $error }
open-failed = تعذّر الفتح: { $error }
free-on-volume = { $free } متاحة في وحدة التخزين

## View options
view-theme = السمة
theme-system = حسب النظام
theme-light = فاتحة
theme-dark = داكنة
view-units = الوحدات
units-auto = تلقائي
view-decimals = المنازل العشرية
treemap-depth = تفاصيل الخريطة الشجرية
treemap-files = إظهار الملفات في الخريطة
treemap-labels = إظهار التسميات
option-cross-fs = فحص أنظمة الملفات المركّبة الأخرى أيضًا
option-mft = فحص NTFS السريع (MFT)

## Help
help-about = حول StorageMap
about-body = أداة سريعة لتحليل مساحة القرص لنظامي Windows وLinux. يتم الفحص محليًا؛ لا شيء يغادر جهازك.
about-version = الإصدار { $version }

## Units
unit-b = بايت
unit-kb = ك.ب
unit-mb = م.ب
unit-gb = ج.ب
unit-tb = ت.ب

## File categories
cat-image = صور
cat-video = فيديو
cat-audio = صوت
cat-archive = أرشيفات وحزم
cat-document = مستندات
cat-code = شيفرة ومخرجات البناء
cat-executable = برامج ومكتبات
cat-disk-image = صور أقراص
cat-other = أخرى

## Snapshots and changes
tab-changes = التغييرات
compare-title = المقارنة بفحص سابق
compare-none = لا توجد فحوصات سابقة لهذا المجلد بعد. يُحفظ كل فحص كلقطة، لذا سترى في المرة القادمة بالضبط ما الذي كبر وما الذي اختفى.
compare-browse = فتح ملف لقطة…
compare-summary = { $change } منذ { $date }
compare-other = المقارنة مع…
compare-clear = إيقاف المقارنة
compare-unchanged = لم يتغير شيء هنا منذ اللقطة.
compare-failed = تعذّرت المقارنة: { $error }
snapshot-entry = { $date } · { $size }
action-save-snapshot = حفظ لقطة الآن
snapshot-saved = تم حفظ اللقطة
snapshot-failed = تعذّر حفظ اللقطة: { $error }
option-auto-snapshot = حفظ لقطة بعد كل فحص
col-before = قبل
col-now = الآن
col-change = التغيير
change-new = جديد
change-removed = محذوف

## Duplicates
tab-duplicates = الملفات المكررة
dup-find = البحث عن المكررات
dup-min-size = الحد الأدنى للحجم
dup-scope = في { $path }
dup-progress = جارٍ مقارنة المحتوى… { $files } من { $total } ملف · تمت قراءة { $size }
dup-copies =
    { $count ->
        [one] نسخة واحدة
        [two] نسختان
        [few] { $n } نسخ
       *[other] { $n } نسخة
    }
dup-groups =
    { $count ->
        [one] مجموعة مكررات واحدة
        [two] مجموعتا مكررات
        [few] { $n } مجموعات مكررات
       *[other] { $n } مجموعة مكررات
    }
dup-each = { $size } لكل منها
dup-reclaimable = يمكن تحرير { $size }
dup-none = لم يتم العثور على ملفات مكررة.
dup-hint = يعثر على الملفات ذات المحتوى المتطابق في المجلد الحالي. تُقارن الملفات حسب الحجم أولًا، ثم ببصمة سريعة لبدايتها ونهايتها، وأخيرًا بتجزئة كاملة للمحتوى. يتم تجاهل الروابط الثابتة لأنها تتشارك المساحة نفسها أصلًا.
