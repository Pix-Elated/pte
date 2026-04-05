//! Creature brush — places creatures and spawn points.
//!
//! Creatures in OT are stored in spawn.xml, not in the OTBM tile data.
//! These brushes write to the spawns Vec on EditorState (via pending actions)
//! and also ensure the tile exists so the map includes the position.

use pte_otbm::MapData;
use crate::state::UndoAction;
use super::{Brush, BrushId, BrushStroke, BrushType};

/// A creature definition.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CreatureDef {
    pub name: String,
    pub look_id: u16,
    pub spawn_time: u32, // in seconds
}

/// Creature brush — places a creature on the map.
/// The creature name and spawn_time are used when writing to spawn.xml.
pub struct CreatureBrush {
    pub brush_id: BrushId,
    pub creature: CreatureDef,
}

impl CreatureBrush {
    pub fn new(id: BrushId, creature: CreatureDef) -> Self {
        Self {
            brush_id: id,
            creature,
        }
    }
}

impl Brush for CreatureBrush {
    fn id(&self) -> BrushId { self.brush_id }
    fn name(&self) -> &str { &self.creature.name }
    fn brush_type(&self) -> BrushType { BrushType::Creature }
    fn look_id(&self) -> u16 { self.creature.look_id }
    fn can_drag(&self) -> bool { false }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));
            // Ensure tile exists so the position is serialized in OTBM
            let _ = map.get_tile_mut(x, y, z);
            tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            // Signal that spawn data should be updated for these positions
            dirty_positions: positions.to_vec(),
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));
            // Don't remove the tile — just remove the creature from spawn data
            tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: positions.to_vec(),
        }
    }
}

/// Spawn brush — creates a spawn area on the map.
#[allow(dead_code)]
pub struct SpawnBrush {
    pub brush_id: BrushId,
    pub radius: u8,
}

impl SpawnBrush {
    pub fn new(id: BrushId) -> Self {
        Self {
            brush_id: id,
            radius: 5,
        }
    }
}

impl Brush for SpawnBrush {
    fn id(&self) -> BrushId { self.brush_id }
    fn name(&self) -> &str { "Spawn" }
    fn brush_type(&self) -> BrushType { BrushType::Spawn }
    fn look_id(&self) -> u16 { 0 }
    fn can_drag(&self) -> bool { false }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));
            let _ = map.get_tile_mut(x, y, z);
            tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: positions.to_vec(),
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));
            tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: positions.to_vec(),
        }
    }
}

/// Waypoint brush — places a named waypoint.
/// Waypoints are stored in MapData.waypoints, not in tile data.
pub struct WaypointBrush {
    pub brush_id: BrushId,
    pub waypoint_name: String,
}

impl WaypointBrush {
    pub fn new(id: BrushId, name: String) -> Self {
        Self {
            brush_id: id,
            waypoint_name: name,
        }
    }
}

impl Brush for WaypointBrush {
    fn id(&self) -> BrushId { self.brush_id }
    fn name(&self) -> &str { &self.waypoint_name }
    fn brush_type(&self) -> BrushType { BrushType::Waypoint }
    fn look_id(&self) -> u16 { 0 }
    fn can_drag(&self) -> bool { false }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));
            let _ = map.get_tile_mut(x, y, z);
            tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));

            // Write waypoint into the map data itself
            let pos = pte_otbm::Position { x, y, z };
            // Remove existing waypoint at this position
            map.waypoints.retain(|w| w.position != pos);
            map.waypoints.push(pte_otbm::Waypoint {
                name: self.waypoint_name.clone(),
                position: pos,
            });
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
            tiles_after.push((x, y, z, map.get_tile(x, y, z).cloned()));

            // Remove waypoint at this position
            let pos = pte_otbm::Position { x, y, z };
            map.waypoints.retain(|w| w.position != pos);
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }
}
