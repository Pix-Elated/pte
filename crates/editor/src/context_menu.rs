//! Context menu — right-click on a tile for quick actions.

use crate::state::{EditorState, ToolType, TileSelection, UndoAction};
use crate::theme;

/// Call this after viewport rendering. Shows a context menu on right-click.
pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    // Only show when we have a map and a right-click happened
    let right_click_pos = ctx.input(|i| {
        if i.pointer.button_clicked(egui::PointerButton::Secondary) {
            i.pointer.interact_pos()
        } else {
            None
        }
    });

    if let Some(pos) = right_click_pos {
        // Capture the click position once — anchor the menu here
        state.context_menu_pos = Some(pos);
        ctx.memory_mut(|mem| {
            mem.open_popup(egui::Id::new("tile_context_menu"));
        });
    }

    let popup_id = egui::Id::new("tile_context_menu");

    if ctx.memory(|mem| mem.is_popup_open(popup_id)) {
        let anchor = state.context_menu_pos.unwrap_or(egui::pos2(100.0, 100.0));
        let hover = state.hover_tile;
        let has_sel = state.selection.is_some();
        let has_clip = state.clipboard.is_some();

        egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(160.0);

                    // Tile info header
                    if let Some((hx, hy)) = hover {
                        ui.label(
                            egui::RichText::new(format!("Tile ({}, {}, {})", hx, hy, state.camera.z_level))
                                .size(10.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.separator();
                    }

                    // Selection operations
                    if has_sel {
                        if ui.button("✂ Cut  (Ctrl+X)").clicked() {
                            crate::clipboard::cut_selection(state);
                            close_popup(ctx, popup_id);
                        }
                        if ui.button("📋 Copy  (Ctrl+C)").clicked() {
                            crate::clipboard::copy_selection(state);
                            close_popup(ctx, popup_id);
                        }
                        if ui.button("🗑 Delete Selection  (Del)").clicked() {
                            crate::clipboard::delete_selection(state);
                            close_popup(ctx, popup_id);
                        }
                        ui.separator();
                    }

                    if has_clip {
                        if ui.button("📌 Paste Here  (Ctrl+V)").clicked() {
                            if let Some((hx, hy)) = hover {
                                crate::clipboard::paste_at(state, hx, hy);
                            }
                            close_popup(ctx, popup_id);
                        }
                        ui.separator();
                    }

                    // Quick select around click
                    if let Some((hx, hy)) = hover {
                        if ui.button("⬚ Select This Tile").clicked() {
                            state.selection = Some(TileSelection {
                                x1: hx, y1: hy, x2: hx, y2: hy,
                            });
                            state.active_tool = ToolType::Select;
                            close_popup(ctx, popup_id);
                        }
                    }

                    // Eyedropper shortcut
                    if let Some((hx, hy)) = hover {
                        if ui.button("💧 Pick Item Here").clicked() {
                            if let Some(ref map) = state.map_data {
                                if let Some(id) = crate::tools::eyedropper::pick_item_id(map, hx, hy, state.camera.z_level) {
                                    state.selected_item_id = Some(id);
                                    if let Some(brush) = state.brush_registry.ground_brush_for_item(id as u16) {
                                        state.active_brush = Some(brush.id());
                                    }
                                    state.active_tool = ToolType::Brush;
                                }
                            }
                            close_popup(ctx, popup_id);
                        }
                    }

                    // Delete single tile
                    if let Some((hx, hy)) = hover {
                        if ui.button("🗑 Delete This Tile").clicked() {
                            if let Some(ref mut map) = state.map_data {
                                let z = state.camera.z_level;
                                let old = map.get_tile(hx, hy, z).cloned();
                                if old.is_some() {
                                    map.remove_tile(hx, hy, z);
                                    state.push_undo(UndoAction {
                                        tiles_before: vec![(hx, hy, z, old)],
                                        tiles_after: vec![(hx, hy, z, None)],
                                    });
                                }
                            }
                            close_popup(ctx, popup_id);
                        }
                    }

                    // Delete specific item from the tile stack
                    if let Some((hx, hy)) = hover {
                        let z = state.camera.z_level;
                        // Collect item info before mutable borrow
                        let items_info: Vec<(usize, u16, Option<String>)> = state.map_data.as_ref()
                            .and_then(|m| m.get_tile(hx, hy, z))
                            .map(|tile| {
                                let mut info = Vec::new();
                                if let Some(gid) = tile.ground {
                                    info.push((usize::MAX, gid, item_name(state, gid)));
                                }
                                for (i, item) in tile.items.iter().enumerate() {
                                    info.push((i, item.id, item_name(state, item.id)));
                                }
                                info
                            })
                            .unwrap_or_default();

                        if items_info.len() > 1 {
                            ui.menu_button("🧹 Delete Item...", |ui| {
                                for (idx, item_id, name) in &items_info {
                                    let label = if let Some(n) = name {
                                        format!("#{} — {}", item_id, n)
                                    } else if *idx == usize::MAX {
                                        format!("#{} (ground)", item_id)
                                    } else {
                                        format!("#{}", item_id)
                                    };
                                    if ui.button(&label).clicked() {
                                        let idx = *idx;
                                        if let Some(ref mut map) = state.map_data {
                                            let old = map.get_tile(hx, hy, z).cloned();
                                            let tile = map.get_tile_mut(hx, hy, z);
                                            if idx == usize::MAX {
                                                tile.ground = None;
                                            } else if idx < tile.items.len() {
                                                tile.items.remove(idx);
                                            }
                                            let after = if tile.ground.is_none() && tile.items.is_empty() {
                                                map.remove_tile(hx, hy, z);
                                                None
                                            } else {
                                                Some(tile.clone())
                                            };
                                            state.push_undo(UndoAction {
                                                tiles_before: vec![(hx, hy, z, old)],
                                                tiles_after: vec![(hx, hy, z, after)],
                                            });
                                        }
                                        ui.close_menu();
                                    }
                                }
                            });
                        }
                    }

                    // Item properties
                    if hover.is_some() {
                        if ui.button("🔧 Properties...").clicked() {
                            state.show_item_props = true;
                            close_popup(ctx, popup_id);
                        }
                    }

                    // Go to teleport destination
                    if let Some((hx, hy)) = hover {
                        // Extract teleport destination before borrowing state mutably
                        let tele_dest = state.map_data.as_ref().and_then(|map| {
                            map.get_tile(hx, hy, state.camera.z_level).and_then(|tile| {
                                tile.items.iter().find_map(|item| item.tele_dest)
                            })
                        });
                        if let Some(dest) = tele_dest {
                            if ui.button(format!("🔗 Go to TP dest ({}, {}, {})", dest.x, dest.y, dest.z)).clicked() {
                                state.camera.center_x = dest.x as f64;
                                state.camera.center_y = dest.y as f64;
                                state.camera.z_level = dest.z;
                                close_popup(ctx, popup_id);
                            }
                        }
                    }

                    ui.separator();

                    // Navigation
                    if ui.button("🔍 Go To Position...").clicked() {
                        state.show_goto_dialog = true;
                        close_popup(ctx, popup_id);
                    }

                    // Close
                    if ui.button("Cancel").clicked() {
                        close_popup(ctx, popup_id);
                    }
                });
            });

        // Close on click outside
        if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
            state.context_menu_pos = None;
            close_popup(ctx, popup_id);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.context_menu_pos = None;
            close_popup(ctx, popup_id);
        }
    }
}

fn close_popup(ctx: &egui::Context, id: egui::Id) {
    ctx.memory_mut(|mem| mem.close_popup());
    let _ = id; // consumed
}

/// Try to get a human-readable name for an item ID from appearances.
fn item_name(state: &EditorState, item_id: u16) -> Option<String> {
    state.appearances.as_ref().and_then(|apps| {
        apps.get(pte_appearances::Category::Object, item_id as u32)
            .and_then(|app| app.name.clone())
    })
}
