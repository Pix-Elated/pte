//! Tile properties panel — inspect the hovered tile.
//! Each item on the tile is shown as an interactive row: click to select (eyedrop),
//! click the × button to delete just that sub-layer.

use egui::Color32;

use crate::state::EditorState;
use crate::theme;

/// Action returned from the properties panel for the parent to handle.
#[derive(Debug, Clone)]
pub enum PropertiesAction {
    None,
    /// Delete a specific item at (x, y, z, item_index). index=usize::MAX means ground.
    DeleteItem {
        x: u16,
        y: u16,
        z: u8,
        index: usize,
    },
    /// Select/eyedrop item_id
    SelectItem {
        item_id: u32,
    },
}

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) -> PropertiesAction {
    let mut action = PropertiesAction::None;

    let (tx, ty) = match state.hover_tile {
        Some(pos) => pos,
        None => {
            ui.label(
                egui::RichText::new("Hover a tile to inspect")
                    .size(11.0)
                    .color(theme::TEXT_MUTED),
            );
            return action;
        }
    };
    let z = state.camera.z_level;

    ui.label(
        egui::RichText::new(format!("{}, {}, {}", tx, ty, z))
            .size(11.0)
            .color(theme::TEXT_SECONDARY)
            .strong(),
    );
    ui.add_space(4.0);

    let map = match &state.map_data {
        Some(m) => m,
        None => return action,
    };

    match map.get_tile(tx, ty, z) {
        Some(tile) => {
            // Ground layer
            if let Some(ground_id) = tile.ground {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Ground")
                            .size(10.5)
                            .color(theme::TEXT_MUTED),
                    );
                    let ground_btn = egui::Button::new(
                        egui::RichText::new(format!("#{}", ground_id))
                            .size(10.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(Color32::TRANSPARENT)
                    .min_size(egui::vec2(0.0, 16.0));
                    if ui
                        .add(ground_btn)
                        .on_hover_text("Click to select")
                        .clicked()
                    {
                        action = PropertiesAction::SelectItem {
                            item_id: ground_id as u32,
                        };
                    }

                    // Delete ground
                    let del =
                        egui::Button::new(egui::RichText::new("×").size(12.0).color(theme::ERROR))
                            .fill(Color32::TRANSPARENT)
                            .min_size(egui::vec2(16.0, 16.0));
                    if ui.add(del).on_hover_text("Delete ground").clicked() {
                        action = PropertiesAction::DeleteItem {
                            x: tx,
                            y: ty,
                            z,
                            index: usize::MAX,
                        };
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new("Ground: —")
                        .size(10.5)
                        .color(theme::TEXT_MUTED),
                );
            }

            // Item stack — each item as interactive row
            if tile.items.is_empty() {
                ui.label(
                    egui::RichText::new("Items: —")
                        .size(10.5)
                        .color(theme::TEXT_MUTED),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!("Items ({})", tile.items.len()))
                        .size(10.5)
                        .color(theme::TEXT_MUTED),
                );
                for (i, item) in tile.items.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Stack position indicator
                        ui.label(
                            egui::RichText::new(format!(" [{}]", i))
                                .size(9.5)
                                .color(theme::TEXT_MUTED),
                        );

                        // Item button — click to select
                        let mut label = format!("#{}", item.id);
                        if let Some(aid) = item.action_id {
                            label.push_str(&format!(" aid={}", aid));
                        }
                        if let Some(uid) = item.unique_id {
                            label.push_str(&format!(" uid={}", uid));
                        }

                        let item_btn = egui::Button::new(
                            egui::RichText::new(&label)
                                .size(10.0)
                                .color(theme::TEXT_SECONDARY),
                        )
                        .fill(Color32::TRANSPARENT)
                        .min_size(egui::vec2(0.0, 16.0));
                        if ui
                            .add(item_btn)
                            .on_hover_text("Click to select item")
                            .clicked()
                        {
                            action = PropertiesAction::SelectItem {
                                item_id: item.id as u32,
                            };
                        }

                        // Delete button for this specific item
                        let del = egui::Button::new(
                            egui::RichText::new("×").size(12.0).color(theme::ERROR),
                        )
                        .fill(Color32::TRANSPARENT)
                        .min_size(egui::vec2(16.0, 16.0));
                        if ui.add(del).on_hover_text("Delete this item").clicked() {
                            action = PropertiesAction::DeleteItem {
                                x: tx,
                                y: ty,
                                z,
                                index: i,
                            };
                        }
                    });
                }
            }

            // Flags
            let f = &tile.flags;
            let flags: Vec<&str> = [
                (f.protection_zone, "PZ"),
                (f.no_pvp, "NoPvP"),
                (f.no_logout, "NoLogout"),
                (f.pvp_zone, "PvP"),
                (f.refresh, "Refresh"),
            ]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, s)| *s)
            .collect();

            if !flags.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Flags")
                            .size(10.5)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(flags.join(", "))
                            .size(10.5)
                            .color(theme::WARNING),
                    );
                });
            }

            if let Some(hid) = tile.house_id {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("House")
                            .size(10.5)
                            .color(theme::TEXT_MUTED),
                    );
                    // Look up house name from map
                    let name = state
                        .map_data
                        .as_ref()
                        .and_then(|m| m.houses.iter().find(|h| h.id == hid))
                        .map(|h| h.name.as_str())
                        .unwrap_or("(unknown)");
                    let btn = egui::Button::new(
                        egui::RichText::new(format!("#{} {}", hid, name))
                            .size(10.5)
                            .color(theme::ACCENT),
                    )
                    .fill(Color32::TRANSPARENT)
                    .min_size(egui::vec2(0.0, 16.0));
                    if ui
                        .add(btn)
                        .on_hover_text("Click to select as house brush")
                        .clicked()
                    {
                        state.active_house_id = Some(hid);
                    }
                });
            }
        }
        None => {
            ui.label(
                egui::RichText::new("Empty")
                    .size(10.5)
                    .color(theme::TEXT_MUTED),
            );
        }
    }

    action
}
