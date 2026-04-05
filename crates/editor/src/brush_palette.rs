//! Brush palette panel — sidebar for selecting brushes by type/category.

use egui::Color32;
use crate::brushes::BrushType;
use crate::state::EditorState;
use crate::theme;

const BRUSH_TABS: &[BrushType] = &[
    BrushType::Ground,
    BrushType::Wall,
    BrushType::Door,
    BrushType::Doodad,
    BrushType::Table,
    BrushType::Carpet,
    BrushType::Raw,
    BrushType::Flag,
    BrushType::Creature,
    BrushType::Spawn,
    BrushType::Waypoint,
];

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) {
    if state.brush_registry.count() == 0 {
        ui.label(
            egui::RichText::new("No brushes loaded")
                .size(11.0)
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("File > Load Materials…")
                .size(10.0)
                .color(theme::TEXT_MUTED),
        );
        return;
    }

    // Type filter tabs
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
        for &bt in BRUSH_TABS {
            let count = state.brush_registry.brushes_of_type(bt).len();
            if count == 0 { continue; }
            let selected = state.brush_palette_filter == Some(bt);
            let text = format!("{} {}", bt.label(), count);
            let btn = egui::Button::new(
                egui::RichText::new(&text)
                    .size(10.0)
                    .color(if selected { Color32::WHITE } else { theme::TEXT_SECONDARY }),
            )
            .fill(if selected { theme::ACCENT } else { theme::BG_RAISED })
            .corner_radius(egui::CornerRadius::same(3))
            .min_size(egui::vec2(0.0, 20.0));
            if ui.add(btn).clicked() {
                state.brush_palette_filter = Some(bt);
            }
        }
    });

    ui.add_space(4.0);

    // Tilesets or flat list
    let tilesets = state.brush_registry.tilesets();
    if !tilesets.is_empty() {
        show_tilesets(ui, state);
        return;
    }

    let filter = state.brush_palette_filter.unwrap_or(BrushType::Ground);
    let brushes = state.brush_registry.brushes_of_type(filter);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let active = state.active_brush;
            for brush in &brushes {
                let is_active = active == Some(brush.id());
                let name = brush.name().to_string();
                let look = brush.look_id();

                ui.horizontal(|ui| {
                    // Sprite preview
                    let preview_size = egui::vec2(22.0, 22.0);
                    let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
                    if let Some(tex) = crate::viewport::get_or_upload(&mut state.sprite_textures, &state.sprite_sheets, ui.ctx(), look as u32) {
                        ui.painter().image(
                            tex.id(), rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        let hue = (look as f32 * 137.508) % 360.0;
                        let c = Color32::from_rgb(
                            (((hue / 360.0) * 200.0) as u8).wrapping_add(55), 80, 120,
                        );
                        ui.painter().rect_filled(rect, 2.0, c);
                    }

                    let text_color = if is_active { Color32::WHITE } else { theme::TEXT_PRIMARY };
                    let btn = egui::Button::new(
                        egui::RichText::new(&name).size(11.0).color(text_color),
                    )
                    .fill(if is_active { theme::ACCENT } else { Color32::TRANSPARENT })
                    .corner_radius(egui::CornerRadius::same(2));

                    if ui.add(btn).clicked() {
                        if is_active {
                            // Toggle off — deselect
                            state.active_brush = None;
                            state.selected_item_id = None;
                        } else {
                            state.active_brush = Some(brush.id());
                            state.selected_item_id = Some(look as u32);
                        }
                    }
                });
            }
        });
}

fn show_tilesets(ui: &mut egui::Ui, state: &mut EditorState) {
    let filter = state.brush_palette_filter;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let tilesets: Vec<_> = state.brush_registry.tilesets().to_vec();

            for tileset in &tilesets {
                ui.collapsing(&tileset.name, |ui| {
                    for palette in &tileset.palettes {
                        ui.collapsing(&palette.name, |ui| {
                            for &brush_id in &palette.brushes {
                                if let Some(brush) = state.brush_registry.get(brush_id) {
                                    if let Some(f) = filter {
                                        if brush.brush_type() != f { continue; }
                                    }

                                    let is_active = state.active_brush == Some(brush_id);
                                    let name = brush.name().to_string();
                                    let look = brush.look_id();

                                    let text_color = if is_active { Color32::WHITE } else { theme::TEXT_PRIMARY };
                                    let btn = egui::Button::new(
                                        egui::RichText::new(&name).size(11.0).color(text_color),
                                    )
                                    .fill(if is_active { theme::ACCENT } else { Color32::TRANSPARENT });

                                    if ui.add(btn).clicked() {
                                        if is_active {
                                            state.active_brush = None;
                                            state.selected_item_id = None;
                                        } else {
                                            state.active_brush = Some(brush_id);
                                            state.selected_item_id = Some(look as u32);
                                        }
                                    }
                                }
                            }
                        });
                    }
                });
            }
        });
}
