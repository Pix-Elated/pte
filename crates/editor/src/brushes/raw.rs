//! RAW brush — direct item placement without auto-border logic.
//! Equivalent to RME's RAW brush, or our original "simple brush".

use pte_otbm::MapData;
use crate::state::UndoAction;
use super::{Brush, BrushId, BrushStroke, BrushType};

/// Raw brush places a specific item ID as ground or on the item stack.
#[allow(dead_code)]
pub struct RawBrush {
    pub id: BrushId,
    pub name: String,
    pub item_id: u16,
    pub as_ground: bool,
}

impl RawBrush {
    pub fn new(id: BrushId, name: String, item_id: u16, as_ground: bool) -> Self {
        Self {
            id,
            name,
            item_id,
            as_ground,
        }
    }
}

impl Brush for RawBrush {
    fn id(&self) -> BrushId { self.id }
    fn name(&self) -> &str { &self.name }
    fn brush_type(&self) -> BrushType { BrushType::Raw }
    fn look_id(&self) -> u16 { self.item_id }
    fn is_ground(&self) -> bool { self.as_ground }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            let tile = map.get_tile_mut(x, y, z);
            if self.as_ground {
                tile.ground = Some(self.item_id);
            } else {
                tile.items.push(pte_otbm::MapItem::new(self.item_id));
            }
            tiles_after.push((x, y, z, Some(tile.clone())));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            if self.as_ground {
                let tile = map.get_tile_mut(x, y, z);
                if tile.ground == Some(self.item_id) {
                    tile.ground = None;
                }
                if tile.ground.is_none() && tile.items.is_empty() {
                    map.remove_tile(x, y, z);
                    tiles_after.push((x, y, z, None));
                } else {
                    tiles_after.push((x, y, z, Some(map.get_tile_mut(x, y, z).clone())));
                }
            } else {
                let tile = map.get_tile_mut(x, y, z);
                if let Some(pos) = tile.items.iter().rposition(|item| item.id == self.item_id) {
                    tile.items.remove(pos);
                }
                if tile.ground.is_none() && tile.items.is_empty() {
                    map.remove_tile(x, y, z);
                    tiles_after.push((x, y, z, None));
                } else {
                    tiles_after.push((x, y, z, Some(map.get_tile_mut(x, y, z).clone())));
                }
            }
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }
}
