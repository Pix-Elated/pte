//! OTBM data types.

use std::collections::HashMap;

/// Chunk size for sparse tile storage.
pub const CHUNK_SIZE: i32 = 64;

/// OTBM node type constants.
pub mod node_type {
    pub const ROOT: u8 = 0;
    pub const MAP_DATA: u8 = 2;
    pub const TILE_AREA: u8 = 4;
    pub const TILE: u8 = 5;
    pub const ITEM: u8 = 6;
    pub const TOWNS: u8 = 12;
    pub const TOWN: u8 = 13;
    pub const HOUSE_TILE: u8 = 14;
    pub const WAYPOINTS: u8 = 15;
    pub const WAYPOINT: u8 = 16;
}

/// OTBM attribute IDs.
pub mod attr {
    pub const DESCRIPTION: u8 = 1;
    pub const EXT_FILE: u8 = 2;
    pub const TILE_FLAGS: u8 = 3;
    pub const ACTION_ID: u8 = 4;
    pub const UNIQUE_ID: u8 = 5;
    pub const TEXT: u8 = 6;
    pub const DESC: u8 = 7;
    pub const TELE_DEST: u8 = 8;
    pub const ITEM: u8 = 9;
    pub const DEPOT_ID: u8 = 10;
    pub const EXT_SPAWN_FILE: u8 = 11;
    pub const RUNE_CHARGES: u8 = 12;
    pub const EXT_HOUSE_FILE: u8 = 13;
    pub const HOUSEDOOR_ID: u8 = 14;
    pub const COUNT: u8 = 15;
    pub const DURATION: u8 = 16;
    pub const DECAYING_STATE: u8 = 17;
    pub const CHARGES: u8 = 21;
    pub const ATTRIBUTE_MAP: u8 = 128;
}

/// Tile flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct TileFlags {
    pub protection_zone: bool,
    pub no_pvp: bool,
    pub no_logout: bool,
    pub pvp_zone: bool,
    pub refresh: bool,
}

impl TileFlags {
    pub fn from_u32(v: u32) -> Self {
        Self {
            protection_zone: v & 1 != 0,
            no_pvp: v & 4 != 0,
            no_logout: v & 8 != 0,
            pvp_zone: v & 16 != 0,
            refresh: v & 32 != 0,
        }
    }

    pub fn to_u32(self) -> u32 {
        let mut v = 0u32;
        if self.protection_zone {
            v |= 1;
        }
        if self.no_pvp {
            v |= 4;
        }
        if self.no_logout {
            v |= 8;
        }
        if self.pvp_zone {
            v |= 16;
        }
        if self.refresh {
            v |= 32;
        }
        v
    }

    pub fn any(self) -> bool {
        self.protection_zone || self.no_pvp || self.no_logout || self.pvp_zone || self.refresh
    }
}

/// A position in the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

/// Teleport destination.
#[derive(Debug, Clone, Copy)]
pub struct TeleportDest {
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

/// An item on a tile.
#[derive(Debug, Clone)]
pub struct MapItem {
    pub id: u16,
    pub action_id: Option<u16>,
    pub unique_id: Option<u16>,
    pub text: Option<String>,
    pub description: Option<String>,
    pub count: Option<u8>,
    pub depot_id: Option<u16>,
    pub door_id: Option<u8>,
    pub tele_dest: Option<TeleportDest>,
    pub rune_charges: Option<u16>,
    pub charges: Option<u16>,
    pub duration: Option<u32>,
    /// Unknown/unrecognized attributes preserved for forward-compatibility.
    pub unknown_attrs: Vec<(u8, Vec<u8>)>,
}

impl MapItem {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            action_id: None,
            unique_id: None,
            text: None,
            description: None,
            count: None,
            depot_id: None,
            door_id: None,
            tele_dest: None,
            rune_charges: None,
            charges: None,
            duration: None,
            unknown_attrs: Vec::new(),
        }
    }
}

/// A single map tile.
#[derive(Debug, Clone)]
pub struct Tile {
    pub x: u16,
    pub y: u16,
    pub z: u8,
    pub ground: Option<u16>,
    pub items: Vec<MapItem>,
    pub flags: TileFlags,
    pub house_id: Option<u32>,
}

impl Tile {
    pub fn new(x: u16, y: u16, z: u8) -> Self {
        Self {
            x,
            y,
            z,
            ground: None,
            items: Vec::new(),
            flags: TileFlags::default(),
            house_id: None,
        }
    }
}

/// A town.
#[derive(Debug, Clone)]
pub struct Town {
    pub id: u32,
    pub name: String,
    pub position: Position,
}

/// A waypoint.
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub name: String,
    pub position: Position,
}

/// A house definition (metadata for house tiles).
#[derive(Debug, Clone)]
pub struct House {
    pub id: u32,
    pub name: String,
    pub rent: u32,
    pub town_id: u32,
    pub exit: Position,
}

/// Chunk key for sparse storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub cx: i32,
    pub cy: i32,
    pub z: u8,
}

/// The complete map data.
#[derive(Debug)]
pub struct MapData {
    pub width: u16,
    pub height: u16,
    pub description: String,
    pub spawn_file: String,
    pub house_file: String,
    pub version: u32,
    pub item_major_version: u32,
    pub item_minor_version: u32,
    /// Sparse tile storage: chunk_key → HashMap<(local_x, local_y), Tile>
    pub chunks: HashMap<ChunkKey, HashMap<(u8, u8), Tile>>,
    pub towns: Vec<Town>,
    pub waypoints: Vec<Waypoint>,
    pub houses: Vec<House>,
}

impl Default for MapData {
    fn default() -> Self {
        Self::new()
    }
}

impl MapData {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            description: String::new(),
            spawn_file: String::new(),
            house_file: String::new(),
            version: 0,
            item_major_version: 0,
            item_minor_version: 0,
            chunks: HashMap::new(),
            towns: Vec::new(),
            waypoints: Vec::new(),
            houses: Vec::new(),
        }
    }

    /// Get a tile at world coordinates.
    pub fn get_tile(&self, x: u16, y: u16, z: u8) -> Option<&Tile> {
        let key = ChunkKey {
            cx: x as i32 / CHUNK_SIZE,
            cy: y as i32 / CHUNK_SIZE,
            z,
        };
        let local = ((x as i32 % CHUNK_SIZE) as u8, (y as i32 % CHUNK_SIZE) as u8);
        self.chunks.get(&key)?.get(&local)
    }

    /// Get a mutable tile (creates chunk if needed).
    pub fn get_tile_mut(&mut self, x: u16, y: u16, z: u8) -> &mut Tile {
        let key = ChunkKey {
            cx: x as i32 / CHUNK_SIZE,
            cy: y as i32 / CHUNK_SIZE,
            z,
        };
        let local = ((x as i32 % CHUNK_SIZE) as u8, (y as i32 % CHUNK_SIZE) as u8);
        self.chunks
            .entry(key)
            .or_default()
            .entry(local)
            .or_insert_with(|| Tile::new(x, y, z))
    }

    /// Get a mutable tile only if it already exists (does not create).
    pub fn get_tile_mut_if_exists(&mut self, x: u16, y: u16, z: u8) -> Option<&mut Tile> {
        let key = ChunkKey {
            cx: x as i32 / CHUNK_SIZE,
            cy: y as i32 / CHUNK_SIZE,
            z,
        };
        let local = ((x as i32 % CHUNK_SIZE) as u8, (y as i32 % CHUNK_SIZE) as u8);
        self.chunks.get_mut(&key)?.get_mut(&local)
    }

    /// Set a tile at world coordinates.
    pub fn set_tile(&mut self, tile: Tile) {
        let key = ChunkKey {
            cx: tile.x as i32 / CHUNK_SIZE,
            cy: tile.y as i32 / CHUNK_SIZE,
            z: tile.z,
        };
        let local = (
            (tile.x as i32 % CHUNK_SIZE) as u8,
            (tile.y as i32 % CHUNK_SIZE) as u8,
        );
        self.chunks.entry(key).or_default().insert(local, tile);
    }

    /// Remove a tile.
    pub fn remove_tile(&mut self, x: u16, y: u16, z: u8) {
        let key = ChunkKey {
            cx: x as i32 / CHUNK_SIZE,
            cy: y as i32 / CHUNK_SIZE,
            z,
        };
        let local = ((x as i32 % CHUNK_SIZE) as u8, (y as i32 % CHUNK_SIZE) as u8);
        if let Some(chunk) = self.chunks.get_mut(&key) {
            chunk.remove(&local);
            if chunk.is_empty() {
                self.chunks.remove(&key);
            }
        }
    }

    /// Get all tiles in a rectangular area at a given z-level.
    pub fn get_tiles_in_area(&self, x1: u16, y1: u16, x2: u16, y2: u16, z: u8) -> Vec<&Tile> {
        let cx1 = x1 as i32 / CHUNK_SIZE;
        let cy1 = y1 as i32 / CHUNK_SIZE;
        let cx2 = x2 as i32 / CHUNK_SIZE;
        let cy2 = y2 as i32 / CHUNK_SIZE;

        let mut tiles = Vec::new();
        for cx in cx1..=cx2 {
            let is_edge_x = cx == cx1 || cx == cx2;
            for cy in cy1..=cy2 {
                let key = ChunkKey { cx, cy, z };
                if let Some(chunk) = self.chunks.get(&key) {
                    if is_edge_x || cy == cy1 || cy == cy2 {
                        // Edge chunk — filter tiles at boundaries
                        for tile in chunk.values() {
                            if tile.x >= x1 && tile.x <= x2 && tile.y >= y1 && tile.y <= y2 {
                                tiles.push(tile);
                            }
                        }
                    } else {
                        // Interior chunk — all tiles are within bounds
                        tiles.extend(chunk.values());
                    }
                }
            }
        }
        tiles
    }

    /// Total tile count.
    pub fn tile_count(&self) -> usize {
        self.chunks.values().map(|c| c.len()).sum()
    }

    /// Returns the set of z-levels that actually contain tiles, sorted ascending.
    pub fn occupied_z_levels(&self) -> Vec<u8> {
        let mut present = [false; 256];
        for key in self.chunks.keys() {
            present[key.z as usize] = true;
        }
        present.iter().enumerate()
            .filter(|(_, &p)| p)
            .map(|(i, _)| i as u8)
            .collect()
    }

    /// Returns (min_z, max_z) of tiles present in the map, or None if empty.
    pub fn z_range(&self) -> Option<(u8, u8)> {
        let mut min = u8::MAX;
        let mut max = 0u8;
        for key in self.chunks.keys() {
            if key.z < min {
                min = key.z;
            }
            if key.z > max {
                max = key.z;
            }
        }
        if min <= max {
            Some((min, max))
        } else {
            None
        }
    }

    /// Returns (min_x, min_y, max_x, max_y) tile extents for a given z-level.
    pub fn xy_extents(&self, z: u8) -> Option<(u16, u16, u16, u16)> {
        let mut min_x = u16::MAX;
        let mut min_y = u16::MAX;
        let mut max_x = 0u16;
        let mut max_y = 0u16;
        let mut found = false;
        for (key, chunk) in &self.chunks {
            if key.z != z {
                continue;
            }
            for tile in chunk.values() {
                min_x = min_x.min(tile.x);
                min_y = min_y.min(tile.y);
                max_x = max_x.max(tile.x);
                max_y = max_y.max(tile.y);
                found = true;
            }
        }
        if found {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }

    /// Count tiles belonging to a specific house.
    pub fn house_tile_count(&self, house_id: u32) -> usize {
        self.chunks
            .values()
            .flat_map(|c| c.values())
            .filter(|t| t.house_id == Some(house_id))
            .count()
    }

    /// Find house IDs referenced by tiles but not in the houses list.
    pub fn orphan_house_ids(&self) -> Vec<u32> {
        let defined: std::collections::HashSet<u32> = self.houses.iter().map(|h| h.id).collect();
        let mut used: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for chunk in self.chunks.values() {
            for tile in chunk.values() {
                if let Some(hid) = tile.house_id {
                    if !defined.contains(&hid) {
                        used.insert(hid);
                    }
                }
            }
        }
        let mut v: Vec<u32> = used.into_iter().collect();
        v.sort_unstable();
        v
    }

    /// Merge all tiles from another MapData into self.
    pub fn merge_chunk(&mut self, other: MapData) {
        for (key, tiles) in other.chunks {
            self.chunks.entry(key).or_default().extend(tiles);
        }
    }

    /// Evict all internal chunks belonging to a 256x256 file chunk area.
    pub fn evict_file_chunk(&mut self, z: u8, base_x: u16, base_y: u16) {
        let cx_start = base_x as i32 / CHUNK_SIZE;
        let cy_start = base_y as i32 / CHUNK_SIZE;
        // 256 / 64 = 4 internal chunks per dimension
        for cx in cx_start..cx_start + 4 {
            for cy in cy_start..cy_start + 4 {
                self.chunks.remove(&ChunkKey { cx, cy, z });
            }
        }
    }
}
