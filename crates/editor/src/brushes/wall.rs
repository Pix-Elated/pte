//! Wall brush — automatic wall placement with 17 alignment types.
//!
//! Replicates RME's WallBrush. When you paint a wall, it automatically
//! selects the correct wall piece (horizontal, vertical, corner, T-junction,
//! intersection, etc.) based on neighboring wall tiles of the same brush.

use pte_otbm::MapData;
use crate::state::UndoAction;
use super::{Brush, BrushId, BrushStroke, BrushType};

/// The 17 wall alignment types (matching RME's WallNode::type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum WallAlignment {
    Pole = 0,          // No neighbors
    SouthEnd = 1,      // Wall continues north
    EastEnd = 2,       // Wall continues west
    NorthWestDiag = 3, // Diagonal NW-SE
    WestEnd = 4,       // Wall continues east
    NorthEastDiag = 5, // Diagonal NE-SW
    Horizontal = 6,    // Continues east and west
    SouthT = 7,        // T-junction opening south
    NorthEnd = 8,      // Wall continues south
    Vertical = 9,      // Continues north and south
    SouthWestDiag = 10,
    EastT = 11,
    NorthT = 12,
    WestT = 13,
    Intersection = 14,
    SouthEastDiag = 15,
    Untouchable = 16,
}

#[allow(dead_code)]
impl WallAlignment {
    /// Compute alignment from 4-connected neighbor bitmask (N=1, E=2, S=4, W=8).
    pub fn from_cardinal_mask(mask: u8) -> Self {
        match mask & 0x0F {
            0b0000 => Self::Pole,
            0b0001 => Self::SouthEnd,       // N only → this is the south end
            0b0010 => Self::WestEnd,         // E only → this is the west end (wall goes east)
            0b0011 => Self::NorthEastDiag,   // N+E
            0b0100 => Self::NorthEnd,        // S only
            0b0101 => Self::Vertical,        // N+S
            0b0110 => Self::SouthEastDiag,   // S+E → actually NW corner style
            0b0111 => Self::EastT,           // N+S+E
            0b1000 => Self::EastEnd,         // W only → east end
            0b1001 => Self::NorthWestDiag,   // N+W
            0b1010 => Self::Horizontal,      // E+W
            0b1011 => Self::NorthT,          // N+E+W
            0b1100 => Self::SouthWestDiag,   // S+W
            0b1101 => Self::WestT,           // N+S+W
            0b1110 => Self::SouthT,          // S+E+W
            0b1111 => Self::Intersection,    // All four
            _ => Self::Pole,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pole => "Pole",
            Self::SouthEnd => "South End",
            Self::EastEnd => "East End",
            Self::NorthWestDiag => "NW Diagonal",
            Self::WestEnd => "West End",
            Self::NorthEastDiag => "NE Diagonal",
            Self::Horizontal => "Horizontal",
            Self::SouthT => "South T",
            Self::NorthEnd => "North End",
            Self::Vertical => "Vertical",
            Self::SouthWestDiag => "SW Diagonal",
            Self::EastT => "East T",
            Self::NorthT => "North T",
            Self::WestT => "West T",
            Self::Intersection => "Intersection",
            Self::SouthEastDiag => "SE Diagonal",
            Self::Untouchable => "Untouchable",
        }
    }
}

/// A door that can be placed within a wall.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DoorEntry {
    pub item_id: u16,
    pub door_type: DoorType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DoorType {
    Normal,
    Locked,
    Quest,
    Magic,
    Window,
    HatchWindow,
}

/// Wall brush with 17 alignment slots.
#[allow(dead_code)]
pub struct WallBrush {
    pub brush_id: BrushId,
    pub brush_name: String,
    pub look_id: u16,

    /// Item IDs for each of the 17 wall alignments.
    /// Index matches WallAlignment enum value.
    pub wall_items: [Vec<u16>; 17],

    /// Door items that can be placed on horizontal/vertical walls.
    pub doors: Vec<DoorEntry>,

    /// Friendly wall brushes that share borders.
    pub friends: Vec<String>,
}

#[allow(dead_code)]
impl WallBrush {
    pub fn new(id: BrushId, name: String, look_id: u16) -> Self {
        Self {
            brush_id: id,
            brush_name: name,
            look_id,
            wall_items: Default::default(),
            doors: Vec::new(),
            friends: Vec::new(),
        }
    }

    /// Set the item(s) for a given wall alignment.
    pub fn set_alignment_item(&mut self, alignment: WallAlignment, item_id: u16) {
        self.wall_items[alignment as usize].push(item_id);
    }

    /// Get the best item for an alignment (first available).
    pub fn get_alignment_item(&self, alignment: WallAlignment) -> Option<u16> {
        self.wall_items[alignment as usize].first().copied()
    }

    /// Compute the cardinal neighbor mask for wall auto-alignment.
    /// Checks N, E, S, W for same-brush walls.
    fn cardinal_wall_mask(
        &self,
        x: u16,
        y: u16,
        z: u8,
        is_same_wall: &dyn Fn(u16, u16, u8) -> bool,
    ) -> u8 {
        let mut mask: u8 = 0;
        let cardinals: [(i32, i32, u8); 4] = [
            (0, -1, 0x01), // N
            (1, 0, 0x02),  // E
            (0, 1, 0x04),  // S
            (-1, 0, 0x08), // W
        ];

        for &(dx, dy, bit) in &cardinals {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32
                && is_same_wall(nx as u16, ny as u16, z) {
                    mask |= bit;
                }
        }
        mask
    }

    /// Select the correct wall item for a position based on neighbors.
    pub fn select_wall_item(
        &self,
        x: u16,
        y: u16,
        z: u8,
        is_same_wall: &dyn Fn(u16, u16, u8) -> bool,
    ) -> Option<u16> {
        let mask = self.cardinal_wall_mask(x, y, z, is_same_wall);
        let alignment = WallAlignment::from_cardinal_mask(mask);
        self.get_alignment_item(alignment)
            .or_else(|| self.get_alignment_item(WallAlignment::Pole))
    }
}

impl Brush for WallBrush {
    fn id(&self) -> BrushId { self.brush_id }
    fn name(&self) -> &str { &self.brush_name }
    fn brush_type(&self) -> BrushType { BrushType::Wall }
    fn look_id(&self) -> u16 { self.look_id }
    fn needs_border_update(&self) -> bool { true }

    fn wall_item_for_mask(&self, mask: u8) -> Option<u16> {
        let alignment = WallAlignment::from_cardinal_mask(mask);
        self.get_alignment_item(alignment)
            .or_else(|| self.get_alignment_item(WallAlignment::Pole))
    }

    fn all_item_ids(&self) -> Vec<u16> {
        self.wall_items.iter().flatten().copied().collect()
    }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        let mut dirty = Vec::new();

        // First pass: record before state and mark positions
        let _pos_set: std::collections::HashSet<(u16, u16, u8)> =
            positions.iter().copied().collect();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));
            dirty.push((x, y, z));

            // Also dirty the 4 cardinal neighbors for re-alignment
            for &(dx, dy) in &[(0i32, -1), (1, 0), (0, 1), (-1, 0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                    dirty.push((nx as u16, ny as u16, z));
                }
            }
        }

        // Second pass: place wall items (we'll need re-alignment later via update_borders)
        for &(x, y, z) in positions {
            // For initial placement, use Pole alignment (will be updated)
            if let Some(item_id) = self.get_alignment_item(WallAlignment::Pole) {
                let tile = map.get_tile_mut(x, y, z);
                tile.items.push(pte_otbm::MapItem::new(item_id));
                tiles_after.push((x, y, z, Some(tile.clone())));
            } else {
                tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
            }
        }

        dirty.sort();
        dirty.dedup();

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: dirty,
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        let mut dirty = Vec::new();

        // Collect all item IDs this brush uses
        let all_ids: std::collections::HashSet<u16> = self
            .wall_items
            .iter()
            .flatten()
            .copied()
            .collect();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            let tile = map.get_tile_mut(x, y, z);
            tile.items.retain(|item| !all_ids.contains(&item.id));

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
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: dirty,
        }
    }
}
