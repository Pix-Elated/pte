//! Door brush — places doors within wall segments.
//!
//! Doors are associated with walls. When placed, they replace the wall item at
//! that position with the appropriate door variant (horizontal/vertical,
//! normal/locked/quest/magic).

use pte_otbm::MapData;
use crate::state::UndoAction;
use super::{Brush, BrushId, BrushStroke, BrushType};

/// Door variant types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorVariant {
    Normal,
    Locked,
    Quest,
    Magic,
}

impl DoorVariant {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Locked => "Locked",
            Self::Quest => "Quest",
            Self::Magic => "Magic",
        }
    }
}

/// Door orientation — inferred from the wall it's placed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DoorOrientation {
    Horizontal,
    Vertical,
}

/// A door brush entry — item ID for a specific variant+orientation combo.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DoorItem {
    pub variant: DoorVariant,
    pub orientation: DoorOrientation,
    pub open_id: u16,
    pub closed_id: u16,
}

/// Door brush.
#[allow(dead_code)]
pub struct DoorBrush {
    pub brush_id: BrushId,
    pub brush_name: String,
    pub look_id: u16,
    pub doors: Vec<DoorItem>,
}

impl DoorBrush {
    pub fn new(id: BrushId, name: String, look_id: u16) -> Self {
        Self {
            brush_id: id,
            brush_name: name,
            look_id,
            doors: Vec::new(),
        }
    }

    pub fn add_door(&mut self, door: DoorItem) {
        if self.look_id == 0 {
            self.look_id = door.closed_id;
        }
        self.doors.push(door);
    }

    /// Find the best door for a given variant and orientation.
    pub fn find_door(&self, variant: DoorVariant, orientation: DoorOrientation) -> Option<&DoorItem> {
        self.doors
            .iter()
            .find(|d| d.variant == variant && d.orientation == orientation)
            .or_else(|| self.doors.iter().find(|d| d.orientation == orientation))
            .or_else(|| self.doors.first())
    }
}

impl Brush for DoorBrush {
    fn id(&self) -> BrushId { self.brush_id }
    fn name(&self) -> &str { &self.brush_name }
    fn brush_type(&self) -> BrushType { BrushType::Door }
    fn look_id(&self) -> u16 { self.look_id }
    fn can_drag(&self) -> bool { false } // Doors are click-to-place

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            // Detect wall orientation by checking the tile and its neighbors for wall items
            let orientation = detect_wall_orientation(map, x, y, z);

            if let Some(door) = self.find_door(DoorVariant::Normal, orientation) {
                let tile = map.get_tile_mut(x, y, z);
                tile.items.push(pte_otbm::MapItem::new(door.closed_id));
                tiles_after.push((x, y, z, Some(tile.clone())));
            } else {
                tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
            }
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        let all_ids: std::collections::HashSet<u16> = self
            .doors
            .iter()
            .flat_map(|d| [d.open_id, d.closed_id])
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
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }
}

/// Detect wall orientation at a tile position by checking for wall-like items.
///
/// Heuristic: If the tile or its E/W neighbors have wall items, it's likely
/// horizontal. If the tile or its N/S neighbors have wall items, it's likely
/// vertical. Falls back to Horizontal.
///
/// Wall items are identified by checking their sprite IDs against common
/// wall ID ranges (this is imperfect without a registry, but works for most cases).
fn detect_wall_orientation(map: &MapData, x: u16, y: u16, z: u8) -> DoorOrientation {
    // Count wall-like items in horizontal vs vertical neighbors
    let has_east_wall = has_wall_item(map, x.wrapping_add(1), y, z);
    let has_west_wall = has_wall_item(map, x.wrapping_sub(1), y, z);
    let has_north_wall = has_wall_item(map, x, y.wrapping_sub(1), z);
    let has_south_wall = has_wall_item(map, x, y.wrapping_add(1), z);

    let h_score = has_east_wall as u8 + has_west_wall as u8;
    let v_score = has_north_wall as u8 + has_south_wall as u8;

    if v_score > h_score {
        DoorOrientation::Vertical
    } else {
        DoorOrientation::Horizontal
    }
}

/// Check if a tile has any items (likely walls) besides ground.
/// This is a simplified heuristic — items on a tile alongside a door placement
/// are most likely wall segments.
fn has_wall_item(map: &MapData, x: u16, y: u16, z: u8) -> bool {
    map.get_tile(x, y, z)
        .map(|t| !t.items.is_empty())
        .unwrap_or(false)
}
