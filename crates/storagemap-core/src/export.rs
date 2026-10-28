//! CSV export of a scanned subtree.

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

/// The report's human-facing text, so the GUI can write it in the user's
/// language. `Default` is the stable English, machine-friendly form.
pub struct CsvLabels<'a> {
    /// Column titles: path, type, size, allocated, files, folders,
    /// percent of parent, modified (UTC).
    pub headers: [String; 8],
    /// Type names: folder, file, link, other.
    pub kinds: [String; 4],
    /// Writes the size columns with this instead of as exact byte counts.
    pub readable_size: Option<&'a dyn Fn(u64) -> String>,
    /// Which rows to write.
    pub include: Include,
    /// Start with a UTF-8 byte order mark: Excel needs it to show
    /// non-Latin names correctly.
    pub bom: bool,
}

impl Default for CsvLabels<'_> {
    fn default() -> Self {
        CsvLabels {
            headers: [
                "path",
                "type",
                "size",
                "allocated",
                "files",
                "folders",
                "percent_of_parent",
                "modified_utc",
            ]
            .map(str::to_owned),
            kinds: ["folder", "file", "link", "other"].map(str::to_owned),
            readable_size: None,
            include: Include::All,
            bom: false,
        }
    }
}

/// Writes `root` and its descendants down to `max_depth` levels (0 = only
/// `root`) as RFC 4180 CSV.
pub fn write_csv<W: Write>(
    tree: &Tree,
    root: NodeId,
    max_depth: usize,
    labels: &CsvLabels,
    out: W,
) -> io::Result<()> {
    let mut out = io::BufWriter::new(out);
    if labels.bom {
        out.write_all("\u{FEFF}".as_bytes())?;
    }
    let header: Vec<String> = labels.headers.iter().map(|h| csv_field(h)).collect();
    writeln!(out, "{}", header.join(","))?;
    let base_depth = tree.depth(root);
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        if tree.depth(id) - base_depth < max_depth {
            stack.extend(tree.children(id).rev());
        }
        if !labels.include.accepts(node.kind) {
            continue;
        }
        let size = |bytes: u64| match labels.readable_size {
            Some(readable) => csv_field(&readable(bytes)),
            None => bytes.to_string(),
        };
        let kind = &labels.kinds[match node.kind {
            Kind::Dir => 0,
            Kind::File => 1,
            Kind::Symlink => 2,
            Kind::Other => 3,
        }];
        writeln!(
            out,
            "{},{},{},{},{},{},{:.2},{}",
            csv_field(&tree.path(id).to_string_lossy()),
            csv_field(kind),
            size(node.size),
            size(node.allocated),
            node.files,
            node.dirs,
            tree.share_of_parent(id, Metric::Size) * 100.0,
            iso_date(node.modified),
        )?;
    }
    out.flush()
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
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
    fn quotes_awkward_paths() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("plain"), "plain");
    }

    #[test]
    fn respects_depth() {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
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
        write_csv(&t, t.root(), 1, &CsvLabels::default(), &mut buf).expect("csv");
        let text = String::from_utf8(buf).expect("utf8");
        assert_eq!(text.lines().count(), 3); // header, root, d
    }

    fn sample() -> Tree {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
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

    fn render(t: &Tree, labels: &CsvLabels) -> String {
        let mut buf = Vec::new();
        write_csv(t, t.root(), 8, labels, &mut buf).expect("csv");
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn localised_labels() {
        let t = sample();
        let readable = |n: u64| format!("{} KB", n / 1024);
        let labels = CsvLabels {
            headers: ["נתיב", "סוג", "גודל", "b", "c", "d", "e", "f"].map(str::to_owned),
            kinds: ["תיקייה", "קובץ", "קישור", "אחר"].map(str::to_owned),
            readable_size: Some(&readable),
            include: Include::All,
            bom: true,
        };
        let text = render(&t, &labels);
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
                &CsvLabels {
                    include,
                    ..CsvLabels::default()
                },
            );
            text.lines()
                .skip(1)
                .map(|l| l.split(',').next().unwrap_or("").to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(only(Include::Folders), ["/r", "/r/d"]);
        assert_eq!(only(Include::Files), ["/r/d/f"]);
        assert_eq!(only(Include::All).len(), 3);
    }
}
