//! A virtualised, direction-aware data table shared by the list views.

use egui::{FontId, Response, RichText, Sense, Ui, vec2};
use egui_extras::{Column, TableBuilder};

use super::Env;
use crate::app::Action;
use crate::widgets::{paint_text, share_bar};

pub const ROW_HEIGHT: f32 = 22.0;
const HEADER_HEIGHT: f32 = 24.0;
/// Space kept for the flexible (remainder) column before others are hidden.
const FLEX_MIN_WIDTH: f32 = 160.0;

/// Column description in *logical* order; RTL reverses the display order.
/// `title_id` doubles as the column's stable id in the user's preferences.
#[derive(Debug, Clone, Copy)]
pub struct Col {
    pub title_id: &'static str,
    /// Initial width; `None` takes the remaining space.
    pub width: Option<f32>,
    /// Lower shows first; columns that do not fit are hidden, highest first.
    pub priority: u8,
}

impl Col {
    /// The flexible column (always shown).
    pub const fn flex(title_id: &'static str) -> Col {
        Col {
            title_id,
            width: None,
            priority: 0,
        }
    }
    pub const fn fixed(title_id: &'static str, width: f32, priority: u8) -> Col {
        Col {
            title_id,
            width: Some(width),
            priority,
        }
    }
}

/// Logical indices of the columns that are not `hidden` and fit in
/// `available`, in logical order. The flexible column is always shown.
fn visible_columns(
    cols: &[Col],
    available: f32,
    spacing: f32,
    hidden: impl Fn(&Col) -> bool,
) -> Vec<usize> {
    let mut by_priority: Vec<usize> = (0..cols.len())
        .filter(|&i| cols[i].width.is_none() || !hidden(&cols[i]))
        .collect();
    by_priority.sort_by_key(|&i| cols[i].priority);
    let mut budget = available - FLEX_MIN_WIDTH;
    let mut shown: Vec<usize> = by_priority
        .into_iter()
        .filter(|&i| match cols[i].width {
            None => true,
            Some(w) if w + spacing <= budget => {
                budget -= w + spacing;
                true
            }
            Some(_) => false,
        })
        .collect();
    shown.sort_unstable();
    shown
}

/// Callbacks a table needs from its view.
pub trait TableSource {
    fn rows(&self) -> usize;
    fn is_selected(&self, row: usize) -> bool;
    /// Paints `col` of `row`; `cell` is the full cell rect.
    fn cell(&mut self, ui: &mut Ui, env: &mut Env<'_>, row: usize, col: usize);
    /// Called with the row's response (click, double-click, context menu).
    fn row_response(&mut self, env: &mut Env<'_>, row: usize, response: &Response);
    /// Header clicked (for sorting); `sorted` reports the current sort.
    fn header_clicked(&mut self, _col: usize) {}
    fn sorted(&self, _col: usize) -> Option<bool> {
        None
    }
}

/// Draws the table. `salt` identifies it in the user's column preferences.
pub fn show(
    ui: &mut Ui,
    env: &mut Env<'_>,
    salt: &'static str,
    cols: &[Col],
    source: &mut dyn TableSource,
) {
    let spacing = ui.spacing().item_spacing.x;
    let settings = env.settings;
    let mut order = visible_columns(cols, ui.available_width(), spacing, |c| {
        settings.column_hidden(salt, c.title_id)
    });
    let ranks = arranged(cols, settings.column_order.get(salt));
    order.sort_by_key(|&c| ranks.iter().position(|&r| r == c));
    if env.i18n.is_rtl() {
        order.reverse();
    }
    let mut builder = TableBuilder::new(ui)
        // Widths are remembered by position, so each arrangement keeps its own.
        .id_salt((salt, &order))
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(env.i18n.dir().row())
        .auto_shrink(false);
    for &c in &order {
        builder = builder.column(match cols[c].width {
            Some(w) => Column::initial(w).at_least(36.0).clip(true),
            // Not resizable, so it always takes the space the others leave.
            None => Column::remainder()
                .at_least(120.0)
                .clip(true)
                .resizable(false),
        });
    }
    builder
        .header(HEADER_HEIGHT, |mut header| {
            for &c in &order {
                header.col(|ui| {
                    let arrow = match source.sorted(c) {
                        Some(true) => " ▼",
                        Some(false) => " ▲",
                        None => "",
                    };
                    let title = format!("{}{arrow}", env.t(cols[c].title_id));
                    let response = ui
                        .add(egui::Label::new(RichText::new(title).strong()).sense(Sense::click()));
                    if response.clicked() {
                        source.header_clicked(c);
                    }
                    response.context_menu(|ui| column_menu(ui, env, salt, cols));
                });
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, source.rows(), |mut row| {
                let index = row.index();
                row.set_selected(source.is_selected(index));
                for &c in &order {
                    row.col(|ui| source.cell(ui, env, index, c));
                }
                let response = row.response();
                source.row_response(env, index, &response);
            });
        });
}

/// Logical column indices in the user's order: the columns `saved` names
/// first, in that order, then the rest in their default order.
fn arranged(cols: &[Col], saved: Option<&Vec<String>>) -> Vec<usize> {
    let mut order: Vec<usize> = saved
        .into_iter()
        .flatten()
        .filter_map(|id| cols.iter().position(|c| c.title_id == id))
        .collect();
    order.dedup();
    for i in 0..cols.len() {
        if !order.contains(&i) {
            order.push(i);
        }
    }
    order
}

/// Every column with a checkbox to show it (the flexible one always shows)
/// and buttons to move it towards the start or end of the row.
fn column_menu(ui: &mut Ui, env: &mut Env<'_>, salt: &'static str, cols: &[Col]) {
    ui.label(RichText::new(env.t("columns-menu")).weak());
    let order = arranged(cols, env.settings.column_order.get(salt));
    let dir = env.i18n.dir();
    let (earlier, later) = if env.i18n.is_rtl() {
        (crate::icons::CARET_RIGHT, crate::icons::CARET_LEFT)
    } else {
        (crate::icons::CARET_LEFT, crate::icons::CARET_RIGHT)
    };
    for (pos, &c) in order.iter().enumerate() {
        let col = cols[c];
        crate::widgets::row(ui, dir, |ui| {
            let mut moved = None;
            let button = |ui: &mut Ui, icon: &str, enabled: bool, tip: String| {
                ui.add_enabled(enabled, egui::Button::new(icon).small())
                    .on_hover_text(tip)
                    .clicked()
            };
            if button(ui, earlier, pos > 0, env.t("column-earlier")) {
                moved = Some(pos - 1);
            }
            if button(ui, later, pos + 1 < order.len(), env.t("column-later")) {
                moved = Some(pos + 1);
            }
            if col.width.is_some() {
                let mut shown = !env.settings.column_hidden(salt, col.title_id);
                if ui.checkbox(&mut shown, env.t(col.title_id)).changed() {
                    env.push(Action::ToggleColumn(salt, col.title_id));
                }
            } else {
                ui.add_enabled(false, egui::Checkbox::new(&mut true, env.t(col.title_id)));
            }
            if let Some(to) = moved {
                let mut new = order.clone();
                new.swap(pos, to);
                env.push(Action::SetColumnOrder(
                    salt,
                    new.into_iter().map(|i| cols[i].title_id).collect(),
                ));
            }
        });
    }
    ui.separator();
    if ui.button(env.t("columns-reset")).clicked() {
        env.push(Action::ResetColumns(salt));
        ui.close();
    }
}

/// Single-line text filling the cell, elided in logical order.
pub fn text(ui: &mut Ui, env: &Env<'_>, logical: &str, numeric: bool) {
    let color = ui.visuals().text_color();
    colored(ui, env, logical, numeric, color);
}

/// Like [`text`] in a given colour.
pub fn colored(ui: &mut Ui, env: &Env<'_>, logical: &str, numeric: bool, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    paint_text(
        ui.painter(),
        env.i18n,
        rect,
        logical,
        FontId::proportional(13.5),
        color,
        numeric,
    );
}

/// A share bar with its percentage.
pub fn share(ui: &mut Ui, env: &Env<'_>, value: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let dir = env.i18n.dir();
    let bar_w = (rect.width() - 52.0).max(10.0);
    let bar = dir.span(rect.shrink2(vec2(0.0, 6.0)), 0.0, bar_w);
    let visuals = ui.visuals();
    share_bar(
        ui.painter(),
        dir,
        bar,
        value,
        visuals.selection.bg_fill.gamma_multiply(1.6),
        visuals.extreme_bg_color,
    );
    let label = dir.span(rect, bar_w + 4.0, rect.width() - bar_w - 4.0);
    paint_text(
        ui.painter(),
        env.i18n,
        label,
        &crate::fmt::percent(value),
        FontId::proportional(12.5),
        visuals.weak_text_color(),
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLS: [Col; 4] = [
        Col::flex("a"),
        Col::fixed("b", 100.0, 1),
        Col::fixed("c", 100.0, 3),
        Col::fixed("d", 100.0, 2),
    ];

    #[test]
    fn hides_lowest_priority_columns_first() {
        let none = |_: &Col| false;
        assert_eq!(visible_columns(&COLS, 1000.0, 0.0, none), [0, 1, 2, 3]);
        assert_eq!(visible_columns(&COLS, 400.0, 0.0, none), [0, 1, 3]);
        assert_eq!(visible_columns(&COLS, 100.0, 0.0, none), [0]);
    }

    #[test]
    fn saved_order_comes_first_then_the_defaults() {
        let saved = vec!["d".to_owned(), "gone".to_owned(), "b".to_owned()];
        assert_eq!(arranged(&COLS, Some(&saved)), [3, 1, 0, 2]);
        assert_eq!(arranged(&COLS, None), [0, 1, 2, 3]);
    }

    #[test]
    fn hidden_columns_are_left_out_but_never_the_flexible_one() {
        let hide = |c: &Col| matches!(c.title_id, "a" | "b");
        assert_eq!(visible_columns(&COLS, 1000.0, 0.0, hide), [0, 2, 3]);
    }
}
