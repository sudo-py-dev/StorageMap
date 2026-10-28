//! User preferences, persisted by eframe between runs.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use storagemap_core::export::Include;
use storagemap_core::{Metric, ScanFilter};

use crate::fmt::Unit;
use crate::i18n::Lang;

pub const STORAGE_KEY: &str = "storagemap.settings.v1";
const MAX_RECENT: usize = 8;
pub const MAX_TREEMAP_DEPTH: u8 = 12;
pub const DEFAULT_DUPLICATE_MIN_SIZE: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn label_id(self) -> &'static str {
        match self {
            Theme::System => "theme-system",
            Theme::Light => "theme-light",
            Theme::Dark => "theme-dark",
        }
    }

    pub fn preference(self) -> egui::ThemePreference {
        match self {
            Theme::System => egui::ThemePreference::System,
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `None` follows the system language.
    pub lang: Option<Lang>,
    pub metric: Metric,
    pub theme: Theme,
    pub unit: Unit,
    pub decimals: u8,
    pub treemap_depth: u8,
    pub treemap_files: bool,
    pub treemap_labels: bool,
    /// Colour the names of items that take an unusually large share.
    pub highlight_large: bool,
    pub chart_style: crate::views::chart::Style,
    pub cross_filesystems: bool,
    pub prefer_mft: bool,
    pub recent: Vec<PathBuf>,
    /// Save a snapshot after every completed scan (enables "what changed").
    pub auto_snapshot: bool,
    /// Smallest file the duplicate finder considers.
    pub duplicate_min_size: u64,
    /// Ask GitHub once a day whether a newer release exists.
    pub check_updates: bool,
    /// Unix seconds of the last update check.
    pub last_update_check: i64,
    /// egui zoom factor (also changed with Ctrl +/-).
    pub ui_scale: f32,
    /// The window has been sized to the monitor once.
    pub window_sized: bool,
    /// A release the user chose to skip.
    pub skipped_update: Option<String>,
    /// Rows the CSV report lists.
    pub export_include: Include,
    /// Write sizes in the CSV report as "1.4 GB" rather than bytes.
    pub export_readable: bool,
    /// Files and folders every scan leaves out or limits itself to.
    pub scan_filter: ScanFilter,
    /// Columns the user hid, by table id, as column title ids.
    pub hidden_columns: BTreeMap<String, Vec<String>>,
    /// Column order the user chose, by table id; columns not listed keep
    /// their default place after the listed ones.
    pub column_order: BTreeMap<String, Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: None,
            metric: Metric::Size,
            theme: Theme::System,
            unit: Unit::Auto,
            decimals: 1,
            treemap_depth: 6,
            treemap_files: true,
            treemap_labels: true,
            highlight_large: true,
            chart_style: crate::views::chart::Style::Donut,
            cross_filesystems: false,
            prefer_mft: true,
            recent: Vec::new(),
            auto_snapshot: true,
            duplicate_min_size: DEFAULT_DUPLICATE_MIN_SIZE,
            check_updates: true,
            last_update_check: 0,
            ui_scale: 1.0,
            window_sized: false,
            skipped_update: None,
            export_include: Include::All,
            export_readable: true,
            scan_filter: ScanFilter::default(),
            hidden_columns: BTreeMap::new(),
            column_order: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn effective_lang(&self) -> Lang {
        self.lang.unwrap_or_else(Lang::system)
    }

    pub fn column_hidden(&self, table: &str, column: &str) -> bool {
        self.hidden_columns
            .get(table)
            .is_some_and(|cols| cols.iter().any(|c| c == column))
    }

    pub fn toggle_column(&mut self, table: &str, column: &str) {
        let cols = self.hidden_columns.entry(table.to_owned()).or_default();
        if let Some(i) = cols.iter().position(|c| c == column) {
            cols.remove(i);
        } else {
            cols.push(column.to_owned());
        }
        if cols.is_empty() {
            self.hidden_columns.remove(table);
        }
    }

    /// Shows every column of `table` again, in the default order.
    pub fn reset_columns(&mut self, table: &str) {
        self.hidden_columns.remove(table);
        self.column_order.remove(table);
    }

    pub fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_is_deduplicated_and_bounded() {
        let mut s = Settings::default();
        for i in 0..20 {
            s.remember(PathBuf::from(format!("/p{i}")));
        }
        s.remember(PathBuf::from("/p15"));
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert_eq!(s.recent[0], PathBuf::from("/p15"));
        assert_eq!(
            s.recent
                .iter()
                .filter(|p| p.as_path() == std::path::Path::new("/p15"))
                .count(),
            1
        );
    }

    #[test]
    fn columns_toggle_per_table() {
        let mut s = Settings::default();
        s.toggle_column("details", "col-type");
        assert!(s.column_hidden("details", "col-type"));
        assert!(!s.column_hidden("search", "col-type"));
        s.toggle_column("details", "col-type");
        assert!(!s.column_hidden("details", "col-type"));
        assert!(s.hidden_columns.is_empty());
    }

    #[test]
    fn old_settings_files_still_load() {
        let s: Settings =
            serde_json::from_str(r#"{"decimals":2,"unknown_future_field":true}"#).expect("parse");
        assert_eq!(s.decimals, 2);
        assert!(s.treemap_files);
    }
}
