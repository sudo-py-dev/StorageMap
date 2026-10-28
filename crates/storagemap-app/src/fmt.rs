//! Number, size and date formatting.

use serde::{Deserialize, Serialize};

const KIB: f64 = 1024.0;

/// Size unit choice; `Auto` picks the largest unit with a value ≥ 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Unit {
    #[default]
    Auto,
    B,
    Kb,
    Mb,
    Gb,
    Tb,
}

impl Unit {
    pub const ALL: [Unit; 6] = [Unit::Auto, Unit::B, Unit::Kb, Unit::Mb, Unit::Gb, Unit::Tb];

    /// Power of 1024 for fixed units.
    fn exponent(self) -> Option<i32> {
        match self {
            Unit::Auto => None,
            Unit::B => Some(0),
            Unit::Kb => Some(1),
            Unit::Mb => Some(2),
            Unit::Gb => Some(3),
            Unit::Tb => Some(4),
        }
    }

    /// Translation id of the unit label.
    pub fn label_id(self) -> &'static str {
        match self {
            Unit::Auto => "units-auto",
            Unit::B => "unit-b",
            Unit::Kb => "unit-kb",
            Unit::Mb => "unit-mb",
            Unit::Gb => "unit-gb",
            Unit::Tb => "unit-tb",
        }
    }
}

const UNIT_LADDER: [Unit; 5] = [Unit::B, Unit::Kb, Unit::Mb, Unit::Gb, Unit::Tb];

/// Splits a byte count into a scaled number and its unit.
pub fn scale(bytes: u64, unit: Unit) -> (f64, Unit) {
    let exp = unit.exponent().unwrap_or_else(|| {
        let mut e = 0;
        let mut v = bytes as f64;
        while v >= KIB && e < 4 {
            v /= KIB;
            e += 1;
        }
        e
    });
    let unit = UNIT_LADDER[exp as usize];
    (bytes as f64 / KIB.powi(exp), unit)
}

/// `1.5 GB`-style text; `unit_label` translates a unit's label id.
pub fn size(
    bytes: u64,
    unit: Unit,
    decimals: usize,
    unit_label: impl Fn(&str) -> String,
) -> String {
    let (value, unit) = scale(bytes, unit);
    let decimals = if unit == Unit::B { 0 } else { decimals };
    format!(
        "{} {}",
        group(&format!("{value:.decimals$}")),
        unit_label(unit.label_id())
    )
}

/// English size text for the CLI.
pub fn size_en(bytes: u64) -> String {
    size(bytes, Unit::Auto, 1, |id| match id {
        "unit-kb" => "KB".into(),
        "unit-mb" => "MB".into(),
        "unit-gb" => "GB".into(),
        "unit-tb" => "TB".into(),
        _ => "B".into(),
    })
}

/// Thousands separators for the integer part of a decimal string.
pub fn group(number: &str) -> String {
    let (int, frac) = number
        .split_once('.')
        .map_or((number, None), |(i, f)| (i, Some(f)));
    let (sign, digits) = int.strip_prefix('-').map_or(("", int), |d| ("-", d));
    let mut out = String::with_capacity(number.len() + digits.len() / 3);
    out.push_str(sign);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

pub fn count(n: u64) -> String {
    group(&n.to_string())
}

pub fn percent(share: f64) -> String {
    format!("{:.1}%", share * 100.0)
}

/// `YYYY-MM-DD` in local time; ISO dates read the same in every locale.
pub fn date(unix_secs: i64) -> String {
    local(unix_secs, "%Y-%m-%d")
}

/// `YYYY-MM-DD HH:MM` in local time.
pub fn date_time(unix_secs: i64) -> String {
    local(unix_secs, "%Y-%m-%d %H:%M")
}

fn local(unix_secs: i64, pattern: &str) -> String {
    match chrono::DateTime::from_timestamp(unix_secs, 0) {
        Some(t) if unix_secs > 0 => t.with_timezone(&chrono::Local).format(pattern).to_string(),
        _ => "—".into(),
    }
}

/// `+1.5 GB` / `−200 MB`; zero has no sign.
pub fn signed_size(
    delta: i128,
    unit: Unit,
    decimals: usize,
    unit_label: impl Fn(&str) -> String,
) -> String {
    let magnitude = u64::try_from(delta.unsigned_abs()).unwrap_or(u64::MAX);
    let sign = match delta.signum() {
        1 => "+",
        -1 => "\u{2212}",
        _ => "",
    };
    format!("{sign}{}", size(magnitude, unit, decimals, unit_label))
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        assert_eq!(group("1234567"), "1,234,567");
        assert_eq!(group("123"), "123");
        assert_eq!(group("-1234.50"), "-1,234.50");
        assert_eq!(group("0"), "0");
    }

    #[test]
    fn sizes() {
        assert_eq!(size_en(0), "0 B");
        assert_eq!(size_en(1023), "1,023 B");
        assert_eq!(size_en(1536), "1.5 KB");
        assert_eq!(size_en(5 * 1024 * 1024 * 1024), "5.0 GB");
        assert_eq!(size_en(u64::MAX), "16,777,216.0 TB");
    }

    #[test]
    fn fixed_units() {
        assert_eq!(scale(1024 * 1024, Unit::Kb), (1024.0, Unit::Kb));
    }

    #[test]
    fn dates() {
        assert_eq!(date(0), "—");
        assert_eq!(date(1_700_000_000).len(), 10);
        assert_eq!(date_time(1_700_000_000).len(), 16);
    }

    #[test]
    fn signed_sizes() {
        let en = |id: &str| {
            if id == "unit-kb" {
                "KB".to_owned()
            } else {
                "B".to_owned()
            }
        };
        assert_eq!(signed_size(2048, Unit::Auto, 1, en), "+2.0 KB");
        assert_eq!(signed_size(-5, Unit::Auto, 1, en), "\u{2212}5 B");
        assert_eq!(signed_size(0, Unit::Auto, 1, en), "0 B");
    }
}
