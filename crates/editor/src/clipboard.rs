//! Clipboard operations — Copy, Cut, Paste, Delete for tile selections.

use crate::state::{EditorState, TileClipboard, UndoAction};

/// Copy the selected tiles into the clipboard buffer.
pub fn copy_selection(state: &mut EditorState) {
    let Some(sel) = state.selection else { return };
    let Some(ref map) = state.map_data else {
        return;
    };

    let z = state.camera.z_level;
    let tiles = map.get_tiles_in_area(sel.x1, sel.y1, sel.x2, sel.y2, z);
    let mut buf = Vec::with_capacity(tiles.len());
    for tile in &tiles {
        let dx = tile.x - sel.x1;
        let dy = tile.y - sel.y1;
        buf.push((dx, dy, (*tile).clone()));
    }

    state.clipboard = Some(TileClipboard {
        width: sel.x2 - sel.x1 + 1,
        height: sel.y2 - sel.y1 + 1,
        source_z: z,
        tiles: buf,
    });
}

/// Cut the selected tiles — copy then delete.
pub fn cut_selection(state: &mut EditorState) {
    copy_selection(state);
    delete_selection(state);
}

/// Delete all tiles in the current selection.
pub fn delete_selection(state: &mut EditorState) {
    let Some(sel) = state.selection else { return };
    let Some(ref mut map) = state.map_data else {
        return;
    };
    let z = state.camera.z_level;

    let mut before = Vec::new();
    let mut after = Vec::new();

    for y in sel.y1..=sel.y2 {
        for x in sel.x1..=sel.x2 {
            let old = map.get_tile(x, y, z).cloned();
            if old.is_some() {
                before.push((x, y, z, old));
                after.push((x, y, z, None));
                map.remove_tile(x, y, z);
            }
        }
    }

    if !before.is_empty() {
        state.push_undo(UndoAction {
            tiles_before: before,
            tiles_after: after,
        });
    }

    state.selection = None;
}

/// Paste the clipboard buffer at the given world position.
/// Preserves relative z-offset from the original copy z-level.
pub fn paste_at(state: &mut EditorState, target_x: u16, target_y: u16) {
    let Some(ref clip) = state.clipboard else {
        return;
    };
    let Some(ref mut map) = state.map_data else {
        return;
    };
    let target_z = state.camera.z_level;

    let mut before = Vec::new();
    let mut after = Vec::new();

    for (dx, dy, src_tile) in &clip.tiles {
        let tx = target_x.wrapping_add(*dx);
        let ty = target_y.wrapping_add(*dy);
        // Compute z offset: if tile was on z=8 and source_z=7, offset is +1
        let z_offset = src_tile.z as i16 - clip.source_z as i16;
        let tz = (target_z as i16 + z_offset).clamp(0, crate::state::MAP_MAX_Z as i16) as u8;

        let old = map.get_tile(tx, ty, tz).cloned();
        before.push((tx, ty, tz, old));

        // Build the pasted tile at the target position
        let mut new_tile = src_tile.clone();
        new_tile.x = tx;
        new_tile.y = ty;
        new_tile.z = tz;

        map.set_tile(new_tile.clone());
        after.push((tx, ty, tz, Some(new_tile)));
    }

    if !before.is_empty() {
        state.push_undo(UndoAction {
            tiles_before: before,
            tiles_after: after,
        });
    }

    state.paste_preview = false;
}

/// Check if we have content in the clipboard.
pub fn has_clipboard(state: &EditorState) -> bool {
    state.clipboard.is_some()
}

/// Check if there is an active selection.
pub fn has_selection(state: &EditorState) -> bool {
    state.selection.is_some()
}
