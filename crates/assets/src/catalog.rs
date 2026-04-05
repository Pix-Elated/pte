use anyhow::{bail, Result};
use serde::Deserialize;

/// Raw JSON entry from catalog-content.json.
#[derive(Debug, Deserialize)]
pub struct CatalogEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub file: String,
    #[serde(rename = "spritetype")]
    pub sprite_type: Option<u32>,
    #[serde(rename = "firstspriteid")]
    pub first_sprite_id: Option<u32>,
    #[serde(rename = "lastspriteid")]
    pub last_sprite_id: Option<u32>,
}

/// Sprite type determines the individual sprite dimensions within a 384×384 sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpriteType {
    /// 32×32 sprites (most items)
    Size32x32 = 0,
    Size32x64 = 1,
    Size64x32 = 2,
    Size64x64 = 3,
}

impl SpriteType {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Size32x64,
            2 => Self::Size64x32,
            3 => Self::Size64x64,
            _ => Self::Size32x32,
        }
    }

    /// Width and height of a single sprite in this sheet type.
    pub fn sprite_dimensions(self) -> (u32, u32) {
        match self {
            Self::Size32x32 => (32, 32),
            Self::Size32x64 => (32, 64),
            Self::Size64x32 => (64, 32),
            Self::Size64x64 => (64, 64),
        }
    }
}

/// Info about a single sprite sheet file.
#[derive(Debug, Clone)]
pub struct SpriteSheetInfo {
    pub file: String,
    pub first_sprite_id: u32,
    pub last_sprite_id: u32,
    pub sprite_type: SpriteType,
}

/// Parsed catalog with deduped, sorted sprite sheet list.
#[derive(Debug)]
pub struct ParsedCatalog {
    pub sprite_sheets: Vec<SpriteSheetInfo>,
    pub appearances_file: Option<String>,
    pub total_sprites: u32,
}

pub fn parse_catalog(json: &str) -> Result<ParsedCatalog> {
    let entries: Vec<CatalogEntry> = serde_json::from_str(json)?;

    let mut sheets = Vec::new();
    let mut appearances_file = None;
    let mut max_sprite = 0u32;

    for entry in &entries {
        match entry.entry_type.as_str() {
            "sprite" => {
                let first = entry.first_sprite_id.unwrap_or(0);
                let last = entry.last_sprite_id.unwrap_or(0);
                if first == 0 && last == 0 {
                    continue;
                }
                max_sprite = max_sprite.max(last);
                sheets.push(SpriteSheetInfo {
                    file: entry.file.clone(),
                    first_sprite_id: first,
                    last_sprite_id: last,
                    sprite_type: SpriteType::from_u32(entry.sprite_type.unwrap_or(0)),
                });
            }
            "appearances" => {
                appearances_file = Some(entry.file.clone());
            }
            _ => {} // ignore other types
        }
    }

    // Sort by first sprite ID for binary search
    sheets.sort_by_key(|s| s.first_sprite_id);

    // Deduplicate (CIP catalogs sometimes have overlapping entries)
    sheets.dedup_by_key(|s| s.first_sprite_id);

    if sheets.is_empty() {
        bail!("No sprite sheet entries found in catalog");
    }

    Ok(ParsedCatalog {
        sprite_sheets: sheets,
        appearances_file,
        total_sprites: max_sprite,
    })
}

/// Find which sheet contains a given sprite ID (binary search).
pub fn find_sheet_for_sprite(
    sheets: &[SpriteSheetInfo],
    sprite_id: u32,
) -> Option<&SpriteSheetInfo> {
    let idx = sheets.partition_point(|s| s.first_sprite_id <= sprite_id);
    if idx == 0 {
        return None;
    }
    let sheet = &sheets[idx - 1];
    if sprite_id >= sheet.first_sprite_id && sprite_id <= sheet.last_sprite_id {
        Some(sheet)
    } else {
        None
    }
}
