//! Carpet brush — automatic carpet joining with 13 types.
//!
//! Like table brush but with 8-directional awareness: N, E, S, W edges,
//! NW/NE/SW/SE corners, center tiles, and 4 diagonal pieces.

use super::{Brush, BrushId, BrushStroke, BrushType};
use crate::state::UndoAction;
use pte_otbm::MapData;

/// The 13 carpet alignment types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum CarpetAlignment {
    Center = 0,
    North = 1,
    East = 2,
    South = 3,
    West = 4,
    NorthWest = 5,
    NorthEast = 6,
    SouthWest = 7,
    SouthEast = 8,
    /// Full carpet, all neighbors present
    Full = 9,
    /// Alone, no neighbors
    Alone = 10,
    /// Horizontal strip (E+W neighbors only)
    Horizontal = 11,
    /// Vertical strip (N+S neighbors only)
    Vertical = 12,
}

#[allow(dead_code)]
impl CarpetAlignment {
    /// Compute from 8-neighbor bitmask.
    /// Bit layout: NW=0, N=1, NE=2, W=3, E=4, SW=5, S=6, SE=7
    pub fn from_neighbor_mask(mask: u8) -> Self {
        let nw = mask & 0x01 != 0;
        let n = mask & 0x02 != 0;
        let ne = mask & 0x04 != 0;
        let w = mask & 0x08 != 0;
        let e = mask & 0x10 != 0;
        let sw = mask & 0x20 != 0;
        let s = mask & 0x40 != 0;
        let se = mask & 0x80 != 0;

        // If all 8 neighbors → center/full
        if n && e && s && w && nw && ne && sw && se {
            return Self::Full;
        }

        // If no neighbors → alone
        if !n && !e && !s && !w {
            return Self::Alone;
        }

        // Corner pieces: only 2 cardinal neighbors meeting at a corner
        if n && w && !e && !s {
            return Self::SouthEast;
        } // NW corner of carpet → SE alignment
        if n && e && !w && !s {
            return Self::SouthWest;
        }
        if s && w && !e && !n {
            return Self::NorthEast;
        }
        if s && e && !w && !n {
            return Self::NorthWest;
        }

        // Edge pieces: cardinal but not opposite cardinal
        if n && s && !e && !w {
            return Self::Vertical;
        }
        if e && w && !n && !s {
            return Self::Horizontal;
        }

        // 3-way edges
        if n && e && s && w {
            return Self::Center;
        }
        if n && !s {
            return Self::South;
        } // Neighbor to north → this is south edge
        if s && !n {
            return Self::North;
        }
        if e && !w {
            return Self::West;
        }
        if w && !e {
            return Self::East;
        }

        Self::Center
    }
}

/// Carpet brush with alignment slots.
pub struct CarpetBrush {
    pub brush_id: BrushId,
    pub brush_name: String,
    pub look_id: u16,
    /// Item IDs for each alignment type.
    pub items: [Option<u16>; 13],
}

#[allow(dead_code)]
impl CarpetBrush {
    pub fn new(id: BrushId, name: String, look_id: u16) -> Self {
        Self {
            brush_id: id,
            brush_name: name,
            look_id,
            items: [None; 13],
        }
    }

    pub fn set_item(&mut self, alignment: CarpetAlignment, item_id: u16) {
        self.items[alignment as usize] = Some(item_id);
    }

    pub fn get_item(&self, alignment: CarpetAlignment) -> Option<u16> {
        self.items[alignment as usize]
    }

    fn collect_item_ids(&self) -> Vec<u16> {
        self.items.iter().filter_map(|&x| x).collect()
    }
}

impl Brush for CarpetBrush {
    fn id(&self) -> BrushId {
        self.brush_id
    }
    fn name(&self) -> &str {
        &self.brush_name
    }
    fn brush_type(&self) -> BrushType {
        BrushType::Carpet
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

            if let Some(item_id) = self.get_item(CarpetAlignment::Alone) {
                let tile = map.get_tile_mut(x, y, z);
                tile.items.push(pte_otbm::MapItem::new(item_id));
                tiles_after.push((x, y, z, Some(tile.clone())));
            } else {
                tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
            }

            dirty.push((x, y, z));
            // 8 neighbors for carpet
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                        dirty.push((nx as u16, ny as u16, z));
                    }
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
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                        dirty.push((nx as u16, ny as u16, z));
                    }
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
