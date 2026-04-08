//! Viewport-driven lazy chunk cache.
//!
//! Manages loading and evicting 256x256-tile file chunks based on the current
//! camera position so that only the visible portion of a large map is kept in
//! memory at any time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use pte_otbm::chunk_io::{ChunkManifest, ManifestChunkEntry};
use pte_otbm::MapData;

/// Size of a file chunk in tiles (each axis).
const FILE_CHUNK_SIZE: u16 = 256;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Identifies a 256x256 file chunk on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileChunkKey {
    pub z: u8,
    pub base_x: u16,
    pub base_y: u16,
}

/// Lifecycle state of a file chunk.
#[derive(Debug, Clone, Copy)]
pub enum ChunkStatus {
    /// Not yet loaded – data resides only on disk.
    Unloaded,
    /// A background load request has been issued.
    Loading,
    /// Fully loaded into the in-memory `MapData`.
    Loaded { last_access_frame: u64 },
    /// Loaded *and* modified by the user – must not be evicted without saving.
    Dirty { last_access_frame: u64 },
}

// ---------------------------------------------------------------------------
// ChunkCache
// ---------------------------------------------------------------------------

pub struct ChunkCache {
    /// Per-chunk lifecycle state.
    pub status: HashMap<FileChunkKey, ChunkStatus>,
    /// Maps each file chunk to its full path on disk.
    pub index: HashMap<FileChunkKey, PathBuf>,
    /// Sender for batched load requests to the background thread.
    load_tx: mpsc::Sender<Vec<FileChunkKey>>,
    /// Receiver for completed loads coming back from the background thread.
    result_rx: mpsc::Receiver<(FileChunkKey, MapData)>,
    /// Monotonically increasing frame counter (bumped each tick).
    pub frame: u64,
    /// Maximum number of loaded (non-dirty) chunks before eviction kicks in.
    pub max_loaded: usize,
    /// Extra 256-tile rings around the viewport to preload.
    pub buffer_rings: u32,
    /// World dimensions from the manifest.
    pub world_width: u16,
    pub world_height: u16,
}

impl ChunkCache {
    /// Build a new cache from a chunk manifest.
    ///
    /// Spawns a background loader thread that reads OTBM files and sends the
    /// parsed `MapData` back through a channel.
    pub fn new(manifest: &ChunkManifest, chunk_dir: &Path) -> Self {
        let mut index = HashMap::new();
        let mut status = HashMap::new();

        for entry in &manifest.chunks {
            let key = FileChunkKey {
                z: entry.z,
                base_x: entry.base_x,
                base_y: entry.base_y,
            };
            let full_path = chunk_dir.join(&entry.path);
            index.insert(key, full_path);
            status.insert(key, ChunkStatus::Unloaded);
        }

        // Channels
        let (load_tx, load_rx) = mpsc::channel::<Vec<FileChunkKey>>();
        let (result_tx, result_rx) = mpsc::channel::<(FileChunkKey, MapData)>();

        // Clone the index for the background thread.
        let bg_index = index.clone();

        // Background loader thread — uses rayon for intra-batch parallelism.
        std::thread::Builder::new()
            .name("chunk-loader".into())
            .spawn(move || {
                use rayon::prelude::*;

                while let Ok(batch) = load_rx.recv() {
                    let tx = result_tx.clone();
                    let idx = &bg_index;
                    batch.par_iter().for_each(|key| {
                        if let Some(path) = idx.get(key) {
                            if let Ok(data) = std::fs::read(path) {
                                if let Ok(map) = pte_otbm::parse_otbm_bytes(&data) {
                                    let _ = tx.send((*key, map));
                                }
                            }
                        }
                    });
                }
            })
            .expect("failed to spawn chunk-loader thread");

        Self {
            status,
            index,
            load_tx,
            result_rx,
            frame: 0,
            max_loaded: 200,
            buffer_rings: 2,
            world_width: manifest.world_width,
            world_height: manifest.world_height,
        }
    }

    // ------------------------------------------------------------------
    // Frame lifecycle
    // ------------------------------------------------------------------

    /// Advance the frame counter (call once per frame).
    pub fn tick_frame(&mut self) {
        self.frame += 1;
    }

    /// Drain all completed loads from the background thread and merge them
    /// into the live `MapData`.
    pub fn drain_loaded(&mut self, map: &mut MapData) {
        while let Ok((key, chunk_map)) = self.result_rx.try_recv() {
            map.merge_chunk(chunk_map);
            self.status.insert(
                key,
                ChunkStatus::Loaded {
                    last_access_frame: self.frame,
                },
            );
        }
    }

    /// Compute which file chunks are visible (plus buffer rings) and request
    /// any that are still `Unloaded`.
    pub fn request_visible(
        &mut self,
        cam_x: f32,
        cam_y: f32,
        cam_z: u8,
        viewport_w: f32,
        viewport_h: f32,
        zoom: f32,
    ) {
        let visible = self.visible_keys(cam_x, cam_y, cam_z, viewport_w, viewport_h, zoom);

        let mut to_load: Vec<FileChunkKey> = Vec::new();

        for key in &visible {
            if let Some(status) = self.status.get_mut(key) {
                match status {
                    ChunkStatus::Unloaded => {
                        *status = ChunkStatus::Loading;
                        to_load.push(*key);
                    }
                    ChunkStatus::Loaded { ref mut last_access_frame }
                    | ChunkStatus::Dirty { ref mut last_access_frame } => {
                        *last_access_frame = self.frame;
                    }
                    ChunkStatus::Loading => {}
                }
            }
        }

        if !to_load.is_empty() {
            let _ = self.load_tx.send(to_load);
        }
    }

    /// Evict the oldest `Loaded` (non-dirty) chunks when we exceed
    /// `max_loaded`, removing their tiles from `map`.
    pub fn evict_stale(&mut self, map: &mut MapData) {
        let loaded_count = self
            .status
            .values()
            .filter(|s| matches!(s, ChunkStatus::Loaded { .. }))
            .count();

        if loaded_count <= self.max_loaded {
            return;
        }

        // Evict one at a time — find the oldest non-dirty loaded chunk.
        let to_evict = loaded_count - self.max_loaded;
        for _ in 0..to_evict {
            let oldest = self
                .status
                .iter()
                .filter_map(|(k, s)| match s {
                    ChunkStatus::Loaded { last_access_frame } => Some((*k, *last_access_frame)),
                    _ => None,
                })
                .min_by_key(|&(_, frame)| frame);

            if let Some((key, _)) = oldest {
                map.evict_file_chunk(key.z, key.base_x, key.base_y);
                self.status.insert(key, ChunkStatus::Unloaded);
            }
        }
    }

    // ------------------------------------------------------------------
    // Dirty tracking
    // ------------------------------------------------------------------

    /// Mark a file chunk as dirty (user has edited tiles inside it).
    pub fn mark_dirty(&mut self, key: FileChunkKey) {
        if let Some(s) = self.status.get(&key) {
            match s {
                ChunkStatus::Loaded { last_access_frame }
                | ChunkStatus::Dirty { last_access_frame } => {
                    let frame = *last_access_frame;
                    self.status.insert(
                        key,
                        ChunkStatus::Dirty {
                            last_access_frame: frame,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    pub fn status(&self, key: &FileChunkKey) -> &ChunkStatus {
        self.status.get(key).unwrap_or(&ChunkStatus::Unloaded)
    }

    /// Returns `true` if any chunk is currently being loaded in the
    /// background.
    pub fn any_loading(&self) -> bool {
        self.status
            .values()
            .any(|s| matches!(s, ChunkStatus::Loading))
    }

    /// Number of chunks currently loaded in memory (includes dirty).
    pub fn loaded_count(&self) -> usize {
        self.status
            .values()
            .filter(|s| matches!(s, ChunkStatus::Loaded { .. } | ChunkStatus::Dirty { .. }))
            .count()
    }

    /// Total number of file chunks known from the manifest.
    pub fn total_chunks(&self) -> usize {
        self.index.len()
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Compute the set of `FileChunkKey`s visible from the current camera
    /// state, including buffer rings.
    fn visible_keys(
        &self,
        cam_x: f32,
        cam_y: f32,
        cam_z: u8,
        viewport_w: f32,
        viewport_h: f32,
        zoom: f32,
    ) -> Vec<FileChunkKey> {
        // tile_px = 32.0 * zoom  →  visible half-extent in tiles = viewport / (2 * tile_px)
        let tile_px = 32.0 * zoom;
        if tile_px <= 0.0 {
            return Vec::new();
        }

        let half_w_tiles = viewport_w / (2.0 * tile_px);
        let half_h_tiles = viewport_h / (2.0 * tile_px);

        let buffer = (self.buffer_rings * FILE_CHUNK_SIZE as u32) as f32;

        let min_x = (cam_x - half_w_tiles - buffer).max(0.0) as u32;
        let max_x = (cam_x + half_w_tiles + buffer).min(self.world_width as f32) as u32;
        let min_y = (cam_y - half_h_tiles - buffer).max(0.0) as u32;
        let max_y = (cam_y + half_h_tiles + buffer).min(self.world_height as f32) as u32;

        // Align to FILE_CHUNK_SIZE boundaries.
        let chunk_x_start = (min_x / FILE_CHUNK_SIZE as u32) * FILE_CHUNK_SIZE as u32;
        let chunk_y_start = (min_y / FILE_CHUNK_SIZE as u32) * FILE_CHUNK_SIZE as u32;

        let mut keys = Vec::new();
        let mut bx = chunk_x_start;
        while bx <= max_x {
            let mut by = chunk_y_start;
            while by <= max_y {
                let key = FileChunkKey {
                    z: cam_z,
                    base_x: bx as u16,
                    base_y: by as u16,
                };
                // Only include keys that actually exist in the index.
                if self.index.contains_key(&key) {
                    keys.push(key);
                }
                by += FILE_CHUNK_SIZE as u32;
            }
            bx += FILE_CHUNK_SIZE as u32;
        }

        keys
    }
}
