//! A virtualised, direction-aware data table shared by the list views.

use egui::{FontId, Response, RichText, Sense, Ui, vec2};
use egui_extras::{Column, TableBuilder};

use super::Env;
use crate::widgets::{paint_text, share_bar};

pub const ROW_HEIGHT: f32 = 22.0;
const HEADER_HEIGHT: f32 = 24.0;
/// Space kept for the flexible (remainder) column before others are hidden.
const FLEX_MIN_WIDTH: f32 = 160.0;

/// Column description in *logical* order; RTL reverses the display order.
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

/// Logical indices of the columns that fit in `available`, in logical order.
fn visible_columns(cols: &[Col], available: f32, spacing: f32) -> Vec<usize> {
    let mut by_priority: Vec<usize> = (0..cols.len()).collect();
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

pub fn show(
    ui: &mut Ui,
    env: &mut Env<'_>,
    salt: &str,
    cols: &[Col],
    source: &mut dyn TableSource,
) {
    let spacing = ui.spacing().item_spacing.x;
    let mut order = visible_columns(cols, ui.available_width(), spacing);
    if env.i18n.is_rtl() {
        order.reverse();
    }
    let mut builder = TableBuilder::new(ui)
        .id_salt(salt)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(env.i18n.dir().row())
        .auto_shrink(false);
    for &c in &order {
        builder = builder.column(match cols[c].width {
            Some(w) => Column::initial(w).at_least(36.0).clip(true),
            None => Column::remainder().at_least(120.0).clip(true),
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
                    if ui
                        .add(egui::Label::new(RichText::new(title).strong()).sense(Sense::click()))
                        .clicked()
                    {
                        source.header_clicked(c);
                    }
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
        assert_eq!(visible_columns(&COLS, 1000.0, 0.0), [0, 1, 2, 3]);
        assert_eq!(visible_columns(&COLS, 400.0, 0.0), [0, 1, 3]);
        assert_eq!(visible_columns(&COLS, 100.0, 0.0), [0]);
    }
}
