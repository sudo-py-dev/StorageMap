//! The folder tree: a virtualised list of expanded folders, sorted by the
//! active metric, painted manually so it mirrors cleanly in RTL.

use std::collections::HashSet;

use egui::{FontId, Rect, Role, Sense, Ui, Vec2, WidgetInfo};
use storagemap_core::analysis::{AgeBucket, extension_of};
use storagemap_core::{Flags, Kind, Metric, NodeId, Tree};

use super::{Env, ScanView, node_menu};
use crate::app::Action;
use crate::filetype::Category;
use crate::widgets::{paint_text, row, share_bar};

const ROW_HEIGHT: f32 = 22.0;
const INDENT: f32 = 14.0;
const ARROW_WIDTH: f32 = 16.0;
const BAR_WIDTH: f32 = 44.0;
const VALUE_WIDTH: f32 = 78.0;
const GAP: f32 = 6.0;

/// Which files the folder tree counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeFilter {
    #[default]
    All,
    Type(Category),
    Age(AgeBucket),
}

impl TreeFilter {
    fn matches(self, tree: &Tree, id: NodeId, now: i64) -> bool {
        match self {
            TreeFilter::All => true,
            TreeFilter::Type(cat) => {
                Category::of_extension(&extension_of(tree.name_bytes(id))) == cat
            }
            TreeFilter::Age(bucket) => {
                AgeBucket::of(now.saturating_sub(tree.node(id).modified).max(0)) == bucket
            }
        }
    }

    fn label(self, env: &Env<'_>) -> String {
        match self {
            TreeFilter::All => env.t("tree-filter-all"),
            TreeFilter::Type(cat) => env.t(cat.label_id()),
            TreeFilter::Age(bucket) => env.t(super::lists::age_label_id(bucket)),
        }
    }
}

/// Per-node totals of the files `filter` keeps, indexed by node.
pub fn filtered_values(tree: &Tree, filter: TreeFilter, metric: Metric, now: i64) -> Vec<u64> {
    let mut values = vec![0u64; tree.len()];
    let order: Vec<NodeId> = tree.walk(tree.root()).collect();
    // Pre-order: walking it backwards finishes children before parents.
    for &id in order.iter().rev() {
        let node = tree.node(id);
        if node.kind != Kind::Dir
            && !node.flags.contains(Flags::HARDLINK_DUP)
            && filter.matches(tree, id, now)
        {
            values[id.index()] = node.metric(metric);
        }
        if let Some(parent) = tree.parent(id) {
            values[parent.index()] += values[id.index()];
        }
    }
    values
}

/// One visible folder row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub id: NodeId,
    pub depth: u16,
    pub has_children: bool,
    pub expanded: bool,
}

/// Flattens the expanded part of the folder hierarchy, largest first.
/// With `values`, folders holding nothing the filter keeps are left out and
/// the rest are sorted by the filtered value.
pub fn build_rows(
    tree: &Tree,
    expanded: &HashSet<NodeId>,
    metric: Metric,
    values: Option<&[u64]>,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut stack = vec![(tree.root(), 0u16)];
    while let Some((id, depth)) = stack.pop() {
        let mut dirs: Vec<NodeId> = tree
            .children(id)
            .filter(|&c| tree.node(c).is_dir())
            .filter(|&c| values.is_none_or(|v| v[c.index()] > 0))
            .collect();
        let is_open = expanded.contains(&id);
        rows.push(Row {
            id,
            depth,
            has_children: !dirs.is_empty(),
            expanded: is_open,
        });
        if is_open {
            if let Some(v) = values {
                dirs.sort_by_key(|&d| std::cmp::Reverse(v[d.index()]));
            } else if metric != Metric::Size {
                dirs.sort_by_key(|&d| std::cmp::Reverse(tree.node(d).metric(metric)));
            }
            stack.extend(dirs.into_iter().rev().map(|d| (d, depth.saturating_add(1))));
        }
    }
    rows
}

/// Brings the cached rows (and filtered values, when a filter is active)
/// up to date; read them with [`rows`] and [`values`].
pub fn refresh(scan: &mut ScanView, metric: Metric) {
    let filter = scan.tree_filter;
    let tree = &scan.tree;
    let values: Option<&[u64]> = (filter != TreeFilter::All).then(|| {
        let now = crate::fmt::now_unix();
        scan.filtered
            .get((filter, metric, scan.generation), || {
                filtered_values(tree, filter, metric, now)
            })
            .as_slice()
    });
    let key = (metric, scan.generation, scan.expansion_version, filter);
    scan.tree_rows
        .get(key, || build_rows(tree, &scan.expanded, metric, values));
}

/// The visible rows as of the last [`refresh`].
pub fn rows(scan: &ScanView) -> &[Row] {
    scan.tree_rows.value()
}

/// Draws the panel; returns nothing — intents go to `env`.
pub fn show(ui: &mut Ui, env: &mut Env<'_>, scan: &mut ScanView) {
    let filter = scan.tree_filter;
    refresh(scan, env.settings.metric);
    let rows = &scan.tree_rows.value()[..];
    let values = (filter != TreeFilter::All).then(|| scan.filtered.value().as_slice());
    let tree = &scan.tree;

    let mut collapse = false;
    let mut new_filter = None;
    let root = tree.node(tree.root());
    let root_text = match values {
        Some(v) => env.metric_value_text(v[tree.root().index()]),
        None => env.metric_text(root),
    };
    row(ui, env.i18n.dir(), |ui| {
        ui.label(
            egui::RichText::new(env.i18n.v(&root_text))
                .strong()
                .size(17.0),
        );
        ui.with_layout(env.i18n.dir().end_row(), |ui| {
            collapse = ui.small_button(env.t("action-collapse-all")).clicked();
            new_filter = filter_menu(ui, env, filter);
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
            let selected = scan.selected == Some(row.id);
            if let Some(id) = paint_row(ui, env, tree, values, row, rect, selected) {
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
    if let Some(f) = new_filter {
        scan.tree_filter = f;
    }
}

/// A drop-down choosing which files the tree counts; returns a new choice.
fn filter_menu(ui: &mut Ui, env: &Env<'_>, current: TreeFilter) -> Option<TreeFilter> {
    let mut chosen = None;
    let text = format!("{}  {}", crate::icons::FILTER, current.label(env));
    egui::ComboBox::from_id_salt("tree-filter")
        .selected_text(egui::RichText::new(text).small())
        .width(130.0)
        .truncate()
        .height(480.0)
        .show_ui(ui, |ui| {
            let mut option = |ui: &mut Ui, f: TreeFilter| {
                if ui.selectable_label(current == f, f.label(env)).clicked() {
                    chosen = Some(f);
                }
            };
            option(ui, TreeFilter::All);
            ui.separator();
            ui.label(egui::RichText::new(env.t("tree-filter-types")).weak());
            for cat in Category::ALL {
                option(ui, TreeFilter::Type(cat));
            }
            ui.separator();
            ui.label(egui::RichText::new(env.t("tree-filter-modified")).weak());
            for bucket in AgeBucket::ALL {
                option(ui, TreeFilter::Age(bucket));
            }
        });
    chosen.filter(|&f| f != current)
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
    values: Option<&[u64]>,
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
    let share = match (values, tree.parent(row.id)) {
        (Some(v), Some(p)) => storagemap_core::ratio(v[row.id.index()], v[p.index()]),
        (Some(_), None) => 1.0,
        (None, _) => tree.share_of_parent(row.id, env.settings.metric),
    };
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
        &match values {
            Some(v) => env.metric_value_text(v[row.id.index()]),
            None => env.metric_text(node),
        },
        font.clone(),
        weak,
        true,
    );

    let name_start = indent + ARROW_WIDTH + BAR_WIDTH + GAP;
    let name_width = (rect.width() - name_start - VALUE_WIDTH - 2.0 * GAP).max(0.0);
    let flagged = node.flags.contains(Flags::ERROR)
        || node.flags.contains(Flags::INCOMPLETE)
        || node.flags.contains(Flags::MOUNT_POINT);
    let name_color = if flagged {
        weak
    } else if !selected && env.is_large(tree, row.id) {
        crate::theme::large_text(visuals)
    } else {
        text_color
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use storagemap_core::{EntryInfo, TreeBuilder};

    #[test]
    fn filter_counts_only_matching_files_and_hides_empty_folders() {
        let dir = EntryInfo {
            kind: Kind::Dir,
            flags: Flags::empty(),
            modified: 0,
            accessed: 0,
            created: 0,
            size: 0,
            allocated: 0,
        };
        let file = |size| EntryInfo {
            kind: Kind::File,
            size,
            ..dir
        };
        let mut b = TreeBuilder::new(PathBuf::from("/r"), dir);
        let videos = b.push(NodeId::ROOT, b"videos", dir);
        b.push(videos, b"a.mp4", file(100));
        b.push(videos, b"notes.txt", file(5));
        let docs = b.push(NodeId::ROOT, b"docs", dir);
        b.push(docs, b"b.pdf", file(50));
        let t = b.finish();

        let filter = TreeFilter::Type(Category::Video);
        let values = filtered_values(&t, filter, Metric::Size, 0);
        let videos = t.find(std::path::Path::new("/r/videos")).expect("videos");
        assert_eq!(values[t.root().index()], 100);
        assert_eq!(values[videos.index()], 100);

        let expanded = HashSet::from([t.root()]);
        let rows = build_rows(&t, &expanded, Metric::Size, Some(&values));
        let ids: Vec<NodeId> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, [t.root(), videos]);
    }
}
