//! Editor theme — colors, spacing, and style configuration.
//!
//! Inspired by professional editors like Blender, Godot, Unity rather
//! than generic egui defaults. Dark neutral palette with a warm accent.

use egui::{Color32, CornerRadius, Stroke, Style, Visuals};

// ── Palette ────────────────────────────────────────────────────────

/// Background layers (darkest → lightest)
pub const BG_BASE: Color32 = Color32::from_rgb(24, 24, 28);
pub const BG_PANEL: Color32 = Color32::from_rgb(30, 30, 35);
pub const BG_SURFACE: Color32 = Color32::from_rgb(38, 38, 44);
pub const BG_RAISED: Color32 = Color32::from_rgb(48, 48, 56);

/// Text
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(210, 210, 215);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 150);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(90, 90, 100);

/// Accent (warm red-coral)
pub const ACCENT: Color32 = Color32::from_rgb(220, 75, 85);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(240, 95, 105);
pub const ACCENT_MUTED: Color32 = Color32::from_rgba_premultiplied(220, 75, 85, 40);

/// Semantic
pub const SUCCESS: Color32 = Color32::from_rgb(80, 190, 120);
pub const WARNING: Color32 = Color32::from_rgb(220, 180, 60);
pub const ERROR: Color32 = Color32::from_rgb(220, 70, 70);

/// Borders & separators
pub const BORDER: Color32 = Color32::from_rgb(55, 55, 65);
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(65, 65, 75);

/// Tool button colors
pub const TOOL_ACTIVE_BG: Color32 = Color32::from_rgb(220, 75, 85);
pub const TOOL_HOVER_BG: Color32 = Color32::from_rgb(55, 55, 65);

// ── Apply ──────────────────────────────────────────────────────────

/// Apply the editor theme to an egui context.
pub fn apply(ctx: &egui::Context) {
    let mut style = Style::default();
    let v = &mut style.visuals;

    // Dark mode base
    *v = Visuals::dark();

    // Window / panel backgrounds
    v.window_fill = BG_PANEL;
    v.panel_fill = BG_PANEL;
    v.extreme_bg_color = BG_BASE;
    v.faint_bg_color = BG_SURFACE;

    // Widgets
    v.widgets.noninteractive.bg_fill = BG_SURFACE;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    v.widgets.noninteractive.bg_stroke = Stroke::new(0.5, BORDER);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(3);

    v.widgets.inactive.bg_fill = BG_RAISED;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.inactive.bg_stroke = Stroke::new(0.5, BORDER);
    v.widgets.inactive.corner_radius = CornerRadius::same(3);

    v.widgets.hovered.bg_fill = TOOL_HOVER_BG;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_LIGHT);
    v.widgets.hovered.corner_radius = CornerRadius::same(3);

    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.corner_radius = CornerRadius::same(3);

    v.widgets.open.bg_fill = BG_SURFACE;
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.open.corner_radius = CornerRadius::same(3);

    // Selection
    v.selection.bg_fill = ACCENT_MUTED;
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    // Misc
    v.window_corner_radius = CornerRadius::same(6);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.menu_corner_radius = CornerRadius::same(4);
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(60),
    };

    v.striped = false;
    v.slider_trailing_fill = true;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    // Spacing
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(8);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.spacing.indent = 16.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 6.0,
        ..style.spacing.scroll
    };

    ctx.set_style(style);
}
