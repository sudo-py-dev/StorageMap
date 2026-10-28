//! Reports of a scanned subtree: one row model, several output formats.

use std::borrow::Cow;
use std::io::{self, Write};

use crate::tree::{Kind, Metric, NodeId, Tree};

/// Which rows a report lists. Folders are always walked; the filter only
/// decides what is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Include {
    #[default]
    All,
    Folders,
    Files,
}

impl Include {
    pub const ALL: [Include; 3] = [Include::All, Include::Folders, Include::Files];

    fn accepts(self, kind: Kind) -> bool {
        match self {
            Include::All => true,
            Include::Folders => kind == Kind::Dir,
            Include::Files => kind != Kind::Dir,
        }
    }
}

/// One cell of a report row. Writers decide how each kind is written, so
/// the same report can become CSV, a spreadsheet or a web page.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Text(Cow<'a, str>),
    /// A size in bytes.
    Bytes(u64),
    Count(u64),
    /// A fraction in `0.0..=1.0`.
    Share(f64),
    /// Unix seconds.
    Date(i64),
}

/// Receives a report row by row, so even multi-million row reports stream
/// without being held in memory.
pub trait ReportWriter {
    fn header(&mut self, titles: &[String]) -> io::Result<()>;
    fn row(&mut self, values: &[Value<'_>]) -> io::Result<()>;
    fn finish(&mut self) -> io::Result<()>;
}

/// Stable, machine-friendly names of the report's columns, in their
/// default order (also the keys of JSON reports).
pub const COLUMN_KEYS: [&str; 8] = [
    "path",
    "type",
    "size",
    "allocated",
    "files",
    "folders",
    "percent_of_parent",
    "modified_utc",
];

/// The tree report's human-facing text, so the GUI can write it in the
/// user's language. `Default` is the stable English, machine-friendly form.
pub struct ReportLabels {
    /// Column titles: path, type, size, allocated, files, folders,
    /// percent of parent, modified (UTC).
    pub headers: [String; 8],
    /// Type names: folder, file, link, other.
    pub kinds: [String; 4],
    /// Which rows to write.
    pub include: Include,
    /// Which columns to write, in order, as indices into [`COLUMN_KEYS`].
    pub columns: Vec<usize>,
}

impl ReportLabels {
    /// Titles of the chosen columns, in order.
    pub fn titles(&self) -> Vec<String> {
        self.columns
            .iter()
            .filter_map(|&c| self.headers.get(c).cloned())
            .collect()
    }

    /// Machine names of the chosen columns, in order.
    pub fn keys(&self) -> Vec<&'static str> {
        self.columns
            .iter()
            .filter_map(|&c| COLUMN_KEYS.get(c).copied())
            .collect()
    }
}

impl Default for ReportLabels {
    fn default() -> Self {
        ReportLabels {
            headers: COLUMN_KEYS.map(str::to_owned),
            kinds: ["folder", "file", "link", "other"].map(str::to_owned),
            include: Include::All,
            columns: (0..COLUMN_KEYS.len()).collect(),
        }
    }
}

/// Writes `root` and its descendants down to `max_depth` levels (0 = only
/// `root`) to `writer`.
pub fn write_tree(
    tree: &Tree,
    root: NodeId,
    max_depth: usize,
    labels: &ReportLabels,
    writer: &mut dyn ReportWriter,
) -> io::Result<()> {
    writer.header(&labels.titles())?;
    let base_depth = tree.depth(root);
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        if tree.depth(id) - base_depth < max_depth {
            stack.extend(tree.children(id).rev());
        }
        if labels.include.accepts(node.kind) {
            write_row(tree, id, labels, writer)?;
        }
    }
    writer.finish()
}

/// Writes exactly the given entries (search results, for example, possibly
/// from several scans), in that order, with the same columns as [`write_tree`].
pub fn write_entries<'a>(
    entries: impl IntoIterator<Item = (&'a Tree, NodeId)>,
    labels: &ReportLabels,
    writer: &mut dyn ReportWriter,
) -> io::Result<()> {
    writer.header(&labels.titles())?;
    for (tree, id) in entries {
        write_row(tree, id, labels, writer)?;
    }
    writer.finish()
}

fn write_row(
    tree: &Tree,
    id: NodeId,
    labels: &ReportLabels,
    writer: &mut dyn ReportWriter,
) -> io::Result<()> {
    let node = tree.node(id);
    let kind = &labels.kinds[match node.kind {
        Kind::Dir => 0,
        Kind::File => 1,
        Kind::Symlink => 2,
        Kind::Other => 3,
    }];
    let path = tree.path(id);
    let all = [
        Value::Text(path.to_string_lossy()),
        Value::Text(Cow::Borrowed(kind)),
        Value::Bytes(node.size),
        Value::Bytes(node.allocated),
        Value::Count(node.files),
        Value::Count(node.dirs),
        Value::Share(tree.share_of_parent(id, Metric::Size)),
        Value::Date(node.modified),
    ];
    let chosen: Vec<Value<'_>> = labels
        .columns
        .iter()
        .filter_map(|&c| all.get(c).cloned())
        .collect();
    writer.row(&chosen)
}

/// Delimited text: RFC 4180 CSV, or tab-separated text that pastes into
/// spreadsheets.
pub struct Delimited<'a, W: Write> {
    out: io::BufWriter<W>,
    separator: char,
    readable_size: Option<&'a dyn Fn(u64) -> String>,
    bom: bool,
}

impl<'a, W: Write> Delimited<'a, W> {
    pub fn csv(out: W) -> Self {
        Delimited {
            out: io::BufWriter::new(out),
            separator: ',',
            readable_size: None,
            bom: false,
        }
    }

    pub fn tsv(out: W) -> Self {
        Delimited {
            separator: '\t',
            ..Delimited::csv(out)
        }
    }

    /// Writes sizes with `f` instead of as exact byte counts.
    pub fn readable_sizes(mut self, f: &'a dyn Fn(u64) -> String) -> Self {
        self.readable_size = Some(f);
        self
    }

    /// Starts with a UTF-8 byte order mark: Excel needs it to show non-Latin
    /// names correctly.
    pub fn with_bom(mut self) -> Self {
        self.bom = true;
        self
    }

    fn field(&self, s: &str) -> String {
        if s.contains([self.separator, '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    }

    fn line(&mut self, fields: &[String]) -> io::Result<()> {
        let mut sep = [0u8; 4];
        let sep = self.separator.encode_utf8(&mut sep);
        writeln!(self.out, "{}", fields.join(sep))
    }
}

impl<W: Write> ReportWriter for Delimited<'_, W> {
    fn header(&mut self, titles: &[String]) -> io::Result<()> {
        if self.bom {
            self.out.write_all("\u{FEFF}".as_bytes())?;
        }
        let fields: Vec<String> = titles.iter().map(|t| self.field(t)).collect();
        self.line(&fields)
    }

    fn row(&mut self, values: &[Value<'_>]) -> io::Result<()> {
        let fields: Vec<String> = values
            .iter()
            .map(|v| match v {
                Value::Text(s) => self.field(s),
                Value::Bytes(b) => match self.readable_size {
                    Some(readable) => self.field(&readable(*b)),
                    None => b.to_string(),
                },
                Value::Count(n) => n.to_string(),
                Value::Share(f) => format!("{:.2}", f * 100.0),
                Value::Date(secs) => iso_date(*secs),
            })
            .collect();
        self.line(&fields)
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// `YYYY-MM-DD HH:MM:SS` in UTC for Unix seconds.
pub fn iso_date(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-since-epoch to proleptic Gregorian date.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{EntryInfo, Flags, TreeBuilder};
    use std::path::PathBuf;

    #[test]
    fn dates() {
        assert_eq!(iso_date(0), "1970-01-01 00:00:00");
        assert_eq!(iso_date(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(iso_date(-1), "1969-12-31 23:59:59");
    }

    #[test]
    fn quotes_awkward_fields() {
        let csv = Delimited::csv(Vec::new());
        assert_eq!(csv.field("a,b"), "\"a,b\"");
        assert_eq!(csv.field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv.field("plain"), "plain");
        let tsv = Delimited::tsv(Vec::new());
        assert_eq!(tsv.field("a,b"), "a,b");
        assert_eq!(tsv.field("a\tb"), "\"a\tb\"");
    }

    #[test]
    fn respects_depth() {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
            size: 0,
            allocated: 0,
        };
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir);
        let d = b.push(NodeId::ROOT, b"d", dir);
        b.push(
            d,
            b"f",
            EntryInfo {
                kind: Kind::File,
                size: 3,
                allocated: 4,
                ..dir
            },
        );
        let t = b.finish();
        let mut buf = Vec::new();
        let mut csv = Delimited::csv(&mut buf);
        write_tree(&t, t.root(), 1, &ReportLabels::default(), &mut csv).expect("csv");
        drop(csv);
        let text = String::from_utf8(buf).expect("utf8");
        assert_eq!(text.lines().count(), 3); // header, root, d
    }

    fn sample() -> Tree {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            owner: crate::NO_OWNER,
            size: 2048,
            allocated: 4096,
        };
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir);
        let d = b.push(NodeId::ROOT, b"d", dir);
        b.push(
            d,
            b"f",
            EntryInfo {
                kind: Kind::File,
                ..dir
            },
        );
        b.finish()
    }

    fn render(t: &Tree, labels: &ReportLabels) -> String {
        let mut buf = Vec::new();
        let mut csv = Delimited::csv(&mut buf);
        write_tree(t, t.root(), 8, labels, &mut csv).expect("csv");
        drop(csv);
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn writes_just_the_given_entries() {
        let t = sample();
        let file = t.find(std::path::Path::new("/r/d/f")).expect("file");
        let mut buf = Vec::new();
        let mut csv = Delimited::csv(&mut buf);
        write_entries([(&t, file)], &ReportLabels::default(), &mut csv).expect("csv");
        drop(csv);
        let text = String::from_utf8(buf).expect("utf8");
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().nth(1).expect("row").contains(",file,"));
    }

    #[test]
    fn writes_chosen_columns_in_order() {
        let t = sample();
        let labels = ReportLabels {
            columns: vec![2, 0],
            ..ReportLabels::default()
        };
        let mut buf = Vec::new();
        let mut csv = Delimited::csv(&mut buf);
        write_tree(&t, t.root(), 0, &labels, &mut csv).expect("csv");
        drop(csv);
        let text = String::from_utf8(buf).expect("utf8");
        assert_eq!(text, "size,path\n6144,/r\n");
    }

    #[test]
    fn localised_labels() {
        let t = sample();
        let readable = |n: u64| format!("{} KB", n / 1024);
        let labels = ReportLabels {
            headers: ["נתיב", "סוג", "גודל", "b", "c", "d", "e", "f"].map(str::to_owned),
            kinds: ["תיקייה", "קובץ", "קישור", "אחר"].map(str::to_owned),
            ..ReportLabels::default()
        };
        let mut buf = Vec::new();
        let mut csv = Delimited::csv(&mut buf)
            .readable_sizes(&readable)
            .with_bom();
        write_tree(&t, t.root(), 8, &labels, &mut csv).expect("csv");
        drop(csv);
        let text = String::from_utf8(buf).expect("utf8");
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("\u{FEFF}נתיב,סוג,גודל,b,c,d,e,f"));
        assert!(lines.next().expect("row").starts_with("/r,תיקייה,"));
        assert!(text.contains(",2 KB,"), "{text}");
    }

    #[test]
    fn filters_rows_but_walks_folders() {
        let t = sample();
        let only = |include| {
            let text = render(
                &t,
                &ReportLabels {
                    include,
                    ..ReportLabels::default()
                },
            );
            text.lines()
                .skip(1)
                .map(|l| l.split(',').next().unwrap_or("").to_owned())
                .collect::<Vec<_>>()
        };
        // Paths use the platform's separator (`\\` on Windows).
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            only(Include::Folders),
            ["/r".to_owned(), format!("/r{sep}d")]
        );
        assert_eq!(only(Include::Files), [format!("/r{sep}d{sep}f")]);
        assert_eq!(only(Include::All).len(), 3);
    }
}
