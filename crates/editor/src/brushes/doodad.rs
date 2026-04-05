//! Doodad brush — multi-tile composite item placement.
//!
//! Doodads are decorative objects that span multiple tiles (e.g., a 2×2 fountain,
//! a sand dune, tree patches). Each composite has weighted random variants.

use super::{Brush, BrushId, BrushStroke, BrushType};
use crate::state::UndoAction;
use pte_otbm::MapData;
use rand::Rng;

/// A single item placed at a relative offset within a composite.
#[derive(Debug, Clone)]
pub struct CompositeEntry {
    pub dx: i32,
    pub dy: i32,
    pub item_id: u16,
}

/// A composite — one possible multi-tile arrangement.
#[derive(Debug, Clone)]
pub struct Composite {
    pub entries: Vec<CompositeEntry>,
    pub chance: u32,
}

/// Doodad brush with multiple composite variants.
pub struct DoodadBrush {
    pub brush_id: BrushId,
    pub brush_name: String,
    pub look_id: u16,
    /// Whether the doodad can be dragged (painted continuously).
    pub draggable: bool,
    /// Whether it's placed on blocking terrain.
    pub on_blocking: bool,
    /// Whether to redo borders after placement.
    pub redo_borders: bool,
    /// Whether it's a single-tile doodad (1×1).
    pub one_size: bool,
    /// Single-tile items for one_size doodads (weighted random).
    pub single_items: Vec<(u16, u32)>,
    /// Multi-tile composites.
    pub composites: Vec<Composite>,
    /// Total weight for composites.
    pub total_composite_weight: u32,
    /// Total weight for single items.
    pub total_single_weight: u32,
}

impl DoodadBrush {
    pub fn new(id: BrushId, name: String, look_id: u16) -> Self {
        Self {
            brush_id: id,
            brush_name: name,
            look_id,
            draggable: false,
            on_blocking: false,
            redo_borders: false,
            one_size: false,
            single_items: Vec::new(),
            composites: Vec::new(),
            total_composite_weight: 0,
            total_single_weight: 0,
        }
    }

    pub fn add_single_item(&mut self, id: u16, chance: u32) {
        self.total_single_weight += chance;
        self.single_items.push((id, chance));
    }

    pub fn add_composite(&mut self, composite: Composite) {
        self.total_composite_weight += composite.chance;
        self.composites.push(composite);
    }

    /// Pick a random single item.
    fn random_single(&self) -> Option<u16> {
        if self.single_items.is_empty() {
            return None;
        }
        let mut rng = rand::rng();
        let roll = rng.random_range(0..self.total_single_weight.max(1));
        let mut accum = 0;
        for &(id, chance) in &self.single_items {
            accum += chance;
            if roll < accum {
                return Some(id);
            }
        }
        Some(self.single_items.last().unwrap().0)
    }

    /// Pick a random composite.
    fn random_composite(&self) -> Option<&Composite> {
        if self.composites.is_empty() {
            return None;
        }
        let mut rng = rand::rng();
        let roll = rng.random_range(0..self.total_composite_weight.max(1));
        let mut accum = 0;
        for comp in &self.composites {
            accum += comp.chance;
            if roll < accum {
                return Some(comp);
            }
        }
        self.composites.last()
    }
}

impl Brush for DoodadBrush {
    fn id(&self) -> BrushId {
        self.brush_id
    }
    fn name(&self) -> &str {
        &self.brush_name
    }
    fn brush_type(&self) -> BrushType {
        BrushType::Doodad
    }
    fn look_id(&self) -> u16 {
        self.look_id
    }
    fn can_drag(&self) -> bool {
        self.draggable
    }
    fn needs_border_update(&self) -> bool {
        self.redo_borders
    }

    fn all_item_ids(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = self.single_items.iter().map(|&(id, _)| id).collect();
        for comp in &self.composites {
            for entry in &comp.entries {
                ids.push(entry.item_id);
            }
        }
        ids
    }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        let mut dirty = Vec::new();

        for &(cx, cy, z) in positions {
            if self.one_size || self.composites.is_empty() {
                // Single-tile placement
                if let Some(item_id) = self.random_single() {
                    let before = map.get_tile(cx, cy, z).cloned();
                    tiles_before.push((cx, cy, z, before));

                    let tile = map.get_tile_mut(cx, cy, z);
                    tile.items.push(pte_otbm::MapItem::new(item_id));
                    tiles_after.push((cx, cy, z, Some(tile.clone())));
                }
            } else {
                // Multi-tile composite
                if let Some(comp) = self.random_composite() {
                    for entry in &comp.entries {
                        let x = cx as i32 + entry.dx;
                        let y = cy as i32 + entry.dy;
                        if x < 0 || y < 0 || x > u16::MAX as i32 || y > u16::MAX as i32 {
                            continue;
                        }
                        let x = x as u16;
                        let y = y as u16;

                        let before = map.get_tile(x, y, z).cloned();
                        tiles_before.push((x, y, z, before));

                        let tile = map.get_tile_mut(x, y, z);
                        tile.items.push(pte_otbm::MapItem::new(entry.item_id));
                        tiles_after.push((x, y, z, Some(tile.clone())));

                        if self.redo_borders {
                            dirty.push((x, y, z));
                        }
                    }
                }
            }
        }

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

        // Collect all possible item IDs from this doodad
        let mut all_ids: std::collections::HashSet<u16> =
            self.single_items.iter().map(|&(id, _)| id).collect();
        for comp in &self.composites {
            for entry in &comp.entries {
                all_ids.insert(entry.item_id);
            }
        }

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
            undo: UndoAction {
                tiles_before,
                tiles_after,
            },
            dirty_positions: vec![],
        }
    }
}
