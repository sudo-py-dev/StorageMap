//! Report writers besides delimited text: Excel workbooks, web pages, JSON
//! and aligned plain text.

use std::io::{self, Seek, Write};

use zip::write::SimpleFileOptions;

use crate::export::{ReportWriter, Value, iso_date};

/// A report file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Format {
    #[default]
    Csv,
    Excel,
    Html,
    Json,
    Text,
}

impl Format {
    pub const ALL: [Format; 5] = [
        Format::Csv,
        Format::Excel,
        Format::Html,
        Format::Json,
        Format::Text,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Excel => "xlsx",
            Format::Html => "html",
            Format::Json => "json",
            Format::Text => "txt",
        }
    }
}

/// How a value reads as text, for formats without typed cells.
fn display(value: &Value<'_>, readable: Option<&dyn Fn(u64) -> String>) -> String {
    match value {
        Value::Text(s) => s.to_string(),
        Value::Bytes(b) => match readable {
            Some(r) => r(*b),
            None => b.to_string(),
        },
        Value::Count(n) => n.to_string(),
        Value::Share(f) => format!("{:.2}", f * 100.0),
        Value::Date(secs) => iso_date(*secs),
    }
}

fn is_number(value: &Value<'_>) -> bool {
    !matches!(value, Value::Text(_) | Value::Date(_))
}

/// Escapes text for XML and HTML, dropping control characters XML forbids.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// An Excel workbook with one sheet, streamed row by row.
pub struct Excel<'a, W: Write + Seek> {
    /// Taken by `finish`.
    zip: Option<zip::ZipWriter<W>>,
    readable: Option<&'a dyn Fn(u64) -> String>,
    rtl: bool,
    rows: u32,
}

/// Rows a sheet holds, including the header.
const EXCEL_MAX_ROWS: u32 = 1_048_576;

impl<'a, W: Write + Seek> Excel<'a, W> {
    /// `rtl` lays the sheet out right to left, for Hebrew and Arabic.
    pub fn new(out: W, rtl: bool) -> Self {
        Excel {
            zip: Some(zip::ZipWriter::new(out)),
            readable: None,
            rtl,
            rows: 0,
        }
    }

    /// Writes sizes with `f` (as text) instead of as byte counts (numbers).
    pub fn readable_sizes(mut self, f: &'a dyn Fn(u64) -> String) -> Self {
        self.readable = Some(f);
        self
    }

    fn zip(&mut self) -> io::Result<&mut zip::ZipWriter<W>> {
        self.zip
            .as_mut()
            .ok_or_else(|| io::Error::other("already finished"))
    }

    fn part(&mut self, name: &str, xml: &str) -> io::Result<()> {
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        self.zip()?
            .start_file(name, options)
            .map_err(io::Error::other)?;
        self.zip()?.write_all(xml.as_bytes())
    }

    fn cell(&mut self, value: &Value<'_>, style: &str) -> io::Result<()> {
        let typed = match value {
            Value::Bytes(_) if self.readable.is_some() => None,
            Value::Bytes(n) | Value::Count(n) => Some(n.to_string()),
            Value::Share(f) => Some(format!("{:.4}", f * 100.0)),
            _ => None,
        };
        match typed {
            Some(number) => write!(self.zip()?, "<c{style}><v>{number}</v></c>"),
            None => {
                let text = escape(&display(value, self.readable));
                write!(
                    self.zip()?,
                    "<c t=\"inlineStr\"{style}><is><t xml:space=\"preserve\">{text}</t></is></c>"
                )
            }
        }
    }

    fn next_row(&mut self) -> io::Result<()> {
        if self.rows >= EXCEL_MAX_ROWS {
            return Err(io::Error::other(
                "an Excel sheet holds at most 1,048,576 rows; export fewer levels, or as CSV",
            ));
        }
        self.rows += 1;
        Ok(())
    }
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="StorageMap" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
/// Style 0 is plain, style 1 bold (the header).
const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs></styleSheet>"#;

impl<W: Write + Seek> ReportWriter for Excel<'_, W> {
    fn header(&mut self, titles: &[String]) -> io::Result<()> {
        self.part("[Content_Types].xml", CONTENT_TYPES)?;
        self.part("_rels/.rels", ROOT_RELS)?;
        self.part("xl/workbook.xml", WORKBOOK)?;
        self.part("xl/_rels/workbook.xml.rels", WORKBOOK_RELS)?;
        self.part("xl/styles.xml", STYLES)?;
        // Big sheets need ZIP64 headers; decided before the data is written.
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        self.zip()?
            .start_file("xl/worksheets/sheet1.xml", options)
            .map_err(io::Error::other)?;
        let rtl = if self.rtl { " rightToLeft=\"1\"" } else { "" };
        write!(
            self.zip()?,
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
             <sheetViews><sheetView workbookViewId=\"0\"{rtl}>\
             <pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/>\
             </sheetView></sheetViews><sheetData>"
        )?;
        self.next_row()?;
        self.zip()?.write_all(b"<row>")?;
        for title in titles {
            self.cell(&Value::Text(title.as_str().into()), " s=\"1\"")?;
        }
        self.zip()?.write_all(b"</row>")
    }

    fn row(&mut self, values: &[Value<'_>]) -> io::Result<()> {
        self.next_row()?;
        self.zip()?.write_all(b"<row>")?;
        for value in values {
            self.cell(value, "")?;
        }
        self.zip()?.write_all(b"</row>")
    }

    fn finish(&mut self) -> io::Result<()> {
        self.zip()?.write_all(b"</sheetData></worksheet>")?;
        let zip = self
            .zip
            .take()
            .ok_or_else(|| io::Error::other("already finished"))?;
        zip.finish().map(drop).map_err(io::Error::other)
    }
}

/// A self-contained web page with the report as a table; prints well too.
pub struct Html<'a, W: Write> {
    out: io::BufWriter<W>,
    title: String,
    lang: String,
    rtl: bool,
    readable: Option<&'a dyn Fn(u64) -> String>,
}

impl<'a, W: Write> Html<'a, W> {
    /// `lang` is the page language (`en`, `he`, …); `rtl` its direction.
    pub fn new(out: W, title: &str, lang: &str, rtl: bool) -> Self {
        Html {
            out: io::BufWriter::new(out),
            title: title.to_owned(),
            lang: lang.to_owned(),
            rtl,
            readable: None,
        }
    }

    pub fn readable_sizes(mut self, f: &'a dyn Fn(u64) -> String) -> Self {
        self.readable = Some(f);
        self
    }
}

const HTML_STYLE: &str = "body{font:14px system-ui,sans-serif;margin:24px;color:#1f2430}\
h1{font-size:20px;margin:0 0 12px}table{border-collapse:collapse;width:100%}\
th,td{padding:4px 8px;border-bottom:1px solid #e3e6ec;text-align:start;white-space:nowrap}\
th{background:#f3f5f8;position:sticky;top:0}td.n{text-align:end;font-variant-numeric:tabular-nums}\
td.p{white-space:normal;overflow-wrap:anywhere}\
@media print{body{margin:0}th{position:static}}";

impl<W: Write> ReportWriter for Html<'_, W> {
    fn header(&mut self, titles: &[String]) -> io::Result<()> {
        let dir = if self.rtl { "rtl" } else { "ltr" };
        write!(
            self.out,
            "<!doctype html>\n<html lang=\"{}\" dir=\"{dir}\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
             <title>{t}</title><style>{HTML_STYLE}</style></head><body><h1>{t}</h1>\
             <table><thead><tr>",
            escape(&self.lang),
            t = escape(&self.title)
        )?;
        for title in titles {
            write!(self.out, "<th>{}</th>", escape(title))?;
        }
        writeln!(self.out, "</tr></thead><tbody>")
    }

    fn row(&mut self, values: &[Value<'_>]) -> io::Result<()> {
        self.out.write_all(b"<tr>")?;
        for (i, value) in values.iter().enumerate() {
            // The first column (usually the path) may wrap; paths read left to right.
            let class = if is_number(value) {
                " class=\"n\""
            } else if i == 0 {
                " class=\"p\" dir=\"auto\""
            } else {
                ""
            };
            write!(
                self.out,
                "<td{class}>{}</td>",
                escape(&display(value, self.readable))
            )?;
        }
        writeln!(self.out, "</tr>")
    }

    fn finish(&mut self) -> io::Result<()> {
        writeln!(self.out, "</tbody></table></body></html>")?;
        self.out.flush()
    }
}

/// A JSON array of objects keyed by stable English names, with exact
/// numbers (sizes in bytes, shares in percent, dates in UTC), for scripts.
pub struct Json<W: Write> {
    out: io::BufWriter<W>,
    keys: Vec<&'static str>,
    first: bool,
}

impl<W: Write> Json<W> {
    pub fn new(out: W, keys: Vec<&'static str>) -> Self {
        Json {
            out: io::BufWriter::new(out),
            keys,
            first: true,
        }
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl<W: Write> ReportWriter for Json<W> {
    fn header(&mut self, _titles: &[String]) -> io::Result<()> {
        self.out.write_all(b"[")
    }

    fn row(&mut self, values: &[Value<'_>]) -> io::Result<()> {
        self.out
            .write_all(if self.first { b"\n  {" } else { b",\n  {" })?;
        self.first = false;
        for (i, (key, value)) in self.keys.iter().zip(values).enumerate() {
            let text = match value {
                Value::Text(s) => json_string(s),
                Value::Date(secs) => json_string(&iso_date(*secs)),
                Value::Bytes(n) | Value::Count(n) => n.to_string(),
                Value::Share(f) => format!("{:.2}", f * 100.0),
            };
            let sep = if i == 0 { "" } else { ", " };
            write!(self.out, "{sep}{}: {text}", json_string(key))?;
        }
        self.out.write_all(b"}")
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.write_all(b"\n]\n")?;
        self.out.flush()
    }
}

/// Columns aligned with spaces, for reading in any text viewer. Rows are
/// held until the end to size the columns.
pub struct Text<'a, W: Write> {
    out: W,
    readable: Option<&'a dyn Fn(u64) -> String>,
    rows: Vec<Vec<(String, bool)>>,
}

impl<'a, W: Write> Text<'a, W> {
    pub fn new(out: W) -> Self {
        Text {
            out,
            readable: None,
            rows: Vec::new(),
        }
    }

    pub fn readable_sizes(mut self, f: &'a dyn Fn(u64) -> String) -> Self {
        self.readable = Some(f);
        self
    }
}

impl<W: Write> ReportWriter for Text<'_, W> {
    fn header(&mut self, titles: &[String]) -> io::Result<()> {
        self.rows
            .push(titles.iter().map(|t| (t.clone(), false)).collect());
        Ok(())
    }

    fn row(&mut self, values: &[Value<'_>]) -> io::Result<()> {
        let row = values
            .iter()
            .map(|v| (display(v, self.readable), is_number(v)))
            .collect();
        self.rows.push(row);
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0; columns];
        for row in &self.rows {
            for (i, (text, _)) in row.iter().enumerate() {
                widths[i] = widths[i].max(text.chars().count());
            }
        }
        let mut out = io::BufWriter::new(&mut self.out);
        for row in &self.rows {
            let mut line = String::new();
            for (i, (text, number)) in row.iter().enumerate() {
                let pad = widths[i] - text.chars().count();
                if i > 0 {
                    line.push_str("  ");
                }
                if *number {
                    line.extend(std::iter::repeat_n(' ', pad));
                    line.push_str(text);
                } else {
                    line.push_str(text);
                    if i + 1 < row.len() {
                        line.extend(std::iter::repeat_n(' ', pad));
                    }
                }
            }
            writeln!(out, "{line}")?;
        }
        out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(writer: &mut dyn ReportWriter) {
        writer
            .header(&["path".into(), "size".into(), "modified".into()])
            .expect("header");
        for (path, size) in [("/a & b <x>", 2048u64), ("/c", 5)] {
            writer
                .row(&[Value::Text(path.into()), Value::Bytes(size), Value::Date(0)])
                .expect("row");
        }
        writer.finish().expect("finish");
    }

    #[test]
    fn json_is_valid_and_exact() {
        let mut buf = Vec::new();
        rows(&mut Json::new(
            &mut buf,
            vec!["path", "size", "modified_utc"],
        ));
        let text = String::from_utf8(buf).expect("utf8");
        assert!(
            text.contains(
                r#"{"path": "/a & b <x>", "size": 2048, "modified_utc": "1970-01-01 00:00:00"}"#
            ),
            "{text}"
        );
        assert!(text.trim_end().ends_with(']'));
    }

    #[test]
    fn html_escapes_and_sets_direction() {
        let mut buf = Vec::new();
        rows(&mut Html::new(&mut buf, "דוח", "he", true));
        let text = String::from_utf8(buf).expect("utf8");
        assert!(text.contains(r#"dir="rtl""#));
        assert!(text.contains("/a &amp; b &lt;x&gt;"));
        assert!(text.contains("<td class=\"n\">2048</td>"));
    }

    #[test]
    fn text_aligns_columns() {
        let mut buf = Vec::new();
        rows(&mut Text::new(&mut buf));
        let text = String::from_utf8(buf).expect("utf8");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "/a & b <x>  2048  1970-01-01 00:00:00");
        assert_eq!(lines[2], "/c             5  1970-01-01 00:00:00");
    }

    #[test]
    fn excel_workbook_holds_the_sheet() {
        let mut buf = io::Cursor::new(Vec::new());
        rows(&mut Excel::new(&mut buf, false));
        let mut zip = zip::ZipArchive::new(io::Cursor::new(buf.into_inner())).expect("zip");
        let mut sheet = String::new();
        io::Read::read_to_string(
            &mut zip.by_name("xl/worksheets/sheet1.xml").expect("sheet"),
            &mut sheet,
        )
        .expect("read");
        assert!(sheet.contains("<c><v>2048</v></c>"), "{sheet}");
        assert!(sheet.contains("/a &amp; b &lt;x&gt;"));
        assert!(zip.by_name("xl/workbook.xml").is_ok());
    }
}
