//! Ribbon toolbar — two-row tool strip with icons, brush options, undo/redo, zoom.

use crate::brushes::door::DoorVariant;
use crate::brushes::shape::BrushShape;
use crate::state::{EditorState, ToolType};
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

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    // ── Row 1: Tools + Undo/Redo + Zoom ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;

        // ── Primary tools group ──
        group_label(ui, "TOOLS");
        for tool in [
            ToolType::Brush,
            ToolType::Eraser,
            ToolType::Fill,
            ToolType::Select,
            ToolType::Eyedropper,
        ] {
            tool_button(ui, &mut state.active_tool, tool);
        }

        ui.add_space(4.0);
        thin_separator(ui);
        ui.add_space(4.0);

        // ── Placement tools group ──
        let has_doors = !state.brush_registry.brushes_of_type(crate::brushes::BrushType::Door).is_empty();
        let has_creatures = !state.brush_registry.brushes_of_type(crate::brushes::BrushType::Creature).is_empty();
        let has_spawns = !state.brush_registry.brushes_of_type(crate::brushes::BrushType::Spawn).is_empty();
        let has_waypoints = !state.brush_registry.brushes_of_type(crate::brushes::BrushType::Waypoint).is_empty();

        if has_doors || has_creatures || has_spawns || has_waypoints {
            group_label(ui, "PLACE");
            if has_doors { tool_button(ui, &mut state.active_tool, ToolType::Door); }
            if has_creatures { tool_button(ui, &mut state.active_tool, ToolType::Creature); }
            if has_spawns { tool_button(ui, &mut state.active_tool, ToolType::Spawn); }
            if has_waypoints { tool_button(ui, &mut state.active_tool, ToolType::Waypoint); }

            ui.add_space(4.0);
            thin_separator(ui);
            ui.add_space(4.0);
        }

        // ── Undo / Redo group ──
        group_label(ui, "HISTORY");

        let undo_btn = icon_btn(ui, "↩", state.can_undo(), "Undo [Ctrl+Z]");
        if undo_btn.clicked() { action = ToolbarAction::Undo; }

        let redo_btn = icon_btn(ui, "↪", state.can_redo(), "Redo [Ctrl+Y]");
        if redo_btn.clicked() { action = ToolbarAction::Redo; }

        ui.add_space(4.0);
        thin_separator(ui);
        ui.add_space(4.0);

        // ── Zoom group ──
        group_label(ui, "ZOOM");
        if icon_btn(ui, "−", true, "Zoom Out").clicked() { action = ToolbarAction::ZoomOut; }
        ui.label(
            egui::RichText::new(format!("{:.0}%", state.camera.zoom * 100.0))
                .size(10.0)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        );
        if icon_btn(ui, "+", true, "Zoom In").clicked() { action = ToolbarAction::ZoomIn; }
        if icon_btn(ui, "⊙", true, "Reset Zoom [1:1]").clicked() { action = ToolbarAction::ZoomReset; }
        if icon_btn(ui, "⊞", true, "Fit Map").clicked() { action = ToolbarAction::FitToMap; }

        // ── Right-aligned: active brush indicator ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(brush_id) = state.active_brush {
                if let Some(brush) = state.brush_registry.get(brush_id) {
                    ui.label(
                        egui::RichText::new(format!("▸ {}", brush.name()))
                            .size(11.0)
                            .color(theme::ACCENT),
                    );
                }
            } else if let Some(id) = state.selected_item_id {
                ui.label(
                    egui::RichText::new(format!("▸ Item #{}", id))
                        .size(11.0)
                        .color(theme::ACCENT),
                );
            }

            let count = state.brush_registry.count();
            if count > 0 {
                ui.label(
                    egui::RichText::new(format!("[{} brushes]", count))
                        .size(9.5)
                        .color(theme::TEXT_MUTED),
                );
            }
        });
    });

    // ── Row 2: Context-sensitive options ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        match state.active_tool {
            ToolType::Brush | ToolType::Eraser => {
                // Zone brush indicator (if active)
                if let Some(zone) = state.active_zone_flag {
                    ui.label(
                        egui::RichText::new(format!("ZONE: {}", zone.label()))
                            .size(10.0)
                            .color(egui::Color32::WHITE)
                            .strong()
                            .background_color(zone.color()),
                    );
                    if ui.add(
                        egui::Button::new(egui::RichText::new("✕").size(10.0))
                            .min_size(egui::vec2(18.0, 16.0))
                    ).on_hover_text("Clear zone brush").clicked() {
                        state.active_zone_flag = None;
                    }
                    ui.add_space(6.0);
                    thin_separator(ui);
                    ui.add_space(4.0);
                }

                // House brush indicator (if active)
                if let Some(hid) = state.active_house_id {
                    ui.label(
                        egui::RichText::new(format!("HOUSE: #{}", hid))
                            .size(10.0)
                            .color(egui::Color32::WHITE)
                            .strong()
                            .background_color(egui::Color32::from_rgb(120, 80, 200)),
                    );
                    if ui.add(
                        egui::Button::new(egui::RichText::new("✕").size(10.0))
                            .min_size(egui::vec2(18.0, 16.0))
                    ).on_hover_text("Clear house brush").clicked() {
                        state.active_house_id = None;
                    }
                    ui.add_space(6.0);
                    thin_separator(ui);
                    ui.add_space(4.0);
                }

                // Brush size with visual + and – buttons
                ui.label(egui::RichText::new("Size").size(10.0).color(theme::TEXT_MUTED));

                if ui.add(
                    egui::Button::new(egui::RichText::new("−").size(11.0))
                        .min_size(egui::vec2(20.0, 18.0))
                ).clicked() {
                    let cur = state.brush_size as i32;
                    let new = (cur - 2).max(1);
                    state.brush_size = if new % 2 == 0 { (new + 1) as u32 } else { new as u32 };
                }

                ui.label(
                    egui::RichText::new(format!("{}", state.brush_size))
                        .size(12.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                );

                if ui.add(
                    egui::Button::new(egui::RichText::new("+").size(11.0))
                        .min_size(egui::vec2(20.0, 18.0))
                ).clicked() {
                    let cur = state.brush_size as i32;
                    let new = (cur + 2).min(15);
                    state.brush_size = if new % 2 == 0 { (new + 1) as u32 } else { new as u32 };
                }

                // Discrete size presets
                ui.add_space(4.0);
                for &preset in &[1u32, 3, 5, 7, 9, 11] {
                    let selected = state.brush_size == preset;
                    let btn = egui::Button::new(
                        egui::RichText::new(format!("{}", preset))
                            .size(9.5)
                            .color(if selected { egui::Color32::WHITE } else { theme::TEXT_SECONDARY }),
                    )
                    .fill(if selected { theme::ACCENT } else { theme::BG_RAISED })
                    .corner_radius(egui::CornerRadius::same(2))
                    .min_size(egui::vec2(20.0, 16.0));
                    if ui.add(btn).clicked() {
                        state.brush_size = preset;
                    }
                }

                ui.add_space(6.0);
                thin_separator(ui);
                ui.add_space(4.0);

                // Shape toggles
                ui.label(egui::RichText::new("Shape").size(10.0).color(theme::TEXT_MUTED));

                let sq_sel = state.brush_shape == BrushShape::Square;
                let sq_btn = egui::Button::new(
                    egui::RichText::new("■")
                        .size(12.0)
                        .color(if sq_sel { egui::Color32::WHITE } else { theme::TEXT_SECONDARY }),
                )
                .fill(if sq_sel { theme::ACCENT } else { theme::BG_RAISED })
                .corner_radius(egui::CornerRadius::same(2))
                .min_size(egui::vec2(22.0, 18.0));
                if ui.add(sq_btn).on_hover_text("Square brush").clicked() {
                    state.brush_shape = BrushShape::Square;
                }

                let ci_sel = state.brush_shape == BrushShape::Circle;
                let ci_btn = egui::Button::new(
                    egui::RichText::new("●")
                        .size(12.0)
                        .color(if ci_sel { egui::Color32::WHITE } else { theme::TEXT_SECONDARY }),
                )
                .fill(if ci_sel { theme::ACCENT } else { theme::BG_RAISED })
                .corner_radius(egui::CornerRadius::same(2))
                .min_size(egui::vec2(22.0, 18.0));
                if ui.add(ci_btn).on_hover_text("Circle brush").clicked() {
                    state.brush_shape = BrushShape::Circle;
                }

                // Mode indicator for eraser
                if state.active_tool == ToolType::Eraser {
                    ui.add_space(6.0);
                    thin_separator(ui);
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("ERASER")
                            .size(10.0)
                            .color(theme::ERROR)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Shift+click = erase only items")
                            .size(9.5)
                            .color(theme::TEXT_MUTED),
                    );
                }
            }

            ToolType::Door => {
                ui.label(egui::RichText::new("Variant").size(10.0).color(theme::TEXT_MUTED));
                for variant in [DoorVariant::Normal, DoorVariant::Locked, DoorVariant::Quest, DoorVariant::Magic] {
                    let selected = state.door_variant == variant;
                    let btn = egui::Button::new(
                        egui::RichText::new(variant.label())
                            .size(10.0)
                            .color(if selected { egui::Color32::WHITE } else { theme::TEXT_PRIMARY }),
                    )
                    .fill(if selected { theme::ACCENT } else { theme::BG_RAISED })
                    .corner_radius(egui::CornerRadius::same(2))
                    .min_size(egui::vec2(0.0, 18.0));
                    if ui.add(btn).clicked() {
                        state.door_variant = variant;
                    }
                }
            }
            ToolType::Spawn => {
                ui.label(egui::RichText::new("Radius").size(10.0).color(theme::TEXT_MUTED));
                let mut r = state.spawn_radius as i32;
                if ui.add(
                    egui::DragValue::new(&mut r)
                        .range(1..=15)
                        .speed(0.1)
                ).changed() {
                    state.spawn_radius = r.clamp(1, 15) as u8;
                }
            }
            ToolType::Waypoint => {
                ui.label(egui::RichText::new("Name").size(10.0).color(theme::TEXT_MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut state.waypoint_name)
                        .desired_width(120.0)
                );
            }
            ToolType::Fill => {
                ui.label(
                    egui::RichText::new("Click a tile to flood-fill with the selected item")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            }
            ToolType::Eyedropper => {
                ui.label(
                    egui::RichText::new("Click a tile to pick up its top item")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            }
            ToolType::Select => {
                ui.label(
                    egui::RichText::new("Click and drag to select tiles")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            }
            ToolType::Creature => {
                ui.label(
                    egui::RichText::new("Click to place creature")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            }
        }
    });

    action
}

fn tool_button(ui: &mut egui::Ui, active: &mut ToolType, tool: ToolType) {
    let selected = *active == tool;
    let icon = tool_icon(tool);
    let label = tool.label();

    let btn = egui::Button::new(
        egui::RichText::new(format!("{} {}", icon, label))
            .size(11.0)
            .color(if selected { egui::Color32::WHITE } else { theme::TEXT_PRIMARY }),
    )
    .fill(if selected { theme::TOOL_ACTIVE_BG } else { egui::Color32::TRANSPARENT })
    .stroke(if selected {
        egui::Stroke::new(1.0, theme::ACCENT_HOVER)
    } else {
        egui::Stroke::NONE
    })
    .corner_radius(egui::CornerRadius::same(4))
    .min_size(egui::vec2(0.0, 26.0));

    if ui.add(btn)
        .on_hover_text(format!("{} [{}]", label, tool.hotkey()))
        .clicked()
    {
        *active = tool;
    }
}

fn tool_icon(tool: ToolType) -> &'static str {
    match tool {
        ToolType::Brush => "🖌",
        ToolType::Eraser => "⌫",
        ToolType::Fill => "🪣",
        ToolType::Select => "⬚",
        ToolType::Eyedropper => "💧",
        ToolType::Door => "🚪",
        ToolType::Creature => "🐾",
        ToolType::Spawn => "⊕",
        ToolType::Waypoint => "📍",
    }
}

fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(8.5)
            .color(theme::TEXT_MUTED)
            .strong(),
    );
    ui.add_space(2.0);
}

fn icon_btn(ui: &mut egui::Ui, icon: &str, enabled: bool, tooltip: &str) -> egui::Response {
    let btn = egui::Button::new(
        egui::RichText::new(icon)
            .size(13.0)
            .color(if enabled { theme::TEXT_PRIMARY } else { theme::TEXT_MUTED }),
    )
    .min_size(egui::vec2(24.0, 22.0));
    ui.add_enabled(enabled, btn).on_hover_text(tooltip)
}

/// Thin vertical separator.
fn thin_separator(ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    let x = rect.left();
    let top = rect.top() + 2.0;
    let bot = rect.bottom() - 2.0;
    ui.painter().line_segment(
        [egui::pos2(x, top), egui::pos2(x, bot)],
        egui::Stroke::new(0.5, theme::BORDER),
    );
    ui.add_space(1.0);
}
