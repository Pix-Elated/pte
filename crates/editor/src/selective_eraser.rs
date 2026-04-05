//! Selective eraser dialog — lists all items on a tile for targeted removal.

use crate::state::EditorState;
use crate::theme;

/// Show the selective eraser popup. Returns true if an item was deleted.
pub fn show(ctx: &egui::Context, state: &mut EditorState) -> bool {
    if !state.selective_eraser.open {
        return false;
    }

    let mut deleted = false;
    let mut open = state.selective_eraser.open;

    egui::Window::new("Selective Eraser")
        .collapsible(false)
        .resizable(false)
        .default_width(260.0)
        .open(&mut open)
        .show(ctx, |ui| {
            let tx = state.selective_eraser.tile_x;
            let ty = state.selective_eraser.tile_y;
            let tz = state.selective_eraser.tile_z;

            ui.label(
                egui::RichText::new(format!("Tile ({}, {}, {})", tx, ty, tz))
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Ground
            if state.selective_eraser.has_ground {
                ui.horizontal(|ui| {
                    // Preview sprite
                    let preview_rect = draw_item_preview(
                        ui, state.selective_eraser.ground_id,
                        &state.appearances, &mut state.sprite_textures,
                        &state.sprite_sheets, ctx,
                    );
                    let _ = preview_rect;

                    ui.label(
                        egui::RichText::new(format!("Ground #{}", state.selective_eraser.ground_id))
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(
                                egui::RichText::new("✕ Delete")
                                    .size(10.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(theme::ERROR)
                            .corner_radius(egui::CornerRadius::same(3))
                            .min_size(egui::vec2(60.0, 20.0))
                        ).clicked() {
                            if let Some(ref mut map) = state.map_data {
                                let action = crate::tools::eraser::erase_specific(
                                    map, tx, ty, tz, None, true,
                                );
                                state.undo_stack.truncate(state.undo_cursor);
                                state.undo_stack.push(action);
                                state.undo_cursor = state.undo_stack.len();
                            }
                            deleted = true;
                            state.selective_eraser.open = false;
                        }
                    });
                });
                ui.add_space(2.0);
            }

            // Items (bottom-to-top, displayed top-to-bottom so topmost is first)
            let items: Vec<_> = state.selective_eraser.items.iter().cloned().collect();
            for (display_idx, (item_id, desc)) in items.iter().enumerate().rev() {
                ui.horizontal(|ui| {
                    draw_item_preview(
                        ui, *item_id,
                        &state.appearances, &mut state.sprite_textures,
                        &state.sprite_sheets, ctx,
                    );

                    ui.label(
                        egui::RichText::new(desc)
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(
                                egui::RichText::new("✕ Delete")
                                    .size(10.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(theme::ERROR)
                            .corner_radius(egui::CornerRadius::same(3))
                            .min_size(egui::vec2(60.0, 20.0))
                        ).clicked() {
                            if let Some(ref mut map) = state.map_data {
                                let action = crate::tools::eraser::erase_specific(
                                    map,
                                    state.selective_eraser.tile_x,
                                    state.selective_eraser.tile_y,
                                    state.selective_eraser.tile_z,
                                    Some(display_idx),
                                    false,
                                );
                                state.undo_stack.truncate(state.undo_cursor);
                                state.undo_stack.push(action);
                                state.undo_cursor = state.undo_stack.len();
                            }
                            deleted = true;
                            state.selective_eraser.open = false;
                        }
                    });
                });
                ui.add_space(2.0);
            }

            if !state.selective_eraser.has_ground && items.is_empty() {
                ui.label(
                    egui::RichText::new("Empty tile")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
            }
        });

    if !open {
        state.selective_eraser.open = false;
    }

    deleted
}

/// Draw a small sprite preview and return the rect.
fn draw_item_preview(
    ui: &mut egui::Ui,
    item_id: u32,
    appearances: &Option<pte_appearances::LoadedAppearances>,
    textures: &mut std::collections::HashMap<u32, egui::TextureHandle>,
    sheets: &std::collections::HashMap<String, pte_assets::SpriteSheet>,
    ctx: &egui::Context,
) -> egui::Rect {
    let size = egui::vec2(24.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

    if let Some(ref apps) = appearances {
        if let Some(appearance) = apps.get(pte_appearances::Category::Object, item_id) {
            if let Some(sid) = crate::viewport::resolve_appearance_sprite(appearance, 0, false, 0) {
                if let Some(tex) = crate::viewport::get_or_upload(textures, sheets, ctx, sid) {
                    ui.painter().image(
                        tex.id(), rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                    return rect;
                }
            }
        }
    }

    // Fallback
    ui.painter().rect_filled(rect, 2.0, theme::BG_RAISED);
    rect
}
