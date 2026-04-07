//! Map cleanup operations — remove corpses, empty tiles, unused data.

use crate::state::EditorState;

/// Result of a cleanup operation.
pub struct CleanupResult {
    #[allow(dead_code)]
    pub removed_count: usize,
    pub description: String,
}

/// Remove all tiles that have no ground, no items, no flags, and no house_id.
pub fn remove_empty_tiles(state: &mut EditorState) -> CleanupResult {
    let Some(ref mut map) = state.map_data else {
        return CleanupResult {
            removed_count: 0,
            description: "No map loaded".into(),
        };
    };

    let mut removed = 0;
    let mut keys_to_clean = Vec::new();

    for (key, chunk) in &map.chunks {
        let empties: Vec<(u8, u8)> = chunk
            .iter()
            .filter(|&(_, tile)| {
                tile.ground.is_none()
                    && tile.items.is_empty()
                    && !tile.flags.any()
                    && tile.house_id.is_none()
            })
            .map(|(k, _)| *k)
            .collect();

        if !empties.is_empty() {
            keys_to_clean.push((*key, empties));
        }
    }

    for (key, empties) in keys_to_clean {
        if let Some(chunk) = map.chunks.get_mut(&key) {
            for local in empties {
                chunk.remove(&local);
                removed += 1;
            }
            if chunk.is_empty() {
                map.chunks.remove(&key);
            }
        }
    }

    state.cache_dirty = true;
    CleanupResult {
        removed_count: removed,
        description: format!("Removed {} empty tiles", removed),
    }
}

/// Remove items that are commonly corpses or decay items.
/// This uses a simple heuristic: items in a known corpse ID range.
#[allow(dead_code)]
pub fn remove_corpses(state: &mut EditorState, corpse_ids: &[u16]) -> CleanupResult {
    let Some(ref mut map) = state.map_data else {
        return CleanupResult {
            removed_count: 0,
            description: "No map loaded".into(),
        };
    };

    let mut removed = 0;
    let id_set: std::collections::HashSet<u16> = corpse_ids.iter().copied().collect();

    for chunk in map.chunks.values_mut() {
        for tile in chunk.values_mut() {
            let before_len = tile.items.len();
            tile.items.retain(|item| !id_set.contains(&item.id));
            removed += before_len - tile.items.len();
        }
    }

    state.cache_dirty = true;
    CleanupResult {
        removed_count: removed,
        description: format!("Removed {} corpse items", removed),
    }
}

/// Remove duplicate items stacked on the same tile (items with identical ID at the same position).
pub fn remove_duplicate_items(state: &mut EditorState) -> CleanupResult {
    let Some(ref mut map) = state.map_data else {
        return CleanupResult {
            removed_count: 0,
            description: "No map loaded".into(),
        };
    };

    let mut removed = 0;

    for chunk in map.chunks.values_mut() {
        for tile in chunk.values_mut() {
            let before_len = tile.items.len();
            let mut seen = std::collections::HashSet::new();
            tile.items.retain(|item| seen.insert(item.id));
            removed += before_len - tile.items.len();
        }
    }

    state.cache_dirty = true;
    CleanupResult {
        removed_count: removed,
        description: format!("Removed {} duplicate items", removed),
    }
}

/// Clear house_id from tiles where the house doesn't exist in the houses list.
pub fn clear_orphan_house_refs(state: &mut EditorState) -> CleanupResult {
    let Some(ref mut map) = state.map_data else {
        return CleanupResult {
            removed_count: 0,
            description: "No map loaded".into(),
        };
    };

    let defined: std::collections::HashSet<u32> = map.houses.iter().map(|h| h.id).collect();
    let mut cleared = 0;

    for chunk in map.chunks.values_mut() {
        for tile in chunk.values_mut() {
            if let Some(hid) = tile.house_id {
                if !defined.contains(&hid) {
                    tile.house_id = None;
                    cleared += 1;
                }
            }
        }
    }

    state.cache_dirty = true;
    CleanupResult {
        removed_count: cleared,
        description: format!("Cleared {} orphaned house references", cleared),
    }
}

/// Show the cleanup dialog.
pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_cleanup_dialog {
        return;
    }

    let mut open = true;

    egui::Window::new("Map Cleanup")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size([320.0, 0.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let has_map = state.map_data.is_some();

            ui.label(
                egui::RichText::new("⚠ Cleanup operations cannot be undone!")
                    .size(11.0)
                    .color(crate::theme::WARNING),
            );
            ui.add_space(6.0);

            if ui
                .add_enabled(has_map, egui::Button::new("Remove empty tiles"))
                .clicked()
            {
                let result = remove_empty_tiles(state);
                state.cleanup_last_result = Some(result.description);
            }

            if ui
                .add_enabled(has_map, egui::Button::new("Remove duplicate items"))
                .clicked()
            {
                let result = remove_duplicate_items(state);
                state.cleanup_last_result = Some(result.description);
            }

            if ui
                .add_enabled(has_map, egui::Button::new("Clear orphan house references"))
                .clicked()
            {
                let result = clear_orphan_house_refs(state);
                state.cleanup_last_result = Some(result.description);
            }

            ui.add_space(8.0);

            if let Some(ref msg) = state.cleanup_last_result {
                ui.label(
                    egui::RichText::new(msg)
                        .size(11.0)
                        .color(crate::theme::SUCCESS),
                );
            }
        });

    if !open {
        state.show_cleanup_dialog = false;
    }
}
