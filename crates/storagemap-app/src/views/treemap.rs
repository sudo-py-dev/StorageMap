//! Nested, squarified treemap with cushion shading.

use std::collections::BTreeSet;

use egui::{
    Align, Color32, FontId, Layout, Mesh, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2,
};
use storagemap_core::analysis::extension_of;
use storagemap_core::treemap::{self, squarify};
use storagemap_core::{Metric, NodeId, Tree};

use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::filetype::Category;
use crate::theme;
use crate::widgets::paint_text;
use std::time::Instant;

/// Hard cap on drawn tiles; keeps frames fast on million-file scans.
const MAX_TILES: usize = 30_000;
/// Tiles narrower or shorter than this are not drawn.
const MIN_SIDE: f32 = 3.0;
const PAD: f32 = 2.0;
const HEADER: f32 = 16.0;
const LEGEND_LINE: f32 = 24.0;
/// Rough width of one legend entry, to predict how many lines it wraps to.
const LEGEND_ENTRY_WIDTH: f32 = 160.0;
/// Seconds for the zoom transition when the focused folder changes.
const ZOOM_TIME: f32 = 0.32;
/// Relative inset the layout grows from when not zooming into a tile.
const SETTLE_SHRINK: f32 = 0.03;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    focus: NodeId,
    metric: Metric,
    generation: u64,
    size: [u32; 2],
    depth: u8,
    files: bool,
    rtl: bool,
}

#[derive(Debug, Clone, Copy)]
struct Tile {
    id: NodeId,
    rect: Rect,
    depth: u8,
    category: Option<Category>,
}

impl Tile {
    fn is_dir(&self) -> bool {
        self.category.is_none()
    }
}

#[derive(Default)]
pub struct Cache {
    key: Option<Key>,
    tiles: Vec<Tile>,
    /// Tile the user zoomed into, consumed by the next layout.
    zoom_from: Option<Rect>,
    /// Running transition: tiles grow out of this rect.
    anim: Option<(Rect, Instant)>,
}

impl Cache {
    pub fn invalidate(&mut self) {
        self.key = None;
    }
}

struct Params<'a> {
    tree: &'a Tree,
    metric: Metric,
    max_depth: u8,
    files: bool,
}

fn layout(p: &Params<'_>, id: NodeId, area: Rect, depth: u8, tiles: &mut Vec<Tile>) {
    let mut kids: Vec<(NodeId, u64)> = p
        .tree
        .children(id)
        .filter_map(|c| {
            let n = p.tree.node(c);
            let v = n.metric(p.metric);
            (v > 0 && (p.files || n.is_dir())).then_some((c, v))
        })
        .collect();
    kids.sort_unstable_by_key(|k| std::cmp::Reverse(k.1));
    let weights: Vec<f64> = kids.iter().map(|k| k.1 as f64).collect();
    let core_area = treemap::Rect {
        x: area.left(),
        y: area.top(),
        w: area.width(),
        h: area.height(),
    };
    for ((kid, _), r) in kids.iter().zip(squarify(&weights, core_area)) {
        let rect = Rect::from_min_size(pos2(r.x, r.y), vec2(r.w, r.h));
        if rect.width() < MIN_SIDE || rect.height() < MIN_SIDE {
            continue;
        }
        if tiles.len() >= MAX_TILES {
            return;
        }
        let node = p.tree.node(*kid);
        let category = (!node.is_dir())
            .then(|| Category::of_extension(&extension_of(p.tree.name_bytes(*kid))));
        tiles.push(Tile {
            id: *kid,
            rect,
            depth,
            category,
        });
        if node.is_dir()
            && depth + 1 < p.max_depth
            && let Some(inner) = inner_rect(rect)
        {
            layout(p, *kid, inner, depth + 1, tiles);
        }
    }
}

/// Space for a folder's children: below a header when there is room.
fn inner_rect(r: Rect) -> Option<Rect> {
    let min = 2.0 * PAD + MIN_SIDE;
    if r.width() >= 48.0 && r.height() >= HEADER + min + 10.0 {
        Some(Rect::from_min_max(
            r.min + vec2(PAD, HEADER),
            r.max - vec2(PAD, PAD),
        ))
    } else if r.width() > min && r.height() > min {
        Some(r.shrink(PAD))
    } else {
        None
    }
}

fn mirror(tiles: &mut [Tile], area: Rect) {
    for t in tiles {
        let (left, right) = (
            area.left() + area.right() - t.rect.right(),
            area.left() + area.right() - t.rect.left(),
        );
        t.rect = Rect::from_x_y_ranges(left..=right, t.rect.y_range());
    }
}

pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    // Reserve room for the legend: as many lines as last frame's categories need.
    let categories = scan
        .treemap
        .tiles
        .iter()
        .filter_map(|t| t.category)
        .collect::<BTreeSet<_>>()
        .len()
        .max(1);
    let lines = ((categories as f32 * LEGEND_ENTRY_WIDTH) / ui.available_width().max(1.0))
        .ceil()
        .max(1.0);
    let legend_height = lines * LEGEND_LINE + 4.0;
    let size = ui.available_size() - vec2(0.0, legend_height);
    let (rect, response) = ui.allocate_exact_size(size.max(vec2(10.0, 10.0)), Sense::click());
    let key = Key {
        focus: scan.focus,
        metric: env.settings.metric,
        generation: scan.generation,
        size: [rect.width() as u32, rect.height() as u32],
        depth: env.settings.treemap_depth,
        files: env.settings.treemap_files,
        rtl: env.i18n.is_rtl(),
    };
    if scan.treemap.key != Some(key) {
        let params = Params {
            tree: &scan.tree,
            metric: key.metric,
            max_depth: key.depth.max(1),
            files: key.files,
        };
        let mut tiles = Vec::new();
        layout(&params, scan.focus, rect.shrink(1.0), 0, &mut tiles);
        if key.rtl {
            mirror(&mut tiles, rect);
        }
        let focus_changed = scan.treemap.key.is_some_and(|k| k.focus != key.focus);
        let anim = match scan.treemap.zoom_from.take() {
            Some(from) => Some((from, Instant::now())),
            // Going up or jumping elsewhere: a gentle settle-in instead.
            None if focus_changed => {
                Some((rect.shrink2(rect.size() * SETTLE_SHRINK), Instant::now()))
            }
            None => None,
        };
        scan.treemap = Cache {
            key: Some(key),
            tiles,
            zoom_from: None,
            anim,
        };
    }
    let tiles = &scan.treemap.tiles;
    let tree = &scan.tree;
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals().clone();
    painter.rect_filled(rect, 4.0, visuals.extreme_bg_color);
    if tiles.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            env.t("empty-folder"),
            FontId::proportional(15.0),
            visuals.weak_text_color(),
        );
        return;
    }

    match scan.treemap.anim {
        Some((from, start)) if start.elapsed().as_secs_f32() < ZOOM_TIME => {
            let t = theme::ease_out(start.elapsed().as_secs_f32() / ZOOM_TIME);
            let moving: Vec<Tile> = tiles
                .iter()
                .map(|tile| Tile {
                    rect: zoom_rect(tile.rect, rect, from, t),
                    ..*tile
                })
                .collect();
            paint_tiles(&painter, env, tree, &moving, &visuals);
            ui.ctx().request_repaint();
        }
        _ => paint_tiles(&painter, env, tree, tiles, &visuals),
    }

    let hovered = response
        .hover_pos()
        .and_then(|p| tiles.iter().rev().find(|t| t.rect.contains(p)))
        .copied();
    if let Some(sel) = scan.selected.and_then(|s| tiles.iter().find(|t| t.id == s)) {
        painter.rect_stroke(
            sel.rect,
            0.0,
            Stroke::new(2.5, visuals.selection.stroke.color),
            StrokeKind::Inside,
        );
    }
    if let Some(t) = hovered {
        let outline = if visuals.dark_mode {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        painter.rect_stroke(t.rect, 0.0, Stroke::new(1.5, outline), StrokeKind::Inside);
    }

    if response.secondary_clicked() {
        scan.context_target = hovered.map(|t| t.id);
    }
    if let Some(t) = hovered {
        if response.double_clicked() {
            if tree.node(t.id).is_dir() {
                scan.treemap.zoom_from = Some(t.rect);
            }
            env.push(Action::Focus(t.id));
        } else if response.clicked() {
            env.push(Action::Select(t.id));
        }
    }
    let target = scan.context_target;
    response.context_menu(|ui| {
        if let Some(id) = target {
            node_menu::show(ui, env, tree, id);
        }
    });
    if let Some(t) = hovered {
        response.on_hover_ui_at_pointer(|ui| node_menu::tooltip(ui, env, tree, t.id));
    }

    legend(ui, env, tiles);
}

/// Where `r` (laid out in `area`) appears while zooming out of `from`:
/// at `t = 0` the whole layout is squeezed into `from`, at `t = 1` it is final.
fn zoom_rect(r: Rect, area: Rect, from: Rect, t: f32) -> Rect {
    let sx = from.width() / area.width().max(1.0);
    let sy = from.height() / area.height().max(1.0);
    let squeezed = Rect::from_min_size(
        egui::pos2(
            from.left() + (r.left() - area.left()) * sx,
            from.top() + (r.top() - area.top()) * sy,
        ),
        vec2(r.width() * sx, r.height() * sy),
    );
    Rect::from_min_max(squeezed.min.lerp(r.min, t), squeezed.max.lerp(r.max, t))
}

fn paint_tiles(
    painter: &egui::Painter,
    env: &Env<'_>,
    tree: &Tree,
    tiles: &[Tile],
    visuals: &egui::Visuals,
) {
    let dark = visuals.dark_mode;
    let border = if dark {
        Color32::from_black_alpha(160)
    } else {
        Color32::from_black_alpha(70)
    };

    for t in tiles.iter().filter(|t| t.is_dir()) {
        painter.rect_filled(t.rect, 2.0, folder_color(t.depth, dark));
        painter.rect_stroke(t.rect, 2.0, Stroke::new(1.0, border), StrokeKind::Inside);
    }

    let mut mesh = Mesh::default();
    for t in tiles.iter() {
        if let Some(cat) = t.category {
            cushion(&mut mesh, t.rect.shrink(0.5), cat.color());
        }
    }
    painter.add(Shape::mesh(mesh));

    if !env.settings.treemap_labels {
        return;
    }
    let small = FontId::proportional(11.5);
    for t in tiles {
        let (w, h) = (t.rect.width(), t.rect.height());
        if t.is_dir() && w >= 48.0 && h >= HEADER + 12.0 {
            let header = Rect::from_min_size(t.rect.min, vec2(w, HEADER)).shrink2(vec2(4.0, 0.0));
            let label = format!("{} · {}", tree.name(t.id), env.metric_text(tree.node(t.id)));
            paint_text(
                painter,
                env.i18n,
                header,
                &label,
                small.clone(),
                visuals.text_color(),
                false,
            );
        } else if let Some(cat) = t.category
            && w >= 56.0
            && h >= 18.0
        {
            let ink = contrast_ink(cat.color());
            let line = Rect::from_min_size(t.rect.min + vec2(3.0, 2.0), vec2(w - 6.0, 14.0));
            paint_text(
                painter,
                env.i18n,
                line,
                &tree.name(t.id),
                small.clone(),
                ink,
                false,
            );
            if h >= 34.0 {
                let line2 = line.translate(vec2(0.0, 14.0));
                paint_text(
                    painter,
                    env.i18n,
                    line2,
                    &env.metric_text(tree.node(t.id)),
                    small.clone(),
                    ink.gamma_multiply(0.8),
                    false,
                );
            }
        }
    }
}

/// Lit from the top-left: a cheap stand-in for van Wijk cushions.
fn cushion(mesh: &mut Mesh, r: Rect, base: Color32) {
    let light = base.lerp_to_gamma(Color32::WHITE, 0.30);
    let dark = base.lerp_to_gamma(Color32::BLACK, 0.35);
    let i = mesh.vertices.len() as u32;
    mesh.colored_vertex(r.left_top(), light);
    mesh.colored_vertex(r.right_top(), base);
    mesh.colored_vertex(r.right_bottom(), dark);
    mesh.colored_vertex(r.left_bottom(), base);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
}

fn folder_color(depth: u8, dark: bool) -> Color32 {
    let step = f32::from(depth.min(10));
    if dark {
        Color32::from_gray((38.0 + step * 7.0) as u8)
    } else {
        Color32::from_gray((236.0 - step * 9.0) as u8)
    }
}

/// Black or white, whichever reads better on `bg`.
fn contrast_ink(bg: Color32) -> Color32 {
    let lum = 0.299 * f32::from(bg.r()) + 0.587 * f32::from(bg.g()) + 0.114 * f32::from(bg.b());
    if lum > 150.0 {
        Color32::from_gray(20)
    } else {
        Color32::from_gray(245)
    }
}

fn legend(ui: &mut Ui, env: &Env<'_>, tiles: &[Tile]) {
    let present: BTreeSet<usize> = tiles
        .iter()
        .filter_map(|t| t.category)
        .filter_map(|c| Category::ALL.iter().position(|&x| x == c))
        .collect();
    let layout = match env.i18n.dir() {
        crate::i18n::Direction::Ltr => Layout::left_to_right(Align::Center),
        crate::i18n::Direction::Rtl => Layout::right_to_left(Align::Center),
    };
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), ui.available_height()),
        layout.with_main_wrap(true),
        |ui| {
            for cat in present.into_iter().map(|i| Category::ALL[i]) {
                let (swatch, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                ui.painter().rect_filled(swatch, 2.0, cat.color());
                ui.small(env.t(cat.label_id()));
                ui.add_space(6.0);
            }
        },
    );
}
