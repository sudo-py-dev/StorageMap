//! Chart tab: the focused folder's largest items as a donut or as bars.

use std::f32::consts::{FRAC_PI_2, TAU};

use egui::{Color32, FontId, Mesh, Pos2, Rect, Response, Sense, Ui, pos2, vec2};
use storagemap_core::{Metric, NodeId, Tree, ratio};

use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::widgets::{empty_state, paint_text};

/// Items shown on their own; the rest are grouped.
const MAX_SLICES: usize = 10;
const LEGEND_ROW: f32 = 26.0;
const BAR_ROW: f32 = 30.0;
const SWATCH: f32 = 12.0;
/// Donut hole as a fraction of the radius.
const HOLE: f32 = 0.55;
/// Pixels between mesh points along the rim.
const RIM_STEP: f32 = 3.0;

/// Distinct, colour-blind friendly hues; "everything else" is grey.
const PALETTE: [Color32; MAX_SLICES] = [
    Color32::from_rgb(37, 99, 235),
    Color32::from_rgb(230, 159, 0),
    Color32::from_rgb(0, 158, 115),
    Color32::from_rgb(204, 121, 167),
    Color32::from_rgb(86, 180, 233),
    Color32::from_rgb(213, 94, 0),
    Color32::from_rgb(120, 94, 240),
    Color32::from_rgb(160, 120, 70),
    Color32::from_rgb(0, 114, 178),
    Color32::from_rgb(170, 70, 120),
];
const OTHER: Color32 = Color32::from_rgb(140, 140, 150);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Style {
    #[default]
    Donut,
    Bars,
}

/// One slice: an item, or the grouped rest (`id` is `None`).
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub id: Option<NodeId>,
    pub value: u64,
    /// How many items the grouped rest stands for.
    pub grouped: usize,
}

impl Slice {
    fn color(&self, index: usize) -> Color32 {
        match self.id {
            Some(_) => PALETTE[index % PALETTE.len()],
            None => OTHER,
        }
    }
}

/// The largest children of `focus` for `metric`, plus one slice for the rest.
pub fn slices(tree: &Tree, focus: NodeId, metric: Metric) -> Vec<Slice> {
    let mut items: Vec<(NodeId, u64)> = tree
        .children(focus)
        .map(|c| (c, tree.node(c).metric(metric)))
        .filter(|&(_, v)| v > 0)
        .collect();
    items.sort_by_key(|&(_, v)| std::cmp::Reverse(v));
    let rest = items.split_off(items.len().min(MAX_SLICES));
    let mut out: Vec<Slice> = items
        .into_iter()
        .map(|(id, value)| Slice {
            id: Some(id),
            value,
            grouped: 0,
        })
        .collect();
    if !rest.is_empty() {
        out.push(Slice {
            id: None,
            value: rest.iter().map(|&(_, v)| v).sum(),
            grouped: rest.len(),
        });
    }
    out
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let metric = env.settings.metric;
    let (tree, focus) = (&scan.tree, scan.focus);
    let data = scan.chart.get((focus, metric, scan.generation), || {
        slices(tree, focus, metric)
    });
    let total: u64 = data.iter().map(|s| s.value).sum();

    crate::widgets::row(ui, env.i18n.dir(), |ui| {
        let current = env.settings.chart_style;
        for (style, icon, label) in [
            (Style::Donut, crate::icons::CHART_DONUT, "chart-donut"),
            (Style::Bars, crate::icons::CHART_BARS, "chart-bars"),
        ] {
            if ui
                .selectable_label(current == style, format!("{icon}  {}", env.t(label)))
                .clicked()
            {
                env.push(Action::SetChartStyle(style));
            }
        }
    });
    ui.add_space(8.0);
    if total == 0 {
        empty_state(
            ui,
            env.i18n,
            crate::icons::CHART_DONUT,
            "empty-folder",
            "empty-folder-hint",
            |_| {},
        );
        return;
    }
    let hovered = ui.scope_id().with("chart-hover");
    let mut hover: Option<usize> = ui.data(|d| d.get_temp(hovered));
    let mut now_hovered = None;
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| match env.settings.chart_style {
            Style::Donut => {
                let dir = env.i18n.dir();
                ui.with_layout(dir.row().with_cross_align(egui::Align::Min), |ui| {
                    let side = (ui.available_width() * 0.4).clamp(160.0, 360.0);
                    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
                    now_hovered = donut(ui, rect, &response, data, total, hover);
                    if let Some(i) = now_hovered {
                        item_response(env, tree, &data[i], &response);
                    }
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        for (i, slice) in data.iter().enumerate() {
                            let r = legend_row(ui, env, tree, slice, i, total, hover == Some(i));
                            if r.hovered() {
                                now_hovered = Some(i);
                            }
                            item_response(env, tree, slice, &r);
                        }
                    });
                });
            }
            Style::Bars => {
                let max = data.iter().map(|s| s.value).max().unwrap_or(1);
                for (i, slice) in data.iter().enumerate() {
                    let r = bar_row(ui, env, tree, slice, i, total, max, hover == Some(i));
                    if r.hovered() {
                        now_hovered = Some(i);
                    }
                    item_response(env, tree, slice, &r);
                }
            }
        });
    if now_hovered != hover {
        hover = now_hovered;
        ui.data_mut(|d| d.insert_temp(hovered, hover));
        ui.ctx().request_repaint();
    }
}

/// Click selects, double-click opens a folder, right-click shows the menu.
fn item_response(env: &mut Env<'_>, tree: &Tree, slice: &Slice, response: &Response) {
    let Some(id) = slice.id else { return };
    if response.double_clicked() && tree.node(id).is_dir() {
        env.push(Action::Focus(id));
    } else if response.clicked() {
        env.push(Action::Select(id));
    }
    response.context_menu(|ui| node_menu::show(ui, env, tree, id));
}

fn label(env: &Env<'_>, tree: &Tree, slice: &Slice) -> String {
    match slice.id {
        Some(id) => tree.name(id).into_owned(),
        None => env.i18n.count("chart-other", slice.grouped as u64),
    }
}

/// Draws the donut; returns the slice under the pointer.
fn donut(
    ui: &Ui,
    rect: Rect,
    response: &Response,
    data: &[Slice],
    total: u64,
    hover: Option<usize>,
) -> Option<usize> {
    let center = rect.center();
    let radius = rect.width() / 2.0 - 6.0;
    let mut start = -FRAC_PI_2;
    let mut under = None;
    let pointer = response.hover_pos();
    let mut mesh = Mesh::default();
    for (i, slice) in data.iter().enumerate() {
        let sweep = ratio(slice.value, total) as f32 * TAU;
        let end = start + sweep;
        let grow = if hover == Some(i) { 5.0 } else { 0.0 };
        ring_segment(
            &mut mesh,
            center,
            radius * HOLE,
            radius + grow,
            start,
            end,
            slice.color(i),
        );
        if let Some(p) = pointer {
            let d = p - center;
            let dist = d.length();
            // Angles measured from the top, clockwise, like the slices.
            let angle = (d.y.atan2(d.x) + FRAC_PI_2).rem_euclid(TAU) - FRAC_PI_2;
            if dist >= radius * HOLE && dist <= radius + 5.0 && angle >= start && angle < end {
                under = Some(i);
            }
        }
        start = end;
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    under
}

fn ring_segment(
    mesh: &mut Mesh,
    center: Pos2,
    inner: f32,
    outer: f32,
    start: f32,
    end: f32,
    color: Color32,
) {
    let steps = (((end - start) * outer) / RIM_STEP).ceil().max(1.0) as u32;
    let at = |r: f32, a: f32| pos2(center.x + r * a.cos(), center.y + r * a.sin());
    let base = mesh.vertices.len() as u32;
    for k in 0..=steps {
        let a = start + (end - start) * k as f32 / steps as f32;
        mesh.colored_vertex(at(inner, a), color);
        mesh.colored_vertex(at(outer, a), color);
    }
    for k in 0..steps {
        let i = base + 2 * k;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 3, i + 2);
    }
}

fn legend_row(
    ui: &mut Ui,
    env: &Env<'_>,
    tree: &Tree,
    slice: &Slice,
    index: usize,
    total: u64,
    hovered: bool,
) -> Response {
    let width = ui.available_width().min(520.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, LEGEND_ROW), Sense::click());
    let dir = env.i18n.dir();
    let visuals = ui.visuals();
    if hovered || response.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let swatch = dir.span(rect, 6.0, SWATCH);
    let swatch = Rect::from_center_size(swatch.center(), vec2(SWATCH, SWATCH));
    ui.painter().rect_filled(swatch, 3.0, slice.color(index));
    let font = FontId::proportional(13.5);
    let value_w = 150.0;
    let name_rect = dir.span(
        rect,
        6.0 + SWATCH + 10.0,
        rect.width() - value_w - SWATCH - 26.0,
    );
    paint_text(
        ui.painter(),
        env.i18n,
        name_rect,
        &label(env, tree, slice),
        font.clone(),
        visuals.text_color(),
        false,
    );
    let value = format!(
        "{}  {}",
        env.metric_value_text(slice.value),
        crate::fmt::percent(ratio(slice.value, total))
    );
    paint_text(
        ui.painter(),
        env.i18n,
        dir.span(rect, rect.width() - value_w - 6.0, value_w),
        &value,
        font,
        visuals.weak_text_color(),
        true,
    );
    response
}

#[allow(clippy::too_many_arguments)]
fn bar_row(
    ui: &mut Ui,
    env: &Env<'_>,
    tree: &Tree,
    slice: &Slice,
    index: usize,
    total: u64,
    max: u64,
    hovered: bool,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), BAR_ROW), Sense::click());
    let dir = env.i18n.dir();
    let visuals = ui.visuals();
    if hovered || response.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let name_w = (rect.width() * 0.28).clamp(120.0, 280.0);
    let value_w = 150.0;
    let font = FontId::proportional(13.5);
    paint_text(
        ui.painter(),
        env.i18n,
        dir.span(rect, 6.0, name_w - 12.0),
        &label(env, tree, slice),
        font.clone(),
        visuals.text_color(),
        false,
    );
    let track_w = (rect.width() - name_w - value_w - 12.0).max(20.0);
    let track = dir.span(rect.shrink2(vec2(0.0, 7.0)), name_w, track_w);
    ui.painter()
        .rect_filled(track, 4.0, visuals.extreme_bg_color);
    let filled = dir.span(track, 0.0, track_w * ratio(slice.value, max) as f32);
    ui.painter().rect_filled(filled, 4.0, slice.color(index));
    let value = format!(
        "{}  {}",
        env.metric_value_text(slice.value),
        crate::fmt::percent(ratio(slice.value, total))
    );
    paint_text(
        ui.painter(),
        env.i18n,
        dir.span(rect, rect.width() - value_w - 6.0, value_w),
        &value,
        font,
        visuals.weak_text_color(),
        true,
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use storagemap_core::{EntryInfo, Flags, Kind, TreeBuilder};

    #[test]
    fn groups_everything_past_the_largest_items() {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            size: 0,
            allocated: 0,
        };
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir);
        for i in 0..14u64 {
            let name = format!("f{i}");
            b.push(
                NodeId(0),
                name.as_bytes(),
                EntryInfo {
                    kind: Kind::File,
                    size: (i + 1) * 10,
                    ..dir
                },
            );
        }
        b.push(
            NodeId(0),
            b"empty",
            EntryInfo {
                kind: Kind::File,
                ..dir
            },
        );
        let t = b.finish();
        let s = slices(&t, t.root(), Metric::Size);
        assert_eq!(s.len(), MAX_SLICES + 1);
        assert_eq!(s[0].value, 140);
        let rest = s.last().expect("rest");
        assert_eq!((rest.id, rest.grouped), (None, 4));
        assert_eq!(rest.value, 10 + 20 + 30 + 40);
    }
}
