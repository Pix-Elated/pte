//! Lazy sprite sheet loader — loads sheets on-demand instead of all at startup.
//!
//! Keeps a catalog index for O(log n) sprite-to-sheet lookup via binary search.
//! Sheets are loaded and cached when first needed, evicted when over memory budget.

use crate::catalog::{ParsedCatalog, SpriteSheetInfo};
use crate::sprite_sheet::SpriteSheet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Memory budget for loaded sprite sheets (default 2 GB).
const DEFAULT_MEMORY_BUDGET: usize = 2 * 1024 * 1024 * 1024;

/// Lazy sprite sheet loader with LRU eviction.
pub struct LazySheetLoader {
    /// Directory containing sprite sheet files.
    asset_dir: PathBuf,
    /// Catalog entries sorted by first_sprite_id (for binary search).
    index: Vec<SpriteSheetInfo>,
    /// Loaded sheets keyed by file name.
    loaded: HashMap<String, SpriteSheet>,
    /// LRU generation counter per sheet file.
    access_gen: HashMap<String, u64>,
    /// Global generation counter.
    generation: u64,
    /// Current memory usage of loaded sheets (bytes).
    memory_used: usize,
    /// Maximum memory budget (bytes).
    memory_budget: usize,
}

impl LazySheetLoader {
    /// Create a new lazy loader from a parsed catalog.
    pub fn new(asset_dir: &Path, catalog: &ParsedCatalog) -> Self {
        let mut index = catalog.sprite_sheets.clone();
        index.sort_by_key(|s| s.first_sprite_id);

        Self {
            asset_dir: asset_dir.to_path_buf(),
            index,
            loaded: HashMap::new(),
            access_gen: HashMap::new(),
            generation: 0,
            memory_used: 0,
            memory_budget: DEFAULT_MEMORY_BUDGET,
        }
    }

    /// Set memory budget in bytes.
    pub fn set_memory_budget(&mut self, bytes: usize) {
        self.memory_budget = bytes;
    }

    /// Find which sheet info contains a given sprite ID (binary search).
    pub fn find_sheet_info(&self, sprite_id: u32) -> Option<&SpriteSheetInfo> {
        let idx = self.index.partition_point(|s| s.first_sprite_id <= sprite_id);
        if idx == 0 {
            return None;
        }
        let info = &self.index[idx - 1];
        if sprite_id >= info.first_sprite_id && sprite_id <= info.last_sprite_id {
            Some(info)
        } else {
            None
        }
    }

    /// Get a loaded sheet, loading it on-demand if necessary.
    /// Returns None if the sprite ID doesn't belong to any known sheet.
    pub fn get_sheet(&mut self, sprite_id: u32) -> Option<&SpriteSheet> {
        let info = self.find_sheet_info(sprite_id)?;
        let file = info.file.clone();

        if !self.loaded.contains_key(&file) {
            // Load the sheet
            let path = self.asset_dir.join(&file);
            let sheet = crate::decompress_sprite_sheet(&path, info)
                .map_err(|e| tracing::warn!("Failed to load sheet {}: {e:#}", file))
                .ok()?;

            let sheet_bytes = sheet.pixels.len();
            self.memory_used += sheet_bytes;
            self.loaded.insert(file.clone(), sheet);

            // Evict old sheets if over budget
            self.evict_if_needed();
        }

        // Update LRU
        self.access_gen.insert(file.clone(), self.generation);
        self.generation += 1;

        self.loaded.get(&file)
    }

    /// Get a mutable reference to a loaded sheet (for editing).
    /// Returns None if the sheet isn't loaded yet.
    pub fn get_sheet_mut(&mut self, file: &str) -> Option<&mut SpriteSheet> {
        self.loaded.get_mut(file)
    }

    /// Check if a sheet is currently loaded.
    pub fn is_loaded(&self, file: &str) -> bool {
        self.loaded.contains_key(file)
    }

    /// Total number of sheets in the catalog.
    pub fn total_sheets(&self) -> usize {
        self.index.len()
    }

    /// Number of currently loaded sheets.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    /// Current memory usage in bytes.
    pub fn memory_used(&self) -> usize {
        self.memory_used
    }

    /// Evict least-recently-used sheets until under memory budget.
    fn evict_if_needed(&mut self) {
        while self.memory_used > self.memory_budget && !self.loaded.is_empty() {
            // Find the sheet with the lowest generation (least recently used)
            let oldest = self.access_gen.iter()
                .filter(|(file, _)| self.loaded.contains_key(*file))
                .min_by_key(|(_, &gen)| gen)
                .map(|(file, _)| file.clone());

            if let Some(file) = oldest {
                if let Some(sheet) = self.loaded.remove(&file) {
                    self.memory_used -= sheet.pixels.len();
                    self.access_gen.remove(&file);
                    tracing::debug!("Evicted sheet {} ({} bytes)", file, sheet.pixels.len());
                }
            } else {
                break;
            }
        }
    }

    /// Pre-load specific sheets (e.g., for sprite editing).
    pub fn preload_sheet(&mut self, file: &str) -> Option<&SpriteSheet> {
        if let Some(info) = self.index.iter().find(|s| s.file == file) {
            let info = info.clone();
            if !self.loaded.contains_key(file) {
                let path = self.asset_dir.join(file);
                if let Ok(sheet) = crate::decompress_sprite_sheet(&path, &info) {
                    let bytes = sheet.pixels.len();
                    self.memory_used += bytes;
                    self.loaded.insert(file.to_string(), sheet);
                    self.evict_if_needed();
                }
            }
            self.access_gen.insert(file.to_string(), self.generation);
            self.generation += 1;
            self.loaded.get(file)
        } else {
            None
        }
    }

    /// Get a reference to all currently loaded sheets (for compatibility).
    pub fn loaded_sheets(&self) -> &HashMap<String, SpriteSheet> {
        &self.loaded
    }

    /// Get the catalog index.
    pub fn catalog_index(&self) -> &[SpriteSheetInfo] {
        &self.index
    }
}
