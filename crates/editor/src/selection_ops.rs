//! Selection operations — randomize variants, borderize, and eraser flags mode.

use crate::state::{EditorState, UndoAction};
use rand::Rng;

/// Borderize selection — recalculate auto-borders for all tiles in the selection.
pub fn borderize_selection(state: &mut EditorState) {
    let Some(ref sel) = state.selection else {
        return;
    };
    let sel = *sel;
    let z = state.camera.z_level;

    // Collect dirty positions
    let mut dirty: Vec<(u16, u16, u8)> = Vec::new();
    for ty in sel.y1..=sel.y2 {
        for tx in sel.x1..=sel.x2 {
            dirty.push((tx, ty, z));
        }
    }

    if !dirty.is_empty() {
        if let Some(ref mut map) = state.map_data {
            crate::brushes::process_borders(map, &state.brush_registry, &dirty);
        }
    }
}

/// Randomize selection — for any ground tile that has variants in the brush registry,
/// swap to a random variant.
pub fn randomize_selection(state: &mut EditorState) {
    let Some(ref sel) = state.selection else {
        return;
    };
    let sel = *sel;
    let z = state.camera.z_level;

    // First pass: collect ground IDs and their variants (immutable borrows)
    let mut tile_variants: Vec<(u16, u16, Vec<u16>)> = Vec::new();
    if let Some(ref map) = state.map_data {
        for ty in sel.y1..=sel.y2 {
            for tx in sel.x1..=sel.x2 {
                if let Some(tile) = map.get_tile(tx, ty, z) {
                    if let Some(gid) = tile.ground {
                        let variants = state.brush_registry.ground_variants(gid);
                        if variants.len() > 1 {
                            tile_variants.push((tx, ty, variants));
                        }
                    }
                }
            }
        }
    }

    if tile_variants.is_empty() {
        return;
    }

    // Second pass: apply random variants (mutable borrow)
    let mut rng = rand::rng();
    let mut tiles_before = Vec::new();
    let mut tiles_after = Vec::new();

    if let Some(ref mut map) = state.map_data {
        for (tx, ty, variants) in tile_variants {
            if let Some(tile) = map.get_tile(tx, ty, z) {
                let before = tile.clone();
                let new_gid = variants[rng.random_range(0..variants.len())];
                let t = map.get_tile_mut(tx, ty, z);
                t.ground = Some(new_gid);
                tiles_before.push((tx, ty, z, Some(before)));
                tiles_after.push((tx, ty, z, Some(t.clone())));
            }
        }
    }

    if !tiles_before.is_empty() {
        state.push_undo(UndoAction {
            tiles_before,
            tiles_after,
        });
    }
}
