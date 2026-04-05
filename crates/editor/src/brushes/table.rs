//! Table brush — automatic table joining with 7 alignment types.
//!
//! When you paint a table, it selects the correct piece (alone, north end,
//! south end, east end, west end, horizontal, vertical) based on neighbors.

use super::{Brush, BrushId, BrushStroke, BrushType};
use crate::state::UndoAction;
use pte_otbm::MapData;

/// The 7 table alignment types (matching RME).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum TableAlignment {
    Alone = 0,
    NorthEnd = 1,
    EastEnd = 2,
    SouthEnd = 3,
    WestEnd = 4,
    Horizontal = 5,
    Vertical = 6,
}

#[allow(dead_code)]
impl TableAlignment {
    /// Compute from cardinal neighbor mask (N=1, E=2, S=4, W=8).
    pub fn from_cardinal_mask(mask: u8) -> Self {
        let n = mask & 0x01 != 0;
        let e = mask & 0x02 != 0;
        let s = mask & 0x04 != 0;
        let w = mask & 0x08 != 0;

        match (n, e, s, w) {
            (false, false, false, false) => Self::Alone,
            (true, false, false, false) => Self::SouthEnd, // Neighbor north → south end
            (false, true, false, false) => Self::WestEnd,
            (false, false, true, false) => Self::NorthEnd,
            (false, false, false, true) => Self::EastEnd,
            (true, false, true, false) | (true, false, true, _) => Self::Vertical,
            (false, true, false, true) | (_, true, _, true) => Self::Horizontal,
            // For 3+ neighbors, pick dominant axis
            (true, _, false, _) => Self::SouthEnd,
            (false, _, true, _) => Self::NorthEnd,
            _ => Self::Alone,
        }
    }
}

/// Table brush with alignment slots.
pub struct TableBrush {
    pub brush_id: BrushId,
    pub brush_name: String,
    pub look_id: u16,
    /// Item IDs for each alignment type.
    pub items: [Option<u16>; 7],
}

#[allow(dead_code)]
impl TableBrush {
    pub fn new(id: BrushId, name: String, look_id: u16) -> Self {
        Self {
            brush_id: id,
            brush_name: name,
            look_id,
            items: [None; 7],
        }
    }

    pub fn set_item(&mut self, alignment: TableAlignment, item_id: u16) {
        self.items[alignment as usize] = Some(item_id);
    }

    pub fn get_item(&self, alignment: TableAlignment) -> Option<u16> {
        self.items[alignment as usize]
    }

    fn collect_item_ids(&self) -> Vec<u16> {
        self.items.iter().filter_map(|&x| x).collect()
    }
}

impl Brush for TableBrush {
    fn id(&self) -> BrushId {
        self.brush_id
    }
    fn name(&self) -> &str {
        &self.brush_name
    }
    fn brush_type(&self) -> BrushType {
        BrushType::Table
    }
    fn look_id(&self) -> u16 {
        self.look_id
    }
    fn needs_border_update(&self) -> bool {
        true
    }

    fn all_item_ids(&self) -> Vec<u16> {
        self.collect_item_ids()
    }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        let mut dirty = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            // Place with Alone alignment initially (will be updated via dirty)
            if let Some(item_id) = self.get_item(TableAlignment::Alone) {
                let tile = map.get_tile_mut(x, y, z);
                tile.items.push(pte_otbm::MapItem::new(item_id));
                tiles_after.push((x, y, z, Some(tile.clone())));
            } else {
                tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
            }

            dirty.push((x, y, z));
            for &(dx, dy) in &[(0i32, -1), (1, 0), (0, 1), (-1, 0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                    dirty.push((nx as u16, ny as u16, z));
                }
            }
        }

        dirty.sort();
        dirty.dedup();

        BrushStroke {
            undo: UndoAction {
                tiles_before,
                tiles_after,
            },
            dirty_positions: dirty,
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        let mut dirty = Vec::new();
        let ids: std::collections::HashSet<u16> = self.all_item_ids().into_iter().collect();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            let tile = map.get_tile_mut(x, y, z);
            tile.items.retain(|item| !ids.contains(&item.id));

            if tile.ground.is_none() && tile.items.is_empty() {
                map.remove_tile(x, y, z);
                tiles_after.push((x, y, z, None));
            } else {
                tiles_after.push((x, y, z, Some(tile.clone())));
            }

            dirty.push((x, y, z));
            for &(dx, dy) in &[(0i32, -1), (1, 0), (0, 1), (-1, 0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                    dirty.push((nx as u16, ny as u16, z));
                }
            }
        }

        dirty.sort();
        dirty.dedup();

        BrushStroke {
            undo: UndoAction {
                tiles_before,
                tiles_after,
            },
            dirty_positions: dirty,
        }
    }
}
