//! Ground brush — terrain painting with auto-borders, weighted random items,
//! z-order priority, and friend/enemy system.
//!
//! Replicates RME's GroundBrush, the most complex and valuable brush type.

use super::border::{compute_neighbor_mask, get_border_items, AutoBorder, NEIGHBOR_OFFSETS};
use super::{Brush, BrushId, BrushStroke, BrushType};
use crate::state::UndoAction;
use pte_otbm::MapData;
use rand::Rng;

/// A weighted item entry for random ground selection.
#[derive(Debug, Clone)]
pub struct WeightedItem {
    pub id: u16,
    pub chance: u32,
}

/// How this brush relates to another brush for bordering purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BrushRelation {
    /// Same brush or a declared friend — no border needed.
    Friend,
    /// Different brush — border required.
    Enemy,
}

/// Ground brush with full RME-compatible features.
#[allow(dead_code)]
pub struct GroundBrush {
    pub brush_id: BrushId,
    pub brush_name: String,
    pub look_id: u16,
    pub z_order: i32,

    /// Weighted item list for random ground tile selection.
    pub items: Vec<WeightedItem>,
    /// Total weight sum for random selection.
    pub total_weight: u32,

    /// Outer border definition (placed on neighboring tiles of lower z-order).
    pub outer_border: Option<AutoBorder>,
    /// Inner border definition (placed on this tile toward same terrain).
    pub inner_border: Option<AutoBorder>,
    /// Optional border (e.g., mountain gravel).
    pub optional_border: Option<AutoBorder>,

    /// Brush names that blend seamlessly with this one.
    pub friends: Vec<String>,
    /// If true, all brushes are enemies (borders everywhere).
    pub enemy_all: bool,
    /// If true, all brushes are friends (no borders anywhere).
    pub friend_all: bool,
}

#[allow(dead_code)]
impl GroundBrush {
    pub fn new(id: BrushId, name: String, look_id: u16, z_order: i32) -> Self {
        Self {
            brush_id: id,
            brush_name: name,
            look_id,
            z_order,
            items: Vec::new(),
            total_weight: 0,
            outer_border: None,
            inner_border: None,
            optional_border: None,
            friends: Vec::new(),
            enemy_all: false,
            friend_all: false,
        }
    }

    /// Add a weighted random item.
    pub fn add_item(&mut self, id: u16, chance: u32) {
        self.total_weight += chance;
        self.items.push(WeightedItem { id, chance });
    }

    /// Pick a random ground tile from the weighted item list.
    pub fn random_item(&self) -> u16 {
        if self.items.is_empty() {
            return self.look_id;
        }
        if self.total_weight == 0 || self.items.len() == 1 {
            return self.items[0].id;
        }

        let mut rng = rand::rng();
        let roll = rng.random_range(0..self.total_weight);
        let mut accum = 0;
        for item in &self.items {
            accum += item.chance;
            if roll < accum {
                return item.id;
            }
        }
        self.items.last().unwrap().id
    }

    /// Check if this brush is friends with another.
    pub fn is_friend(&self, other_name: &str) -> bool {
        if self.friend_all {
            return true;
        }
        self.friends.iter().any(|f| f == other_name || f == "all")
    }

    /// Determine the relation to another brush.
    pub fn relation(&self, other_name: &str) -> BrushRelation {
        if self.enemy_all {
            return BrushRelation::Enemy;
        }
        if self.is_friend(other_name) {
            BrushRelation::Friend
        } else {
            BrushRelation::Enemy
        }
    }

    /// Calculate and place border items on a tile and its neighbors.
    /// Returns the dirty positions (tiles that were modified).
    pub fn update_borders(
        &self,
        map: &mut MapData,
        x: u16,
        y: u16,
        z: u8,
        get_ground_brush: &dyn Fn(u16, u16, u8) -> Option<(BrushId, i32, String)>,
    ) -> Vec<(u16, u16, u8)> {
        let mut dirty = Vec::new();

        // Build neighbor info for the center tile and its ring
        let _center_info = get_ground_brush(x, y, z);

        // Update borders on center tile
        if let Some(ref border) = self.outer_border {
            let mask = compute_neighbor_mask(x, y, z, Some(self.brush_id), |nx, ny, nz| {
                get_ground_brush(nx, ny, nz).map(|(id, _, _)| id)
            });

            if mask != 0 {
                let items = get_border_items(mask, border);
                // Remove old border items for this brush, add new ones
                let tile = map.get_tile_mut(x, y, z);
                // We'll tag border items with a special marker in the future;
                // for now, just add them
                for &item_id in &items {
                    tile.items.push(pte_otbm::MapItem::new(item_id));
                }
                dirty.push((x, y, z));
            }
        }

        // Also update each neighbor's borders
        for &(dx, dy) in &NEIGHBOR_OFFSETS {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx > u16::MAX as i32 || ny > u16::MAX as i32 {
                continue;
            }
            let nx = nx as u16;
            let ny = ny as u16;

            if let Some((_nid, _nz, _nname)) = get_ground_brush(nx, ny, z) {
                dirty.push((nx, ny, z));
            }
        }

        dirty
    }
}

impl Brush for GroundBrush {
    fn id(&self) -> BrushId {
        self.brush_id
    }
    fn name(&self) -> &str {
        &self.brush_name
    }
    fn brush_type(&self) -> BrushType {
        BrushType::Ground
    }
    fn look_id(&self) -> u16 {
        self.look_id
    }
    fn is_ground(&self) -> bool {
        true
    }
    fn needs_border_update(&self) -> bool {
        true
    }
    fn outer_border(&self) -> Option<&AutoBorder> {
        self.outer_border.as_ref()
    }

    fn all_item_ids(&self) -> Vec<u16> {
        self.items.iter().map(|w| w.id).collect()
    }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        let mut dirty = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            let tile = map.get_tile_mut(x, y, z);
            tile.ground = Some(self.random_item());
            tiles_after.push((x, y, z, Some(tile.clone())));

            // Mark this tile and its 8 neighbors as needing border recalc
            dirty.push((x, y, z));
            for &(dx, dy) in &NEIGHBOR_OFFSETS {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32 {
                    dirty.push((nx as u16, ny as u16, z));
                }
            }
        }

        // Deduplicate dirty list
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

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before.clone()));

            // Check if this tile's ground belongs to this brush
            if let Some(tile) = &before {
                if let Some(ground_id) = tile.ground {
                    let is_ours =
                        self.items.iter().any(|w| w.id == ground_id) || ground_id == self.look_id;
                    if is_ours {
                        let tile_mut = map.get_tile_mut(x, y, z);
                        tile_mut.ground = None;
                        // Remove border items too (in the future with proper tracking)
                        if tile_mut.ground.is_none() && tile_mut.items.is_empty() {
                            map.remove_tile(x, y, z);
                            tiles_after.push((x, y, z, None));
                        } else {
                            tiles_after.push((x, y, z, Some(tile_mut.clone())));
                        }

                        dirty.push((x, y, z));
                        for &(dx, dy) in &NEIGHBOR_OFFSETS {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx >= 0 && ny >= 0 && nx <= u16::MAX as i32 && ny <= u16::MAX as i32
                            {
                                dirty.push((nx as u16, ny as u16, z));
                            }
                        }
                        continue;
                    }
                }
            }

            tiles_after.push((x, y, z, before));
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
