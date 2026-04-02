//! Brush registry — central store mapping item IDs to brushes.
//!
//! Loaded from XML materials (RME format) or built programmatically.
//! Provides O(1) lookup from item ID to brush, plus brush palettes/categories.

use std::collections::HashMap;
use std::sync::Arc;

use super::{Brush, BrushId, BrushType};

/// A palette category grouping brushes for the UI.
#[derive(Debug, Clone)]
pub struct BrushPalette {
    pub name: String,
    pub brushes: Vec<BrushId>,
}

/// A tileset — a named collection of palettes (matching RME's tileset concept).
#[derive(Debug, Clone)]
pub struct Tileset {
    pub name: String,
    pub palettes: Vec<BrushPalette>,
}

/// The brush registry.
#[allow(dead_code)]
pub struct BrushRegistry {
    /// All registered brushes, keyed by BrushId.
    brushes: HashMap<BrushId, Arc<dyn Brush>>,
    /// Reverse mapping: brush name → BrushId.
    name_to_id: HashMap<String, BrushId>,
    /// Mapping: item ID → BrushId (for ground items).
    item_to_ground_brush: HashMap<u16, BrushId>,
    /// Mapping: item ID → BrushId (for wall items).
    item_to_wall_brush: HashMap<u16, BrushId>,
    /// Mapping: item ID → BrushId (for table items).
    item_to_table_brush: HashMap<u16, BrushId>,
    /// Mapping: item ID → BrushId (for carpet items).
    item_to_carpet_brush: HashMap<u16, BrushId>,
    /// Mapping: item ID → BrushId (for doodad items).
    item_to_doodad_brush: HashMap<u16, BrushId>,
    /// All tilesets for UI organization.
    tilesets: Vec<Tileset>,
    /// Next auto-assigned ID.
    next_id: BrushId,
}

#[allow(dead_code)]
impl BrushRegistry {
    pub fn new() -> Self {
        Self {
            brushes: HashMap::new(),
            name_to_id: HashMap::new(),
            item_to_ground_brush: HashMap::new(),
            item_to_wall_brush: HashMap::new(),
            item_to_table_brush: HashMap::new(),
            item_to_carpet_brush: HashMap::new(),
            item_to_doodad_brush: HashMap::new(),
            tilesets: Vec::new(),
            next_id: 1,
        }
    }

    /// Assign the next available BrushId.
    pub fn next_brush_id(&mut self) -> BrushId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Register a brush.
    pub fn register(&mut self, brush: Arc<dyn Brush>) {
        let id = brush.id();
        let name = brush.name().to_string();
        self.name_to_id.insert(name, id);
        self.brushes.insert(id, brush);
    }

    /// Register item-to-brush mapping for ground brushes.
    pub fn register_ground_item(&mut self, item_id: u16, brush_id: BrushId) {
        self.item_to_ground_brush.insert(item_id, brush_id);
    }

    /// Register item-to-brush mapping for wall brushes.
    pub fn register_wall_item(&mut self, item_id: u16, brush_id: BrushId) {
        self.item_to_wall_brush.insert(item_id, brush_id);
    }

    /// Register item-to-brush mapping for table brushes.
    pub fn register_table_item(&mut self, item_id: u16, brush_id: BrushId) {
        self.item_to_table_brush.insert(item_id, brush_id);
    }

    /// Register item-to-brush mapping for carpet brushes.
    pub fn register_carpet_item(&mut self, item_id: u16, brush_id: BrushId) {
        self.item_to_carpet_brush.insert(item_id, brush_id);
    }

    /// Register item-to-brush mapping for doodad brushes.
    pub fn register_doodad_item(&mut self, item_id: u16, brush_id: BrushId) {
        self.item_to_doodad_brush.insert(item_id, brush_id);
    }

    /// Add a tileset.
    pub fn add_tileset(&mut self, tileset: Tileset) {
        self.tilesets.push(tileset);
    }

    /// Look up a brush by ID.
    pub fn get(&self, id: BrushId) -> Option<&Arc<dyn Brush>> {
        self.brushes.get(&id)
    }

    /// Look up a brush by name.
    pub fn get_by_name(&self, name: &str) -> Option<&Arc<dyn Brush>> {
        self.name_to_id.get(name).and_then(|id| self.brushes.get(id))
    }

    /// Find which ground brush owns a given item ID.
    pub fn ground_brush_for_item(&self, item_id: u16) -> Option<&Arc<dyn Brush>> {
        self.item_to_ground_brush
            .get(&item_id)
            .and_then(|id| self.brushes.get(id))
    }

    /// Find which wall brush owns a given item ID.
    pub fn wall_brush_for_item(&self, item_id: u16) -> Option<&Arc<dyn Brush>> {
        self.item_to_wall_brush
            .get(&item_id)
            .and_then(|id| self.brushes.get(id))
    }

    /// Get the ground brush ID for a tile's ground item.
    pub fn ground_brush_id_for_item(&self, item_id: u16) -> Option<BrushId> {
        self.item_to_ground_brush.get(&item_id).copied()
    }

    /// Get all brushes of a given type.
    pub fn brushes_of_type(&self, brush_type: BrushType) -> Vec<&Arc<dyn Brush>> {
        self.brushes
            .values()
            .filter(|b| b.brush_type() == brush_type)
            .collect()
    }

    /// Get all tilesets.
    pub fn tilesets(&self) -> &[Tileset] {
        &self.tilesets
    }

    /// Total number of registered brushes.
    pub fn count(&self) -> usize {
        self.brushes.len()
    }

    /// Get all ground variant item IDs for a given ground item.
    /// Returns all items from the same ground brush if found.
    pub fn ground_variants(&self, item_id: u16) -> Vec<u16> {
        if let Some(brush_id) = self.item_to_ground_brush.get(&item_id) {
            if let Some(brush) = self.brushes.get(brush_id) {
                return brush.all_item_ids();
            }
        }
        vec![item_id]
    }

    /// Iterate all brushes.
    pub fn iter(&self) -> impl Iterator<Item = (&BrushId, &Arc<dyn Brush>)> {
        self.brushes.iter()
    }
}
