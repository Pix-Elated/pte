mod bmp;
mod catalog;
mod lzma;
mod sprite_sheet;

pub use bmp::encode_rgba_to_bmp;
pub use catalog::{
    find_sheet_for_sprite, CatalogEntry, ParsedCatalog, SpriteSheetInfo, SpriteType,
};
pub use lzma::compress_cip;
pub use sprite_sheet::{SpriteSheet, SPRITE_SIZE};

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Load and parse a catalog-content.json file.
pub fn load_catalog(path: &Path) -> Result<ParsedCatalog> {
    let data = std::fs::read_to_string(path).context("reading catalog-content.json")?;
    catalog::parse_catalog(&data)
}

/// Decompress a single CIP sprite sheet file into RGBA pixel data.
pub fn decompress_sprite_sheet(path: &Path, info: &SpriteSheetInfo) -> Result<SpriteSheet> {
    let compressed = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    decompress_sprite_sheet_bytes(&compressed, info)
}

/// Decompress from in-memory bytes.
pub fn decompress_sprite_sheet_bytes(data: &[u8], info: &SpriteSheetInfo) -> Result<SpriteSheet> {
    let raw = lzma::decompress_cip(data).context("LZMA decompression")?;
    let sprite_count = info.last_sprite_id - info.first_sprite_id + 1;
    let (width, height) = sprite_sheet::sheet_dimensions(sprite_count, info.sprite_type);
    let rgba = bmp::decode_bmp_to_rgba(&raw, width, height).context("BMP decode")?;

    Ok(SpriteSheet {
        first_sprite_id: info.first_sprite_id,
        last_sprite_id: info.last_sprite_id,
        sprite_type: info.sprite_type,
        width,
        height,
        pixels: rgba,
    })
}

/// Load ALL sprite sheets from a directory, using rayon for parallelism.
/// Returns a map from sheet filename → decoded SpriteSheet.
pub fn load_all_sheets(
    asset_dir: &Path,
    catalog: &ParsedCatalog,
    on_progress: impl Fn(usize, usize) + Send + Sync,
) -> HashMap<String, SpriteSheet> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let total = catalog.sprite_sheets.len();
    let done = AtomicUsize::new(0);

    let results: Vec<_> = catalog
        .sprite_sheets
        .par_iter()
        .filter_map(|info| {
            let path = asset_dir.join(&info.file);
            let sheet = decompress_sprite_sheet(&path, info)
                .map_err(|e| tracing::warn!("Failed to load {}: {e:#}", info.file))
                .ok()?;
            let count = done.fetch_add(1, Ordering::Relaxed) + 1;
            on_progress(count, total);
            Some((info.file.clone(), sheet))
        })
        .collect();

    results.into_iter().collect()
}

/// Save a modified sprite sheet back to a CIP LZMA file.
/// Encodes RGBA → BMP → LZMA with CIP header.
pub fn save_sprite_sheet(sheet: &SpriteSheet, path: &Path) -> Result<()> {
    let bmp_data = bmp::encode_rgba_to_bmp(&sheet.pixels, sheet.width, sheet.height)?;
    let compressed = lzma::compress_cip(&bmp_data)?;
    std::fs::write(path, &compressed).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
