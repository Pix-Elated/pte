//! Ribbon toolbar — professional two-row tool strip inspired by RME.
//!
//! Row 1: Icon-only tool buttons | Undo/Redo | Zoom | Active brush indicator
//! Row 2: Context-sensitive options (brush size, shape, eraser mode, etc.)

use crate::brushes::door::DoorVariant;
use crate::brushes::shape::BrushShape;
use crate::icons;
use crate::state::{EditorState, EraserMode, ToolType};
use crate::theme;

/// Actions the toolbar can request from the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    None,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitToMap,
}

// ── Sizing constants ──────────────────────────────────────────────

const TOOL_BTN_SIZE: f32 = 28.0; // Square tool buttons
const TOOL_ICON_SIZE: f32 = 15.0; // Icon font size
const SMALL_BTN_H: f32 = 20.0; // Secondary buttons height
const OPTION_LABEL_SIZE: f32 = 10.0; // Row 2 label size
const PRESET_BTN_SIZE: f32 = 20.0; // Size preset button dimensions

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    // ── Row 1: Icon tools | History | Zoom | Active brush ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;

        // ── Drawing tools ──
        for tool in [
            ToolType::Brush,
            ToolType::Eraser,
            ToolType::Fill,
            ToolType::Eyedropper,
            ToolType::Select,
        ] {
            icon_tool_button(ui, &mut state.active_tool, tool);
        }

        separator(ui);

        // ── Placement tools ──
        let has_doors = !state
            .brush_registry
            .brushes_of_type(crate::brushes::BrushType::Door)
            .is_empty();
        let has_creatures = !state
            .brush_registry
            .brushes_of_type(crate::brushes::BrushType::Creature)
            .is_empty();
        let has_spawns = !state
            .brush_registry
            .brushes_of_type(crate::brushes::BrushType::Spawn)
            .is_empty();
        let has_waypoints = !state
            .brush_registry
            .brushes_of_type(crate::brushes::BrushType::Waypoint)
            .is_empty();

        if has_doors {
            icon_tool_button(ui, &mut state.active_tool, ToolType::Door);
        }
        if has_creatures {
            icon_tool_button(ui, &mut state.active_tool, ToolType::Creature);
        }
        if has_spawns {
            icon_tool_button(ui, &mut state.active_tool, ToolType::Spawn);
        }
        if has_waypoints {
            icon_tool_button(ui, &mut state.active_tool, ToolType::Waypoint);
        }

        if has_doors || has_creatures || has_spawns || has_waypoints {
            separator(ui);
        }

        // ── Undo / Redo ──
        if lucide_btn(ui, icons::UNDO, state.can_undo(), "Undo [Ctrl+Z]").clicked() {
            action = ToolbarAction::Undo;
        }
        if lucide_btn(ui, icons::REDO, state.can_redo(), "Redo [Ctrl+Y]").clicked() {
            action = ToolbarAction::Redo;
        }

        separator(ui);

        // ── Zoom ──
        if lucide_btn(ui, icons::ZOOM_OUT, true, "Zoom Out [-]").clicked() {
            action = ToolbarAction::ZoomOut;
        }
        ui.label(
            egui::RichText::new(format!("{:.0}%", state.camera.zoom * 100.0))
                .size(10.0)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        );
        if lucide_btn(ui, icons::ZOOM_IN, true, "Zoom In [+]").clicked() {
            action = ToolbarAction::ZoomIn;
        }
        if lucide_btn(ui, icons::SCAN, true, "Reset Zoom [Ctrl+0]").clicked() {
            action = ToolbarAction::ZoomReset;
        }
        if lucide_btn(ui, icons::GRID, true, "Fit Map [Ctrl+Shift+F]").clicked() {
            action = ToolbarAction::FitToMap;
        }

        // ── Right-aligned: Active brush indicator ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(brush_id) = state.active_brush {
                if let Some(brush) = state.brush_registry.get(brush_id) {
                    // Sprite preview
                    let look = brush.look_id() as u32;
                    let preview_size = egui::vec2(20.0, 20.0);
                    let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    if let Some(tex) = crate::viewport::get_or_upload(
                        &mut state.sprite_textures,
                        &state.sprite_sheets,
                        ui.ctx(),
                        look,
                    ) {
                        ui.painter().image(
                            tex.id(),
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    ui.label(
                        egui::RichText::new(brush.name())
                            .size(11.0)
                            .color(theme::ACCENT),
                    );
                }
            } else if let Some(id) = state.selected_item_id {
                ui.label(
                    egui::RichText::new(format!("Item #{}", id))
                        .size(11.0)
                        .color(theme::TEXT_SECONDARY),
                );
            }
        });
    });

    // ── Row 2: Context-sensitive options ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        match state.active_tool {
            ToolType::Brush | ToolType::Eraser => {
                // Zone/House indicators
                show_active_markers(ui, state);

                // Brush size
                option_label(ui, "Size");
                if small_btn(ui, "−").clicked() {
                    let v = (state.brush_size as i32 - 2).max(1);
                    state.brush_size = if v % 2 == 0 { (v + 1) as u32 } else { v as u32 };
                }
                ui.label(
                    egui::RichText::new(format!("{}", state.brush_size))
                        .size(11.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );
                if small_btn(ui, "+").clicked() {
                    let v = (state.brush_size as i32 + 2).min(15);
                    state.brush_size = if v % 2 == 0 { (v + 1) as u32 } else { v as u32 };
                }

                // Size presets
                for &preset in &[1u32, 3, 5, 7, 9, 11] {
                    preset_btn(ui, &mut state.brush_size, preset);
                }

                separator(ui);

                // Shape toggles
                option_label(ui, "Shape");
                shape_toggle(
                    ui,
                    &mut state.brush_shape,
                    BrushShape::Square,
                    "■",
                    "Square",
                );
                shape_toggle(
                    ui,
                    &mut state.brush_shape,
                    BrushShape::Circle,
                    "●",
                    "Circle",
                );

                // Eraser-specific: mode selector
                if state.active_tool == ToolType::Eraser {
                    separator(ui);
                    option_label(ui, "Mode");
                    eraser_mode_btn(
                        ui,
                        &mut state.eraser_mode,
                        EraserMode::TopItem,
                        "Top Item",
                        "Remove topmost item [default]",
                    );
                    eraser_mode_btn(
                        ui,
                        &mut state.eraser_mode,
                        EraserMode::Selective,
                        "Selective",
                        "Pick which item to delete",
                    );
                    eraser_mode_btn(
                        ui,
                        &mut state.eraser_mode,
                        EraserMode::FullTile,
                        "Full Tile",
                        "Clear entire tile",
                    );
                }
            }

            ToolType::Door => {
                option_label(ui, "Variant");
                for variant in [
                    DoorVariant::Normal,
                    DoorVariant::Locked,
                    DoorVariant::Quest,
                    DoorVariant::Magic,
                ] {
                    let selected = state.door_variant == variant;
                    let btn =
                        egui::Button::new(egui::RichText::new(variant.label()).size(10.0).color(
                            if selected {
                                egui::Color32::WHITE
                            } else {
                                theme::TEXT_PRIMARY
                            },
                        ))
                        .fill(if selected {
                            theme::ACCENT
                        } else {
                            theme::BG_RAISED
                        })
                        .corner_radius(egui::CornerRadius::same(3))
                        .min_size(egui::vec2(0.0, SMALL_BTN_H));
                    if ui.add(btn).clicked() {
                        state.door_variant = variant;
                    }
                }
            }
            ToolType::Spawn => {
                option_label(ui, "Radius");
                let mut r = state.spawn_radius as i32;
                if ui
                    .add(egui::DragValue::new(&mut r).range(1..=15).speed(0.1))
                    .changed()
                {
                    state.spawn_radius = r.clamp(1, 15) as u8;
                }
            }
            ToolType::Waypoint => {
                option_label(ui, "Name");
                ui.add(egui::TextEdit::singleline(&mut state.waypoint_name).desired_width(140.0));
            }
            ToolType::Fill => {
                hint_label(ui, "Click a tile to flood-fill with the selected brush");
            }
            ToolType::Eyedropper => {
                hint_label(ui, "Click a tile to pick up its top item as your brush");
            }
            ToolType::Select => {
                hint_label(ui, "Click and drag to select a tile region");
            }
            ToolType::Creature => {
                hint_label(ui, "Click to place a creature from the creatures palette");
            }
        }
    });

    action
}

// ── Painted vector tool button ─────────────────────────────────

fn icon_tool_button(ui: &mut egui::Ui, active: &mut ToolType, tool: ToolType) {
    let selected = *active == tool;
    let icon_char = tool_icon(tool);

    let fill = if selected {
        theme::TOOL_ACTIVE_BG
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, theme::ACCENT_HOVER)
    } else {
        egui::Stroke::NONE
    };

    // Allocate a square button area
    let size = egui::vec2(TOOL_BTN_SIZE, TOOL_BTN_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    // Background
    if response.hovered() && !selected {
        ui.painter().rect_filled(rect, 4.0, theme::BG_RAISED);
    }
    if selected {
        ui.painter().rect_filled(rect, 4.0, fill);
        ui.painter()
            .rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Outside);
    }

    // Render Lucide icon
    let icon_color = if selected {
        egui::Color32::WHITE
    } else if response.hovered() {
        theme::ACCENT
    } else {
        theme::TEXT_PRIMARY
    };

    let galley = ui.painter().layout_no_wrap(
        icon_char.to_string(),
        egui::FontId::new(
            TOOL_ICON_SIZE,
            egui::FontFamily::Name(icons::FONT_FAMILY.into()),
        ),
        icon_color,
    );
    let text_pos = rect.center() - galley.size() / 2.0;
    ui.painter().galley(text_pos, galley, icon_color);

    // Tooltip
    response
        .clone()
        .on_hover_text(format!("{} [{}]", tool.label(), tool.hotkey()));

    if response.clicked() {
        *active = tool;
    }
}

/// Map each tool type to its Lucide icon codepoint.
fn tool_icon(tool: ToolType) -> char {
    match tool {
        ToolType::Brush => icons::PAINTBRUSH,
        ToolType::Eraser => icons::ERASER,
        ToolType::Fill => icons::PAINT_BUCKET,
        ToolType::Eyedropper => icons::PIPETTE,
        ToolType::Select => icons::SQUARE_DASHED,
        ToolType::Door => icons::DOOR_OPEN,
        ToolType::Creature => icons::PAW_PRINT,
        ToolType::Spawn => icons::CROSSHAIR,
        ToolType::Waypoint => icons::MAP_PIN,
    }
}

// ── Eraser mode toggle button ─────────────────────────────────────

fn eraser_mode_btn(
    ui: &mut egui::Ui,
    current: &mut EraserMode,
    mode: EraserMode,
    label: &str,
    tooltip: &str,
) {
    let selected = *current == mode;
    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        theme::TEXT_SECONDARY
    };
    let bg = if selected {
        theme::ERROR
    } else {
        theme::BG_RAISED
    };

    let btn = egui::Button::new(egui::RichText::new(label).size(10.0).color(text_color))
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(3))
        .min_size(egui::vec2(0.0, SMALL_BTN_H));

    if ui.add(btn).on_hover_text(tooltip).clicked() {
        *current = mode;
    }
}

// ── Small helpers ─────────────────────────────────────────────────

fn lucide_btn(ui: &mut egui::Ui, icon_char: char, enabled: bool, tooltip: &str) -> egui::Response {
    let color = if enabled {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    let btn = egui::Button::new(icons::icon_colored(icon_char, 14.0, color))
        .min_size(egui::vec2(24.0, TOOL_BTN_SIZE));
    ui.add_enabled(enabled, btn).on_hover_text(tooltip)
}

#[allow(dead_code)]
fn compact_icon_btn(ui: &mut egui::Ui, icon: &str, enabled: bool, tooltip: &str) -> egui::Response {
    let color = if enabled {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    let btn = egui::Button::new(egui::RichText::new(icon).size(13.0).color(color))
        .min_size(egui::vec2(24.0, TOOL_BTN_SIZE));
    ui.add_enabled(enabled, btn).on_hover_text(tooltip)
}

fn separator(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let rect = ui.available_rect_before_wrap();
    let x = rect.left();
    let top = rect.top() + 4.0;
    let bot = rect.bottom() - 4.0;
    ui.painter().line_segment(
        [egui::pos2(x, top), egui::pos2(x, bot)],
        egui::Stroke::new(1.0, theme::BORDER),
    );
    ui.add_space(4.0);
}

fn option_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(OPTION_LABEL_SIZE)
            .color(theme::TEXT_MUTED)
            .strong(),
    );
}

fn hint_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(OPTION_LABEL_SIZE)
            .color(theme::TEXT_MUTED)
            .italics(),
    );
}

fn small_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(11.0))
            .min_size(egui::vec2(SMALL_BTN_H, SMALL_BTN_H - 2.0)),
    )
}

fn preset_btn(ui: &mut egui::Ui, current: &mut u32, preset: u32) {
    let selected = *current == preset;
    let btn = egui::Button::new(egui::RichText::new(format!("{}", preset)).size(9.5).color(
        if selected {
            egui::Color32::WHITE
        } else {
            theme::TEXT_SECONDARY
        },
    ))
    .fill(if selected {
        theme::ACCENT
    } else {
        theme::BG_RAISED
    })
    .corner_radius(egui::CornerRadius::same(2))
    .min_size(egui::vec2(PRESET_BTN_SIZE, SMALL_BTN_H - 2.0));
    if ui.add(btn).clicked() {
        *current = preset;
    }
}

fn shape_toggle(
    ui: &mut egui::Ui,
    current: &mut BrushShape,
    shape: BrushShape,
    icon: &str,
    tooltip: &str,
) {
    let selected = *current == shape;
    let btn = egui::Button::new(egui::RichText::new(icon).size(12.0).color(if selected {
        egui::Color32::WHITE
    } else {
        theme::TEXT_SECONDARY
    }))
    .fill(if selected {
        theme::ACCENT
    } else {
        theme::BG_RAISED
    })
    .corner_radius(egui::CornerRadius::same(3))
    .min_size(egui::vec2(22.0, SMALL_BTN_H));
    if ui.add(btn).on_hover_text(tooltip).clicked() {
        *current = shape;
    }
}

/// Show active zone brush / house brush markers.
fn show_active_markers(ui: &mut egui::Ui, state: &mut EditorState) {
    if let Some(zone) = state.active_zone_flag {
        let badge = egui::RichText::new(format!(" {} ", zone.label()))
            .size(9.5)
            .color(egui::Color32::WHITE)
            .strong()
            .background_color(zone.color());
        ui.label(badge);
        if ui
            .add(
                egui::Button::new(egui::RichText::new("✕").size(9.0))
                    .min_size(egui::vec2(16.0, 16.0)),
            )
            .on_hover_text("Clear zone brush")
            .clicked()
        {
            state.active_zone_flag = None;
        }
        separator(ui);
    }

    if let Some(hid) = state.active_house_id {
        let badge = egui::RichText::new(format!(" House #{} ", hid))
            .size(9.5)
            .color(egui::Color32::WHITE)
            .strong()
            .background_color(egui::Color32::from_rgb(120, 80, 200));
        ui.label(badge);
        if ui
            .add(
                egui::Button::new(egui::RichText::new("✕").size(9.0))
                    .min_size(egui::vec2(16.0, 16.0)),
            )
            .on_hover_text("Clear house brush")
            .clicked()
        {
            state.active_house_id = None;
        }
        separator(ui);
    }
}
