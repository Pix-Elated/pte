//! Tile stack reorder dialog — view and reorder items on a specific tile.

use crate::state::EditorState;
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_tile_stack {
        return;
    }

    let mut open = true;

    egui::Window::new("Tile Stack")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([300.0, 400.0])
        .show(ctx, |ui| {
            let Some((tx, ty)) = state.hover_tile else {
                ui.label(egui::RichText::new("Hover a tile to view its stack").color(theme::TEXT_MUTED));
                return;
            };
            let z = state.camera.z_level;

            let Some(ref mut map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            let tile = match map.get_tile_mut_if_exists(tx, ty, z) {
                Some(t) => t,
                None => {
                    ui.label(egui::RichText::new("Empty tile").color(theme::TEXT_MUTED));
                    return;
                }
            };

            ui.label(
                egui::RichText::new(format!("Tile: {}, {}, {}", tx, ty, z))
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY)
                    .strong(),
            );

            // Ground
            if let Some(gid) = tile.ground {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⬛").size(12.0));
                    ui.label(
                        egui::RichText::new(format!("Ground: #{}", gid))
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                });
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            if tile.items.is_empty() {
                ui.label(egui::RichText::new("No items").size(10.0).color(theme::TEXT_MUTED));
            } else {
                let mut swap: Option<(usize, usize)> = None;

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let item_count = tile.items.len();
                        for i in 0..item_count {
                            let item = &tile.items[i];
                            ui.push_id(format!("stack_{}", i), |ui| {
                                egui::Frame::NONE
                                    .fill(theme::BG_SURFACE)
                                    .corner_radius(egui::CornerRadius::same(3))
                                    .inner_margin(egui::Margin::same(4))
                                    .outer_margin(egui::Margin::symmetric(0, 1))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(format!("[{}]", i))
                                                    .size(9.5)
                                                    .color(theme::TEXT_MUTED),
                                            );
                                            ui.label(
                                                egui::RichText::new(format!("#{}", item.id))
                                                    .size(10.5)
                                                    .color(theme::TEXT_PRIMARY),
                                            );

                                            if let Some(aid) = item.action_id {
                                                ui.label(
                                                    egui::RichText::new(format!("aid={}", aid))
                                                        .size(9.0)
                                                        .color(theme::TEXT_MUTED),
                                                );
                                            }

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if i + 1 < item_count {
                                                        if ui.small_button("▼")
                                                            .on_hover_text("Move down (higher in stack)")
                                                            .clicked()
                                                        {
                                                            swap = Some((i, i + 1));
                                                        }
                                                    }
                                                    if i > 0 {
                                                        if ui.small_button("▲")
                                                            .on_hover_text("Move up (lower in stack)")
                                                            .clicked()
                                                        {
                                                            swap = Some((i, i - 1));
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                    });
                            });
                        }
                    });

                if let Some((a, b)) = swap {
                    tile.items.swap(a, b);
                }
            }
        });

    if !open {
        state.show_tile_stack = false;
    }
}
