//! Legacy-to-protobuf asset converter.
//!
//! Converts Ravendawn/OT legacy formats to standard CIP protobuf format on disk:
//! - Tibia.spr → CIP sprite sheets (.cip LZMA BMP) + catalog-content.json
//! - Tibia.dat → appearances.dat (protobuf)
//! - RCHK chunk maps → .otbm

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};

/// Progress callback info.
pub struct ConvertProgress {
    pub progress: f32,
    pub message: String,
}

/// Convert a complete legacy project to protobuf format.
///
/// Reads SPR/DAT/chunks from `src_dir` and writes protobuf assets to `out_dir`.
pub fn convert_project(
    dat_path: &Path,
    spr_path: &Path,
    chunk_dir: Option<&Path>,
    out_dir: &Path,
    progress: &Arc<Mutex<ConvertProgress>>,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;

    // ── Phase 1: DAT → appearances.dat ──
    set_progress(progress, 0.0, "Parsing DAT…");
    let dat = tibia_spr_dat::parse_dat(dat_path).context("Failed to parse DAT")?;
    tracing::info!(
        "DAT: {} items, {} outfits, {} effects, {} missiles",
        dat.item_count(),
        dat.outfit_count(),
        dat.effect_count(),
        dat.missile_count(),
    );

    set_progress(progress, 0.05, "Converting appearances…");
    let appearances = crate::legacy_adapter::dat_to_appearances(&dat);
    let app_path = out_dir.join("appearances.dat");
    pte_appearances::save_appearances(&appearances, &app_path)
        .context("Failed to write appearances.dat")?;
    tracing::info!("Wrote appearances.dat ({} total)", appearances.total_count());

    // ── Phase 2: SPR → CIP sprite sheets (streaming, batch by batch) ──
    set_progress(progress, 0.10, "Parsing SPR…");
    convert_spr_to_sheets(spr_path, out_dir, progress)?;

    // ── Phase 3: RCHK chunks → OTBM (if chunk dir provided) ──
    if let Some(cd) = chunk_dir {
        if cd.is_dir() {
            set_progress(progress, 0.85, "Converting map chunks…");
            convert_chunks_to_otbm(cd, out_dir, progress)?;
        }
    }

    set_progress(progress, 1.0, "Done!");
    Ok(())
}

/// Convert SPR file to CIP sprite sheets + catalog-content.json, processing in batches.
fn convert_spr_to_sheets(
    spr_path: &Path,
    out_dir: &Path,
    progress: &Arc<Mutex<ConvertProgress>>,
) -> Result<()> {
    let spr = tibia_spr_dat::parse_spr(spr_path).context("Failed to parse SPR")?;
    let total_sprites = spr.sprite_count();
    tracing::info!("SPR: {} sprites", total_sprites);

    const SPRITES_PER_SHEET: u32 = 144; // 12x12 grid of 32x32 in a 384x384 sheet
    const SHEET_COLS: u32 = 12;
    let num_sheets = if total_sprites == 0 {
        0
    } else {
        total_sprites.div_ceil(SPRITES_PER_SHEET)
    };

    let mut catalog_entries = Vec::new();

    for sheet_idx in 0..num_sheets {
        let first_id = sheet_idx * SPRITES_PER_SHEET + 1;
        let last_id = ((sheet_idx + 1) * SPRITES_PER_SHEET).min(total_sprites);
        let sprites_in_sheet = last_id - first_id + 1;
        let rows_needed = sprites_in_sheet.div_ceil(SHEET_COLS);
        let sheet_w = SHEET_COLS * 32;
        let sheet_h = rows_needed * 32;

        // Build RGBA pixel buffer for this sheet
        let mut pixels = vec![0u8; (sheet_w * sheet_h * 4) as usize];
        let row_stride = (sheet_w * 4) as usize;

        for local_idx in 0..sprites_in_sheet {
            let sprite_id = first_id + local_idx;
            if let Some(sprite) = spr.get_sprite(sprite_id) {
                let col = local_idx % SHEET_COLS;
                let row = local_idx / SHEET_COLS;
                let x0 = (col * 32) as usize;
                let y0 = (row * 32) as usize;

                for y in 0..32usize {
                    let src_start = y * 32 * 4;
                    let src_end = src_start + 32 * 4;
                    let dst_start = (y0 + y) * row_stride + x0 * 4;
                    let dst_end = dst_start + 32 * 4;
                    if src_end <= sprite.pixels.len() && dst_end <= pixels.len() {
                        pixels[dst_start..dst_end]
                            .copy_from_slice(&sprite.pixels[src_start..src_end]);
                    }
                }
            }
        }

        // Encode to BMP then LZMA compress
        let bmp = pte_assets::encode_rgba_to_bmp(&pixels, sheet_w, sheet_h)
            .context("BMP encode failed")?;
        let compressed = pte_assets::compress_cip(&bmp).context("LZMA compression failed")?;

        let filename = format!("{}.cip", first_id);
        let sheet_path = out_dir.join(&filename);
        std::fs::write(&sheet_path, &compressed)
            .with_context(|| format!("Writing {}", sheet_path.display()))?;

        catalog_entries.push(serde_json::json!({
            "type": "sprite",
            "file": filename,
            "spritetype": 0,
            "firstspriteid": first_id,
            "lastspriteid": last_id,
        }));

        if sheet_idx % 50 == 0 || sheet_idx == num_sheets - 1 {
            let frac = 0.10 + 0.70 * ((sheet_idx + 1) as f32 / num_sheets as f32);
            set_progress(
                progress,
                frac,
                &format!("Converting sprites… ({}/{})", sheet_idx + 1, num_sheets),
            );
        }
    }

    // Add appearances entry to catalog
    catalog_entries.push(serde_json::json!({
        "type": "appearances",
        "file": "appearances.dat",
    }));

    // Write catalog-content.json
    let catalog_json = serde_json::to_string_pretty(&catalog_entries)?;
    std::fs::write(out_dir.join("catalog-content.json"), &catalog_json)?;
    tracing::info!(
        "Wrote catalog-content.json with {} sheet entries",
        num_sheets
    );

    Ok(())
}

/// Convert RCHK chunk files to a single OTBM map.
fn convert_chunks_to_otbm(
    chunk_dir: &Path,
    out_dir: &Path,
    progress: &Arc<Mutex<ConvertProgress>>,
) -> Result<()> {
    let mut map = pte_otbm::MapData::new();
    map.width = 65535;
    map.height = 65535;
    map.description = "Converted from RCHK chunks".to_string();
    map.version = 2;

    // Determine chunk size by scanning tile coordinates
    let chunk_size: u16 = detect_chunk_size(chunk_dir).unwrap_or(256);
    tracing::info!("Detected chunk size: {}", chunk_size);

    let mut total_tiles = 0u64;
    let mut chunk_count = 0u32;
    let z_dirs: Vec<_> = std::fs::read_dir(chunk_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    for z_entry in &z_dirs {
        let z_name = z_entry.file_name().to_string_lossy().to_string();
        let _z_level: u8 = match z_name.parse() {
            Ok(z) => z,
            Err(_) => continue,
        };

        let chunk_files: Vec<_> = std::fs::read_dir(z_entry.path())?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == "chunk")
            })
            .collect();

        for entry in &chunk_files {
            match parse_rchk_chunk(&entry.path(), chunk_size) {
                Ok(tiles) => {
                    for tile in tiles {
                        map.set_tile(tile);
                        total_tiles += 1;
                    }
                    chunk_count += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse chunk {}: {e:#}", entry.path().display());
                }
            }
        }
    }

    set_progress(
        progress,
        0.95,
        &format!("Writing OTBM ({} tiles)…", total_tiles),
    );

    let otbm_path = out_dir.join("world").join("converted.otbm");
    std::fs::create_dir_all(otbm_path.parent().unwrap())?;
    pte_otbm::serialize_otbm(&map, &otbm_path).context("Failed to write OTBM")?;
    tracing::info!(
        "Wrote OTBM: {} tiles from {} chunks",
        total_tiles,
        chunk_count
    );

    Ok(())
}

/// Read a u16 LE from a byte slice at a given offset.
fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > data.len() {
        return None;
    }
    Some(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

/// Parse a single RCHK chunk file into Tiles with world coordinates.
fn parse_rchk_chunk(path: &Path, chunk_size: u16) -> Result<Vec<pte_otbm::Tile>> {
    let data = std::fs::read(path)?;
    if data.len() < 12 {
        anyhow::bail!("Chunk too small: {} bytes", data.len());
    }
    if &data[0..4] != b"RCHK" {
        anyhow::bail!("Invalid chunk magic");
    }

    // Header: RCHK(4) + version(u8) + chunk_x(u8) + pad(u8) + chunk_y(u8) + pad(u8) + z(u8) + tile_count(u16)
    let chunk_x = data[5] as u16;
    let chunk_y = data[7] as u16;
    let z = data[9];
    let tile_count = read_u16_le(&data, 10).unwrap_or(0);

    let world_x_base = chunk_x * chunk_size;
    let world_y_base = chunk_y * chunk_size;

    let mut tiles = Vec::with_capacity(tile_count as usize);
    let mut pos = 12usize;

    for _ in 0..tile_count {
        if pos + 8 > data.len() {
            break;
        }

        let tile_pos = read_u16_le(&data, pos).unwrap_or(0);
        // Skip flags1 and flags2
        let item_count = read_u16_le(&data, pos + 6).unwrap_or(0) as usize;
        pos += 8;

        let local_x = (tile_pos & 0xFF) as u16;
        let local_y = ((tile_pos >> 8) & 0xFF) as u16;

        let world_x = world_x_base + local_x;
        let world_y = world_y_base + local_y;

        let mut tile = pte_otbm::Tile::new(world_x, world_y, z);

        for i in 0..item_count {
            if pos + 2 > data.len() {
                break;
            }
            let item_id = read_u16_le(&data, pos).unwrap_or(0);
            pos += 2;

            if i == 0 && tile.ground.is_none() {
                tile.ground = Some(item_id);
            } else {
                tile.items.push(pte_otbm::MapItem::new(item_id));
            }
        }

        tiles.push(tile);
    }

    Ok(tiles)
}

/// Detect chunk tile dimensions by scanning the largest chunk for max local coords.
fn detect_chunk_size(chunk_dir: &Path) -> Option<u16> {
    let mut best_path: Option<PathBuf> = None;
    let mut best_size = 0u64;

    for z_entry in std::fs::read_dir(chunk_dir).ok()?.flatten() {
        if !z_entry.path().is_dir() {
            continue;
        }
        for chunk_entry in std::fs::read_dir(z_entry.path()).ok()?.flatten() {
            let size = chunk_entry.metadata().ok()?.len();
            if size > best_size {
                best_size = size;
                best_path = Some(chunk_entry.path());
            }
        }
    }

    let path = best_path?;
    let data = std::fs::read(&path).ok()?;
    if data.len() < 12 || &data[0..4] != b"RCHK" {
        return None;
    }

    let tile_count = read_u16_le(&data, 10)?;
    let mut max_coord: u16 = 0;
    let mut pos = 12usize;

    for _ in 0..tile_count {
        if pos + 8 > data.len() {
            break;
        }

        let tile_pos = read_u16_le(&data, pos)?;
        let item_count = read_u16_le(&data, pos + 6)? as usize;
        pos += 8;

        let local_x = tile_pos & 0xFF;
        let local_y = (tile_pos >> 8) & 0xFF;
        max_coord = max_coord.max(local_x).max(local_y);

        // Skip items
        pos += item_count * 2;
    }

    // Round up to nearest power of 2
    let size = if max_coord < 32 {
        32
    } else if max_coord < 64 {
        64
    } else if max_coord < 128 {
        128
    } else {
        256
    };

    Some(size)
}

fn set_progress(progress: &Arc<Mutex<ConvertProgress>>, frac: f32, msg: &str) {
    if let Ok(mut p) = progress.lock() {
        p.progress = frac;
        p.message = msg.to_string();
    }
}
