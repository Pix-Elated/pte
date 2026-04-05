//! Brush system — RME-compatible brush architecture.
//!
//! Brush types: RAW, Ground (auto-border), Wall (auto-align), Table, Carpet,
//! Doodad (multi-tile), Door, Creature, Spawn, Waypoint, Flag, Eraser.

pub mod border;
pub mod carpet;
pub mod creature;
pub mod doodad;
pub mod door;
pub mod flag;
pub mod ground;
pub mod raw;
pub mod registry;
pub mod shape;
pub mod table;
pub mod wall;
pub mod xml_loader;

use crate::state::UndoAction;
use pte_otbm::MapData;

/// Brush identifier.
pub type BrushId = u32;

/// Outcome of drawing a brush onto the map.
pub struct BrushStroke {
    /// Undo action capturing before/after tile states.
    pub undo: UndoAction,
    /// Tiles whose neighbors need border recalculation.
    pub dirty_positions: Vec<(u16, u16, u8)>,
}

/// Process dirty tiles after a brush draw — recalculates auto-borders
/// and re-aligns walls/tables/carpets based on neighbors.
pub fn process_borders(
    map: &mut MapData,
    registry: &registry::BrushRegistry,
    dirty: &[(u16, u16, u8)],
) {
    for &(x, y, z) in dirty {
        // --- Ground auto-borders ---
        if let Some(ground_id) = map.get_tile(x, y, z).and_then(|t| t.ground) {
            if let Some(brush_id) = registry.ground_brush_id_for_item(ground_id) {
                if let Some(brush) = registry.get(brush_id) {
                    if let Some(auto_border) = brush.outer_border() {
                        let mask =
                            border::compute_neighbor_mask(x, y, z, Some(brush_id), |nx, ny, nz| {
                                map.get_tile(nx, ny, nz)
                                    .and_then(|t| t.ground)
                                    .and_then(|gid| registry.ground_brush_id_for_item(gid))
                            });

                        if mask != 0 {
                            let items = border::get_border_items(mask, auto_border);
                            let tile = map.get_tile_mut(x, y, z);
                            for item_id in items {
                                tile.items.push(pte_otbm::MapItem::new(item_id));
                            }
                        }
                    }
                }
            }
        }

        // --- Wall auto-alignment ---
        // Check each item on this tile. If it belongs to a wall brush,
        // replace it with the correctly aligned item.
        let tile_items: Vec<u16> = map
            .get_tile(x, y, z)
            .map(|t| t.items.iter().map(|i| i.id).collect())
            .unwrap_or_default();

        for item_id in tile_items {
            if let Some(wall_brush) = registry.wall_brush_for_item(item_id) {
                let wall_brush_id = wall_brush.id();
                let all_ids = wall_brush.all_item_ids();

                // Compute cardinal neighbor mask for wall alignment
                let mask = {
                    let mut m: u8 = 0;
                    let cardinals: [(i32, i32, u8); 4] =
                        [(0, -1, 0x01), (1, 0, 0x02), (0, 1, 0x04), (-1, 0, 0x08)];
                    for &(dx, dy, bit) in &cardinals {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                            let has_wall = map
                                .get_tile(nx as u16, ny as u16, z)
                                .map(|t| {
                                    t.items.iter().any(|i| {
                                        registry
                                            .wall_brush_for_item(i.id)
                                            .map(|wb| wb.id() == wall_brush_id)
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false);
                            if has_wall {
                                m |= bit;
                            }
                        }
                    }
                    m
                };

                // Get the correctly aligned item
                if let Some(new_id) = wall_brush.wall_item_for_mask(mask) {
                    if new_id != item_id {
                        // Replace the old wall item with the correctly aligned one
                        let tile = map.get_tile_mut(x, y, z);
                        for item in &mut tile.items {
                            if item.id == item_id && all_ids.contains(&item.id) {
                                item.id = new_id;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Common brush interface.
#[allow(dead_code)]
pub trait Brush: Send + Sync {
    fn id(&self) -> BrushId;
    fn name(&self) -> &str;
    fn brush_type(&self) -> BrushType;
    fn look_id(&self) -> u16;

    /// Draw this brush at the given positions.
    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke;

    /// Remove this brush from the given positions.
    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke;

    /// Whether this brush should trigger border recalculation on neighbors.
    fn needs_border_update(&self) -> bool {
        false
    }

    /// Whether the brush can be dragged (painted continuously).
    fn can_drag(&self) -> bool {
        true
    }

    /// Whether the brush applies to ground vs item stack.
    fn is_ground(&self) -> bool {
        false
    }

    /// Return the outer border definition for auto-border processing.
    /// Only ground brushes have this.
    fn outer_border(&self) -> Option<&border::AutoBorder> {
        None
    }

    /// Return the item ID for a given wall alignment mask (N=1,E=2,S=4,W=8).
    /// Only wall brushes implement this.
    fn wall_item_for_mask(&self, _mask: u8) -> Option<u16> {
        None
    }

    /// Return all item IDs that belong to this brush.
    fn all_item_ids(&self) -> Vec<u16> {
        vec![self.look_id()]
    }

    /// Return a representative item ID for preview/ghost rendering.
    fn preview_item_id(&self) -> Option<u16> {
        let id = self.look_id();
        if id > 0 {
            Some(id)
        } else {
            None
        }
    }
}

/// Discriminant for brush type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum BrushType {
    Raw,
    Ground,
    Wall,
    WallDecoration,
    Table,
    Carpet,
    Doodad,
    Door,
    Creature,
    Spawn,
    Waypoint,
    Flag,
    Eraser,
}

impl BrushType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Raw => "Raw",
            Self::Ground => "Ground",
            Self::Wall => "Wall",
            Self::WallDecoration => "Wall Decoration",
            Self::Table => "Table",
            Self::Carpet => "Carpet",
            Self::Doodad => "Doodad",
            Self::Door => "Door",
            Self::Creature => "Creature",
            Self::Spawn => "Spawn",
            Self::Waypoint => "Waypoint",
            Self::Flag => "Flag",
            Self::Eraser => "Eraser",
        }
    }
}
