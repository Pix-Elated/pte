//! Item Properties Editor — edit action ID, unique ID, text, teleport destination on selected items.

use crate::state::EditorState;
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_item_props {
        return;
    }

    let mut open = true;

    egui::Window::new("Item Properties")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([340.0, 300.0])
        .show(ctx, |ui| {
            let Some((tx, ty)) = state.hover_tile else {
                ui.label(egui::RichText::new("Hover a tile to edit item properties").color(theme::TEXT_MUTED));
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
            ui.add_space(4.0);

            // House ID
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("House ID:").size(10.0).color(theme::TEXT_MUTED));
                let mut hid = tile.house_id.map(|h| h as i64).unwrap_or(-1);
                if ui.add(
                    egui::DragValue::new(&mut hid).range(-1..=999999).speed(1),
                ).on_hover_text("-1 = no house").changed() {
                    tile.house_id = if hid < 0 { None } else { Some(hid as u32) };
                }
            });

            // Tile flags
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Flags:").size(10.0).color(theme::TEXT_MUTED));
                ui.checkbox(&mut tile.flags.protection_zone, "PZ");
                ui.checkbox(&mut tile.flags.no_pvp, "NoPvP");
                ui.checkbox(&mut tile.flags.pvp_zone, "PvP");
                ui.checkbox(&mut tile.flags.no_logout, "NoLog");
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Items
            if tile.items.is_empty() && tile.ground.is_none() {
                ui.label(egui::RichText::new("No items on this tile").size(10.0).color(theme::TEXT_MUTED));
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Ground
                    if let Some(gid) = tile.ground {
                        ui.group(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Ground: #{}", gid))
                                    .size(11.0)
                                    .color(theme::TEXT_PRIMARY)
                                    .strong(),
                            );
                        });
                    }

                    // Individual items
                    for (idx, item) in tile.items.iter_mut().enumerate() {
                        ui.push_id(format!("item_prop_{idx}"), |ui| {
                            egui::Frame::NONE
                                .fill(theme::BG_SURFACE)
                                .corner_radius(egui::CornerRadius::same(3))
                                .inner_margin(egui::Margin::same(5))
                                .outer_margin(egui::Margin::symmetric(0, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("[{}] Item #{}", idx, item.id))
                                            .size(10.5)
                                            .color(theme::TEXT_PRIMARY)
                                            .strong(),
                                    );

                                    // Action ID
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Action ID:").size(9.5).color(theme::TEXT_MUTED));
                                        let mut aid = item.action_id.map(|a| a as i32).unwrap_or(-1);
                                        if ui.add(
                                            egui::DragValue::new(&mut aid).range(-1..=65535).speed(1),
                                        ).on_hover_text("-1 = none").changed() {
                                            item.action_id = if aid < 0 { None } else { Some(aid as u16) };
                                        }
                                    });

                                    // Unique ID
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Unique ID:").size(9.5).color(theme::TEXT_MUTED));
                                        let mut uid = item.unique_id.map(|u| u as i32).unwrap_or(-1);
                                        if ui.add(
                                            egui::DragValue::new(&mut uid).range(-1..=65535).speed(1),
                                        ).on_hover_text("-1 = none").changed() {
                                            item.unique_id = if uid < 0 { None } else { Some(uid as u16) };
                                        }
                                    });

                                    // Text
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("Text:").size(9.5).color(theme::TEXT_MUTED));
                                        let mut text = item.text.clone().unwrap_or_default();
                                        if ui.add(
                                            egui::TextEdit::singleline(&mut text)
                                                .desired_width(160.0)
                                                .hint_text("(none)"),
                                        ).changed() {
                                            item.text = if text.is_empty() { None } else { Some(text) };
                                        }
                                    });

                                    // Teleport destination
                                    if item.tele_dest.is_some() || item.id == 1387 || item.id == 5765 {
                                        // Common teleport item IDs, or any item that already has a dest
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Teleport:").size(9.5).color(theme::TEXT_MUTED));
                                            let mut dest = item.tele_dest.unwrap_or(pte_otbm::TeleportDest { x: 0, y: 0, z: 7 });
                                            let mut dx = dest.x as i32;
                                            let mut dy = dest.y as i32;
                                            let mut dz = dest.z as i32;

                                            ui.label(egui::RichText::new("X").size(9.0).color(theme::TEXT_MUTED));
                                            if ui.add(egui::DragValue::new(&mut dx).range(0..=65535).speed(1)).changed() {
                                                dest.x = dx.clamp(0, 65535) as u16;
                                            }
                                            ui.label(egui::RichText::new("Y").size(9.0).color(theme::TEXT_MUTED));
                                            if ui.add(egui::DragValue::new(&mut dy).range(0..=65535).speed(1)).changed() {
                                                dest.y = dy.clamp(0, 65535) as u16;
                                            }
                                            ui.label(egui::RichText::new("Z").size(9.0).color(theme::TEXT_MUTED));
                                            if ui.add(egui::DragValue::new(&mut dz).range(0..=41).speed(0.1)).changed() {
                                                dest.z = dz.clamp(0, 41) as u8;
                                            }
                                            item.tele_dest = Some(dest);
                                        });
                                    }

                                    // Count / charges
                                    if item.count.is_some() {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Count:").size(9.5).color(theme::TEXT_MUTED));
                                            let mut c = item.count.unwrap_or(0) as i32;
                                            if ui.add(egui::DragValue::new(&mut c).range(0..=255).speed(1)).changed() {
                                                item.count = Some(c.clamp(0, 255) as u8);
                                            }
                                        });
                                    }

                                    if item.door_id.is_some() {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Door ID:").size(9.5).color(theme::TEXT_MUTED));
                                            let mut d = item.door_id.unwrap_or(0) as i32;
                                            if ui.add(egui::DragValue::new(&mut d).range(0..=255).speed(1)).changed() {
                                                item.door_id = Some(d.clamp(0, 255) as u8);
                                            }
                                        });
                                    }

                                    if item.depot_id.is_some() {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Depot ID:").size(9.5).color(theme::TEXT_MUTED));
                                            let mut d = item.depot_id.unwrap_or(0) as i32;
                                            if ui.add(egui::DragValue::new(&mut d).range(0..=65535).speed(1)).changed() {
                                                item.depot_id = Some(d.clamp(0, 65535) as u16);
                                            }
                                        });
                                    }
                                });
                        });
                    }
                });
        });

    if !open {
        state.show_item_props = false;
    }
}
