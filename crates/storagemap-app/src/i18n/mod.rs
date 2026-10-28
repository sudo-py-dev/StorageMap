//! Translations (Project Fluent) plus right-to-left support.
//!
//! Every user-visible string goes through [`I18n`]: `t`/`ta` translate and
//! return text already in visual order; `v` does the same for dynamic text
//! such as file names.

pub mod bidi;

use std::cell::RefCell;
use std::collections::HashMap;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

/// Cached visual strings before the cache is flushed (file names are unbounded).
const VISUAL_CACHE_LIMIT: usize = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    En,
    He,
    Ar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::He, Lang::Ar];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::He => "he",
            Lang::Ar => "ar",
        }
    }

    /// The language's own name, shown in the language menu.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::He => "עברית",
            Lang::Ar => "العربية",
        }
    }

    pub fn direction(self) -> Direction {
        match self {
            Lang::En => Direction::Ltr,
            Lang::He | Lang::Ar => Direction::Rtl,
        }
    }

    fn source(self) -> &'static str {
        match self {
            Lang::En => include_str!("../../locales/en.ftl"),
            Lang::He => include_str!("../../locales/he.ftl"),
            Lang::Ar => include_str!("../../locales/ar.ftl"),
        }
    }

    /// Best match for a BCP 47 / POSIX locale such as `he-IL` or `ar_EG.UTF-8`.
    pub fn from_locale(locale: &str) -> Option<Lang> {
        let primary = locale.split(['-', '_', '.']).next()?.to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Lang::En),
            "he" | "iw" => Some(Lang::He),
            "ar" => Some(Lang::Ar),
            _ => None,
        }
    }

    /// The system's preferred supported language, else English.
    pub fn system() -> Lang {
        sys_locale::get_locales()
            .find_map(|l| Lang::from_locale(&l))
            .unwrap_or(Lang::En)
    }
}

pub struct I18n {
    lang: Lang,
    bundle: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
    visual_cache: RefCell<HashMap<String, String>>,
}

impl I18n {
    pub fn new(lang: Lang) -> Self {
        I18n {
            lang,
            bundle: bundle(lang),
            fallback: bundle(Lang::En),
            visual_cache: RefCell::default(),
        }
    }

    pub fn dir(&self) -> Direction {
        self.lang.direction()
    }

    pub fn is_rtl(&self) -> bool {
        self.dir() == Direction::Rtl
    }

    /// Translated text, visual order.
    pub fn t(&self, id: &str) -> String {
        self.v(&self.raw(id, None))
    }

    /// Translated text with arguments, visual order.
    pub fn ta(&self, id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        self.v(&self.la(id, args))
    }

    /// Translated text with arguments, logical order — for composing into
    /// larger strings that are converted once at the end.
    pub fn la(&self, id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let mut fa = FluentArgs::new();
        for (k, val) in args {
            fa.set(*k, val.clone());
        }
        strip_isolates(&self.raw(id, Some(&fa)))
    }

    /// Translated text in logical order (for clipboard, file names, OS dialogs).
    pub fn logical(&self, id: &str) -> String {
        strip_isolates(&self.raw(id, None))
    }

    /// A plural message: `$count` selects the plural form, `$n` is the
    /// grouped number shown. Logical order.
    pub fn count(&self, id: &str, n: u64) -> String {
        self.la(
            id,
            &[("count", n.into()), ("n", crate::fmt::count(n).into())],
        )
    }

    /// Dynamic text (file names, paths) in visual order for this UI direction.
    pub fn v(&self, text: &str) -> String {
        if !bidi::has_rtl(text) {
            return strip_isolates(text);
        }
        let mut cache = self.visual_cache.borrow_mut();
        if let Some(hit) = cache.get(text) {
            return hit.clone();
        }
        if cache.len() >= VISUAL_CACHE_LIMIT {
            cache.clear();
        }
        let out = bidi::visual(text, self.dir());
        cache.insert(text.to_owned(), out.clone());
        out
    }

    fn raw(&self, id: &str, args: Option<&FluentArgs<'_>>) -> String {
        format(&self.bundle, id, args)
            .or_else(|| format(&self.fallback, id, args))
            .unwrap_or_else(|| {
                tracing::warn!(id, "missing translation");
                id.to_owned()
            })
    }
}

fn format(
    bundle: &FluentBundle<FluentResource>,
    id: &str,
    args: Option<&FluentArgs<'_>>,
) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args, &mut errors);
    if !errors.is_empty() {
        tracing::warn!(id, ?errors, "translation formatting errors");
    }
    Some(text.into_owned())
}

fn bundle(lang: Lang) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = lang.code().parse().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![id]);
    let resource = match FluentResource::try_new(lang.source().to_owned()) {
        Ok(r) => r,
        Err((r, errors)) => {
            tracing::error!(lang = lang.code(), ?errors, "locale file has syntax errors");
            r
        }
    };
    if let Err(errors) = bundle.add_resource(resource) {
        tracing::error!(lang = lang.code(), ?errors, "duplicate translation ids");
    }
    bundle
}

fn strip_isolates(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn ids(lang: Lang) -> BTreeSet<String> {
        lang.source()
            .lines()
            .filter_map(|l| l.split_once(" =").map(|(k, _)| k))
            .filter(|k| !k.is_empty() && !k.starts_with([' ', '#']))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn every_locale_has_every_message() {
        let en = ids(Lang::En);
        for lang in [Lang::He, Lang::Ar] {
            let other = ids(lang);
            assert_eq!(
                en.difference(&other).collect::<Vec<_>>(),
                Vec::<&String>::new(),
                "missing in {}",
                lang.code()
            );
            assert_eq!(
                other.difference(&en).collect::<Vec<_>>(),
                Vec::<&String>::new(),
                "extra in {}",
                lang.code()
            );
        }
    }

    #[test]
    fn locale_files_parse_cleanly() {
        for lang in Lang::ALL {
            assert!(
                FluentResource::try_new(lang.source().to_owned()).is_ok(),
                "{}",
                lang.code()
            );
        }
    }

    #[test]
    fn plurals_follow_language_rules() {
        let he = I18n::new(Lang::He);
        assert_eq!(he.logical_args("files-count", 2), "שני קבצים");
        let ar = I18n::new(Lang::Ar);
        let en = I18n::new(Lang::En);
        assert_eq!(ar.logical_args("files-count", 5), "5 ملفات");
        assert_eq!(ar.logical_args("files-count", 11), "11 ملف");
        assert_eq!(en.logical_args("files-count", 1234), "1,234 files");
        assert_eq!(en.logical_args("files-count", 1), "1 file");
    }

    #[test]
    fn locale_detection() {
        assert_eq!(Lang::from_locale("he-IL"), Some(Lang::He));
        assert_eq!(Lang::from_locale("iw_IL.UTF-8"), Some(Lang::He));
        assert_eq!(Lang::from_locale("ar_EG"), Some(Lang::Ar));
        assert_eq!(Lang::from_locale("fr-FR"), None);
    }

    #[test]
    fn unknown_ids_fall_back_to_the_id() {
        assert_eq!(I18n::new(Lang::He).t("no-such-key"), "no-such-key");
    }

    impl I18n {
        fn logical_args(&self, id: &str, count: u64) -> String {
            self.count(id, count)
        }
    }
}
