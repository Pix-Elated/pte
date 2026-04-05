use crate::brushes::registry::BrushRegistry;
use crate::brushes::shape::{brush_offsets, expand_positions, BrushShape};
use crate::brushes::BrushId;
use crate::state::UndoAction;
use pte_appearances::LoadedAppearances;
use pte_otbm::{MapData, MapItem};

/// Apply a brush at the given center position, using the full brush pipeline.
///
/// If `brush_id` is Some, looks up the registered brush and calls its `draw()`.
/// Otherwise falls back to raw item placement using `item_id`.
#[allow(clippy::too_many_arguments)]
pub fn apply_brush(
    map: &mut MapData,
    center_x: u16,
    center_y: u16,
    z: u8,
    brush_id: Option<BrushId>,
    item_id: Option<u32>,
    registry: &BrushRegistry,
    brush_size: u32,
    brush_shape: BrushShape,
    appearances: &Option<LoadedAppearances>,
) -> ApplyResult {
    let offsets = brush_offsets(brush_shape, brush_size);
    let positions = expand_positions(center_x, center_y, z, &offsets);

    // If we have a registered brush, use the Brush trait
    if let Some(bid) = brush_id {
        if let Some(brush) = registry.get(bid) {
            let stroke = brush.draw(map, &positions);
            return ApplyResult {
                undo: stroke.undo,
                dirty_positions: stroke.dirty_positions,
            };
        }
    }

    // Fallback: raw item placement (no registered brush)
    let raw_id = match item_id {
        Some(id) => id as u16,
        None => return ApplyResult::empty(),
    };

    // Check if this appearance is a ground tile (has bank/ground flag)
    let is_ground = appearances
        .as_ref()
        .and_then(|apps| apps.get(pte_appearances::Category::Object, raw_id as u32))
        .and_then(|app| app.flags.as_ref())
        .map(|f| f.bank.is_some())
        .unwrap_or(false);

    let mut tiles_before = Vec::new();
    let mut tiles_after = Vec::new();

    for &(x, y, z) in &positions {
        let before = map.get_tile(x, y, z).cloned();
        tiles_before.push((x, y, z, before));

        let tile = map.get_tile_mut(x, y, z);
        if is_ground {
            tile.ground = Some(raw_id);
        } else {
            // Don't duplicate — remove existing instance of same item first
            tile.items.retain(|i| i.id != raw_id);
            tile.items.push(MapItem::new(raw_id));
        }
        tiles_after.push((x, y, z, Some(tile.clone())));
    }

    ApplyResult {
        undo: UndoAction {
            tiles_before,
            tiles_after,
        },
        dirty_positions: positions,
    }
}

/// Result of a brush application — undo data + dirty tiles for border updates.
pub struct ApplyResult {
    pub undo: UndoAction,
    pub dirty_positions: Vec<(u16, u16, u8)>,
}

impl ApplyResult {
    pub fn empty() -> Self {
        Self {
            undo: UndoAction {
                tiles_before: vec![],
                tiles_after: vec![],
            },
            dirty_positions: vec![],
        }
    }
}
