//! Adapter layer — converts legacy SPR/DAT data into CIP format types
//! so the rendering pipeline works without modification.

use std::collections::HashMap;

use pte_appearances::{
    Appearance, AppearanceFlags, FrameGroup, LoadedAppearances, SpriteAnimation, SpriteInfo,
    SpritePhase,
};
use pte_assets::{ParsedCatalog, SpriteSheet, SpriteSheetInfo, SpriteType};
use tibia_spr_dat::{DatCategory, DatEntry, DatFile, SprFile};

/// Sprites per sheet: 384 / 32 = 12 columns, 12 rows = 144 sprites.
const SHEET_COLS: u32 = 12;
const SHEET_ROWS: u32 = 12;
const SPRITES_PER_SHEET: u32 = SHEET_COLS * SHEET_ROWS;
const SHEET_WIDTH: u32 = SHEET_COLS * 32;

/// Convert legacy SPR sprites into CIP-style SpriteSheet map.
///
/// Packs individual 32x32 sprites into 384x384 grid sheets.
/// Returns `HashMap<String, SpriteSheet>` keyed by synthetic sheet names.
pub fn spr_to_sheets(spr: &SprFile) -> HashMap<String, SpriteSheet> {
    let total = spr.sprite_count();
    if total == 0 {
        return HashMap::new();
    }

    let num_sheets = total.div_ceil(SPRITES_PER_SHEET);
    let mut sheets = HashMap::with_capacity(num_sheets as usize);

    for sheet_idx in 0..num_sheets {
        let first_id = sheet_idx * SPRITES_PER_SHEET + 1; // 1-based
        let last_id = ((sheet_idx + 1) * SPRITES_PER_SHEET).min(total);
        let sprites_in_sheet = last_id - first_id + 1;

        // Calculate actual sheet height (last sheet may be smaller)
        let rows_needed = sprites_in_sheet.div_ceil(SHEET_COLS);
        let sheet_h = rows_needed * 32;

        let mut pixels = vec![0u8; (SHEET_WIDTH * sheet_h * 4) as usize];
        let row_stride = (SHEET_WIDTH * 4) as usize;

        for local_idx in 0..sprites_in_sheet {
            let sprite_id = first_id + local_idx; // 1-based
            let sprite = match spr.get_sprite(sprite_id) {
                Some(s) => s,
                None => continue,
            };

            let col = local_idx % SHEET_COLS;
            let row = local_idx / SHEET_COLS;
            let x0 = (col * 32) as usize;
            let y0 = (row * 32) as usize;

            // Copy 32x32 sprite pixels into sheet grid
            for y in 0..32usize {
                let src_start = y * 32 * 4;
                let src_end = src_start + 32 * 4;
                let dst_start = (y0 + y) * row_stride + x0 * 4;
                let dst_end = dst_start + 32 * 4;
                if src_end <= sprite.pixels.len() && dst_end <= pixels.len() {
                    pixels[dst_start..dst_end].copy_from_slice(&sprite.pixels[src_start..src_end]);
                }
            }
        }

        let key = format!("legacy_sheet_{}", sheet_idx);
        sheets.insert(
            key,
            SpriteSheet {
                first_sprite_id: first_id,
                last_sprite_id: last_id,
                sprite_type: SpriteType::Size32x32,
                width: SHEET_WIDTH,
                height: sheet_h,
                pixels,
            },
        );
    }

    tracing::info!(
        "Packed {} legacy sprites into {} sheet(s)",
        total,
        sheets.len()
    );
    sheets
}

/// Build a synthetic ParsedCatalog that describes the legacy sheets.
pub fn build_synthetic_catalog(spr: &SprFile) -> ParsedCatalog {
    let total = spr.sprite_count();
    let num_sheets = if total == 0 {
        0
    } else {
        total.div_ceil(SPRITES_PER_SHEET)
    };

    let sprite_sheets = (0..num_sheets)
        .map(|i| {
            let first = i * SPRITES_PER_SHEET + 1;
            let last = ((i + 1) * SPRITES_PER_SHEET).min(total);
            SpriteSheetInfo {
                file: format!("legacy_sheet_{}", i),
                first_sprite_id: first,
                last_sprite_id: last,
                sprite_type: SpriteType::Size32x32,
            }
        })
        .collect();

    ParsedCatalog {
        sprite_sheets,
        appearances_file: None,
        total_sprites: total,
    }
}

/// Convert a legacy DatFile into protobuf LoadedAppearances.
pub fn dat_to_appearances(dat: &DatFile) -> LoadedAppearances {
    let mut loaded = LoadedAppearances::default();

    for entry in &dat.items {
        let app = dat_entry_to_appearance(entry);
        loaded.objects.insert(entry.id, app);
    }
    for entry in &dat.outfits {
        let app = dat_entry_to_appearance(entry);
        loaded.outfits.insert(entry.id, app);
    }
    for entry in &dat.effects {
        let app = dat_entry_to_appearance(entry);
        loaded.effects.insert(entry.id, app);
    }
    for entry in &dat.missiles {
        let app = dat_entry_to_appearance(entry);
        loaded.missiles.insert(entry.id, app);
    }

    tracing::info!(
        "Converted DAT → appearances: {} objects, {} outfits, {} effects, {} missiles",
        loaded.objects.len(),
        loaded.outfits.len(),
        loaded.effects.len(),
        loaded.missiles.len(),
    );
    loaded
}

/// Convert a single DatEntry into a protobuf Appearance.
fn dat_entry_to_appearance(entry: &DatEntry) -> Appearance {
    let layout = &entry.sprite_layout;

    // Build SpriteInfo
    // In legacy DAT: total = w * h * layers * px * py * pz * frames
    // In CIP protobuf: pattern_width/height incorporates the tile grid dimensions.
    // The sprite index formula uses pattern_width * pattern_height * pattern_depth * layers.
    // For a 2x2 tile object with px=1: protobuf pattern_width = w * px = 2.
    let proto_pw = layout.width as u32 * layout.pattern_x as u32;
    let proto_ph = layout.height as u32 * layout.pattern_y as u32;
    let sprite_info = SpriteInfo {
        pattern_width: Some(proto_pw),
        pattern_height: Some(proto_ph),
        pattern_depth: Some(layout.pattern_z as u32),
        layers: Some(layout.layers as u32),
        sprite_id: layout.sprite_ids.clone(),
        bounding_square: if layout.width > 1 || layout.height > 1 {
            Some(layout.width.max(layout.height) as u32 * 32)
        } else {
            None
        },
        animation: if layout.frames > 1 {
            Some(SpriteAnimation {
                default_start_phase: Some(0),
                synchronized: Some(false),
                random_start_phase: Some(false),
                loop_type: Some(0), // INFINITE
                loop_count: Some(0),
                sprite_phase: (0..layout.frames)
                    .map(|_| SpritePhase {
                        duration_min: Some(200),
                        duration_max: Some(200),
                    })
                    .collect(),
            })
        } else {
            None
        },
        is_opaque: None,
        bounding_box_per_direction: Vec::new(),
    };

    // Determine frame group type based on category
    let frame_group_id = match entry.category {
        DatCategory::Outfit => Some(0), // OUTFIT_IDLE
        _ => Some(2),                   // OBJECT_INITIAL
    };

    let frame_group = FrameGroup {
        fixed_frame_group: frame_group_id,
        id: None,
        sprite_info: Some(sprite_info),
    };

    // Build AppearanceFlags
    let flags = dat_flags_to_appearance_flags(&entry.flags);

    Appearance {
        id: Some(entry.id),
        frame_group: vec![frame_group],
        flags: Some(flags),
        name: None,
        description: None,
    }
}

/// Convert DatFlags into protobuf AppearanceFlags.
///
/// NOTE: In the Ravendawn format, bool-type flag markers (0x00-0x0F etc.) are
/// unreliable because the parser treats GROUND as bool, causing data bytes to be
/// misinterpreted as flag markers. Only FLAGS from DATA-BEARING attrs (which have
/// known correct data sizes) are mapped to protobuf. Bool-only flags are omitted
/// since we can't distinguish real flags from absorbed data bytes.
fn dat_flags_to_appearance_flags(f: &tibia_spr_dat::DatFlags) -> AppearanceFlags {
    use pte_appearances::proto::*;

    AppearanceFlags {
        // SKIP bool-only flags (unreliable in Ravendawn format):
        // bank (ground), clip, bottom, top, container, cumulative, forceuse, multiuse,
        // splash, unpass, unmove, unsight, avoid, take, liquidcontainer, hang,
        // rotate, dont_hide, translucent, lying_object, animate_always, fullbank,
        // ignore_look, hookSouth, hookEast, usable

        // ONLY map data-bearing attrs (these are reliably parsed):
        write: f.is_writable.map(|len| AppearanceFlagWrite {
            max_text_length: Some(len as u32),
        }),
        write_once: f.is_writable_once.map(|len| AppearanceFlagWriteOnce {
            max_text_length_once: Some(len as u32),
        }),
        light: f.has_light.map(|(intensity, color)| AppearanceFlagLight {
            brightness: Some(intensity as u32),
            color: Some(color as u32),
        }),
        shift: f.has_offset.map(|(x, y)| AppearanceFlagShift {
            x: Some(x as u32),
            y: Some(y as u32),
        }),
        height: f
            .has_elevation
            .map(|e| AppearanceFlagHeight {
                elevation: Some(e as u32),
            }),
        automap: f.has_minimap_color.map(|c| AppearanceFlagAutomap {
            color: Some(c as u32),
        }),
        lenshelp: f.has_lens_help.map(|id| AppearanceFlagLenshelp {
            id: Some(id as u32),
        }),
        fullbank: if f.is_full_ground { Some(true) } else { None },
        ignore_look: if f.ignore_look { Some(true) } else { None },
        clothes: f.is_cloth.map(|slot| AppearanceFlagClothes {
            slot: Some(slot as u32),
        }),
        default_action: f.has_default_action.map(|a| AppearanceFlagDefaultAction {
            action: Some(a as i32),
        }),
        market: None, // Market data mapping is complex; skip for initial support
        hook_south: if f.hook_south { Some(true) } else { None },
        hook_east: if f.hook_east { Some(true) } else { None },
        // All remaining fields default to None
        ..Default::default()
    }
}
