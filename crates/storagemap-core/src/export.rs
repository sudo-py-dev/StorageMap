//! CSV export of a scanned subtree.

use std::io::{self, Write};

use crate::tree::{Kind, Metric, NodeId, Tree};

/// Writes `root` and its descendants down to `max_depth` levels (0 = only
/// `root`) as RFC 4180 CSV.
pub fn write_csv<W: Write>(tree: &Tree, root: NodeId, max_depth: usize, out: W) -> io::Result<()> {
    let mut out = io::BufWriter::new(out);
    writeln!(
        out,
        "path,type,size,allocated,files,folders,percent_of_parent,modified_utc"
    )?;
    let base_depth = tree.depth(root);
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = tree.node(id);
        let kind = match node.kind {
            Kind::Dir => "folder",
            Kind::File => "file",
            Kind::Symlink => "link",
            Kind::Other => "other",
        };
        writeln!(
            out,
            "{},{kind},{},{},{},{},{:.2},{}",
            csv_field(&tree.path(id).to_string_lossy()),
            node.size,
            node.allocated,
            node.files,
            node.dirs,
            tree.share_of_parent(id, Metric::Size) * 100.0,
            iso_date(node.modified),
        )?;
        if tree.depth(id) - base_depth < max_depth {
            stack.extend(tree.children(id).rev());
        }
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
        write_csv(&t, t.root(), 1, &mut buf).expect("csv");
        let text = String::from_utf8(buf).expect("utf8");
        assert_eq!(text.lines().count(), 3); // header, root, d
    }
}
