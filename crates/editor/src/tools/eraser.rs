use pte_otbm::MapData;
use crate::brushes::shape::{BrushShape, brush_offsets, expand_positions};
use crate::state::UndoAction;

/// Erase the top item (or entire tile with shift), respecting brush shape.
///
/// - `clear_all`: Shift-click removes the entire tile.
/// - `flags_only`: Only clear zone flags and house_id, leave ground/items intact.
pub fn apply_eraser(
    map: &mut MapData,
    center_x: u16,
    center_y: u16,
    z: u8,
    brush_size: u32,
    brush_shape: BrushShape,
    clear_all: bool,
    flags_only: bool,
) -> UndoAction {
    let offsets = brush_offsets(brush_shape, brush_size);
    let positions = expand_positions(center_x, center_y, z, &offsets);

    let mut tiles_before = Vec::new();
    let mut tiles_after = Vec::new();

    for &(x, y, z) in &positions {
        let before = map.get_tile(x, y, z).cloned();
        tiles_before.push((x, y, z, before.clone()));

        if flags_only {
            // Only clear zone flags and house assignment
            if let Some(tile) = map.get_tile_mut_if_exists(x, y, z) {
                tile.flags = Default::default();
                tile.house_id = None;
                tiles_after.push((x, y, z, Some(tile.clone())));
            } else {
                tiles_after.push((x, y, z, None));
            }
        } else if clear_all {
            map.remove_tile(x, y, z);
            tiles_after.push((x, y, z, None));
        } else if before.is_some() {
            let tile = map.get_tile_mut(x, y, z);
            if !tile.items.is_empty() {
                tile.items.pop();
            } else {
                tile.ground = None;
            }
            // Also clear metadata when tile becomes empty
            if tile.ground.is_none() && tile.items.is_empty() {
                map.remove_tile(x, y, z);
                tiles_after.push((x, y, z, None));
            } else {
                tiles_after.push((x, y, z, Some(tile.clone())));
            }
        } else {
            tiles_after.push((x, y, z, None));
        }
    }

    UndoAction {
        tiles_before,
        tiles_after,
    }
}

/// Erase a specific item by index from a tile, or the ground if `erase_ground` is true.
pub fn erase_specific(
    map: &mut MapData,
    x: u16,
    y: u16,
    z: u8,
    item_index: Option<usize>,
    erase_ground: bool,
) -> UndoAction {
    let before = map.get_tile(x, y, z).cloned();

    if let Some(tile) = map.get_tile_mut_if_exists(x, y, z) {
        if let Some(idx) = item_index {
            if idx < tile.items.len() {
                tile.items.remove(idx);
            }
        }
        if erase_ground {
            tile.ground = None;
        }
    }

    let after = map.get_tile(x, y, z).cloned();

    // Clean up empty tiles
    if let Some(ref t) = after {
        if t.ground.is_none() && t.items.is_empty() {
            map.remove_tile(x, y, z);
        }
    }

    let after = map.get_tile(x, y, z).cloned();

    UndoAction {
        tiles_before: vec![(x, y, z, before)],
        tiles_after: vec![(x, y, z, after)],
    }
}
