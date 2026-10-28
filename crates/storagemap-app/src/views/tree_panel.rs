//! The folder tree: a virtualised list of expanded folders, sorted by the
//! active metric, painted manually so it mirrors cleanly in RTL.

use std::collections::HashSet;

use egui::{FontId, Rect, Role, Sense, Ui, Vec2, WidgetInfo};
use storagemap_core::{Flags, Metric, NodeId, Tree};

use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::widgets::{paint_text, row, share_bar};

const ROW_HEIGHT: f32 = 22.0;
const INDENT: f32 = 14.0;
const ARROW_WIDTH: f32 = 16.0;
const BAR_WIDTH: f32 = 44.0;
const VALUE_WIDTH: f32 = 78.0;
const GAP: f32 = 6.0;

/// One visible folder row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub id: NodeId,
    pub depth: u16,
    pub has_children: bool,
    pub expanded: bool,
}

/// Flattens the expanded part of the folder hierarchy, largest first.
pub fn build_rows(tree: &Tree, expanded: &HashSet<NodeId>, metric: Metric) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut stack = vec![(tree.root(), 0u16)];
    while let Some((id, depth)) = stack.pop() {
        let mut dirs: Vec<NodeId> = tree
            .children(id)
            .filter(|&c| tree.node(c).is_dir())
            .collect();
        let is_open = expanded.contains(&id);
        rows.push(Row {
            id,
            depth,
            has_children: !dirs.is_empty(),
            expanded: is_open,
        });
        if is_open {
            if metric != Metric::Size {
                dirs.sort_by_key(|&d| std::cmp::Reverse(tree.node(d).metric(metric)));
            }
            stack.extend(dirs.into_iter().rev().map(|d| (d, depth.saturating_add(1))));
        }
    }
    rows
}

/// Draws the panel; returns nothing — intents go to `env`.
pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let metric = env.settings.metric;
    let key = (metric, scan.generation, scan.expansion_version);
    let rows = scan
        .tree_rows
        .get(key, || build_rows(&scan.tree, &scan.expanded, metric));
    let tree = &scan.tree;

    let mut collapse = false;
    let root = tree.node(tree.root());
    row(ui, env.i18n.dir(), |ui| {
        ui.label(
            egui::RichText::new(env.i18n.v(&env.metric_text(root)))
                .strong()
                .size(17.0),
        );
        ui.with_layout(env.i18n.dir().end_row(), |ui| {
            collapse = ui.small_button(env.t("action-collapse-all")).clicked();
        });
    });
    // Counts get their own line, elided rather than colliding with the button.
    let counts = format!(
        "{} · {}",
        env.i18n.count("files-count", root.files),
        env.i18n.count("folders-count", root.dirs)
    );
    let weak = ui.visuals().weak_text_color();
    crate::widgets::elided_label(ui, env.i18n, &counts, weak);
    ui.separator();

    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("folder-tree")
        .auto_shrink(false);
    if let Some(target) = scan.scroll_tree_to.take()
        && let Some(index) = rows.iter().position(|r| r.id == target)
    {
        let (offset, viewport) = scan.tree_scroll;
        let y = index as f32 * ROW_HEIGHT;
        if y < offset || y + ROW_HEIGHT > offset + viewport {
            scroll = scroll.vertical_scroll_offset((y - viewport / 2.0).max(0.0));
        }
    }

    let mut toggle = None;
    let output = scroll.show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for row in &rows[range] {
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
            if let Some(id) = paint_row(ui, env, tree, row, rect, scan.selected == Some(row.id)) {
                toggle = Some(id);
            }
        }
    });
    scan.tree_scroll = (output.state.clamped_offset().y, output.inner_rect.height());
    if let Some(id) = toggle {
        scan.toggle(id);
    }
    if collapse {
        scan.collapse_all();
    }
}

/// A disclosure triangle that rotates smoothly between closed (pointing to
/// the reading end) and open (pointing down). Drawn as a polygon because
/// the font glyphs render as colour emoji.
fn disclosure_triangle(
    painter: &egui::Painter,
    c: egui::Pos2,
    open: f32,
    rtl: bool,
    color: egui::Color32,
) {
    const R: f32 = 4.2;
    // Closed shape points towards +x; rotate by 90° when open, mirrored in RTL.
    let base = [
        egui::vec2(-R * 0.6, -R),
        egui::vec2(R * 0.7, 0.0),
        egui::vec2(-R * 0.6, R),
    ];
    let angle = open * std::f32::consts::FRAC_PI_2;
    let (sin, cos) = angle.sin_cos();
    let points = base
        .iter()
        .map(|v| {
            let v = if rtl { egui::vec2(-v.x, v.y) } else { *v };
            let (s, k) = if rtl { (-sin, cos) } else { (sin, cos) };
            c + egui::vec2(v.x * k - v.y * s, v.x * s + v.y * k)
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

/// Paints one row and handles its input; returns a folder to toggle.
fn paint_row(
    ui: &mut Ui,
    env: &mut Env<'_>,
    tree: &Tree,
    row: &Row,
    rect: Rect,
    selected: bool,
) -> Option<NodeId> {
    let dir = env.i18n.dir();
    let node = tree.node(row.id);
    let name = tree.name(row.id).into_owned();
    let response = ui.interact(
        rect,
        ui.scope_id().with(("tree-row", row.id)),
        Sense::click(),
    );
    response.widget_info(|| WidgetInfo::selected(Role::TreeItem, true, selected, &name));

    let visuals = ui.visuals();
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 3.0, visuals.selection.bg_fill);
    } else if response.hovered() {
        painter.rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let text_color = if selected {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };
    let weak = visuals.weak_text_color();
    let font = FontId::proportional(13.5);

    let indent = f32::from(row.depth) * INDENT;
    let arrow_rect = dir.span(rect, indent, ARROW_WIDTH);
    if row.has_children {
        let open = ui.ctx().animate_bool_with_time(
            response.id.with("open"),
            row.expanded,
            crate::theme::ANIMATION_TIME,
        );
        disclosure_triangle(&painter, arrow_rect.center(), open, env.i18n.is_rtl(), weak);
    }
    let bar_rect = dir.span(
        rect.shrink2(egui::vec2(0.0, 7.0)),
        indent + ARROW_WIDTH,
        BAR_WIDTH,
    );
    let share = tree.share_of_parent(row.id, env.settings.metric);
    share_bar(
        &painter,
        dir,
        bar_rect,
        share,
        visuals.selection.bg_fill.gamma_multiply(1.6),
        visuals.extreme_bg_color,
    );

    let value_rect = dir.span(rect, rect.width() - VALUE_WIDTH - GAP, VALUE_WIDTH);
    paint_text(
        &painter,
        env.i18n,
        value_rect,
        &env.metric_text(node),
        font.clone(),
        weak,
        true,
    );

    let name_start = indent + ARROW_WIDTH + BAR_WIDTH + GAP;
    let name_width = (rect.width() - name_start - VALUE_WIDTH - 2.0 * GAP).max(0.0);
    let flagged = node.flags.contains(Flags::ERROR)
        || node.flags.contains(Flags::INCOMPLETE)
        || node.flags.contains(Flags::MOUNT_POINT);
    let name_color = if flagged { weak } else { text_color };
    paint_text(
        &painter,
        env.i18n,
        dir.span(rect, name_start, name_width),
        &name,
        font,
        name_color,
        false,
    );

    let mut toggle = None;
    if response.clicked() {
        let on_arrow = response
            .interact_pointer_pos()
            .is_some_and(|p| arrow_rect.contains(p));
        if on_arrow && row.has_children {
            toggle = Some(row.id);
        } else {
            env.push(Action::Focus(row.id));
        }
    }
    if response.double_clicked() && row.has_children {
        toggle = Some(row.id);
    }
    response.context_menu(|ui| node_menu::show(ui, env, tree, row.id));
    response.on_hover_ui(|ui| node_menu::tooltip(ui, env, tree, row.id));
    toggle
}
