//! Pixel-art sprite editor — embedded paint tool for editing individual sprites.
//!
//! Features: pencil, eraser, color fill, color picker/eyedropper, selection,
//! undo/redo, zoom, grid, animation frame preview, palette.

use egui::{Color32, Pos2, Rect, Vec2};
use std::collections::VecDeque;

// ── Constants ────────────────────────────────────────────────────────────────

/// Default canvas zoom (pixels per sprite pixel).
const DEFAULT_ZOOM: f32 = 12.0;
const MIN_ZOOM: f32 = 2.0;
const MAX_ZOOM: f32 = 40.0;
const MAX_UNDO: usize = 100;
const PALETTE_CELL: f32 = 20.0;

// ── Pixel editor tool type ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelTool {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
    Line,
    Rect,
}

impl PixelTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pencil => "✏ Pencil",
            Self::Eraser => "⌫ Eraser",
            Self::Fill => "🪣 Fill",
            Self::Eyedropper => "💧 Picker",
            Self::Line => "╱ Line",
            Self::Rect => "▢ Rect",
        }
    }

    pub fn hotkey(self) -> &'static str {
        match self {
            Self::Pencil => "P",
            Self::Eraser => "E",
            Self::Fill => "G",
            Self::Eyedropper => "I",
            Self::Line => "L",
            Self::Rect => "R",
        }
    }
}

// ── Sprite editor state ──────────────────────────────────────────────────────

/// Appearance context — describes the full sprite structure for navigation.
#[derive(Debug, Clone, Default)]
pub struct AppearanceContext {
    /// One entry per frame group in the appearance.
    pub frame_groups: Vec<FrameGroupInfo>,
    /// Currently selected frame group index.
    pub fg_index: usize,
    /// Currently selected sprite within the current frame group's sprite_id list.
    pub sprite_index: usize,
}

/// Info about one frame group.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FrameGroupInfo {
    pub label: String,
    pub sprite_ids: Vec<u32>,
    pub layers: u32,
    pub pattern_width: u32,  // directions
    pub pattern_height: u32,
    pub pattern_depth: u32,
    pub num_frames: u32,
    pub sprite_w: u32,
    pub sprite_h: u32,
}

impl AppearanceContext {
    /// Get the currently selected sprite ID.
    pub fn current_sprite_id(&self) -> Option<u32> {
        let fg = self.frame_groups.get(self.fg_index)?;
        fg.sprite_ids.get(self.sprite_index).copied()
    }

    /// Get the current frame group.
    pub fn current_fg(&self) -> Option<&FrameGroupInfo> {
        self.frame_groups.get(self.fg_index)
    }

    /// Total sprites in the current frame group.
    pub fn current_count(&self) -> usize {
        self.frame_groups.get(self.fg_index).map_or(0, |fg| fg.sprite_ids.len())
    }
}

/// State for the embedded pixel editor.
pub struct SpriteEditorState {
    /// Whether the editor panel is open at all.
    pub open: bool,

    /// The sprite ID being edited (from the sprite sheet).
    pub editing_sprite_id: Option<u32>,

    /// Width / height of the current sprite in pixels.
    pub sprite_w: u32,
    pub sprite_h: u32,

    /// RGBA pixel buffer (working copy).
    pub pixels: Vec<u8>,

    /// Zoom level (screen pixels per sprite pixel).
    pub zoom: f32,

    /// Canvas scroll offset.
    pub scroll: Vec2,

    /// Active tool.
    pub tool: PixelTool,

    /// Foreground color.
    pub fg_color: Color32,

    /// Background color (used by eraser).
    pub bg_color: Color32,

    /// Show grid overlay.
    pub show_grid: bool,

    /// Undo stack (snapshots of the full pixel buffer).
    pub undo_stack: VecDeque<Vec<u8>>,
    pub redo_stack: Vec<Vec<u8>>,

    /// For line / rect tool: drag start pixel.
    pub drag_start: Option<(i32, i32)>,

    /// Dirty flag — true if pixels differ from last save.
    pub dirty: bool,

    /// Color palette (recently used / common colors).
    pub palette: Vec<Color32>,

    /// Hex color input string.
    pub hex_input: String,

    /// Preview texture handle (updated each frame if dirty).
    pub preview_tex: Option<egui::TextureHandle>,

    /// Full appearance context for navigation.
    pub appearance_ctx: AppearanceContext,

    /// Whether the user clicked Discard (signals caller to close without save).
    pub discarded: bool,

    /// Signal: the user navigated to a different sprite and needs a pixel reload.
    pub needs_reload: bool,

    /// Thumbnail texture handles for sprites in the current frame group.
    pub thumb_textures: std::collections::HashMap<u32, egui::TextureHandle>,
}

impl Default for SpriteEditorState {
    fn default() -> Self {
        Self {
            open: false,
            editing_sprite_id: None,
            sprite_w: 32,
            sprite_h: 32,
            pixels: vec![0u8; 32 * 32 * 4],
            zoom: DEFAULT_ZOOM,
            scroll: Vec2::ZERO,
            tool: PixelTool::Pencil,
            fg_color: Color32::WHITE,
            bg_color: Color32::TRANSPARENT,
            show_grid: true,
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            drag_start: None,
            dirty: false,
            palette: default_palette(),
            hex_input: String::new(),
            preview_tex: None,
            appearance_ctx: AppearanceContext::default(),
            discarded: false,
            needs_reload: false,
            thumb_textures: std::collections::HashMap::new(),
        }
    }
}

impl SpriteEditorState {
    /// Load a sprite from the sprite sheets into the editor.
    pub fn load_sprite(&mut self, sprite_id: u32, pixels: Vec<u8>, w: u32, h: u32) {
        self.editing_sprite_id = Some(sprite_id);
        self.sprite_w = w;
        self.sprite_h = h;
        self.pixels = pixels;
        self.zoom = DEFAULT_ZOOM;
        self.scroll = Vec2::ZERO;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        self.open = true;
        self.preview_tex = None;
        self.discarded = false;
        self.needs_reload = false;
    }

    /// Set full appearance context (frame groups, sprite lists, etc.).
    pub fn set_appearance_context(&mut self, ctx: AppearanceContext) {
        self.appearance_ctx = ctx;
    }

    /// Navigate to a different sprite within the current frame group.
    pub fn navigate_to(&mut self, sprite_index: usize) {
        self.appearance_ctx.sprite_index = sprite_index;
        self.needs_reload = true;
    }

    /// Navigate to a different frame group.
    pub fn navigate_fg(&mut self, fg_index: usize) {
        self.appearance_ctx.fg_index = fg_index;
        self.appearance_ctx.sprite_index = 0;
        self.needs_reload = true;
    }

    fn push_undo(&mut self) {
        self.undo_stack.push_back(self.pixels.clone());
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.pop_front();
        }
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop_back() {
            self.redo_stack.push(std::mem::replace(&mut self.pixels, prev));
            self.preview_tex = None;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push_back(std::mem::replace(&mut self.pixels, next));
            self.preview_tex = None;
        }
    }

    /// Get pixel color at (x, y).
    fn get_pixel(&self, x: i32, y: i32) -> Color32 {
        if x < 0 || y < 0 || x >= self.sprite_w as i32 || y >= self.sprite_h as i32 {
            return Color32::TRANSPARENT;
        }
        let idx = ((y as u32 * self.sprite_w + x as u32) * 4) as usize;
        Color32::from_rgba_unmultiplied(
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        )
    }

    /// Set pixel color at (x, y).
    fn set_pixel(&mut self, x: i32, y: i32, color: Color32) {
        if x < 0 || y < 0 || x >= self.sprite_w as i32 || y >= self.sprite_h as i32 {
            return;
        }
        let idx = ((y as u32 * self.sprite_w + x as u32) * 4) as usize;
        let [r, g, b, a] = color.to_array();
        self.pixels[idx] = r;
        self.pixels[idx + 1] = g;
        self.pixels[idx + 2] = b;
        self.pixels[idx + 3] = a;
        self.preview_tex = None;
    }

    /// Flood fill from (x, y) with the given color.
    fn flood_fill(&mut self, x: i32, y: i32, fill_color: Color32) {
        let target = self.get_pixel(x, y);
        if target == fill_color {
            return;
        }

        let w = self.sprite_w as i32;
        let h = self.sprite_h as i32;
        let mut stack = vec![(x, y)];

        while let Some((cx, cy)) = stack.pop() {
            if cx < 0 || cy < 0 || cx >= w || cy >= h {
                continue;
            }
            if self.get_pixel(cx, cy) != target {
                continue;
            }
            self.set_pixel(cx, cy, fill_color);
            stack.push((cx + 1, cy));
            stack.push((cx - 1, cy));
            stack.push((cx, cy + 1));
            stack.push((cx, cy - 1));
        }
    }

    /// Draw a line from (x0,y0) to (x1,y1) using Bresenham's.
    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cx = x0;
        let mut cy = y0;

        loop {
            self.set_pixel(cx, cy, color);
            if cx == x1 && cy == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                err += dx;
                cy += sy;
            }
        }
    }

    /// Draw a rect outline from (x0,y0) to (x1,y1).
    fn draw_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color32) {
        let lx = x0.min(x1);
        let ly = y0.min(y1);
        let hx = x0.max(x1);
        let hy = y0.max(y1);
        for x in lx..=hx {
            self.set_pixel(x, ly, color);
            self.set_pixel(x, hy, color);
        }
        for y in ly..=hy {
            self.set_pixel(lx, y, color);
            self.set_pixel(hx, y, color);
        }
    }
}

// ── UI ───────────────────────────────────────────────────────────────────────

/// Show the sprite editor as a modal / panel overlay.
/// Returns true if the user clicked "Save" (caller should write back to sheet).
pub fn show(ctx: &egui::Context, editor: &mut SpriteEditorState) -> bool {
    let mut saved = false;

    if !editor.open {
        return false;
    }

    let mut is_open = editor.open;

    egui::Window::new("Sprite Editor")
        .default_size([800.0, 620.0])
        .min_size([600.0, 400.0])
        .resizable(true)
        .collapsible(false)
        .open(&mut is_open)
        .show(ctx, |ui| {
            // ── Toolbar ──
            ui.horizontal(|ui| {
                for tool in [
                    PixelTool::Pencil,
                    PixelTool::Eraser,
                    PixelTool::Fill,
                    PixelTool::Eyedropper,
                    PixelTool::Line,
                    PixelTool::Rect,
                ] {
                    let selected = editor.tool == tool;
                    let label = format!("{} [{}]", tool.label(), tool.hotkey());
                    let btn = egui::SelectableLabel::new(selected, &label);
                    if ui.add(btn).clicked() {
                        editor.tool = tool;
                    }
                }

                ui.separator();

                // Undo / Redo
                if ui.add_enabled(!editor.undo_stack.is_empty(), egui::Button::new("↩")).on_hover_text("Undo").clicked() {
                    editor.undo();
                }
                if ui.add_enabled(!editor.redo_stack.is_empty(), egui::Button::new("↪")).on_hover_text("Redo").clicked() {
                    editor.redo();
                }

                ui.separator();

                // Grid toggle
                ui.checkbox(&mut editor.show_grid, "Grid");

                ui.separator();

                // Zoom
                ui.label("Zoom:");
                if ui.small_button("−").clicked() {
                    editor.zoom = (editor.zoom - 2.0).max(MIN_ZOOM);
                }
                ui.label(format!("{:.0}×", editor.zoom));
                if ui.small_button("+").clicked() {
                    editor.zoom = (editor.zoom + 2.0).min(MAX_ZOOM);
                }
            });

            ui.separator();

            // ── Main area: canvas left, palette/info right ──
            // Use horizontal_top so canvas gets full height, not centered
            let palette_width = 180.0;
            let canvas_w = editor.sprite_w as f32 * editor.zoom;
            let canvas_h = editor.sprite_h as f32 * editor.zoom;

            ui.horizontal_top(|ui| {
                // Left: canvas in its own scroll area (for zoomed-in panning)
                let max_canvas_w = (ui.available_width() - palette_width - 20.0).max(200.0);
                let max_canvas_h = (ui.available_height() - 4.0).max(200.0);
                egui::ScrollArea::both()
                    .max_width(max_canvas_w.min(canvas_w))
                    .max_height(max_canvas_h.min(canvas_h))
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                    .show(ui, |ui| {
                        draw_canvas(ui, editor);
                    });

                ui.separator();

                // Right: color + palette + info
                ui.vertical(|ui| {
                    ui.set_min_width(140.0);

                    ui.label("Foreground:");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut editor.fg_color,
                        egui::color_picker::Alpha::OnlyBlend,
                    );

                    ui.add_space(8.0);
                    ui.label("Background:");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut editor.bg_color,
                        egui::color_picker::Alpha::OnlyBlend,
                    );

                    ui.add_space(12.0);

                    // Swap FG/BG
                    if ui.button("⇄ Swap Colors").clicked() {
                        std::mem::swap(&mut editor.fg_color, &mut editor.bg_color);
                    }

                    ui.add_space(8.0);

                    // Hex color input
                    ui.label("Hex:");
                    ui.horizontal(|ui| {
                        ui.label("#");
                        let hex_resp = ui.add(
                            egui::TextEdit::singleline(&mut editor.hex_input)
                                .desired_width(70.0)
                                .hint_text("FF00FF")
                                .char_limit(8)
                                .font(egui::TextStyle::Monospace),
                        );
                        if hex_resp.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            if let Some(color) = parse_hex_color(&editor.hex_input) {
                                editor.fg_color = color;
                                if !editor.palette.contains(&color) {
                                    editor.palette.push(color);
                                }
                            }
                        }
                        if ui.small_button("Set").clicked() {
                            if let Some(color) = parse_hex_color(&editor.hex_input) {
                                editor.fg_color = color;
                                if !editor.palette.contains(&color) {
                                    editor.palette.push(color);
                                }
                            }
                        }
                    });

                    // Show current FG color as hex
                    let (r, g, b, a) = (
                        editor.fg_color.r(),
                        editor.fg_color.g(),
                        editor.fg_color.b(),
                        editor.fg_color.a(),
                    );
                    let current_hex = if a == 255 {
                        format!("{:02X}{:02X}{:02X}", r, g, b)
                    } else {
                        format!("{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
                    };
                    ui.label(
                        egui::RichText::new(format!("Current: #{}", current_hex))
                            .size(9.5)
                            .color(Color32::from_gray(160))
                            .monospace(),
                    );

                    ui.add_space(12.0);
                    ui.label("Palette:");
                    show_palette(ui, editor);

                    ui.add_space(12.0);

                    // Preview at 1x
                    ui.label("Preview (1×):");
                    let preview_size = Vec2::new(editor.sprite_w as f32, editor.sprite_h as f32);
                    let (preview_rect, _) = ui.allocate_exact_size(preview_size * 2.0, egui::Sense::hover());

                    // Checkerboard background
                    draw_checkerboard(ui.painter(), preview_rect, 4.0);

                    // Render preview texture
                    let tex = get_or_create_preview(ui.ctx(), editor);
                    ui.painter().image(
                        tex.id(),
                        preview_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );

                    ui.add_space(12.0);

                    // Sprite info
                    if let Some(sid) = editor.editing_sprite_id {
                        ui.label(format!("Sprite #{}", sid));
                    }
                    ui.label(format!("{}×{} px", editor.sprite_w, editor.sprite_h));

                    ui.add_space(12.0);

                    // Save / Discard buttons
                    ui.horizontal(|ui| {
                        let save_btn = egui::Button::new(
                            crate::icons::icon_colored(crate::icons::SAVE, 13.0, crate::theme::SUCCESS)
                        ).min_size(egui::vec2(60.0, 24.0));
                        if ui.add_enabled(editor.dirty, save_btn)
                            .on_hover_text("Save changes back to sprite sheet")
                            .clicked()
                        {
                            saved = true;
                        }
                        let discard_btn = egui::Button::new(
                            crate::icons::icon_colored(crate::icons::X, 13.0, crate::theme::ERROR)
                        ).min_size(egui::vec2(60.0, 24.0));
                        if ui.add(discard_btn)
                            .on_hover_text("Discard changes and close")
                            .clicked()
                        {
                            editor.discarded = true;
                            editor.open = false;
                        }
                    });

                    // ── Frame group / sprite navigation ──
                    let total_fgs = editor.appearance_ctx.frame_groups.len();
                    let total_sprites = editor.appearance_ctx.current_count();

                    if total_fgs > 0 && total_sprites > 0 {
                        ui.add_space(8.0);
                        ui.separator();

                        // Frame group selector (if multiple)
                        if total_fgs > 1 {
                            // Extract labels to avoid borrow conflicts
                            let labels: Vec<String> = editor.appearance_ctx.frame_groups
                                .iter().map(|fg| fg.label.clone()).collect();
                            let current_fg = editor.appearance_ctx.fg_index;

                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Frame Group:")
                                        .size(10.0)
                                        .color(crate::theme::TEXT_MUTED),
                                );
                                for (i, label) in labels.iter().enumerate() {
                                    let selected = i == current_fg;
                                    if ui.add(egui::SelectableLabel::new(selected, label)).clicked() && !selected {
                                        editor.navigate_fg(i);
                                    }
                                }
                            });
                        }

                        // Extract info from current frame group
                        let fg = editor.appearance_ctx.current_fg().unwrap();
                        let count = fg.sprite_ids.len();
                        let idx = editor.appearance_ctx.sprite_index;
                        let info_text = format!(
                            "{}×{} px • {} layers • {} dirs • {} frames",
                            fg.sprite_w, fg.sprite_h,
                            fg.layers, fg.pattern_width, fg.num_frames,
                        );
                        // Clone sprite IDs for the thumbnail strip
                        let thumb_ids: Vec<u32> = fg.sprite_ids.clone();

                        // Show layout info
                        ui.label(
                            egui::RichText::new(info_text)
                                .size(9.5)
                                .color(crate::theme::TEXT_MUTED),
                        );

                        // Navigation: sprite N / total
                        ui.horizontal(|ui| {
                            ui.label(format!("Sprite {}/{}", idx + 1, count));
                            if ui.add_enabled(
                                idx > 0,
                                egui::Button::new(crate::icons::icon(crate::icons::CHEVRON_LEFT, 12.0)),
                            ).clicked() {
                                editor.navigate_to(idx - 1);
                            }
                            if ui.add_enabled(
                                idx + 1 < count,
                                egui::Button::new(crate::icons::icon(crate::icons::CHEVRON_RIGHT, 12.0)),
                            ).clicked() {
                                editor.navigate_to(idx + 1);
                            }
                        });

                        // Thumbnail strip of all sprites in this frame group
                        let thumb_size = 28.0;
                        ui.add_space(4.0);
                        egui::ScrollArea::horizontal()
                            .max_width(ui.available_width())
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                                    let current_idx = editor.appearance_ctx.sprite_index;
                                    for (i, &sid) in thumb_ids.iter().enumerate() {
                                        let is_current = i == current_idx;
                                        let (rect, resp) = ui.allocate_exact_size(
                                            egui::Vec2::splat(thumb_size),
                                            egui::Sense::click(),
                                        );
                                        let bg = if is_current { crate::theme::ACCENT_MUTED } else { crate::theme::BG_SURFACE };
                                        let border = if is_current { crate::theme::ACCENT } else { crate::theme::BORDER };
                                        ui.painter().rect_filled(rect, 2.0, bg);
                                        ui.painter().rect_stroke(rect, 2.0, (0.5, border), egui::StrokeKind::Outside);

                                        // Draw thumbnail texture if available
                                        if let Some(tex) = editor.thumb_textures.get(&sid) {
                                            let inner = rect.shrink(2.0);
                                            ui.painter().image(
                                                tex.id(),
                                                inner,
                                                egui::Rect::from_min_max(
                                                    egui::Pos2::ZERO,
                                                    egui::Pos2::new(1.0, 1.0),
                                                ),
                                                Color32::WHITE,
                                            );
                                        } else {
                                            ui.painter().text(
                                                rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                format!("{}", sid),
                                                egui::FontId::proportional(8.0),
                                                crate::theme::TEXT_MUTED,
                                            );
                                        }

                                        if resp.clicked() && !is_current {
                                            editor.navigate_to(i);
                                        }
                                        resp.on_hover_text(format!("Sprite #{}", sid));
                                    }
                                });
                            });
                    }
                });
            });
        });

    // Sync open state back (respect discard/close from inside the window)
    editor.open = is_open && !editor.discarded;

    // Hotkeys when editor is open
    ctx.input(|i| {
        if i.key_pressed(egui::Key::P) { editor.tool = PixelTool::Pencil; }
        if i.key_pressed(egui::Key::E) { editor.tool = PixelTool::Eraser; }
        if i.key_pressed(egui::Key::G) { editor.tool = PixelTool::Fill; }
        if i.key_pressed(egui::Key::I) { editor.tool = PixelTool::Eyedropper; }
        if i.key_pressed(egui::Key::L) { editor.tool = PixelTool::Line; }
        if i.key_pressed(egui::Key::R) && !i.modifiers.ctrl { editor.tool = PixelTool::Rect; }
        if i.modifiers.ctrl && i.key_pressed(egui::Key::Z) { editor.undo(); }
        if i.modifiers.ctrl && i.key_pressed(egui::Key::Y) { editor.redo(); }
    });

    saved
}

// ── Canvas drawing ───────────────────────────────────────────────────────────

fn draw_canvas(ui: &mut egui::Ui, editor: &mut SpriteEditorState) {
    let w = editor.sprite_w;
    let h = editor.sprite_h;
    let zoom = editor.zoom;
    let canvas_size = Vec2::new(w as f32 * zoom, h as f32 * zoom);

    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());
    let origin = response.rect.min;

    // Checkerboard background (transparency indicator)
    draw_checkerboard(&painter, response.rect, zoom.max(4.0));

    // Draw each pixel
    for py in 0..h {
        for px in 0..w {
            let idx = ((py * w + px) * 4) as usize;
            let a = editor.pixels[idx + 3];
            if a == 0 {
                continue; // transparent — checkerboard shows through
            }
            let c = Color32::from_rgba_unmultiplied(
                editor.pixels[idx],
                editor.pixels[idx + 1],
                editor.pixels[idx + 2],
                a,
            );
            let rect = Rect::from_min_size(
                Pos2::new(origin.x + px as f32 * zoom, origin.y + py as f32 * zoom),
                Vec2::new(zoom, zoom),
            );
            painter.rect_filled(rect, 0.0, c);
        }
    }

    // Grid
    if editor.show_grid && zoom >= 4.0 {
        let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 20);
        for x in 0..=w {
            let sx = origin.x + x as f32 * zoom;
            painter.line_segment(
                [Pos2::new(sx, origin.y), Pos2::new(sx, origin.y + h as f32 * zoom)],
                (0.5, grid_color),
            );
        }
        for y in 0..=h {
            let sy = origin.y + y as f32 * zoom;
            painter.line_segment(
                [Pos2::new(origin.x, sy), Pos2::new(origin.x + w as f32 * zoom, sy)],
                (0.5, grid_color),
            );
        }
    }

    // Handle mouse interaction
    let pixel_from_pos = |pos: Pos2| -> (i32, i32) {
        let px = ((pos.x - origin.x) / zoom).floor() as i32;
        let py = ((pos.y - origin.y) / zoom).floor() as i32;
        (px, py)
    };

    // Hover highlight
    if let Some(hover_pos) = response.hover_pos() {
        let (hx, hy) = pixel_from_pos(hover_pos);
        if hx >= 0 && hy >= 0 && (hx as u32) < w && (hy as u32) < h {
            let rect = Rect::from_min_size(
                Pos2::new(origin.x + hx as f32 * zoom, origin.y + hy as f32 * zoom),
                Vec2::new(zoom, zoom),
            );
            painter.rect_stroke(
                rect, 0.0,
                (1.0, Color32::from_white_alpha(180)),
                egui::StrokeKind::Outside,
            );
        }
    }

    // Tool application (skip when Ctrl is held — that's panning)
    let ctrl_held = response.ctx.input(|i| i.modifiers.ctrl);
    if !ctrl_held && (response.dragged_by(egui::PointerButton::Primary) || response.clicked()) {
        if let Some(pos) = response.interact_pointer_pos() {
            let (px, py) = pixel_from_pos(pos);
            match editor.tool {
                PixelTool::Pencil => {
                    if response.drag_started() || response.clicked() {
                        editor.push_undo();
                    }
                    editor.set_pixel(px, py, editor.fg_color);
                }
                PixelTool::Eraser => {
                    if response.drag_started() || response.clicked() {
                        editor.push_undo();
                    }
                    editor.set_pixel(px, py, Color32::TRANSPARENT);
                }
                PixelTool::Fill => {
                    if response.clicked() {
                        editor.push_undo();
                        editor.flood_fill(px, py, editor.fg_color);
                    }
                }
                PixelTool::Eyedropper => {
                    let c = editor.get_pixel(px, py);
                    if c.a() > 0 {
                        editor.fg_color = c;
                    }
                    // Add to palette if not already there
                    if !editor.palette.contains(&c) {
                        editor.palette.push(c);
                    }
                }
                PixelTool::Line => {
                    if response.drag_started() {
                        editor.push_undo();
                        editor.drag_start = Some((px, py));
                    }
                    if response.drag_stopped() {
                        if let Some((sx, sy)) = editor.drag_start.take() {
                            editor.draw_line(sx, sy, px, py, editor.fg_color);
                        }
                    }
                }
                PixelTool::Rect => {
                    if response.drag_started() {
                        editor.push_undo();
                        editor.drag_start = Some((px, py));
                    }
                    if response.drag_stopped() {
                        if let Some((sx, sy)) = editor.drag_start.take() {
                            editor.draw_rect(sx, sy, px, py, editor.fg_color);
                        }
                    }
                }
            }
        }
    }

    // Ctrl+drag to pan the canvas scroll area
    let ctrl_held = response.ctx.input(|i| i.modifiers.ctrl);
    if ctrl_held && response.dragged_by(egui::PointerButton::Primary) {
        let delta = response.drag_delta();
        // Feed the negative drag delta as scroll input so the parent ScrollArea moves
        ui.scroll_with_delta(delta);
    }

    // Zoom with scroll wheel over canvas — integer 1x steps
    if response.hovered() {
        let raw_scroll = response.ctx.input(|i| i.raw_scroll_delta.y);
        if raw_scroll != 0.0 {
            let step = if raw_scroll > 0.0 { 1.0 } else { -1.0 };
            editor.zoom = (editor.zoom + step).clamp(MIN_ZOOM, MAX_ZOOM);
        }
    }
}

// ── Palette ──────────────────────────────────────────────────────────────────

fn show_palette(ui: &mut egui::Ui, editor: &mut SpriteEditorState) {
    let cols = 6;
    egui::Grid::new("pixel_palette")
        .spacing([2.0, 2.0])
        .min_col_width(PALETTE_CELL)
        .show(ui, |ui| {
            for (i, &color) in editor.palette.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(PALETTE_CELL, PALETTE_CELL),
                    egui::Sense::click(),
                );

                let selected = editor.fg_color == color;
                let stroke_color = if selected {
                    Color32::WHITE
                } else if response.hovered() {
                    Color32::from_gray(200)
                } else {
                    Color32::from_gray(80)
                };

                ui.painter().rect_filled(rect, 2.0, color);
                ui.painter().rect_stroke(rect, 2.0, (1.0, stroke_color), egui::StrokeKind::Outside);

                if response.clicked() {
                    editor.fg_color = color;
                }
                if response.secondary_clicked() {
                    editor.bg_color = color;
                }

                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn draw_checkerboard(painter: &egui::Painter, rect: Rect, cell_size: f32) {
    let c1 = Color32::from_gray(40);
    let c2 = Color32::from_gray(55);

    // Fill with c1 first, then draw c2 squares
    painter.rect_filled(rect, 0.0, c1);

    let cols = ((rect.width() / cell_size).ceil() as i32).max(1);
    let rows = ((rect.height() / cell_size).ceil() as i32).max(1);

    for row in 0..rows {
        for col in 0..cols {
            if (row + col) % 2 == 0 {
                continue;
            }
            let r = Rect::from_min_size(
                Pos2::new(
                    rect.min.x + col as f32 * cell_size,
                    rect.min.y + row as f32 * cell_size,
                ),
                Vec2::new(cell_size, cell_size),
            )
            .intersect(rect);
            painter.rect_filled(r, 0.0, c2);
        }
    }
}

fn get_or_create_preview(ctx: &egui::Context, editor: &mut SpriteEditorState) -> egui::TextureHandle {
    if let Some(ref tex) = editor.preview_tex {
        return tex.clone();
    }

    let image = egui::ColorImage::from_rgba_unmultiplied(
        [editor.sprite_w as _, editor.sprite_h as _],
        &editor.pixels,
    );
    let tex = ctx.load_texture(
        "sprite_editor_preview",
        image,
        egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Nearest,
            ..Default::default()
        },
    );
    editor.preview_tex = Some(tex.clone());
    tex
}

fn default_palette() -> Vec<Color32> {
    vec![
        // Greyscale
        Color32::BLACK,
        Color32::from_gray(64),
        Color32::from_gray(128),
        Color32::from_gray(192),
        Color32::WHITE,
        Color32::TRANSPARENT,
        // Reds
        Color32::from_rgb(128, 0, 0),
        Color32::from_rgb(255, 0, 0),
        Color32::from_rgb(255, 128, 128),
        // Oranges / browns
        Color32::from_rgb(128, 64, 0),
        Color32::from_rgb(255, 128, 0),
        Color32::from_rgb(255, 200, 128),
        // Yellows
        Color32::from_rgb(128, 128, 0),
        Color32::from_rgb(255, 255, 0),
        Color32::from_rgb(255, 255, 180),
        // Greens
        Color32::from_rgb(0, 128, 0),
        Color32::from_rgb(0, 255, 0),
        Color32::from_rgb(128, 255, 128),
        // Cyans
        Color32::from_rgb(0, 128, 128),
        Color32::from_rgb(0, 255, 255),
        Color32::from_rgb(180, 255, 255),
        // Blues
        Color32::from_rgb(0, 0, 128),
        Color32::from_rgb(0, 0, 255),
        Color32::from_rgb(128, 128, 255),
        // Purples
        Color32::from_rgb(128, 0, 128),
        Color32::from_rgb(255, 0, 255),
        Color32::from_rgb(255, 128, 255),
        // Skin tones
        Color32::from_rgb(255, 219, 172),
        Color32::from_rgb(241, 194, 125),
        Color32::from_rgb(198, 134, 66),
    ]
}

/// Parse a hex color string (with or without #).
/// Supports: RGB (3 chars), RRGGBB (6 chars), RRGGBBAA (8 chars).
fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()?;
            let g = u8::from_str_radix(&s[1..2], 16).ok()?;
            let b = u8::from_str_radix(&s[2..3], 16).ok()?;
            Some(Color32::from_rgb(r * 17, g * 17, b * 17))
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => None,
    }
}
