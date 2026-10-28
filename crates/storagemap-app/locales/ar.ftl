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
action-export-csv = تصدير تقرير…
action-export-short = تصدير
action-export-hint = حفظ محتويات هذا المجلد كتقرير: Excel أو CSV أو صفحة ويب أو JSON أو نص
csv-bytes = الحجم (بايت)
csv-allocated-bytes = على القرص (بايت)
csv-modified-utc = تاريخ التعديل (UTC)
export-title = تصدير تقرير
export-format = التنسيق
export-format-csv = CSV
export-format-excel = Excel
export-format-html = صفحة ويب
export-format-json = JSON
export-format-text = نص
export-depth = مستويات المجلدات
export-depth-levels =
    { $count ->
        [one] مستوى واحد
        [two] مستويان
        [few] { $count } مستويات
       *[other] { $count } مستوى
    }
export-depth-all = كل المستويات
export-columns = الأعمدة
export-include = عرض
export-include-all = الكل
export-include-folders = المجلدات فقط
export-include-files = الملفات فقط
export-sizes = الأحجام
export-sizes-readable = مقروءة (1.4 ج.ب)
export-sizes-bytes = بايت بالضبط
export-save = حفظ…
export-copy = نسخ
export-copy-hint = نسخ هذا المجلد وما يحتويه مباشرةً، جاهزًا للصق في جدول بيانات.
report-copied = تم نسخ التقرير
action-quit = خروج
action-up = لأعلى
action-open = فتح
action-show-in-folder = إظهار في مدير الملفات
action-copy-path = نسخ المسار
action-trash = نقل إلى سلة المحذوفات…
action-scan-this = فحص هذا المجلد
action-collapse-all = طي الكل
tree-filter-all = كل الملفات
tree-filter-types = النوع
tree-filter-modified = آخر تعديل
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
tab-chart = مخطط
chart-donut = حلقي
chart-bars = أشرطة
chart-other =
    { $count ->
        [zero] لا عناصر أخرى
        [one] عنصر آخر
        [two] عنصران آخران
        [few] { $n } عناصر أخرى
        [many] { $n } عنصرًا آخر
       *[other] { $n } عنصر آخر
    }
tab-search = بحث

## Columns
columns-menu = الأعمدة
col-mark = ✔
mark-hint = تحديد للنقل أو النسخ أو الضغط أو إعادة التسمية أو سلة المهملات (Ctrl مع النقر على صف يحدده أيضًا)
marked-summary = { $count } محددة · { $size }
marks-clear = مسح التحديد
search-mark-all = تحديد الكل
search-mark-all-hint = تحديد كل النتائج في هذا الفحص للعمل عليها معًا
bulk-move = نقل إلى…
bulk-copy = نسخ إلى…
bulk-zip = ضغط…
bulk-rename = إعادة تسمية…
bulk-trash = نقل إلى سلة المهملات
bulk-move-title = نقل العناصر المحددة إلى
bulk-copy-title = نسخ العناصر المحددة إلى
bulk-zip-title = حفظ الأرشيف باسم
bulk-trash-title = نقل العناصر المحددة إلى سلة المهملات؟
bulk-trash-body = سيُنقل { $count } عنصرًا ({ $size }) إلى سلة المهملات. يمكنك استعادتها من هناك.
bulk-moving = جارٍ النقل… { $done } من { $total }
bulk-copying = جارٍ النسخ… { $done } من { $total }
bulk-zipping = جارٍ الضغط… { $done } من { $total }
bulk-trashing = جارٍ النقل إلى سلة المهملات… { $done } من { $total }
bulk-renaming = جارٍ إعادة التسمية… { $done } من { $total }
bulk-moved =
    { $count ->
        [one] نُقل عنصر واحد
       *[other] نُقل { $n } عنصرًا
    }
bulk-copied =
    { $count ->
        [one] نُسخ عنصر واحد
       *[other] نُسخ { $n } عنصرًا
    }
bulk-zipped =
    { $count ->
        [one] ضُغط عنصر واحد
       *[other] ضُغط { $n } عنصرًا
    }
bulk-trashed =
    { $count ->
        [one] نُقل عنصر واحد إلى سلة المهملات
       *[other] نُقل { $n } عنصرًا إلى سلة المهملات
    }
bulk-renamed =
    { $count ->
        [one] أُعيدت تسمية عنصر واحد
       *[other] أُعيدت تسمية { $n } عنصرًا
    }
rename-title = إعادة تسمية العناصر المحددة
rename-find = بحث
rename-replace = استبدال بـ
rename-match-case = مطابقة حالة الأحرف
rename-preview = ستُعاد تسمية { $count }:
rename-apply = إعادة التسمية
rename-unchanged = بلا تغيير
rename-invalid = اسم غير صالح
rename-exists = هذا الاسم مستخدم
rename-duplicate = سيحصل عنصر آخر على الاسم نفسه
column-earlier = نقل إلى الأمام
column-later = نقل إلى الخلف
columns-reset = إعادة ضبط الأعمدة
col-name = الاسم
col-size = الحجم
col-allocated = على القرص
col-percent = ٪ من الأصل
col-files = الملفات
col-folders = المجلدات
col-modified = آخر تعديل
col-accessed = آخر فتح
col-created = تاريخ الإنشاء
col-attributes = السمات
col-owner = المالك
owner-unknown = مالك غير معروف
owner-id = المستخدم { $id }
types-by-type = حسب النوع
types-by-owner = حسب المالك
attr-read-only = للقراءة فقط
attr-hidden = مخفي
attr-system = نظام
attr-compressed = مضغوط
attr-encrypted = مشفّر
attr-sparse = متناثر
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
tab-new = فحص مجلد آخر
tab-close = إغلاق
wait-for-scan = انتظر انتهاء الفحص أولًا
scan-live-hint = يمكنك الآن تصفّح المجلدات التي انتهى فحصها
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
backend-source = محرك { $name }
admin-hint = شغّل البرنامج كمسؤول لقراءة جدول الملفات الرئيسي في NTFS مباشرة – عادةً أسرع بعدة مرات.

## Trash
confirm-trash-title = نقل إلى سلة المحذوفات؟
confirm-trash-body = سيتم نقل «{ $name }» ({ $size }) إلى سلة المحذوفات.
confirm-trash = نقل إلى السلة
cancel = إلغاء
trash-failed = تعذّر النقل إلى سلة المحذوفات: { $error }
trashed = تم نقل «{ $name }» إلى سلة المحذوفات
action-empty-trash = إفراغ سلة المحذوفات
confirm-empty-title = إفراغ سلة المحذوفات؟
confirm-empty-body = سيتم حذف كل ما في سلة المحذوفات ({ $size }) نهائيًا. لا يمكن التراجع عن ذلك.
emptying-trash = جارٍ إفراغ سلة المحذوفات…
trash-emptied = تم إفراغ سلة المحذوفات — تم تحرير { $size }
empty-trash-failed = تعذّر إفراغ سلة المحذوفات: { $error }
already-in-trash = هذا العنصر موجود بالفعل في سلة المحذوفات

## Misc
search-hint = البحث بالاسم…
search-clear = مسح البحث
search-filters = عوامل التصفية
search-all-tabs = كل عمليات الفحص المفتوحة
search-all-tabs-hint = البحث أيضًا في المجلدات والأقراص المفتوحة في علامات التبويب الأخرى
search-other-tab-hint = في علامة تبويب أخرى: انقر للانتقال إليها
search-saved = المحفوظة
search-saved-none = لا توجد عمليات بحث محفوظة بعد
search-saved-name = الاسم
search-save = حفظ
search-saved-delete = حذف هذا البحث المحفوظ
search-export = تصدير…
search-copy-hint = نسخ النتائج، جاهزة للصق في جدول بيانات.
filter-everything = الملفات والمجلدات
filter-files = الملفات
filter-folders = المجلدات
filter-at-least = على الأقل
filter-at-most = على الأكثر
filter-any-size = أي حجم
filter-any-time = عُدّل في أي وقت
filter-within-week = عُدّل خلال آخر 7 أيام
filter-within-month = عُدّل خلال آخر 30 يومًا
filter-within-year = عُدّل خلال آخر سنة
filter-older-year = لم يُعدَّل منذ سنة
filter-older-two-years = لم يُعدَّل منذ سنتين
filter-older-five-years = لم يُعدَّل منذ 5 سنوات
filter-any-owner = أي مالك
filter-any-attribute = أي سمات
filter-clear = مسح عوامل التصفية
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
highlight-large = إبراز العناصر الكبيرة
setting-highlight-large-hint = تلوين أسماء الملفات والمجلدات التي تشغل حصة كبيرة من مجلدها.
option-cross-fs = فحص أنظمة الملفات المركّبة الأخرى أيضًا
option-mft = فحص NTFS السريع (MFT)

## Help
help-about = حول StorageMap
about-body = أداة سريعة لتحليل مساحة القرص لنظامي Windows وLinux.
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
dup-sha256 = SHA-256
dup-sha256-hint = حساب مجموع اختباري SHA-256 لكل مجموعة أيضًا، للمقارنة مع أدوات أخرى. تُقرأ كل مجموعة مرة إضافية.
dup-col-remove = إزالة
dup-col-group = المجموعة
dup-mark-hint = تحديد هذه النسخة لنقلها إلى سلة المهملات أو استبدالها برابط ثابت
dup-keep = الاحتفاظ بـ:
dup-keep-newest = أحدث نسخة
dup-keep-oldest = أقدم نسخة
dup-keep-shortest = أقصر مسار
dup-mark-others = تحديد النسخ الأخرى
dup-mark-others-hint = في كل مجموعة، تحديد كل النسخ عدا النسخة التي تحتفظ بها القاعدة
dup-clear-marks = مسح التحديد
dup-marked = { $count } محددة · { $size }
dup-trash = نقل إلى سلة المهملات
dup-link = استبدال بروابط ثابتة
dup-link-hint = الاحتفاظ بنسخة واحدة على القرص وجعل النسخ الأخرى تشير إليها. يجب أن تكون كل النسخ على القرص نفسه.
dup-working = جارٍ العمل… { $done } من { $total }
dup-confirm-trash = نقل النسخ المحددة إلى سلة المهملات؟
dup-confirm-trash-body = سيُنقل { $count } ملفًا ({ $size }) إلى سلة المهملات. تبقى نسخة واحدة من كل مجموعة دائمًا.
dup-confirm-link = استبدال النسخ المحددة بروابط ثابتة؟
dup-confirm-link-body = سيصبح { $count } ملفًا ({ $size }) روابط ثابتة للنسخة المحتفَظ بها، فتتوقف عن شغل مساحة. يُفحص كل زوج بايتًا بايتًا أولًا. بعد ذلك يتشارك الاسمان نسخة واحدة: تغيير أحدهما يغيّر الآخر.
dup-more =
    { $count ->
        [one] وعنصر آخر
       *[other] و{ $n } أخرى
    }
dup-trashed =
    { $count ->
        [one] نُقلت نسخة واحدة إلى سلة المهملات
       *[other] نُقلت { $n } نسخ إلى سلة المهملات
    }
dup-linked =
    { $count ->
        [one] استُبدلت نسخة واحدة برابط ثابت
       *[other] استُبدلت { $n } نسخ بروابط ثابتة
    }
dup-failed = فشل { $count }: { $error }
dup-hint = يعثر على الملفات ذات المحتوى المتطابق في المجلد الحالي. تُقارن الملفات حسب الحجم أولًا، ثم ببصمة سريعة لبدايتها ونهايتها، وأخيرًا بتجزئة كاملة للمحتوى. يتم تجاهل الروابط الثابتة لأنها تتشارك المساحة نفسها أصلًا.

## Settings, updates and polish
menu-settings = الإعدادات…
action-zoom-in = تكبير
action-zoom-out = تصغير
action-zoom-reset = الحجم الفعلي
action-check-updates = التحقق من التحديثات…
action-open-github = StorageMap على GitHub
start-drop-hint = أو اسحب مجلدًا إلى أي مكان في هذه النافذة
settings-title = الإعدادات
settings-general = عام
settings-appearance = المظهر
settings-scanning = الفحص
settings-integration = التكامل
settings-updates = التحديثات
settings-about = حول
setting-language = اللغة
setting-scale = حجم الواجهة
setting-scale-hint = يكبّر النصوص وعناصر التحكم أو يصغّرها. يعمل Ctrl + وCtrl − أيضًا.
setting-treemap-depth-hint = عدد مستويات المجلدات التي ترسمها الخريطة الشجرية. المستويات الأقل أسرع في الرسم.
setting-auto-snapshot-hint = يتيح لك رؤية ما تغيّر عند فحص المجلد نفسه في المرة القادمة. تبقى اللقطات على هذا الجهاز.
setting-cross-fs-hint = تتبّع نقاط التركيب إلى أقراص أخرى ومشاركات الشبكة.
scan-exclude = استبعاد من الفحص
scan-exclude-hint = نمط واحد في كل سطر. الاسم مثل node_modules أو ‎*.iso‎ يطابق في أي مكان؛ والمسار مثل build/cache يطابق من المجلد المفحوص. * يطابق أي شيء، و? حرفًا واحدًا.
scan-include = احتساب هذه الملفات فقط
scan-include-hint = نمط واحد في كل سطر، مثل ‎*.mp4‎. اتركه فارغًا لاحتساب كل الملفات. تُفحص المجلدات دائمًا.
setting-mft-hint = يقرأ جدول الملفات الرئيسي في NTFS مباشرة — أسرع بعدة مرات. يتطلب صلاحيات المسؤول.
integration-hint = يضيف «فتح باستخدام StorageMap» للمجلدات في مدير الملفات. يؤثر على حساب المستخدم الخاص بك فقط.
setting-auto-update = التحقق من التحديثات تلقائيًا
setting-auto-update-hint = يسأل GitHub مرة يوميًا عن وجود إصدار أحدث. لا يُرسل أي شيء عنك أو عن ملفاتك.
version-current = الإصدار المثبّت: { $version }
update-check-now = تحقق الآن
update-checking = جارٍ التحقق من التحديثات…
update-up-to-date = لديك أحدث إصدار.
update-available = الإصدار { $version } من StorageMap متاح
update-install = تنزيل وتثبيت
update-manual = فتح صفحة التنزيل
update-notes = ما الجديد
update-downloading = جارٍ التنزيل… { $done } من { $total }
update-ready = جارٍ التثبيت…
update-failed = فشل التحديث: { $error }
update-restart = تم تثبيت التحديث. أعد تشغيل StorageMap لاستخدام الإصدار الجديد.
update-handed-off = تم فتح برنامج التثبيت — اتبع خطواته لإكمال العملية.
update-later = لاحقًا
about-privacy = لا تغادر عمليات الفحص واللقطات وعمليات البحث عن المكررات جهازك أبدًا. الطلب الوحيد عبر الشبكة هو التحقق اليومي الاختياري من التحديثات.
about-source = الشيفرة المصدرية والإصدارات والإبلاغ عن المشكلات
update-skip = تخطي هذا الإصدار
update-restart-now = إعادة التشغيل الآن
update-no-notes = لم تُنشر ملاحظات لهذا الإصدار.
settings-shortcuts = اختصارات لوحة المفاتيح
shortcut-enter = فتح المجلد المحدد
shortcut-arrows = التنقل في شجرة المجلدات
shortcut-double-click = التكبير داخل مجلد في الخريطة الشجرية
search-empty = اكتب في الحقل أعلاه جزءًا من اسم ملف أو مجلد للعثور عليه في المجلد الحالي.
dup-empty-title = اعثر على الملفات التي تهدر المساحة
empty-folder-hint = اختر مجلدًا آخر من الشجرة، أو انتقل مستوى للأعلى.

## Cleanup
tab-cleanup = التنظيف
cleanup-total = يمكن على الأرجح تحرير { $size }
cleanup-hint = هذه أماكن تحتوي عادةً على بيانات يمكن الاستغناء عنها. راجع كل عنصر قبل نقله إلى سلة المحذوفات — يمكنك استعادة أي شيء منها.
cleanup-none-title = لا يوجد ما يستحق التنظيف
cleanup-none = لم يُعثر في هذا المجلد على ذاكرات تخزين مؤقت أو مخرجات بناء أو تنزيلات قديمة أو بيانات مؤقتة أخرى.
cleanup-all = عرض الكل ({ $count })
cleanup-fewer = عرض أقل
cleanup-trash = سلة المحذوفات
cleanup-trash-hint = ملفات حذفتها بالفعل. إفراغ السلة يحرر هذه المساحة نهائيًا.
cleanup-cache = ذاكرات التطبيقات المؤقتة
cleanup-cache-hint = بيانات تحتفظ بها التطبيقات لتعمل أسرع. تُعاد إنشاؤها عند الحاجة؛ أغلق التطبيقات أولًا.
cleanup-build = مخرجات البناء
cleanup-build-hint = ‏node_modules ومجلدات target الخاصة بـ Rust وما شابهها. تُنشأ من جديد في البناء أو التثبيت التالي.
cleanup-packages = ذاكرات الحزم المؤقتة
cleanup-packages-hint = حزم منزّلة تحتفظ بها Cargo وnpm وGradle وMaven وGo وغيرها. تُنزّل مجددًا عند الحاجة.
cleanup-temp = الملفات المؤقتة
cleanup-temp-hint = بقايا برامج التثبيت والتطبيقات. حذفها آمن عندما تكون التطبيقات التي أنشأتها مغلقة.
cleanup-logs = ملفات سجل كبيرة
cleanup-logs-hint = ملفات سجل يزيد حجمها على 50 م.ب. احتفظ بها فقط إذا كنت تشخّص مشكلة.
cleanup-dumps = ملفات تفريغ الأعطال
cleanup-dumps-hint = نسخ من الذاكرة كُتبت عند تعطل برنامج. مفيدة للمطورين فقط.
cleanup-downloads = تنزيلات قديمة
cleanup-downloads-hint = ملفات كبيرة في مجلد التنزيلات لم تلمسها منذ ستة أشهر.
