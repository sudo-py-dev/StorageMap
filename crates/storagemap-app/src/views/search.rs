//! Search tab: a name field, optional filters (type, size, age, owner,
//! attributes), saved searches, and the results as a table.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{ComboBox, Response, Ui, vec2};
use serde::{Deserialize, Serialize};
use storagemap_core::analysis::extension_of;
use storagemap_core::search::{self, Entries, SearchQuery};
use storagemap_core::{Flags, Kind, NodeId, Tree};

use super::lists::ENTRY_COLUMNS;
use super::table::TableSource;
use super::{Env, ScanView, node_menu, table};
use crate::app::Action;
use crate::filetype::Category;
use crate::i18n::{Direction, bidi};
use crate::widgets::empty_state;

const SEARCH_LIMIT: usize = 2_000;
/// Wait this long after the last keystroke before searching.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
/// Room for the full hint and a typical name in the Search tab's field.
const SEARCH_FIELD_WIDTH: f32 = 560.0;
const MB: u64 = 1024 * 1024;
const SIZE_STEPS: [u64; 6] = [
    MB,
    10 * MB,
    100 * MB,
    1024 * MB,
    10 * 1024 * MB,
    100 * 1024 * MB,
];
const DAY: i64 = 86_400;

/// How recently entries were modified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Modified {
    #[default]
    Any,
    /// Within the last this many days.
    Within(u16),
    /// Not for at least this many days.
    OlderThan(u16),
}

impl Modified {
    const PRESETS: [Modified; 7] = [
        Modified::Any,
        Modified::Within(7),
        Modified::Within(30),
        Modified::Within(365),
        Modified::OlderThan(365),
        Modified::OlderThan(2 * 365),
        Modified::OlderThan(5 * 365),
    ];

    /// `(after, before)` bounds in Unix seconds for a search run at `now`.
    fn bounds(self, now: i64) -> (Option<i64>, Option<i64>) {
        match self {
            Modified::Any => (None, None),
            Modified::Within(days) => (Some(now - i64::from(days) * DAY), None),
            Modified::OlderThan(days) => (None, Some(now - i64::from(days) * DAY)),
        }
    }

    fn label(self, env: &Env<'_>) -> String {
        let id = match self {
            Modified::Any => return env.t("filter-any-time"),
            Modified::Within(7) => "filter-within-week",
            Modified::Within(30) => "filter-within-month",
            Modified::Within(_) => "filter-within-year",
            Modified::OlderThan(365) => "filter-older-year",
            Modified::OlderThan(730) => "filter-older-two-years",
            Modified::OlderThan(_) => "filter-older-five-years",
        };
        env.t(id)
    }
}

/// An attribute a search can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attribute {
    ReadOnly,
    Hidden,
    System,
    Compressed,
    Encrypted,
    Sparse,
}

impl Attribute {
    const ALL: [Attribute; 6] = [
        Attribute::ReadOnly,
        Attribute::Hidden,
        Attribute::System,
        Attribute::Compressed,
        Attribute::Encrypted,
        Attribute::Sparse,
    ];

    fn flag(self) -> Flags {
        match self {
            Attribute::ReadOnly => Flags::READ_ONLY,
            Attribute::Hidden => Flags::HIDDEN,
            Attribute::System => Flags::SYSTEM,
            Attribute::Compressed => Flags::COMPRESSED,
            Attribute::Encrypted => Flags::ENCRYPTED,
            Attribute::Sparse => Flags::SPARSE,
        }
    }

    fn label_id(self) -> &'static str {
        match self {
            Attribute::ReadOnly => "attr-read-only",
            Attribute::Hidden => "attr-hidden",
            Attribute::System => "attr-system",
            Attribute::Compressed => "attr-compressed",
            Attribute::Encrypted => "attr-encrypted",
            Attribute::Sparse => "attr-sparse",
        }
    }
}

/// The search filters besides the name. Kept relative (for example "older
/// than a year"), so a saved search means the same thing whenever it runs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filters {
    pub entries: Entries,
    /// Only files of this kind (implies files).
    pub category: Option<Category>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub modified: Modified,
    pub owner: Option<u32>,
    pub attribute: Option<Attribute>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        *self == Filters::default()
    }

    fn query(&self, name: &str, now: i64) -> SearchQuery {
        let (modified_after, modified_before) = self.modified.bounds(now);
        SearchQuery {
            name: name.to_owned(),
            entries: if self.category.is_some() {
                Entries::Files
            } else {
                self.entries
            },
            min_size: self.min_size,
            max_size: self.max_size,
            modified_after,
            modified_before,
            owner: self.owner,
            attributes: self.attribute.map_or(Flags::empty(), Attribute::flag),
        }
    }
}

/// A search the user saved by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSearch {
    pub name: String,
    pub text: String,
    pub filters: Filters,
}

/// The search criteria the results were computed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    focus: NodeId,
    text: String,
    filters: Filters,
    generation: u64,
    /// The other open scans searched too (root and generation), if any.
    others: Option<Vec<(PathBuf, u64)>>,
}

/// One result: an entry of this scan, or of another open scan (by its
/// position among them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub tab: Option<usize>,
    pub id: NodeId,
}

impl Hit {
    /// The tree the hit belongs to.
    pub fn tree<'a>(&self, this: &'a Tree, others: &'a [ScanView]) -> &'a Tree {
        match self.tab {
            Some(i) => &others[i].tree,
            None => this,
        }
    }
}

/// Results with their entry's name, size, containing folder and date.
struct HitList<'a> {
    this: &'a Tree,
    others: &'a [ScanView],
    hits: &'a [Hit],
    selected: Option<NodeId>,
}

impl TableSource for HitList<'_> {
    fn rows(&self) -> usize {
        self.hits.len()
    }

    fn is_selected(&self, row: usize) -> bool {
        let hit = self.hits[row];
        hit.tab.is_none() && self.selected == Some(hit.id)
    }

    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize) {
        let hit = self.hits[row];
        let tree = hit.tree(self.this, self.others);
        let node = tree.node(hit.id);
        match col {
            0 => table::text(ui, env, &tree.name(hit.id), false),
            1 => table::text(ui, env, &env.metric_text(node), true),
            3 => table::text(ui, env, &crate::fmt::date(node.modified), true),
            _ => {
                let parent = tree
                    .parent(hit.id)
                    .map(|p| tree.path(p).to_string_lossy().into_owned())
                    .unwrap_or_default();
                table::text(ui, env, &parent, false);
            }
        }
    }

    fn row_response(&mut self, env: &mut Env<'_>, row: usize, response: &Response) {
        let hit = self.hits[row];
        match hit.tab {
            None => {
                if response.double_clicked() {
                    env.push(Action::Focus(hit.id));
                } else if response.clicked() {
                    env.push(Action::Select(hit.id));
                }
                response.context_menu(|ui| node_menu::show(ui, env, self.this, hit.id));
            }
            // Its scan is in another tab: go there.
            Some(i) => {
                if response.clicked() {
                    let tree = &self.others[i].tree;
                    env.push(Action::RevealInTab {
                        root: tree.root_path().to_path_buf(),
                        path: tree.path(hit.id),
                    });
                }
                response
                    .clone()
                    .on_hover_text(env.t("search-other-tab-hint"));
            }
        }
    }
}

/// A name-search text field bound to the scan's query.
fn query_edit(
    ui: &mut Ui,
    env: &Env<'_>,
    scan: &mut ScanView,
    width: f32,
    margin: egui::Vec2,
) -> Response {
    let mut layouter = bidi_layouter(env.i18n.dir());
    let hint = format!("{}  {}", crate::icons::SEARCH, env.t("search-hint"));
    let edit = egui::TextEdit::singleline(&mut scan.search_query)
        .hint_text(hint)
        .desired_width(width)
        .margin(margin)
        .layouter(&mut layouter);
    let response = ui.add(edit);
    if response.changed() {
        scan.search_edited = Some(Instant::now());
    }
    response
}

/// The toolbar search field, shown outside the Search tab. Typing switches
/// to the Search tab, whose own field takes over.
pub fn search_box(ui: &mut Ui, env: &Env<'_>, scan: &mut ScanView, width: f32) {
    if query_edit(ui, env, scan, width, vec2(4.0, 2.0)).changed() {
        scan.tab = super::Tab::Search;
        scan.tab_changed = Some(Instant::now());
        scan.focus_search = true;
    }
}

/// `others` are the other open scans, searched too when the user asks.
pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView, others: &[ScanView]) {
    crate::widgets::row(ui, env.i18n.dir(), |ui| {
        let width = (ui.available_width() - 260.0).clamp(160.0, SEARCH_FIELD_WIDTH);
        let response = query_edit(ui, env, scan, width, vec2(10.0, 8.0));
        if std::mem::take(&mut scan.focus_search) {
            response.request_focus();
        }
        if !scan.search_query.is_empty()
            && ui
                .button(crate::icons::CLOSE)
                .on_hover_text(env.t("search-clear"))
                .clicked()
        {
            scan.search_query.clear();
            scan.search_edited = None;
            response.request_focus();
        }
        let filters_label = format!("{}  {}", crate::icons::FILTER, env.t("search-filters"));
        ui.toggle_value(&mut scan.show_search_filters, filters_label);
        saved_menu(ui, env, scan);
        if !others.is_empty() {
            let label = format!("{}  {}", crate::icons::TABS, env.t("search-all-tabs"));
            ui.toggle_value(&mut scan.search_all_tabs, label)
                .on_hover_text(env.t("search-all-tabs-hint"));
        }
    });
    if scan.show_search_filters {
        ui.add_space(6.0);
        filters_row(ui, env, scan);
    }
    ui.add_space(8.0);

    let settled = scan
        .search_edited
        .is_none_or(|t| t.elapsed() >= SEARCH_DEBOUNCE);
    if !settled {
        ui.ctx().request_repaint_after(SEARCH_DEBOUNCE);
    }
    let text = scan.search_query.trim().to_owned();
    if text.is_empty() && scan.search_filters.is_empty() {
        empty_state(
            ui,
            env.i18n,
            crate::icons::SEARCH,
            "tab-search",
            "search-empty",
            |_| {},
        );
        return;
    }
    let (tree, focus) = (&scan.tree, scan.focus);
    let all_tabs = scan.search_all_tabs && !others.is_empty();
    let key = Key {
        focus,
        text: text.clone(),
        filters: scan.search_filters.clone(),
        generation: scan.generation,
        others: all_tabs.then(|| {
            others
                .iter()
                .map(|o| (o.tree.root_path().to_path_buf(), o.generation))
                .collect()
        }),
    };
    let ids: &[Hit] = if settled {
        let filters = &scan.search_filters;
        scan.search.get(key, || {
            let other_trees: Vec<&Tree> = if all_tabs {
                others.iter().map(|o| &o.tree).collect()
            } else {
                Vec::new()
            };
            hits(tree, focus, &other_trees, &text, filters)
        })
    } else {
        &[]
    };
    crate::widgets::row(ui, env.i18n.dir(), |ui| {
        ui.weak(
            env.i18n
                .v(&env.i18n.count("search-results", ids.len() as u64)),
        );
        if !ids.is_empty() {
            ui.with_layout(env.i18n.dir().end_row(), |ui| {
                if ui
                    .button(format!(
                        "{}  {}",
                        crate::icons::EXPORT,
                        env.t("search-export")
                    ))
                    .clicked()
                {
                    env.push(Action::ExportSearch);
                }
                if ui
                    .button(format!("{}  {}", crate::icons::COPY, env.t("export-copy")))
                    .on_hover_text(env.t("search-copy-hint"))
                    .clicked()
                {
                    env.push(Action::CopySearch);
                }
            });
        }
    });
    table::show(
        ui,
        env,
        "search",
        &ENTRY_COLUMNS,
        &mut HitList {
            this: tree,
            others,
            hits: ids,
            selected: scan.selected,
        },
    );
}

/// Results below `focus` in `this` and anywhere in `others`, largest first.
fn hits(this: &Tree, focus: NodeId, others: &[&Tree], text: &str, filters: &Filters) -> Vec<Hit> {
    let mut hits: Vec<Hit> = run(this, focus, text, filters)
        .into_iter()
        .map(|id| Hit { tab: None, id })
        .collect();
    if others.is_empty() {
        return hits;
    }
    for (i, other) in others.iter().enumerate() {
        hits.extend(
            run(other, other.root(), text, filters)
                .into_iter()
                .map(|id| Hit { tab: Some(i), id }),
        );
    }
    let size = |h: &Hit| match h.tab {
        Some(i) => others[i].node(h.id).size,
        None => this.node(h.id).size,
    };
    hits.sort_by_key(|h| std::cmp::Reverse(size(h)));
    hits.truncate(SEARCH_LIMIT);
    hits
}

/// Runs the search below `focus`.
pub fn run(tree: &Tree, focus: NodeId, text: &str, filters: &Filters) -> Vec<NodeId> {
    let query = filters.query(text, crate::fmt::now_unix());
    let category = filters.category;
    let in_category = |tree: &Tree, id: NodeId| match category {
        Some(cat) => {
            tree.node(id).kind != Kind::Dir
                && Category::of_extension(&extension_of(tree.name_bytes(id))) == cat
        }
        None => true,
    };
    search::search(tree, focus, &query, in_category, SEARCH_LIMIT)
}

/// Drop-downs for every filter, wrapping onto more lines when narrow.
fn filters_row(ui: &mut Ui, env: &Env<'_>, scan: &mut ScanView) {
    let before = scan.search_filters.clone();
    let tree = &scan.tree;
    let owners: &[u32] = scan
        .all_owners
        .get(scan.generation, || super::tree_panel::known_owners(tree));
    let f = &mut scan.search_filters;
    let layout = env.i18n.dir().row().with_main_wrap(true);
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), layout, |ui| {
        let size_text = |bytes: Option<u64>, any: &str| match bytes {
            Some(b) => env.i18n.v(&env.size(b)),
            None => env.t(any),
        };

        ComboBox::from_id_salt("search-entries")
            .selected_text(match (f.category, f.entries) {
                (Some(cat), _) => env.t(cat.label_id()),
                (None, Entries::All) => env.t("filter-everything"),
                (None, Entries::Files) => env.t("filter-files"),
                (None, Entries::Folders) => env.t("filter-folders"),
            })
            .show_ui(ui, |ui| {
                for (entries, id) in [
                    (Entries::All, "filter-everything"),
                    (Entries::Files, "filter-files"),
                    (Entries::Folders, "filter-folders"),
                ] {
                    let on = f.category.is_none() && f.entries == entries;
                    if ui.selectable_label(on, env.t(id)).clicked() {
                        f.entries = entries;
                        f.category = None;
                    }
                }
                ui.separator();
                for cat in Category::ALL {
                    if ui
                        .selectable_label(f.category == Some(cat), env.t(cat.label_id()))
                        .clicked()
                    {
                        f.category = Some(cat);
                    }
                }
            });

        ComboBox::from_id_salt("search-min-size")
            .selected_text(format!(
                "{} {}",
                env.t("filter-at-least"),
                size_text(f.min_size, "filter-any-size")
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut f.min_size, None, env.t("filter-any-size"));
                for step in SIZE_STEPS {
                    ui.selectable_value(&mut f.min_size, Some(step), env.i18n.v(&env.size(step)));
                }
            });

        ComboBox::from_id_salt("search-max-size")
            .selected_text(format!(
                "{} {}",
                env.t("filter-at-most"),
                size_text(f.max_size, "filter-any-size")
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut f.max_size, None, env.t("filter-any-size"));
                for step in SIZE_STEPS {
                    ui.selectable_value(&mut f.max_size, Some(step), env.i18n.v(&env.size(step)));
                }
            });

        ComboBox::from_id_salt("search-modified")
            .selected_text(f.modified.label(env))
            .show_ui(ui, |ui| {
                for preset in Modified::PRESETS {
                    ui.selectable_value(&mut f.modified, preset, preset.label(env));
                }
            });

        if !owners.is_empty() {
            ComboBox::from_id_salt("search-owner")
                .selected_text(match f.owner {
                    Some(o) => env.i18n.v(&env.owner_name(o)),
                    None => env.t("filter-any-owner"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut f.owner, None, env.t("filter-any-owner"));
                    for &o in owners {
                        ui.selectable_value(&mut f.owner, Some(o), env.i18n.v(&env.owner_name(o)));
                    }
                });
        }

        ComboBox::from_id_salt("search-attribute")
            .selected_text(match f.attribute {
                Some(a) => env.t(a.label_id()),
                None => env.t("filter-any-attribute"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut f.attribute, None, env.t("filter-any-attribute"));
                for a in Attribute::ALL {
                    ui.selectable_value(&mut f.attribute, Some(a), env.t(a.label_id()));
                }
            });

        if !f.is_empty() && ui.button(env.t("filter-clear")).clicked() {
            *f = Filters::default();
        }
    });
    if scan.search_filters != before {
        scan.search_edited = None;
    }
}

/// Saved searches: run one, delete one, or save the current search.
fn saved_menu(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let label = format!("{}  {}", crate::icons::SAVED, env.t("search-saved"));
    ui.menu_button(label, |ui| {
        let saved = &env.settings.saved_searches;
        if saved.is_empty() {
            ui.weak(env.t("search-saved-none"));
        }
        for (index, s) in saved.iter().enumerate() {
            crate::widgets::row(ui, env.i18n.dir(), |ui| {
                if ui.button(env.i18n.v(&s.name)).clicked() {
                    scan.search_query = s.text.clone();
                    scan.search_filters = s.filters.clone();
                    scan.show_search_filters |= !s.filters.is_empty();
                    scan.search_edited = None;
                    ui.close();
                }
                if ui
                    .small_button(crate::icons::CLOSE)
                    .on_hover_text(env.t("search-saved-delete"))
                    .clicked()
                {
                    env.push(Action::DeleteSavedSearch(index));
                }
            });
        }
        ui.separator();
        let can_save = !scan.search_query.trim().is_empty() || !scan.search_filters.is_empty();
        crate::widgets::row(ui, env.i18n.dir(), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut scan.save_search_name)
                    .hint_text(env.t("search-saved-name"))
                    .desired_width(160.0),
            );
            let name = scan.save_search_name.trim().to_owned();
            if ui
                .add_enabled(
                    can_save && !name.is_empty(),
                    egui::Button::new(env.t("search-save")),
                )
                .clicked()
            {
                env.push(Action::SaveSearch(SavedSearch {
                    name,
                    text: scan.search_query.trim().to_owned(),
                    filters: scan.search_filters.clone(),
                }));
                scan.save_search_name.clear();
                ui.close();
            }
        });
    });
}

/// Shows typed RTL text in visual order. Reordering keeps the character
/// count, so cursor positions stay meaningful.
fn bidi_layouter(
    dir: Direction,
) -> impl FnMut(&Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley> {
    move |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
        let logical = text.as_str();
        let visual = bidi::visual(logical, dir);
        let shown = if visual.chars().count() == logical.chars().count() {
            visual
        } else {
            logical.to_owned()
        };
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.painter()
            .layout_no_wrap(shown, font, ui.visuals().text_color())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_other_scans_too_largest_first() {
        use storagemap_core::{EntryInfo, TreeBuilder};
        let tree = |root: &str, files: &[(&str, u64)]| {
            let dir = EntryInfo {
                kind: Kind::Dir,
                flags: Flags::empty(),
                modified: 0,
                accessed: 0,
                created: 0,
                owner: storagemap_core::NO_OWNER,
                size: 0,
                allocated: 0,
            };
            let mut b = TreeBuilder::new(PathBuf::from(root), dir);
            for &(name, size) in files {
                b.push(
                    NodeId::ROOT,
                    name.as_bytes(),
                    EntryInfo {
                        kind: Kind::File,
                        size,
                        ..dir
                    },
                );
            }
            b.finish()
        };
        let a = tree("/a", &[("x.iso", 10), ("note.txt", 1)]);
        let b = tree("/b", &[("y.iso", 30)]);
        let filters = Filters::default();
        let here = hits(&a, a.root(), &[], "*.iso", &filters);
        assert_eq!(
            here,
            [Hit {
                tab: None,
                id: NodeId(1)
            }]
        );
        let all = hits(&a, a.root(), &[&b], "*.iso", &filters);
        assert_eq!(
            all,
            [
                Hit {
                    tab: Some(0),
                    id: NodeId(1)
                },
                Hit {
                    tab: None,
                    id: NodeId(1)
                }
            ]
        );
    }

    #[test]
    fn relative_dates_become_bounds() {
        let now = 1_000 * DAY;
        assert_eq!(Modified::Any.bounds(now), (None, None));
        assert_eq!(Modified::Within(7).bounds(now), (Some(993 * DAY), None));
        assert_eq!(
            Modified::OlderThan(365).bounds(now),
            (None, Some(635 * DAY))
        );
    }

    #[test]
    fn a_category_limits_to_files() {
        let f = Filters {
            category: Some(Category::Video),
            ..Filters::default()
        };
        assert_eq!(f.query("", 0).entries, Entries::Files);
        assert!(!f.is_empty());
        assert!(Filters::default().is_empty());
    }

    #[test]
    fn saved_searches_round_trip() {
        let s = SavedSearch {
            name: "Old videos".into(),
            text: String::new(),
            filters: Filters {
                category: Some(Category::Video),
                modified: Modified::OlderThan(365),
                min_size: Some(MB),
                ..Filters::default()
            },
        };
        let json = serde_json::to_string(&s).expect("json");
        assert_eq!(
            serde_json::from_str::<SavedSearch>(&json).expect("parse"),
            s
        );
    }
}
