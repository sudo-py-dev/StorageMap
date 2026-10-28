//! Phosphor icons (MIT, <https://phosphoricons.com>) as a fallback font, so
//! icons can be mixed into any label: `format!("{} Scan", icons::FOLDER_OPEN)`.
//!
//! One consistent line-icon set instead of colour emoji, which look
//! different on every system. The codepoints are in the Private Use Area,
//! so the font can never shadow real text.

use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{FontData, FontFamily};

const FONT: &[u8] = include_bytes!("../../../assets/fonts/Phosphor.ttf");

pub fn install(ctx: &egui::Context) {
    ctx.add_font(FontInsert::new(
        "phosphor",
        FontData::from_static(FONT),
        vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Lowest,
        }],
    ));
}

// Navigation and actions
pub const FOLDER_OPEN: &str = "\u{E256}";
pub const FOLDER: &str = "\u{E24A}";
pub const FILE: &str = "\u{E230}";
pub const LINK: &str = "\u{E2E2}";
pub const RESCAN: &str = "\u{E094}";
pub const STOP: &str = "\u{E46C}";
pub const UP: &str = "\u{E08E}";
pub const HOME: &str = "\u{E2C2}";
pub const SEARCH: &str = "\u{E30C}";
pub const SETTINGS: &str = "\u{E272}";
pub const LANGUAGE: &str = "\u{E4A2}";
pub const SNAPSHOT: &str = "\u{E10E}";
pub const EXPORT: &str = "\u{E20C}";
pub const QUIT: &str = "\u{E42A}";
pub const TRASH: &str = "\u{E4A6}";
pub const OPEN: &str = "\u{E5DE}";
pub const REVEAL: &str = "\u{E24A}";
pub const COPY: &str = "\u{E1CC}";
pub const ZOOM_IN: &str = "\u{E310}";
pub const ZOOM_OUT: &str = "\u{E30E}";
pub const CLOSE: &str = "\u{E4F6}";
pub const GITHUB: &str = "\u{E576}";
pub const UPDATE: &str = "\u{E030}";
pub const SHIELD: &str = "\u{E40C}";
pub const LIGHTNING: &str = "\u{E2DE}";
pub const CHECK: &str = "\u{E184}";
pub const WARNING: &str = "\u{E4E0}";
pub const INFO: &str = "\u{E2CE}";
pub const SPARKLE: &str = "\u{E6A2}";

// Tabs
pub const TREEMAP: &str = "\u{E464}";
pub const DETAILS: &str = "\u{E2F2}";
pub const CLEANUP: &str = "\u{EC54}";
pub const LARGEST: &str = "\u{E446}";
pub const TYPES: &str = "\u{E158}";
pub const CHART_DONUT: &str = "\u{EAA6}";
pub const CHART_BARS: &str = "\u{E152}";
pub const FILTER: &str = "\u{E266}";
pub const PLUS: &str = "\u{E3D4}";
pub const SAVED: &str = "\u{E0EA}";
pub const CARET_LEFT: &str = "\u{E138}";
pub const CARET_RIGHT: &str = "\u{E13A}";
pub const AGE: &str = "\u{E19A}";
pub const CHANGES: &str = "\u{E1A0}";
pub const DUPLICATES: &str = "\u{E1CA}";

// Settings sections
pub const GENERAL: &str = "\u{E434}";
pub const APPEARANCE: &str = "\u{E6C8}";
pub const INTEGRATION: &str = "\u{E596}";
pub const KEYBOARD: &str = "\u{E2D8}";

// Drives
pub const DRIVE: &str = "\u{E29E}";
pub const USB: &str = "\u{E956}";
pub const NETWORK: &str = "\u{E288}";
pub const DISC: &str = "\u{E564}";
pub const MEMORY: &str = "\u{E9C4}";

// Cleanup categories
pub const DATABASE: &str = "\u{E1DE}";
pub const WRENCH: &str = "\u{E5D4}";
pub const PACKAGE: &str = "\u{E390}";
pub const HOURGLASS: &str = "\u{E2B2}";
pub const SCROLL: &str = "\u{EB7A}";
pub const BUG: &str = "\u{E5F4}";
pub const DOWNLOAD: &str = "\u{E20C}";
