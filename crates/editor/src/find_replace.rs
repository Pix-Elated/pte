//! Find & Replace dialog — search for items by ID across the map, replace them.

use crate::state::{EditorState, UndoAction};
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_find_dialog {
        return;
    }

    let mut open = true;

    egui::Window::new("Find / Replace Items")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size([360.0, 0.0])
        .show(ctx, |ui| {
            // Mode toggle: find by ID or by name
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.find_by_name, false, "By ID");
                ui.selectable_value(&mut state.find_by_name, true, "By Name");
            });

            ui.add_space(4.0);

            if state.find_by_name {
                // Name-based search
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Name:").color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.find_name_query)
                            .desired_width(140.0)
                            .hint_text("e.g. magic plate"),
                    );
                    if ui.button("Find All").clicked() {
                        do_find_by_name(ctx, state);
                    }
                });
            } else {
                // ID-based search (original)
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Find ID:").color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.find_item_id)
                            .desired_width(80.0)
                            .hint_text("e.g. 4526"),
                    );
                    if ui.button("Find All").clicked() {
                        do_find(state);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Replace:").color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.replace_item_id)
                            .desired_width(80.0)
                            .hint_text("e.g. 4527"),
                    );
                    let can_replace =
                        !state.find_results.is_empty() && !state.replace_item_id.is_empty();
                    if ui
                        .add_enabled(can_replace, egui::Button::new("Replace All"))
                        .clicked()
                    {
                        do_replace_all(state);
                    }
                });
            }

            ui.add_space(2.0);

            // Property filters (apply to both ID and name modes)
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Action ID:")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.find_action_id)
                        .desired_width(60.0)
                        .hint_text("any"),
                );
                ui.label(
                    egui::RichText::new("Unique ID:")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.find_unique_id)
                        .desired_width(60.0)
                        .hint_text("any"),
                );
            });
            ui.checkbox(&mut state.find_current_z_only, "Current z-level only");

            ui.add_space(6.0);

            // Results
            if state.find_results.is_empty() {
                ui.label(
                    egui::RichText::new("No results")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "{} tiles found containing item {}",
                        state.find_results.len(),
                        state.find_item_id
                    ))
                    .size(10.0)
                    .color(theme::SUCCESS),
                );

                ui.horizontal(|ui| {
                    if ui.button("◀ Prev").clicked() {
                        if state.find_result_cursor > 0 {
                            state.find_result_cursor -= 1;
                        } else {
                            state.find_result_cursor = state.find_results.len() - 1;
                        }
                        jump_to_result(state);
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{} / {}",
                            state.find_result_cursor + 1,
                            state.find_results.len()
                        ))
                        .color(theme::TEXT_PRIMARY),
                    );
                    if ui.button("Next ▶").clicked() {
                        state.find_result_cursor =
                            (state.find_result_cursor + 1) % state.find_results.len();
                        jump_to_result(state);
                    }
                });

                // List first 50 results
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        let mut clicked_idx: Option<usize> = None;
                        for (i, (x, y, z)) in state.find_results.iter().enumerate().take(50) {
                            let selected = i == state.find_result_cursor;
                            let text = format!("({}, {}, {})", x, y, z);
                            if ui.selectable_label(selected, &text).clicked() {
                                clicked_idx = Some(i);
                            }
                        }
                        if let Some(i) = clicked_idx {
                            state.find_result_cursor = i;
                            jump_to_result(state);
                        }
                        if state.find_results.len() > 50 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "...and {} more",
                                    state.find_results.len() - 50
                                ))
                                .size(9.5)
                                .color(theme::TEXT_MUTED),
                            );
                        }
                    });
            }
        });

    if !open {
        state.show_find_dialog = false;
    }
}

fn do_find(state: &mut EditorState) {
    state.find_results.clear();
    state.find_result_cursor = 0;

    let Ok(item_id) = state.find_item_id.trim().parse::<u16>() else {
        return;
    };

    let filter_aid: Option<u16> = state.find_action_id.trim().parse().ok();
    let filter_uid: Option<u16> = state.find_unique_id.trim().parse().ok();
    let z_filter = if state.find_current_z_only {
        Some(state.camera.z_level)
    } else {
        None
    };

    let Some(ref map) = state.map_data else {
        return;
    };

    // Search all chunks
    for chunk in map.chunks.values() {
        for tile in chunk.values() {
            // Z-level filter
            if let Some(z) = z_filter {
                if tile.z != z {
                    continue;
                }
            }

            let has_item = tile.ground == Some(item_id)
                || tile.items.iter().any(|it| {
                    if it.id != item_id {
                        return false;
                    }
                    if let Some(aid) = filter_aid {
                        if it.action_id != Some(aid) {
                            return false;
                        }
                    }
                    if let Some(uid) = filter_uid {
                        if it.unique_id != Some(uid) {
                            return false;
                        }
                    }
                    true
                });
            if has_item {
                state.find_results.push((tile.x, tile.y, tile.z));
            }
        }
    }

    // Sort for deterministic ordering
    state.find_results.sort();

    // Jump to first result
    if !state.find_results.is_empty() {
        jump_to_result(state);
    }
}

fn do_replace_all(state: &mut EditorState) {
    let Ok(find_id) = state.find_item_id.trim().parse::<u16>() else {
        return;
    };
    let Ok(replace_id) = state.replace_item_id.trim().parse::<u16>() else {
        return;
    };

    let Some(ref mut map) = state.map_data else {
        return;
    };

    let mut before = Vec::new();
    let mut after = Vec::new();

    // Collect positions from find_results
    let positions: Vec<_> = state.find_results.clone();

    for (x, y, z) in &positions {
        let Some(tile) = map.get_tile(*x, *y, *z) else {
            continue;
        };
        let old = tile.clone();

        let mut new_tile = old.clone();
        let mut changed = false;

        if new_tile.ground == Some(find_id) {
            new_tile.ground = Some(replace_id);
            changed = true;
        }
        for item in &mut new_tile.items {
            if item.id == find_id {
                item.id = replace_id;
                changed = true;
            }
        }

        if changed {
            before.push((*x, *y, *z, Some(old)));
            after.push((*x, *y, *z, Some(new_tile.clone())));
            map.set_tile(new_tile);
        }
    }

    if !before.is_empty() {
        state.push_undo(UndoAction {
            tiles_before: before,
            tiles_after: after,
        });
    }

    // Re-run find to update results
    do_find(state);
}

fn jump_to_result(state: &mut EditorState) {
    if let Some(&(x, y, z)) = state.find_results.get(state.find_result_cursor) {
        state.camera.center_x = x as f64;
        state.camera.center_y = y as f64;
        state.camera.z_level = z;
    }
}

/// Search for tiles containing items whose appearance name matches the query.
fn do_find_by_name(_ctx: &egui::Context, state: &mut EditorState) {
    state.find_results.clear();
    state.find_result_cursor = 0;

    let query = state.find_name_query.trim().to_lowercase();
    if query.is_empty() {
        return;
    }

    let filter_aid: Option<u16> = state.find_action_id.trim().parse().ok();
    let filter_uid: Option<u16> = state.find_unique_id.trim().parse().ok();
    let z_filter = if state.find_current_z_only {
        Some(state.camera.z_level)
    } else {
        None
    };

    let Some(ref appearances) = state.appearances else {
        return;
    };
    let Some(ref map) = state.map_data else {
        return;
    };

    // Build a set of item IDs whose appearance name contains the query
    let matching_ids: std::collections::HashSet<u16> = appearances
        .objects
        .values()
        .filter(|app| {
            app.name
                .as_ref()
                .is_some_and(|n| n.to_lowercase().contains(&query))
        })
        .filter_map(|app| app.id.map(|id| id as u16))
        .collect();

    if matching_ids.is_empty() {
        return;
    }

    // Search all tiles for any of those IDs
    for chunk in map.chunks.values() {
        for tile in chunk.values() {
            // Z-level filter
            if let Some(z) = z_filter {
                if tile.z != z {
                    continue;
                }
            }

            let has_item = tile.ground.is_some_and(|g| matching_ids.contains(&g))
                || tile.items.iter().any(|it| {
                    if !matching_ids.contains(&it.id) {
                        return false;
                    }
                    if let Some(aid) = filter_aid {
                        if it.action_id != Some(aid) {
                            return false;
                        }
                    }
                    if let Some(uid) = filter_uid {
                        if it.unique_id != Some(uid) {
                            return false;
                        }
                    }
                    true
                });
            if has_item {
                state.find_results.push((tile.x, tile.y, tile.z));
            }
        }
    }

    state.find_results.sort();

    if !state.find_results.is_empty() {
        jump_to_result(state);
    }
}
