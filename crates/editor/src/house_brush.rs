//! House brush — paint/erase house_id on tiles, with undo support.

use crate::state::{EditorState, UndoAction};

/// Paint a house_id onto a tile (or clear it if `erase` is true).
pub fn apply_house_brush(state: &mut EditorState, x: u16, y: u16, z: u8, house_id: u32, erase: bool) {
    let Some(ref mut map) = state.map_data else { return };

    let tile_before = map.get_tile(x, y, z).cloned();

    let tile = map.get_tile_mut(x, y, z);
    if erase {
        tile.house_id = None;
    } else {
        tile.house_id = Some(house_id);
    }

    let tile_after = map.get_tile(x, y, z).cloned();

    let action = UndoAction {
        tiles_before: vec![(x, y, z, tile_before)],
        tiles_after: vec![(x, y, z, tile_after)],
    };
    state.stroke_add(action);
}

/// Set a tile as the house exit position.
#[allow(dead_code)]
pub fn set_house_exit(state: &mut EditorState, house_id: u32, x: u16, y: u16, z: u8) {
    let Some(ref mut map) = state.map_data else { return };
    if let Some(house) = map.houses.iter_mut().find(|h| h.id == house_id) {
        house.exit = pte_otbm::Position { x, y, z };
    }
}

/// Color for a given house_id (deterministic hash → hue).
pub fn house_color(house_id: u32) -> egui::Color32 {
    // Golden ratio hash for even hue distribution
    let hue = ((house_id as f32 * 0.618_034) % 1.0) * 360.0;
    let (r, g, b) = hsv_to_rgb(hue, 0.6, 0.9);
    egui::Color32::from_rgba_premultiplied(
        (r * 60.0) as u8,
        (g * 60.0) as u8,
        (b * 60.0) as u8,
        60,
    )
}

/// Border color for house overlay (more saturated).
pub fn house_border_color(house_id: u32) -> egui::Color32 {
    let hue = ((house_id as f32 * 0.618_034) % 1.0) * 360.0;
    let (r, g, b) = hsv_to_rgb(hue, 0.7, 1.0);
    egui::Color32::from_rgba_premultiplied(
        (r * 180.0) as u8,
        (g * 180.0) as u8,
        (b * 180.0) as u8,
        140,
    )
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r + m, g + m, b + m)
}
