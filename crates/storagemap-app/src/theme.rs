//! StorageMap's visual design: one accent colour, calm neutrals, consistent
//! rounding, spacing and type scale, tuned separately for dark and light.

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Theme,
    Visuals, vec2,
};

/// Brand blue, matching the app icon.
pub const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
pub const ACCENT_BRIGHT: Color32 = Color32::from_rgb(96, 165, 250);
pub const SUCCESS: Color32 = Color32::from_rgb(34, 160, 95);
/// Names of unusually large items, dark and light theme.
const LARGE_DARK: Color32 = Color32::from_rgb(251, 191, 36);
const LARGE_LIGHT: Color32 = Color32::from_rgb(180, 83, 9);

/// Text colour that marks an unusually large item.
pub fn large_text(visuals: &Visuals) -> Color32 {
    if visuals.dark_mode {
        LARGE_DARK
    } else {
        LARGE_LIGHT
    }
}

/// Seconds for hover/expand/fade animations.
pub const ANIMATION_TIME: f32 = 0.16;
pub const RADIUS: u8 = 6;
pub const CARD_RADIUS: u8 = 10;

/// Installs the theme for both light and dark mode.
pub fn install(ctx: &egui::Context) {
    for theme in [Theme::Dark, Theme::Light] {
        ctx.style_mut_of(theme, |style| apply(style, theme));
    }
}

fn apply(style: &mut Style, theme: Theme) {
    style.animation_time = ANIMATION_TIME;
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.5, FontFamily::Monospace),
        ),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = vec2(8.0, 6.0);
    s.button_padding = vec2(12.0, 5.0);
    s.interact_size.y = 28.0;
    s.menu_margin = Margin::same(8);
    s.window_margin = Margin::same(16);
    s.indent = 16.0;
    s.combo_width = 140.0;
    s.scroll.bar_width = 9.0;

    style.visuals = match theme {
        Theme::Dark => dark(),
        Theme::Light => light(),
    };
}

/// Neutral steps from background to raised surfaces.
struct Palette {
    base: Color32,
    panel: Color32,
    raised: Color32,
    widget: Color32,
    hover: Color32,
    active: Color32,
    border: Color32,
    text: Color32,
    strong: Color32,
    selection: Color32,
    selection_text: Color32,
}

fn visuals(mut v: Visuals, p: &Palette) -> Visuals {
    let r = CornerRadius::same(RADIUS);
    v.panel_fill = p.panel;
    v.window_fill = p.raised;
    v.extreme_bg_color = p.base;
    v.faint_bg_color = p.raised;
    v.code_bg_color = p.widget;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    v.menu_corner_radius = CornerRadius::same(8);
    v.hyperlink_color = if v.dark_mode { ACCENT_BRIGHT } else { ACCENT };
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.selection_text);
    v.striped = true;
    let shadow_alpha = if v.dark_mode { 120 } else { 40 };
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    for (state, fill) in [
        (&mut w.inactive, p.widget),
        (&mut w.hovered, p.hover),
        (&mut w.active, p.active),
        (&mut w.open, p.hover),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.corner_radius = r;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.0, p.strong);
        state.expansion = 0.0;
    }
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.bg_stroke = Stroke::new(1.0, p.border);
    v
}

fn dark() -> Visuals {
    visuals(
        Visuals::dark(),
        &Palette {
            base: Color32::from_rgb(14, 16, 21),
            panel: Color32::from_rgb(22, 25, 31),
            raised: Color32::from_rgb(30, 34, 42),
            widget: Color32::from_rgb(38, 43, 53),
            hover: Color32::from_rgb(50, 56, 69),
            active: Color32::from_rgb(62, 70, 86),
            border: Color32::from_rgb(46, 51, 62),
            text: Color32::from_rgb(200, 205, 214),
            strong: Color32::from_rgb(240, 242, 246),
            selection: Color32::from_rgb(29, 78, 216),
            selection_text: Color32::WHITE,
        },
    )
}

fn light() -> Visuals {
    visuals(
        Visuals::light(),
        &Palette {
            base: Color32::from_rgb(236, 239, 244),
            panel: Color32::from_rgb(248, 249, 251),
            raised: Color32::WHITE,
            widget: Color32::from_rgb(232, 235, 241),
            hover: Color32::from_rgb(219, 224, 233),
            active: Color32::from_rgb(203, 210, 223),
            border: Color32::from_rgb(218, 222, 230),
            text: Color32::from_rgb(55, 61, 72),
            strong: Color32::from_rgb(15, 18, 25),
            selection: Color32::from_rgb(191, 219, 254),
            selection_text: Color32::from_rgb(23, 37, 84),
        },
    )
}

/// A filled, accent-coloured primary button.
pub fn primary_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text.into().color(Color32::WHITE))
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(RADIUS))
}

/// Smooth 0→1 easing for animations.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
