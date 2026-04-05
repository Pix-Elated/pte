//! Lucide icon font integration.
//!
//! Icons from Lucide (https://lucide.dev/) — ISC License.
//! Font embedded at compile time from assets/fonts/lucide.ttf.

/// Font family name for Lucide icons.
pub const FONT_FAMILY: &str = "lucide";

/// Font bytes (embedded at compile time).
pub const FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/lucide.ttf");

// ── Codepoints ──────────────────────────────────────────────────────

// Map editor toolbar tools
pub const PAINTBRUSH: char = '\u{e2e7}';
pub const ERASER: char = '\u{e28f}';
pub const PAINT_BUCKET: char = '\u{e2e6}';
pub const PIPETTE: char = '\u{e13b}';
pub const SQUARE_DASHED: char = '\u{e1cb}';
pub const DOOR_OPEN: char = '\u{e3d6}';
pub const PAW_PRINT: char = '\u{e4f5}';
pub const CROSSHAIR: char = '\u{e0ac}';
pub const MAP_PIN: char = '\u{e111}';
pub const PENCIL: char = '\u{e1f9}';

// History / actions
pub const UNDO: char = '\u{e2a1}';
pub const REDO: char = '\u{e2a0}';
pub const SAVE: char = '\u{e14d}';
pub const TRASH: char = '\u{e18e}';
pub const COPY: char = '\u{e09e}';
pub const DOWNLOAD: char = '\u{e0b2}';
pub const UPLOAD: char = '\u{e19e}';

// Zoom
pub const ZOOM_IN: char = '\u{e1b6}';
pub const ZOOM_OUT: char = '\u{e1b7}';
pub const SCAN: char = '\u{e257}';

// Navigation
pub const PLUS: char = '\u{e13d}';
pub const MINUS: char = '\u{e11c}';
pub const CHECK: char = '\u{e06c}';
pub const X: char = '\u{e1b2}';
pub const CHEVRON_LEFT: char = '\u{e06e}';
pub const CHEVRON_RIGHT: char = '\u{e06f}';

// View
pub const GRID: char = '\u{e4ff}';
pub const EYE: char = '\u{e0ba}';
pub const EYE_OFF: char = '\u{e0bb}';
pub const LAYERS: char = '\u{e529}';
pub const MOVE: char = '\u{e121}';
pub const SETTINGS: char = '\u{e154}';
pub const ROTATE_CCW: char = '\u{e148}';
pub const MOUSE_POINTER: char = '\u{e1c3}';

// ── Helper ──────────────────────────────────────────────────────────

/// Create a RichText with a Lucide icon at the given size.
pub fn icon(codepoint: char, size: f32) -> egui::RichText {
    egui::RichText::new(codepoint.to_string())
        .family(egui::FontFamily::Name(FONT_FAMILY.into()))
        .size(size)
}

/// Create a RichText icon with a specific color.
pub fn icon_colored(codepoint: char, size: f32, color: egui::Color32) -> egui::RichText {
    icon(codepoint, size).color(color)
}

/// Register the Lucide font with an egui context. Call once at startup.
pub fn register_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Add Lucide as a named font family
    fonts.font_data.insert(
        FONT_FAMILY.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(FONT_DATA)),
    );

    fonts
        .families
        .entry(egui::FontFamily::Name(FONT_FAMILY.into()))
        .or_default()
        .push(FONT_FAMILY.to_owned());

    ctx.set_fonts(fonts);
}
