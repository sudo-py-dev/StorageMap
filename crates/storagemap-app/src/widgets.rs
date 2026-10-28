//! Direction-aware layout and text helpers on top of egui.
//!
//! egui is LTR-only, so mirroring is explicit: rows flow from the start
//! edge, text is elided/wrapped in *logical* order and only then reordered
//! for display — otherwise RTL text would lose its beginning, not its end.

use std::sync::Arc;

use egui::{
    Align, Align2, Color32, FontId, Galley, Layout, Painter, Pos2, Rect, Response, Ui, WidgetText,
};

use crate::i18n::{Direction, I18n};

const ELLIPSIS: char = '…';

impl Direction {
    /// Horizontal layout starting at the reading-start edge.
    pub fn row(self) -> Layout {
        match self {
            Direction::Ltr => Layout::left_to_right(Align::Center),
            Direction::Rtl => Layout::right_to_left(Align::Center),
        }
    }

    /// Horizontal layout starting at the reading-end edge (for trailing items).
    pub fn end_row(self) -> Layout {
        match self {
            Direction::Ltr => Layout::right_to_left(Align::Center),
            Direction::Rtl => Layout::left_to_right(Align::Center),
        }
    }

    pub fn start(self) -> Align {
        match self {
            Direction::Ltr => Align::Min,
            Direction::Rtl => Align::Max,
        }
    }

    /// Anchor for text placed at the start edge of a box.
    pub fn start_anchor(self) -> Align2 {
        match self {
            Direction::Ltr => Align2::LEFT_CENTER,
            Direction::Rtl => Align2::RIGHT_CENTER,
        }
    }

    pub fn end_anchor(self) -> Align2 {
        match self {
            Direction::Ltr => Align2::RIGHT_CENTER,
            Direction::Rtl => Align2::LEFT_CENTER,
        }
    }

    /// X coordinate `offset` points in from the start edge of `rect`.
    pub fn x_from_start(self, rect: Rect, offset: f32) -> f32 {
        match self {
            Direction::Ltr => rect.left() + offset,
            Direction::Rtl => rect.right() - offset,
        }
    }

    /// X coordinate `offset` points in from the end edge of `rect`.
    pub fn x_from_end(self, rect: Rect, offset: f32) -> f32 {
        match self {
            Direction::Ltr => rect.right() - offset,
            Direction::Rtl => rect.left() + offset,
        }
    }

    /// A horizontal span `[start, start + width]` measured from the start edge.
    pub fn span(self, rect: Rect, start: f32, width: f32) -> Rect {
        let (a, b) = (
            self.x_from_start(rect, start),
            self.x_from_start(rect, start + width),
        );
        Rect::from_x_y_ranges(a.min(b)..=a.max(b), rect.y_range())
    }
}

/// A single-height horizontal row flowing from the reading-start edge —
/// the direction-aware counterpart of `Ui::horizontal`.
pub fn row<R>(ui: &mut Ui, dir: Direction, add: impl FnOnce(&mut Ui) -> R) -> R {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, dir.row(), add).inner
}

/// Lays out `logical` on one line no wider than `max_width`, eliding the
/// *logical* end, then converts to visual order.
pub fn fit_text(
    painter: &Painter,
    i18n: &I18n,
    logical: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let full = painter.layout_no_wrap(i18n.v(logical), font.clone(), color);
    if full.size().x <= max_width || max_width <= 0.0 {
        return full;
    }
    let chars: Vec<char> = logical.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    let elided = |n: usize| -> String {
        chars[..n]
            .iter()
            .chain(std::iter::once(&ELLIPSIS))
            .collect()
    };
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let w = painter
            .layout_no_wrap(i18n.v(&elided(mid)), font.clone(), color)
            .size()
            .x;
        if w <= max_width {
            lo = mid
        } else {
            hi = mid - 1
        }
    }
    painter.layout_no_wrap(i18n.v(&elided(lo)), font, color)
}

/// Draws a single-line text at the start or end edge of `rect`, elided to fit.
pub fn paint_text(
    painter: &Painter,
    i18n: &I18n,
    rect: Rect,
    logical: &str,
    font: FontId,
    color: Color32,
    at_end: bool,
) {
    let galley = fit_text(painter, i18n, logical, font, color, rect.width());
    let dir = i18n.dir();
    let anchor = if at_end {
        dir.end_anchor()
    } else {
        dir.start_anchor()
    };
    let x = if at_end {
        dir.x_from_end(rect, 0.0)
    } else {
        dir.x_from_start(rect, 0.0)
    };
    let pos = anchor
        .anchor_size(Pos2::new(x, rect.center().y), galley.size())
        .min;
    painter.galley(pos, galley, color);
}

/// A one-line label filling the remaining row width, elided (in logical
/// order) instead of widening the layout — for paths and other long text.
pub fn elided_label(ui: &mut Ui, i18n: &I18n, logical: &str, color: Color32) -> Response {
    let height = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(0.0), height),
        egui::Sense::hover(),
    );
    paint_text(
        ui.painter(),
        i18n,
        rect,
        logical,
        egui::TextStyle::Body.resolve(ui.style()),
        color,
        false,
    );
    response.on_hover_text(i18n.v(logical))
}

/// A wrapped paragraph. Lines are broken in logical order, then each line
/// is reordered, so RTL paragraphs read top-to-bottom correctly.
pub fn paragraph(ui: &mut Ui, i18n: &I18n, logical: &str) -> Response {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let width = ui.available_width().max(40.0);
    let color = ui.visuals().text_color();
    let painter = ui.painter().clone();
    let measure = |s: &str| {
        painter
            .layout_no_wrap(i18n.v(s), font.clone(), color)
            .size()
            .x
    };

    let mut lines: Vec<String> = Vec::new();
    for para in logical.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&candidate) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    let text = lines
        .iter()
        .map(|l| i18n.v(l))
        .collect::<Vec<_>>()
        .join("\n");
    let job_align = match i18n.dir() {
        Direction::Ltr => Align::LEFT,
        Direction::Rtl => Align::RIGHT,
    };
    ui.with_layout(Layout::top_down(i18n.dir().start()), |ui| {
        let mut job = egui::text::LayoutJob::simple(text, font, color, f32::INFINITY);
        job.halign = job_align;
        ui.add(egui::Label::new(WidgetText::LayoutJob(job.into())).extend())
    })
    .inner
}

/// A filled share bar (0..=1) growing from the start edge.
pub fn share_bar(
    painter: &Painter,
    dir: Direction,
    rect: Rect,
    share: f64,
    fill: Color32,
    track: Color32,
) {
    painter.rect_filled(rect, 2.0, track);
    let w = rect.width() * share.clamp(0.0, 1.0) as f32;
    if w > 0.5 {
        painter.rect_filled(dir.span(rect, 0.0, w), 2.0, fill);
    }
}
