//! DAT file reader and writer.
//!
//! Decompiled from ravenquest_dx.exe ThingType::unserialize at VA 0x140410070.
//!
//! Runtime state when loadDat runs:
//!   g_clientVersion = 20125 (remapping active: wire 0x10→0xFD, wire 0x11-0x26 shift down by 1)
//!   Feature 18 (GameSpritesU32) = SET → u32 sprite IDs
//!   Feature 59 (GameEnhancedAnimations) = SET → animation timing when frames > 1
//!   Feature 71 (GameIdleAnimations) = SET → multi-direction outfit frame groups
//!
//! Wire-to-case mapping (after version remapping):
//!   Wire 0x00 → internal 0x00 → case 0 (u16)
//!   Wire 0x08 → internal 0x08 → case 0 (u16)
//!   Wire 0x09 → internal 0x09 → case 0 (u16)
//!   Wire 0x10 → internal 0xFD → default (bool)
//!   Wire 0x16 → internal 0x15 → case 1 (u16+u16, Light)
//!   Wire 0x19 → internal 0x18 → case 2 (u32+u32 discarded, Displacement v>=19600)
//!   Wire 0x1A → internal 0x19 → case 3 (u16, Elevation)
//!   Wire 0x1D → internal 0x1C → case 0 (u16, Minimap)
//!   Wire 0x1E → internal 0x1D → case 0 (u16, Lens)
//!   Wire 0x21 → internal 0x20 → case 0 (u16, Cloth)
//!   Wire 0x22 → internal 0x21 → case 4 (Market: 3×u16 + getString + 2×u16)
//!   Wire 0x23 → internal 0x22 → case 0 (u16, DefaultAction)
//!   All others → case 5 (bool, 0 bytes)
//!   Wire 0xFF → END

use anyhow::{Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::types::*;

/// Parse DAT from a file path.
pub fn parse_dat(path: &Path) -> Result<DatFile> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_dat_bytes(&data)
}

/// Parse DAT from bytes.
pub fn parse_dat_bytes(data: &[u8]) -> Result<DatFile> {
    let mut r = Cursor::new(data);

    let signature = r.read_u32::<LittleEndian>()?;
    let item_count = r.read_u16::<LittleEndian>()?;
    let outfit_count = r.read_u16::<LittleEndian>()?;
    let effect_count = r.read_u16::<LittleEndian>()?;
    let missile_count = r.read_u16::<LittleEndian>()?;

    tracing::info!(
        item_count,
        outfit_count,
        effect_count,
        missile_count,
        signature = format!("{:#010X}", signature),
        "Parsing DAT"
    );

    let mut items = Vec::with_capacity(item_count as usize);
    let mut outfits = Vec::with_capacity(outfit_count as usize);
    let mut effects = Vec::with_capacity(effect_count as usize);
    let mut missiles = Vec::with_capacity(missile_count as usize);

    // Header value is the LAST item ID, not count. Items start at 100.
    // Binary iterates: firstId = 100, while firstId < vectorSize (= headerValue + 1)
    for id in DatFile::ITEM_START_ID..=(item_count as u32) {
        match read_entry(&mut r, id, DatCategory::Item) {
            Ok(entry) => items.push(entry),
            Err(e) => {
                tracing::warn!("DAT: stopping items at ID {} ({} parsed): {e:#}", id, items.len());
                break;
            }
        }
    }

    for id in 1..=outfit_count as u32 {
        match read_entry(&mut r, id, DatCategory::Outfit) {
            Ok(entry) => outfits.push(entry),
            Err(e) => {
                tracing::warn!("DAT: stopping outfits at ID {} ({} parsed): {e:#}", id, outfits.len());
                break;
            }
        }
    }

    for id in 1..=effect_count as u32 {
        match read_entry(&mut r, id, DatCategory::Effect) {
            Ok(entry) => effects.push(entry),
            Err(e) => {
                tracing::warn!("DAT: stopping effects at ID {} ({} parsed): {e:#}", id, effects.len());
                break;
            }
        }
    }

    for id in 1..=missile_count as u32 {
        match read_entry(&mut r, id, DatCategory::Missile) {
            Ok(entry) => missiles.push(entry),
            Err(e) => {
                tracing::warn!("DAT: stopping missiles at ID {} ({} parsed): {e:#}", id, missiles.len());
                break;
            }
        }
    }

    Ok(DatFile {
        signature,
        items,
        outfits,
        effects,
        missiles,
    })
}

fn read_entry(r: &mut Cursor<&[u8]>, id: u32, category: DatCategory) -> Result<DatEntry> {
    // Pre-flags u16 read (upgradeClassification) — unconditional at VA 0x1404100C1
    let _upgrade = r.read_u16::<LittleEndian>()?;

    let flags = read_flags(r)?;

    // Multi-direction outfit frame groups (category == 1 && feature 71)
    // Binary at VA 0x1404103C4: if category == 1 && (g_featureFlags[1] & 0x80)
    let num_frame_groups = if category == DatCategory::Outfit {
        r.read_u8()? // numFrameGroups byte
    } else {
        1 // items/effects/missiles always have 1 frame group
    };

    // Read the FIRST frame group's layout (primary/idle)
    // For outfits with multiple frame groups, skip additional ones
    let mut sprite_layout = None;
    for fg_idx in 0..num_frame_groups {
        // For outfits, each frame group has a frameGroupType byte
        if category == DatCategory::Outfit {
            let _frame_group_type = r.read_u8()?; // 0=idle, 1=walk
        }
        let layout = read_sprite_layout(r)?;
        if fg_idx == 0 {
            sprite_layout = Some(layout);
        }
        // Additional frame groups are read but only the first is kept
        // (the binary stores them in separate animator slots)
    }

    Ok(DatEntry {
        id,
        category,
        flags,
        sprite_layout: sprite_layout.unwrap_or_default(),
    })
}

/// Read attribute flags until 0xFF END marker.
///
/// Implements the EXACT switch from the binary with version remapping active (v=20125).
/// Wire bytes 0x11-0x26 are remapped: internal = wire - 1.
/// Wire byte 0x10 is remapped to 0xFD.
fn read_flags(r: &mut Cursor<&[u8]>) -> Result<DatFlags> {
    let mut flags = DatFlags::default();

    loop {
        let wire = r.read_u8()?;
        if wire == 0xFF {
            break;
        }

        // Version remapping (g_clientVersion = 20125 >= 1000)
        let attr = if wire == 0x10 {
            0xFD // special remap
        } else if wire >= 0x11 && wire <= 0x26 {
            wire - 1 // shift down by 1
        } else {
            wire
        };

        // Switch on internal attr (post-remap)
        // Mapping table at 0x1404113d8:
        //   0x00,0x08,0x09,0x1C,0x1D,0x20,0x22 → case 0 (u16)
        //   0x15 → case 1 (u16+u16)
        //   0x18 → case 2 (displacement)
        //   0x19 → case 3 (u16, stored at +0x2E)
        //   0x21 → case 4 (market)
        //   everything else → case 5 (bool)
        match attr {
            // Case 0: u16
            0x00 => { flags.is_ground = Some(r.read_u16::<LittleEndian>()?); }
            0x08 => { flags.is_writable = Some(r.read_u16::<LittleEndian>()?); }
            0x09 => { flags.is_writable_once = Some(r.read_u16::<LittleEndian>()?); }
            0x1C => { flags.has_minimap_color = Some(r.read_u16::<LittleEndian>()?); }
            0x1D => { flags.has_lens_help = Some(r.read_u16::<LittleEndian>()?); }
            0x20 => { flags.is_cloth = Some(r.read_u16::<LittleEndian>()?); }
            0x22 => { flags.has_default_action = Some(r.read_u16::<LittleEndian>()?); }

            // Case 1: u16+u16 (Light)
            0x15 => {
                let intensity = r.read_u16::<LittleEndian>()?;
                let color = r.read_u16::<LittleEndian>()?;
                flags.has_light = Some((intensity, color));
            }

            // Case 2: Displacement (v=20125 >= 19600: u32+u32, discarded)
            0x18 => {
                let _x = r.read_u32::<LittleEndian>()?;
                let _y = r.read_u32::<LittleEndian>()?;
                flags.has_offset = Some((8, 8)); // hardcode display values
            }

            // Case 3: u16 (Elevation)
            0x19 => {
                flags.has_elevation = Some(r.read_u16::<LittleEndian>()?);
            }

            // Case 4: Market (3×u16 + getString + 2×u16)
            0x21 => {
                let cat = r.read_u16::<LittleEndian>()?;
                let trade_as = r.read_u16::<LittleEndian>()?;
                let show_as = r.read_u16::<LittleEndian>()?;
                // getString: u16 length prefix + N raw bytes
                let name_len = r.read_u16::<LittleEndian>()? as usize;
                let mut name_buf = vec![0u8; name_len];
                r.read_exact(&mut name_buf)?;
                let name = String::from_utf8_lossy(&name_buf).into_owned();
                let restrict_prof = r.read_u16::<LittleEndian>()?;
                let restrict_lvl = r.read_u16::<LittleEndian>()?;
                flags.is_market_item = Some(MarketData {
                    category: cat,
                    trade_as_id: trade_as,
                    show_as_id: show_as,
                    name,
                    restrict_profession: restrict_prof,
                    restrict_level: restrict_lvl,
                });
            }

            // Case 5 (default): bool — 0 bytes read
            _ => {
                // Map wire byte to semantic flags for the bool-type attrs
                match wire {
                    0x01 => flags.is_ground_border = true,
                    0x02 => flags.is_on_bottom = true,
                    0x03 => flags.is_on_top = true,
                    0x04 => flags.is_container = true,
                    0x05 => flags.is_stackable = true,
                    0x06 => flags.is_force_use = true,
                    0x07 => flags.is_multi_use = true,
                    0x0A => flags.is_fluid_container = true,
                    0x0B => flags.is_splash = true,
                    0x0C => flags.is_not_walkable = true,
                    0x0D => flags.is_not_moveable = true,
                    0x0E => flags.blocks_projectile = true,
                    0x0F => flags.is_not_pathable = true,
                    0x11 => flags.is_pickupable = true,
                    0x12 => flags.is_hangable = true,
                    0x13 => flags.hook_south = true,
                    0x14 => flags.hook_east = true,
                    0x15 => flags.is_rotatable = true,
                    0x17 => flags.dont_hide = true,
                    0x18 => flags.is_translucent = true,
                    0x1B => flags.is_lying_object = true,
                    0x1C => flags.animate_always = true,
                    0x1F => flags.is_full_ground = true,
                    0x20 => flags.ignore_look = true,
                    0xFE => flags.is_usable = true,
                    _ => {
                        flags.raw_attrs.push((wire, Vec::new()));
                    }
                }
            }
        }
    }

    Ok(flags)
}

fn read_sprite_layout(r: &mut Cursor<&[u8]>) -> Result<SpriteLayout> {
    let width = r.read_u8()?;
    let height = r.read_u8()?;

    let exact_size = if width > 1 || height > 1 {
        Some(r.read_u8()?)
    } else {
        None
    };

    let layers = r.read_u8()?;
    let pattern_x = r.read_u8()?;
    let pattern_y = r.read_u8()?;
    // patternZ IS read (g_clientVersion = 20125 >= 755)
    let pattern_z = r.read_u8()?;
    let frames = r.read_u8()?;

    // Animation timing (frames > 1 AND feature 59 set)
    if frames > 1 {
        let _anim_type = r.read_u8()?;
        let _loop_count = r.read_u32::<LittleEndian>()?;
        let _start_phase = r.read_u8()?;
        for _ in 0..frames {
            let _min_dur = r.read_u32::<LittleEndian>()?;
            let _max_dur = r.read_u32::<LittleEndian>()?;
        }
    }

    let total_sprites = (width as u32)
        .saturating_mul(height as u32)
        .saturating_mul(layers as u32)
        .saturating_mul(pattern_x as u32)
        .saturating_mul(pattern_y as u32)
        .saturating_mul(pattern_z as u32)
        .saturating_mul(frames as u32);

    if total_sprites > 1_000_000 {
        anyhow::bail!(
            "Sprite layout too large ({} sprites): w={} h={} l={} px={} py={} pz={} f={}",
            total_sprites, width, height, layers, pattern_x, pattern_y, pattern_z, frames,
        );
    }

    // Sprite IDs: u32 (feature 18 set)
    let mut sprite_ids = Vec::with_capacity(total_sprites as usize);
    for _ in 0..total_sprites {
        sprite_ids.push(r.read_u32::<LittleEndian>()?);
    }

    Ok(SpriteLayout {
        width,
        height,
        exact_size,
        layers,
        pattern_x,
        pattern_y,
        pattern_z,
        frames,
        sprite_ids,
    })
}

/// Write DAT to a file.
pub fn write_dat(dat: &DatFile, path: &Path) -> Result<()> {
    let data = serialize_dat_bytes(dat)?;
    std::fs::write(path, &data)?;
    Ok(())
}

/// Serialize DAT to bytes.
pub fn serialize_dat_bytes(dat: &DatFile) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(512 * 1024);

    out.write_u32::<LittleEndian>(dat.signature)?;
    out.write_u16::<LittleEndian>(dat.item_count())?;
    out.write_u16::<LittleEndian>(dat.outfit_count())?;
    out.write_u16::<LittleEndian>(dat.effect_count())?;
    out.write_u16::<LittleEndian>(dat.missile_count())?;

    for entry in &dat.items {
        write_entry(&mut out, entry)?;
    }
    for entry in &dat.outfits {
        write_entry(&mut out, entry)?;
    }
    for entry in &dat.effects {
        write_entry(&mut out, entry)?;
    }
    for entry in &dat.missiles {
        write_entry(&mut out, entry)?;
    }

    Ok(out)
}

fn write_entry(out: &mut Vec<u8>, entry: &DatEntry) -> Result<()> {
    write_flags(out, &entry.flags)?;
    write_sprite_layout(out, &entry.sprite_layout)?;
    Ok(())
}

fn write_flags(out: &mut Vec<u8>, flags: &DatFlags) -> Result<()> {
    if let Some(speed) = flags.is_ground {
        out.write_u8(0x00)?;
        out.write_u16::<LittleEndian>(speed)?;
    }
    if flags.is_ground_border { out.write_u8(0x01)?; }
    if flags.is_on_bottom { out.write_u8(0x02)?; }
    if flags.is_on_top { out.write_u8(0x03)?; }
    if flags.is_container { out.write_u8(0x04)?; }
    if flags.is_stackable { out.write_u8(0x05)?; }
    if flags.is_force_use { out.write_u8(0x06)?; }
    if flags.is_multi_use { out.write_u8(0x07)?; }
    if let Some(len) = flags.is_writable {
        out.write_u8(0x08)?;
        out.write_u16::<LittleEndian>(len)?;
    }
    if let Some(len) = flags.is_writable_once {
        out.write_u8(0x09)?;
        out.write_u16::<LittleEndian>(len)?;
    }
    if flags.is_fluid_container { out.write_u8(0x0A)?; }
    if flags.is_splash { out.write_u8(0x0B)?; }
    if flags.is_not_walkable { out.write_u8(0x0C)?; }
    if flags.is_not_moveable { out.write_u8(0x0D)?; }
    if flags.blocks_projectile { out.write_u8(0x0E)?; }
    if flags.is_not_pathable { out.write_u8(0x0F)?; }
    if flags.is_pickupable { out.write_u8(0x11)?; }
    if flags.is_hangable { out.write_u8(0x12)?; }
    if flags.hook_south { out.write_u8(0x13)?; }
    if flags.hook_east { out.write_u8(0x14)?; }
    if flags.is_rotatable { out.write_u8(0x15)?; }
    if let Some((intensity, color)) = flags.has_light {
        out.write_u8(0x16)?;
        out.write_u16::<LittleEndian>(intensity)?;
        out.write_u16::<LittleEndian>(color)?;
    }
    if flags.dont_hide { out.write_u8(0x17)?; }
    if flags.is_translucent { out.write_u8(0x18)?; }
    if let Some((x, y)) = flags.has_offset {
        out.write_u8(0x19)?;
        out.write_u16::<LittleEndian>(x)?;
        out.write_u16::<LittleEndian>(y)?;
    }
    if let Some(e) = flags.has_elevation {
        out.write_u8(0x1A)?;
        out.write_u16::<LittleEndian>(e)?;
    }
    if flags.is_lying_object { out.write_u8(0x1B)?; }
    if flags.animate_always { out.write_u8(0x1C)?; }
    if let Some(c) = flags.has_minimap_color {
        out.write_u8(0x1D)?;
        out.write_u16::<LittleEndian>(c)?;
    }
    if let Some(h) = flags.has_lens_help {
        out.write_u8(0x1E)?;
        out.write_u16::<LittleEndian>(h)?;
    }
    if flags.is_full_ground { out.write_u8(0x1F)?; }
    if flags.ignore_look { out.write_u8(0x20)?; }
    if let Some(slot) = flags.is_cloth {
        out.write_u8(0x21)?;
        out.write_u16::<LittleEndian>(slot)?;
    }
    if let Some(ref market) = flags.is_market_item {
        out.write_u8(0x22)?;
        out.write_u16::<LittleEndian>(market.category)?;
        out.write_u16::<LittleEndian>(market.trade_as_id)?;
        out.write_u16::<LittleEndian>(market.show_as_id)?;
        let name_bytes = market.name.as_bytes();
        out.write_u16::<LittleEndian>(name_bytes.len() as u16)?;
        out.write_all(name_bytes)?;
        out.write_u16::<LittleEndian>(market.restrict_profession)?;
        out.write_u16::<LittleEndian>(market.restrict_level)?;
    }
    if let Some(a) = flags.has_default_action {
        out.write_u8(0x23)?;
        out.write_u16::<LittleEndian>(a)?;
    }
    if flags.is_usable { out.write_u8(0xFE)?; }
    out.write_u8(0xFF)?; // END
    Ok(())
}

fn write_sprite_layout(out: &mut Vec<u8>, layout: &SpriteLayout) -> Result<()> {
    out.write_u8(layout.width)?;
    out.write_u8(layout.height)?;
    if layout.width > 1 || layout.height > 1 {
        out.write_u8(layout.exact_size.unwrap_or(32))?;
    }
    out.write_u8(layout.layers)?;
    out.write_u8(layout.pattern_x)?;
    out.write_u8(layout.pattern_y)?;
    out.write_u8(layout.pattern_z)?;
    out.write_u8(layout.frames)?;
    for &id in &layout.sprite_ids {
        out.write_u32::<LittleEndian>(id)?;
    }
    Ok(())
}
