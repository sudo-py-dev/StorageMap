//! User preferences, persisted by eframe between runs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use storagemap_core::Metric;

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
    pub cross_filesystems: bool,
    pub prefer_mft: bool,
    pub recent: Vec<PathBuf>,
    /// Save a snapshot after every completed scan (enables "what changed").
    pub auto_snapshot: bool,
    /// Smallest file the duplicate finder considers.
    pub duplicate_min_size: u64,
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
            cross_filesystems: false,
            prefer_mft: true,
            recent: Vec::new(),
            auto_snapshot: true,
            duplicate_min_size: DEFAULT_DUPLICATE_MIN_SIZE,
        }
    }
}

impl Settings {
    pub fn effective_lang(&self) -> Lang {
        self.lang.unwrap_or_else(Lang::system)
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
    fn old_settings_files_still_load() {
        let s: Settings =
            serde_json::from_str(r#"{"decimals":2,"unknown_future_field":true}"#).expect("parse");
        assert_eq!(s.decimals, 2);
        assert!(s.treemap_files);
    }
}
