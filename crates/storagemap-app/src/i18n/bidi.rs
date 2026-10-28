//! Converts logical-order text into the order egui 0.37 needs.
//!
//! egui places shaping runs left to right, but shapes each run with
//! harfrust, which lays out a Hebrew/Arabic run right-to-left (and joins
//! Arabic letters). So egui gets words right but sentences wrong. We run the
//! Unicode Bidirectional Algorithm (UAX #9) to put runs in visual order, then
//! restore logical order *inside* each RTL run so harfrust's own reversal
//! (and Arabic joining, which needs logical input) lands correctly.

use unicode_bidi::{BidiInfo, Level};

use super::Direction;

/// Formatting characters that must not reach the renderer once applied.
const BIDI_CONTROLS: &[char] = &[
    '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Whether a character has strong right-to-left direction.
pub fn is_rtl_char(c: char) -> bool {
    matches!(c,
        '\u{0590}'..='\u{08FF}'      // Hebrew, Arabic, Syriac, Thaana, NKo, Samaritan, Mandaic
        | '\u{FB1D}'..='\u{FDFF}'    // Hebrew & Arabic presentation forms A
        | '\u{FE70}'..='\u{FEFF}'    // Arabic presentation forms B
        | '\u{10800}'..='\u{10FFF}'  // historic RTL scripts
        | '\u{1E800}'..='\u{1EFFF}') // Adlam, Arabic math
}

pub fn has_rtl(text: &str) -> bool {
    text.chars().any(is_rtl_char)
}

/// Returns `text` ready for egui in a paragraph of direction `base`.
/// Text without RTL characters only loses its invisible bidi controls.
pub fn visual(text: &str, base: Direction) -> String {
    if !has_rtl(text) {
        return strip_controls(text);
    }
    let level = match base {
        Direction::Ltr => Level::ltr(),
        Direction::Rtl => Level::rtl(),
    };
    text.split('\n')
        .map(|line| {
            let info = BidiInfo::new(line, Some(level));
            let reordered = match info.paragraphs.first() {
                Some(para) => info.reorder_line(para, para.range.clone()).into_owned(),
                None => line.to_owned(),
            };
            restore_rtl_runs(&strip_controls(&reordered))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reverses each maximal run of RTL characters back to logical order; the
/// shaper reverses it again when drawing.
fn restore_rtl_runs(visual: &str) -> String {
    let chars: Vec<char> = visual.chars().collect();
    let mut out = String::with_capacity(visual.len());
    let mut i = 0;
    while i < chars.len() {
        if is_rtl_char(chars[i]) {
            let start = i;
            while i < chars.len() && is_rtl_char(chars[i]) {
                i += 1;
            }
            out.extend(chars[start..i].iter().rev());
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn strip_controls(text: &str) -> String {
    text.chars()
        .filter(|c| !BIDI_CONTROLS.contains(c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_text_is_untouched() {
        assert_eq!(visual("report.pdf", Direction::Ltr), "report.pdf");
        assert_eq!(visual("report.pdf", Direction::Rtl), "report.pdf");
    }

    #[test]
    fn single_word_is_left_to_the_shaper() {
        assert_eq!(visual("שלום", Direction::Rtl), "שלום");
    }

    #[test]
    fn rtl_words_are_ordered_right_to_left() {
        // Second word must be drawn to the left of the first.
        assert_eq!(visual("אב גד", Direction::Rtl), "גד אב");
    }

    #[test]
    fn numbers_keep_their_order_inside_rtl() {
        // "file 123" in Hebrew: the number goes to the left, digits stay LTR.
        assert_eq!(visual("קובץ 123", Direction::Rtl), "123 קובץ");
        assert_eq!(
            visual("266,942 פריטים ב־0.3 שניות", Direction::Rtl),
            "שניות 0.3ב־ פריטים 266,942"
        );
    }

    #[test]
    fn hebrew_file_name_in_ltr_context() {
        assert_eq!(visual("שלום.txt", Direction::Ltr), "שלום.txt");
        assert_eq!(
            visual("report אב גד.txt", Direction::Ltr),
            "report גד אב.txt"
        );
    }

    #[test]
    fn isolates_from_fluent_are_removed() {
        assert_eq!(visual("a \u{2068}b\u{2069}", Direction::Ltr), "a b");
        assert!(!visual("\u{2068}12\u{2069} קבצים", Direction::Rtl).contains('\u{2068}'));
    }

    #[test]
    fn arabic_stays_logical_for_the_shaper() {
        // harfrust needs logical order to join letters; words still reorder.
        assert_eq!(visual("ملف كبير", Direction::Rtl), "كبير ملف");
    }

    #[test]
    fn multiple_lines_are_reordered_independently() {
        assert_eq!(visual("אב גד\nהו זח", Direction::Rtl), "גד אב\nזח הו");
    }
}
