//! Chunk directory I/O for X-Trails map system.
//!
//! Each chunk is a standard OTBM file containing a single TILE_AREA (256x256 tiles).
//! A chunk directory contains:
//! - `manifest.json` with world metadata, towns, waypoints
//! - `z{N}/XXXXX_YYYYY.otbm` chunk files organized by z-level

use crate::types::*;
use crate::serialize_otbm;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::Path;

/// Chunk tile area size (matches OTBM TILE_AREA alignment).
const CHUNK_AREA_SIZE: u16 = 256;

/// Manifest JSON structure for chunk directories.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkManifest {
    pub version: u32,
    #[serde(default)]
    pub generator: String,
    #[serde(rename = "worldWidth")]
    pub world_width: u16,
    #[serde(rename = "worldHeight")]
    pub world_height: u16,
    #[serde(rename = "chunkSize", default = "default_chunk_size")]
    pub chunk_size: u16,
    #[serde(rename = "surfaceZ", default)]
    pub surface_z: u8,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "spawnFile", default)]
    pub spawn_file: String,
    #[serde(rename = "houseFile", default)]
    pub house_file: String,
    #[serde(default)]
    pub towns: Vec<ManifestTown>,
    #[serde(default)]
    pub waypoints: Vec<ManifestWaypoint>,
    #[serde(default)]
    pub chunks: Vec<ManifestChunkEntry>,
}

fn default_chunk_size() -> u16 {
    256
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestTown {
    pub id: u32,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestWaypoint {
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestChunkEntry {
    pub z: u8,
    #[serde(rename = "baseX")]
    pub base_x: u16,
    #[serde(rename = "baseY")]
    pub base_y: u16,
    pub path: String,
}

/// Load a chunk directory into MapData.
///
/// Reads `manifest.json` for metadata, then loads each chunk OTBM file.
/// All tiles merge into a single MapData.
pub fn load_chunk_dir(dir: &Path) -> anyhow::Result<MapData> {
    let manifest_path = dir.join("manifest.json");
    let manifest: ChunkManifest = if manifest_path.exists() {
        let data = std::fs::read_to_string(&manifest_path)?;
        serde_json::from_str(&data)?
    } else {
        // No manifest — scan directory for .otbm files
        return load_chunk_dir_no_manifest(dir);
    };

    let mut map = MapData::new();
    map.width = manifest.world_width;
    map.height = manifest.world_height;
    map.description = manifest.description.clone();
    map.spawn_file = manifest.spawn_file.clone();
    map.house_file = manifest.house_file.clone();
    map.version = 3;
    map.item_major_version = 3;
    map.item_minor_version = 56;

    // Load towns from manifest
    for t in &manifest.towns {
        map.towns.push(Town {
            id: t.id,
            name: t.name.clone(),
            position: Position {
                x: t.x,
                y: t.y,
                z: t.z,
            },
        });
    }

    // Load waypoints from manifest
    for wp in &manifest.waypoints {
        map.waypoints.push(Waypoint {
            name: wp.name.clone(),
            position: Position {
                x: wp.x,
                y: wp.y,
                z: wp.z,
            },
        });
    }

    // Load all chunk OTBMs in parallel, then merge
    let chunk_paths: Vec<_> = manifest.chunks.iter()
        .map(|entry| dir.join(&entry.path))
        .filter(|p| p.exists())
        .collect();

    let total = chunk_paths.len();
    tracing::info!("Loading {} chunks in parallel from {}...", total, dir.display());

    let chunk_maps: Vec<_> = chunk_paths.par_iter()
        .filter_map(|chunk_path| {
            let data = std::fs::read(chunk_path).ok()?;
            crate::reader::parse_otbm_bytes(&data).ok()
        })
        .collect();

    // Merge all chunk maps into the main map (sequential — HashMap merge)
    let mut loaded = 0;
    for chunk_map in chunk_maps {
        for (key, chunk_tiles) in chunk_map.chunks {
            let entry = map.chunks.entry(key).or_default();
            for (local, tile) in chunk_tiles {
                entry.insert(local, tile);
            }
        }
        loaded += 1;
    }

    tracing::info!(
        "Loaded {} chunks from {}, {} tiles total",
        loaded,
        dir.display(),
        map.tile_count()
    );
    Ok(map)
}

/// Load a chunk directory without a manifest (scan for .otbm files).
fn load_chunk_dir_no_manifest(dir: &Path) -> anyhow::Result<MapData> {
    let mut map = MapData::new();
    map.version = 3;
    map.item_major_version = 3;
    map.item_minor_version = 56;

    let mut loaded = 0;
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("otbm") {
            load_chunk_into(&mut map, path)?;
            loaded += 1;
        }
    }

    // Compute dimensions from loaded tiles
    if let Some((min_x, min_y, max_x, max_y)) = map
        .occupied_z_levels()
        .first()
        .and_then(|&z| map.xy_extents(z))
    {
        map.width = max_x.saturating_sub(min_x) + 1;
        map.height = max_y.saturating_sub(min_y) + 1;
    }

    tracing::info!(
        "Loaded {} chunks (no manifest), {} tiles",
        loaded,
        map.tile_count()
    );
    Ok(map)
}

/// Load a single chunk OTBM file and merge its tiles into an existing MapData.
pub fn load_chunk_into(map: &mut MapData, chunk_path: &Path) -> anyhow::Result<()> {
    let data = std::fs::read(chunk_path)?;
    let chunk_map = crate::reader::parse_otbm_bytes(&data)?;

    // Merge tiles
    for (key, chunk_tiles) in chunk_map.chunks {
        let entry = map.chunks.entry(key).or_default();
        for (local, tile) in chunk_tiles {
            entry.insert(local, tile);
        }
    }

    Ok(())
}

/// Save MapData as a chunk directory.
///
/// Groups tiles by 256x256 TILE_AREA boundaries. Each area becomes a
/// separate mini-OTBM file. Writes manifest.json with metadata.
pub fn save_chunk_dir(map: &MapData, dir: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(dir)?;

    // Group tiles by (z, area_base_x, area_base_y)
    let mut areas: BTreeMap<(u8, u16, u16), Vec<&Tile>> = BTreeMap::new();

    for chunk_tiles in map.chunks.values() {
        for tile in chunk_tiles.values() {
            let area_x = tile.x & 0xFF00; // align to 256
            let area_y = tile.y & 0xFF00;
            areas
                .entry((tile.z, area_x, area_y))
                .or_default()
                .push(tile);
        }
    }

    let mut chunk_entries = Vec::new();

    for (&(z, base_x, base_y), tiles) in &areas {
        // Create z-level directory
        let z_dir = dir.join(format!("z{}", z));
        std::fs::create_dir_all(&z_dir)?;

        let filename = format!(
            "{}_{}.otbm",
            format!("{:05}", base_x),
            format!("{:05}", base_y)
        );
        let chunk_path = z_dir.join(&filename);
        let rel_path = format!("z{}/{}", z, filename);

        // Build a mini MapData for this chunk
        let mut chunk_map = MapData::new();
        chunk_map.width = CHUNK_AREA_SIZE;
        chunk_map.height = CHUNK_AREA_SIZE;
        chunk_map.version = map.version;
        chunk_map.item_major_version = map.item_major_version;
        chunk_map.item_minor_version = map.item_minor_version;
        chunk_map.description = format!("X-Trails chunk z{} ({},{})", z, base_x, base_y);

        for tile in tiles {
            chunk_map.set_tile((*tile).clone());
        }

        serialize_otbm(&chunk_map, &chunk_path)?;

        chunk_entries.push(ManifestChunkEntry {
            z,
            base_x,
            base_y,
            path: rel_path,
        });
    }

    // Write manifest
    let manifest = ChunkManifest {
        version: 1,
        generator: "PTE".to_string(),
        world_width: map.width,
        world_height: map.height,
        chunk_size: CHUNK_AREA_SIZE,
        surface_z: 31,
        description: map.description.clone(),
        spawn_file: map.spawn_file.clone(),
        house_file: map.house_file.clone(),
        towns: map
            .towns
            .iter()
            .map(|t| ManifestTown {
                id: t.id,
                name: t.name.clone(),
                x: t.position.x,
                y: t.position.y,
                z: t.position.z,
            })
            .collect(),
        waypoints: map
            .waypoints
            .iter()
            .map(|wp| ManifestWaypoint {
                name: wp.name.clone(),
                x: wp.position.x,
                y: wp.position.y,
                z: wp.position.z,
            })
            .collect(),
        chunks: chunk_entries.clone(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(dir.join("manifest.json"), manifest_json)?;

    tracing::info!(
        "Saved {} chunks to {}, {} tiles total",
        chunk_entries.len(),
        dir.display(),
        map.tile_count()
    );

    Ok(chunk_entries.len())
}

/// Save a specific 256x256 region from MapData as a single chunk OTBM.
pub fn save_region_as_chunk(
    map: &MapData,
    base_x: u16,
    base_y: u16,
    z: u8,
    path: &Path,
) -> anyhow::Result<usize> {
    let tiles = map.get_tiles_in_area(
        base_x,
        base_y,
        base_x + CHUNK_AREA_SIZE - 1,
        base_y + CHUNK_AREA_SIZE - 1,
        z,
    );

    let mut chunk_map = MapData::new();
    chunk_map.width = CHUNK_AREA_SIZE;
    chunk_map.height = CHUNK_AREA_SIZE;
    chunk_map.version = map.version;
    chunk_map.item_major_version = map.item_major_version;
    chunk_map.item_minor_version = map.item_minor_version;
    chunk_map.description = format!("X-Trails chunk z{} ({},{})", z, base_x, base_y);

    let count = tiles.len();
    for tile in tiles {
        chunk_map.set_tile(tile.clone());
    }

    serialize_otbm(&chunk_map, path)?;
    Ok(count)
}
