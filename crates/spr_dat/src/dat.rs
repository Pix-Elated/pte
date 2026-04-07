//! DAT file reader and writer.
//!
//! DAT format:
//! - u32 signature
//! - u16 item_count (last item ID, items start at 100)
//! - u16 outfit_count
//! - u16 effect_count
//! - u16 missile_count
//! - For each entry: attribute flags (byte markers) + sprite layout

use anyhow::{Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::types::*;

// DAT attribute markers (pre-1050 wire format, used by Ravendawn and most OT servers).
// Note: 0x10 = NoMoveAnimation in wire format; the client remaps internally.
mod attr {
    pub const GROUND: u8 = 0x00;
    pub const GROUND_BORDER: u8 = 0x01;
    pub const ON_BOTTOM: u8 = 0x02;
    pub const ON_TOP: u8 = 0x03;
    pub const CONTAINER: u8 = 0x04;
    pub const STACKABLE: u8 = 0x05;
    pub const FORCE_USE: u8 = 0x06;
    pub const MULTI_USE: u8 = 0x07;
    pub const WRITABLE: u8 = 0x08;
    pub const WRITABLE_ONCE: u8 = 0x09;
    pub const FLUID_CONTAINER: u8 = 0x0A;
    pub const SPLASH: u8 = 0x0B;
    pub const NOT_WALKABLE: u8 = 0x0C;
    pub const NOT_MOVEABLE: u8 = 0x0D;
    pub const BLOCKS_PROJECTILE: u8 = 0x0E;
    pub const NOT_PATHABLE: u8 = 0x0F;
    pub const NO_MOVE_ANIMATION: u8 = 0x10;
    pub const PICKUPABLE: u8 = 0x11;
    pub const HANGABLE: u8 = 0x12;
    pub const HOOK_SOUTH: u8 = 0x13;
    pub const HOOK_EAST: u8 = 0x14;
    pub const ROTATABLE: u8 = 0x15;
    pub const LIGHT: u8 = 0x16;
    pub const DONT_HIDE: u8 = 0x17;
    pub const TRANSLUCENT: u8 = 0x18;
    pub const OFFSET: u8 = 0x19;
    pub const ELEVATION: u8 = 0x1A;
    pub const LYING_OBJECT: u8 = 0x1B;
    pub const ANIMATE_ALWAYS: u8 = 0x1C;
    pub const MINIMAP_COLOR: u8 = 0x1D;
    pub const LENS_HELP: u8 = 0x1E;
    pub const FULL_GROUND: u8 = 0x1F;
    pub const IGNORE_LOOK: u8 = 0x20;
    pub const CLOTH: u8 = 0x21;
    pub const MARKET: u8 = 0x22;
    pub const DEFAULT_ACTION: u8 = 0x23;
    pub const USABLE: u8 = 0xFE; // reads u16 in Ravendawn (not bool)
    pub const END: u8 = 0xFF;
}

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

    // Items: IDs 100..=(99 + item_count)
    for id in DatFile::ITEM_START_ID..=(99 + item_count as u32) {
        match read_entry(&mut r, id, DatCategory::Item) {
            Ok(entry) => items.push(entry),
            Err(_) => {
                if !try_recover_entry(&mut r, &data) {
                    tracing::warn!("DAT: stopping items at ID {} ({} parsed)", id, items.len());
                    break;
                }
            }
        }
    }

    for id in 1..=outfit_count as u32 {
        match read_entry(&mut r, id, DatCategory::Outfit) {
            Ok(entry) => outfits.push(entry),
            Err(_) => {
                if !try_recover_entry(&mut r, &data) {
                    tracing::warn!("DAT: stopping outfits at ID {} ({} parsed)", id, outfits.len());
                    break;
                }
            }
        }
    }

    for id in 1..=effect_count as u32 {
        match read_entry(&mut r, id, DatCategory::Effect) {
            Ok(entry) => effects.push(entry),
            Err(_) => {
                if !try_recover_entry(&mut r, &data) {
                    tracing::warn!("DAT: stopping effects at ID {} ({} parsed)", id, effects.len());
                    break;
                }
            }
        }
    }

    for id in 1..=missile_count as u32 {
        match read_entry(&mut r, id, DatCategory::Missile) {
            Ok(entry) => missiles.push(entry),
            Err(_) => {
                if !try_recover_entry(&mut r, &data) {
                    tracing::warn!("DAT: stopping missiles at ID {} ({} parsed)", id, missiles.len());
                    break;
                }
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

/// Try to recover from a parse error by scanning forward for 0xFF + valid sprite layout.
/// Positions the cursor just after the recovered entry's sprite IDs.
fn try_recover_entry(r: &mut Cursor<&[u8]>, data: &[u8]) -> bool {
    let start = r.position() as usize;
    for i in start..data.len().saturating_sub(12) {
        if data[i] != 0xFF {
            continue;
        }
        let w = data[i + 1];
        let h = data[i + 2];
        if !(1..=4).contains(&w) || !(1..=4).contains(&h) {
            continue;
        }
        let skip = if w > 1 || h > 1 { 1 } else { 0 };
        let li = i + 3 + skip;
        if li + 5 > data.len() {
            continue;
        }
        let l = data[li];
        let px = data[li + 1];
        let py = data[li + 2];
        let pz = data[li + 3];
        let fr = data[li + 4];
        if !(1..=10).contains(&l)
            || !(1..=10).contains(&px)
            || !(1..=10).contains(&py)
            || !(1..=10).contains(&pz)
            || !(1..=200).contains(&fr)
        {
            continue;
        }
        let total = l as u32 * px as u32 * py as u32 * pz as u32 * fr as u32
            * w as u32 * h as u32;
        let end = li + 5 + total as usize * 4;
        if end <= data.len() {
            r.set_position(end as u64);
            return true;
        }
    }
    false
}

fn read_entry(r: &mut Cursor<&[u8]>, id: u32, category: DatCategory) -> Result<DatEntry> {
    let flags = read_flags(r)?;
    let sprite_layout = read_sprite_layout(r)?;

    Ok(DatEntry {
        id,
        category,
        flags,
        sprite_layout,
    })
}

fn read_flags(r: &mut Cursor<&[u8]>) -> Result<DatFlags> {
    let mut flags = DatFlags::default();

    loop {
        let marker = r.read_u8()?;
        match marker {
            attr::END => break,
            attr::GROUND => {
                // Ravendawn: GROUND marker is a bool flag (no u16 speed data).
                // We track it but DON'T set is_ground since we can't distinguish
                // real ground tiles from items that just happen to have 0x00 in flags.
                flags.raw_attrs.push((0x00, Vec::new()));
            }
            attr::GROUND_BORDER => flags.is_ground_border = true,
            attr::ON_BOTTOM => flags.is_on_bottom = true,
            attr::ON_TOP => flags.is_on_top = true,
            attr::CONTAINER => flags.is_container = true,
            attr::STACKABLE => flags.is_stackable = true,
            attr::FORCE_USE => flags.is_force_use = true,
            attr::MULTI_USE => flags.is_multi_use = true,
            attr::WRITABLE => flags.is_writable = Some(r.read_u16::<LittleEndian>()?),
            attr::WRITABLE_ONCE => flags.is_writable_once = Some(r.read_u16::<LittleEndian>()?),
            attr::FLUID_CONTAINER => flags.is_fluid_container = true,
            attr::SPLASH => flags.is_splash = true,
            attr::NOT_WALKABLE => flags.is_not_walkable = true,
            attr::NOT_MOVEABLE => flags.is_not_moveable = true,
            attr::BLOCKS_PROJECTILE => flags.blocks_projectile = true,
            attr::NOT_PATHABLE => flags.is_not_pathable = true,
            attr::NO_MOVE_ANIMATION => {}
            attr::PICKUPABLE => flags.is_pickupable = true,
            attr::HANGABLE => flags.is_hangable = true,
            attr::HOOK_SOUTH => flags.hook_south = true,
            attr::HOOK_EAST => flags.hook_east = true,
            attr::ROTATABLE => flags.is_rotatable = true,
            attr::LIGHT => {
                let intensity = r.read_u16::<LittleEndian>()?;
                let color = r.read_u16::<LittleEndian>()?;
                flags.has_light = Some((intensity, color));
            }
            attr::DONT_HIDE => flags.dont_hide = true,
            attr::TRANSLUCENT => flags.is_translucent = true,
            attr::OFFSET => {
                // Ravendawn (version 20125 >= 19600): reads u32+u32, values discarded.
                let x = r.read_u32::<LittleEndian>()? as u16;
                let y = r.read_u32::<LittleEndian>()? as u16;
                flags.has_offset = Some((x, y));
            }
            attr::ELEVATION => flags.has_elevation = Some(r.read_u16::<LittleEndian>()?),
            attr::LYING_OBJECT => flags.is_lying_object = true,
            attr::ANIMATE_ALWAYS => flags.animate_always = true,
            attr::MINIMAP_COLOR => flags.has_minimap_color = Some(r.read_u16::<LittleEndian>()?),
            attr::LENS_HELP => flags.has_lens_help = Some(r.read_u16::<LittleEndian>()?),
            attr::FULL_GROUND => flags.is_full_ground = true,
            attr::IGNORE_LOOK => flags.ignore_look = true,
            attr::CLOTH => flags.is_cloth = Some(r.read_u16::<LittleEndian>()?),
            attr::MARKET => {
                let cat = r.read_u16::<LittleEndian>()?;
                let trade_as = r.read_u16::<LittleEndian>()?;
                let show_as = r.read_u16::<LittleEndian>()?;
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
            attr::DEFAULT_ACTION => flags.has_default_action = Some(r.read_u16::<LittleEndian>()?),
            attr::USABLE => flags.is_usable = true,
            unknown => {
                // Assume unknown attrs are bool flags (no data payload).
                // This matches OTClient behavior where the default case reads 0 bytes.
                tracing::debug!("Unknown DAT attribute 0x{:02X}, treating as bool", unknown);
                flags.raw_attrs.push((unknown, Vec::new()));
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
    let pattern_z = r.read_u8()?;
    let frames = r.read_u8()?;

    // Animation timing data (present when frames > 1 in extended DAT formats).
    // Format: u8 anim_type + u32 loop_count + u8 start_phase + frames * (u32 min + u32 max)
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

    // Sanity check: unreasonably large layouts indicate corrupted data
    if total_sprites > 1_000_000 {
        anyhow::bail!(
            "Sprite layout too large ({} sprites): w={} h={} l={} px={} py={} pz={} f={}",
            total_sprites,
            width,
            height,
            layers,
            pattern_x,
            pattern_y,
            pattern_z,
            frames,
        );
    }

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
        out.write_u8(attr::GROUND)?;
        out.write_u16::<LittleEndian>(speed)?;
    }
    if flags.is_ground_border {
        out.write_u8(attr::GROUND_BORDER)?;
    }
    if flags.is_on_bottom {
        out.write_u8(attr::ON_BOTTOM)?;
    }
    if flags.is_on_top {
        out.write_u8(attr::ON_TOP)?;
    }
    if flags.is_container {
        out.write_u8(attr::CONTAINER)?;
    }
    if flags.is_stackable {
        out.write_u8(attr::STACKABLE)?;
    }
    if flags.is_force_use {
        out.write_u8(attr::FORCE_USE)?;
    }
    if flags.is_multi_use {
        out.write_u8(attr::MULTI_USE)?;
    }
    if let Some(len) = flags.is_writable {
        out.write_u8(attr::WRITABLE)?;
        out.write_u16::<LittleEndian>(len)?;
    }
    if let Some(len) = flags.is_writable_once {
        out.write_u8(attr::WRITABLE_ONCE)?;
        out.write_u16::<LittleEndian>(len)?;
    }
    if flags.is_fluid_container {
        out.write_u8(attr::FLUID_CONTAINER)?;
    }
    if flags.is_splash {
        out.write_u8(attr::SPLASH)?;
    }
    if flags.is_not_walkable {
        out.write_u8(attr::NOT_WALKABLE)?;
    }
    if flags.is_not_moveable {
        out.write_u8(attr::NOT_MOVEABLE)?;
    }
    if flags.blocks_projectile {
        out.write_u8(attr::BLOCKS_PROJECTILE)?;
    }
    if flags.is_not_pathable {
        out.write_u8(attr::NOT_PATHABLE)?;
    }
    if flags.is_pickupable {
        out.write_u8(attr::PICKUPABLE)?;
    }
    if flags.is_hangable {
        out.write_u8(attr::HANGABLE)?;
    }
    if flags.hook_south {
        out.write_u8(attr::HOOK_SOUTH)?;
    }
    if flags.hook_east {
        out.write_u8(attr::HOOK_EAST)?;
    }
    if flags.is_rotatable {
        out.write_u8(attr::ROTATABLE)?;
    }
    if let Some((intensity, color)) = flags.has_light {
        out.write_u8(attr::LIGHT)?;
        out.write_u16::<LittleEndian>(intensity)?;
        out.write_u16::<LittleEndian>(color)?;
    }
    if flags.dont_hide {
        out.write_u8(attr::DONT_HIDE)?;
    }
    if flags.is_translucent {
        out.write_u8(attr::TRANSLUCENT)?;
    }
    if let Some((x, y)) = flags.has_offset {
        out.write_u8(attr::OFFSET)?;
        out.write_u16::<LittleEndian>(x)?;
        out.write_u16::<LittleEndian>(y)?;
    }
    if let Some(elev) = flags.has_elevation {
        out.write_u8(attr::ELEVATION)?;
        out.write_u16::<LittleEndian>(elev)?;
    }
    if flags.is_lying_object {
        out.write_u8(attr::LYING_OBJECT)?;
    }
    if flags.animate_always {
        out.write_u8(attr::ANIMATE_ALWAYS)?;
    }
    if let Some(color) = flags.has_minimap_color {
        out.write_u8(attr::MINIMAP_COLOR)?;
        out.write_u16::<LittleEndian>(color)?;
    }
    if let Some(help) = flags.has_lens_help {
        out.write_u8(attr::LENS_HELP)?;
        out.write_u16::<LittleEndian>(help)?;
    }
    if flags.is_full_ground {
        out.write_u8(attr::FULL_GROUND)?;
    }
    if flags.ignore_look {
        out.write_u8(attr::IGNORE_LOOK)?;
    }
    if let Some(slot) = flags.is_cloth {
        out.write_u8(attr::CLOTH)?;
        out.write_u16::<LittleEndian>(slot)?;
    }
    if let Some(ref market) = flags.is_market_item {
        out.write_u8(attr::MARKET)?;
        out.write_u16::<LittleEndian>(market.category)?;
        out.write_u16::<LittleEndian>(market.trade_as_id)?;
        out.write_u16::<LittleEndian>(market.show_as_id)?;
        let name_bytes = market.name.as_bytes();
        out.write_u16::<LittleEndian>(name_bytes.len() as u16)?;
        out.write_all(name_bytes)?;
        out.write_u16::<LittleEndian>(market.restrict_profession)?;
        out.write_u16::<LittleEndian>(market.restrict_level)?;
    }
    if let Some(action) = flags.has_default_action {
        out.write_u8(attr::DEFAULT_ACTION)?;
        out.write_u16::<LittleEndian>(action)?;
    }
    if flags.is_usable {
        out.write_u8(attr::USABLE)?;
    }

    out.write_u8(attr::END)?;
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

    for &sid in &layout.sprite_ids {
        out.write_u32::<LittleEndian>(sid)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_entry(id: u32, category: DatCategory) -> DatEntry {
        DatEntry {
            id,
            category,
            flags: DatFlags {
                is_not_walkable: true,
                is_not_moveable: true,
                ..Default::default()
            },
            sprite_layout: SpriteLayout {
                width: 1,
                height: 1,
                exact_size: None,
                layers: 1,
                pattern_x: 1,
                pattern_y: 1,
                pattern_z: 1,
                frames: 1,
                sprite_ids: vec![42],
            },
        }
    }

    #[test]
    fn test_roundtrip_simple() {
        let dat = DatFile {
            signature: 0xDEADBEEF,
            items: vec![make_simple_entry(100, DatCategory::Item)],
            outfits: vec![make_simple_entry(1, DatCategory::Outfit)],
            effects: vec![],
            missiles: vec![],
        };

        let bytes = serialize_dat_bytes(&dat).unwrap();
        let parsed = parse_dat_bytes(&bytes).unwrap();

        assert_eq!(parsed.signature, 0xDEADBEEF);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.outfits.len(), 1);
        assert_eq!(parsed.items[0].flags.is_not_walkable, true);
        assert_eq!(parsed.items[0].sprite_layout.sprite_ids, vec![42]);
    }

    #[test]
    fn test_market_data_roundtrip() {
        let mut entry = make_simple_entry(100, DatCategory::Item);
        entry.flags.is_market_item = Some(MarketData {
            category: 1,
            trade_as_id: 100,
            show_as_id: 100,
            name: "Test Item".to_string(),
            restrict_profession: 0,
            restrict_level: 0,
        });

        let dat = DatFile {
            signature: 0,
            items: vec![entry],
            outfits: vec![],
            effects: vec![],
            missiles: vec![],
        };

        let bytes = serialize_dat_bytes(&dat).unwrap();
        let parsed = parse_dat_bytes(&bytes).unwrap();

        let market = parsed.items[0].flags.is_market_item.as_ref().unwrap();
        assert_eq!(market.name, "Test Item");
        assert_eq!(market.trade_as_id, 100);
    }

    #[test]
    fn test_add_remove_entry() {
        let mut dat = DatFile {
            signature: 0,
            items: vec![make_simple_entry(100, DatCategory::Item)],
            outfits: vec![],
            effects: vec![],
            missiles: vec![],
        };

        let id = dat.add_entry(make_simple_entry(0, DatCategory::Item));
        assert_eq!(id, 101); // Next after existing
        assert_eq!(dat.items.len(), 2);

        assert!(dat.remove_entry(DatCategory::Item, 100));
        assert_eq!(dat.items.len(), 1);
    }
}
