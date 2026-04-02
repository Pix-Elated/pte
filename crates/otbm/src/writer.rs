//! OTBM writer — serialize MapData back to the OTBM binary format.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

use crate::types::*;

const NODE_START: u8 = 0xFE;
const NODE_END: u8 = 0xFF;
const ESCAPE: u8 = 0xFD;

/// Write MapData to a file atomically (write to temp, then rename).
pub fn serialize_otbm(map: &MapData, path: &Path) -> Result<()> {
    let data = serialize_otbm_bytes(map)?;
    // Write to a temp file in the same directory, then rename for atomicity
    let temp_path = path.with_extension("otbm.tmp");
    std::fs::write(&temp_path, &data).with_context(|| format!("writing temp file {}", temp_path.display()))?;
    std::fs::rename(&temp_path, path).with_context(|| format!("renaming {} to {}", temp_path.display(), path.display()))?;
    Ok(())
}

/// Serialize MapData to bytes.
pub fn serialize_otbm_bytes(map: &MapData) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(1024 * 1024);

    // 4-byte header (OTBI version = 0)
    out.extend_from_slice(&[0u8; 4]);

    // Root node
    write_node_start(&mut out, node_type::ROOT);
    write_escaped_u32(&mut out, map.version);
    write_escaped_u16(&mut out, map.width);
    write_escaped_u16(&mut out, map.height);
    write_escaped_u32(&mut out, map.item_major_version);
    write_escaped_u32(&mut out, map.item_minor_version);

    // Map data node
    write_node_start(&mut out, node_type::MAP_DATA);

    // Description
    if !map.description.is_empty() {
        write_escaped_u8(&mut out, attr::DESCRIPTION);
        write_escaped_u16(&mut out, map.description.len() as u16);
        write_escaped_bytes(&mut out, map.description.as_bytes());
    }
    if !map.spawn_file.is_empty() {
        write_escaped_u8(&mut out, attr::EXT_SPAWN_FILE);
        write_escaped_u16(&mut out, map.spawn_file.len() as u16);
        write_escaped_bytes(&mut out, map.spawn_file.as_bytes());
    }
    if !map.house_file.is_empty() {
        write_escaped_u8(&mut out, attr::EXT_HOUSE_FILE);
        write_escaped_u16(&mut out, map.house_file.len() as u16);
        write_escaped_bytes(&mut out, map.house_file.as_bytes());
    }

    // Group tiles into 256×256 tile areas
    write_tile_areas(&mut out, map);

    // Towns
    if !map.towns.is_empty() {
        write_node_start(&mut out, node_type::TOWNS);
        for town in &map.towns {
            write_node_start(&mut out, node_type::TOWN);
            write_escaped_u32(&mut out, town.id);
            write_escaped_u16(&mut out, town.name.len() as u16);
            write_escaped_bytes(&mut out, town.name.as_bytes());
            write_escaped_u16(&mut out, town.position.x);
            write_escaped_u16(&mut out, town.position.y);
            write_escaped_u8(&mut out, town.position.z);
            out.push(NODE_END);
        }
        out.push(NODE_END);
    }

    // Waypoints
    if !map.waypoints.is_empty() {
        write_node_start(&mut out, node_type::WAYPOINTS);
        for wp in &map.waypoints {
            write_node_start(&mut out, node_type::WAYPOINT);
            write_escaped_u16(&mut out, wp.name.len() as u16);
            write_escaped_bytes(&mut out, wp.name.as_bytes());
            write_escaped_u16(&mut out, wp.position.x);
            write_escaped_u16(&mut out, wp.position.y);
            write_escaped_u8(&mut out, wp.position.z);
            out.push(NODE_END);
        }
        out.push(NODE_END);
    }

    out.push(NODE_END); // end MAP_DATA
    out.push(NODE_END); // end ROOT

    Ok(out)
}

/// Group tiles into 256×256 areas and write them.
fn write_tile_areas(out: &mut Vec<u8>, map: &MapData) {
    // Collect all tiles grouped by (area_x, area_y, z)
    let mut areas: BTreeMap<(u16, u16, u8), Vec<&Tile>> = BTreeMap::new();

    for chunk in map.chunks.values() {
        for tile in chunk.values() {
            let area_x = (tile.x / 256) * 256;
            let area_y = (tile.y / 256) * 256;
            areas
                .entry((area_x, area_y, tile.z))
                .or_default()
                .push(tile);
        }
    }

    for ((area_x, area_y, area_z), tiles) in &areas {
        write_node_start(out, node_type::TILE_AREA);
        write_escaped_u16(out, *area_x);
        write_escaped_u16(out, *area_y);
        write_escaped_u8(out, *area_z);

        for tile in tiles {
            let nt = if tile.house_id.is_some() {
                node_type::HOUSE_TILE
            } else {
                node_type::TILE
            };
            write_node_start(out, nt);

            // Offset within the 256×256 area
            write_escaped_u8(out, (tile.x - area_x) as u8);
            write_escaped_u8(out, (tile.y - area_y) as u8);

            if let Some(house_id) = tile.house_id {
                write_escaped_u32(out, house_id);
            }

            // Tile flags
            let flags = tile.flags.to_u32();
            if flags != 0 {
                write_escaped_u8(out, attr::TILE_FLAGS);
                write_escaped_u32(out, flags);
            }

            // Ground item as tile attribute
            if let Some(ground_id) = tile.ground {
                write_escaped_u8(out, attr::ITEM);
                write_escaped_u16(out, ground_id);
            }

            // Stacked items as child nodes
            for item in &tile.items {
                write_item_node(out, item);
            }

            out.push(NODE_END); // end TILE
        }

        out.push(NODE_END); // end TILE_AREA
    }
}

fn write_item_node(out: &mut Vec<u8>, item: &MapItem) {
    write_node_start(out, node_type::ITEM);
    write_escaped_u16(out, item.id);

    if let Some(v) = item.action_id {
        write_escaped_u8(out, attr::ACTION_ID);
        write_escaped_u16(out, v);
    }
    if let Some(v) = item.unique_id {
        write_escaped_u8(out, attr::UNIQUE_ID);
        write_escaped_u16(out, v);
    }
    if let Some(ref v) = item.text {
        write_escaped_u8(out, attr::TEXT);
        write_escaped_u16(out, v.len() as u16);
        write_escaped_bytes(out, v.as_bytes());
    }
    if let Some(ref v) = item.description {
        write_escaped_u8(out, attr::DESC);
        write_escaped_u16(out, v.len() as u16);
        write_escaped_bytes(out, v.as_bytes());
    }
    if let Some(v) = item.count {
        write_escaped_u8(out, attr::COUNT);
        write_escaped_u8(out, v);
    }
    if let Some(v) = item.depot_id {
        write_escaped_u8(out, attr::DEPOT_ID);
        write_escaped_u16(out, v);
    }
    if let Some(v) = item.door_id {
        write_escaped_u8(out, attr::HOUSEDOOR_ID);
        write_escaped_u8(out, v);
    }
    if let Some(ref v) = item.tele_dest {
        write_escaped_u8(out, attr::TELE_DEST);
        write_escaped_u16(out, v.x);
        write_escaped_u16(out, v.y);
        write_escaped_u8(out, v.z);
    }
    if let Some(v) = item.rune_charges {
        write_escaped_u8(out, attr::RUNE_CHARGES);
        write_escaped_u16(out, v);
    }
    if let Some(v) = item.charges {
        write_escaped_u8(out, attr::CHARGES);
        write_escaped_u16(out, v);
    }
    if let Some(v) = item.duration {
        write_escaped_u8(out, attr::DURATION);
        write_escaped_u32(out, v);
    }

    // Write preserved unknown attributes for forward-compatibility
    for (attr_id, data) in &item.unknown_attrs {
        if *attr_id == 255 {
            // Raw trailing data — write bytes directly (already includes attr type)
            write_escaped_bytes(out, data);
        } else {
            write_escaped_u8(out, *attr_id);
            write_escaped_bytes(out, data);
        }
    }

    out.push(NODE_END);
}

// ---- Escaped write helpers ----

fn write_node_start(out: &mut Vec<u8>, nt: u8) {
    out.push(NODE_START);
    out.push(nt); // node type byte is NOT escaped
}

fn escape_byte(out: &mut Vec<u8>, b: u8) {
    if b == NODE_START || b == NODE_END || b == ESCAPE {
        out.push(ESCAPE);
    }
    out.push(b);
}

fn write_escaped_u8(out: &mut Vec<u8>, v: u8) {
    escape_byte(out, v);
}

fn write_escaped_u16(out: &mut Vec<u8>, v: u16) {
    let bytes = v.to_le_bytes();
    escape_byte(out, bytes[0]);
    escape_byte(out, bytes[1]);
}

fn write_escaped_u32(out: &mut Vec<u8>, v: u32) {
    let bytes = v.to_le_bytes();
    for b in bytes {
        escape_byte(out, b);
    }
}

fn write_escaped_bytes(out: &mut Vec<u8>, data: &[u8]) {
    for &b in data {
        escape_byte(out, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::parse_otbm_bytes;

    /// Build a map with various features and verify it roundtrips.
    fn make_test_map() -> MapData {
        let mut map = MapData::new();
        map.width = 2048;
        map.height = 2048;
        map.version = 2;
        map.description = "Test map".to_string();
        map.spawn_file = "spawns.xml".to_string();
        map.house_file = "houses.xml".to_string();
        map.item_major_version = 1;
        map.item_minor_version = 2;

        // Plain ground tile
        let mut t1 = Tile::new(100, 200, 7);
        t1.ground = Some(4526);
        map.set_tile(t1);

        // Tile with items and attributes
        let mut t2 = Tile::new(101, 200, 7);
        t2.ground = Some(4526);
        let mut item = MapItem::new(2160);
        item.action_id = Some(1001);
        item.unique_id = Some(5000);
        item.text = Some("Hello".to_string());
        item.count = Some(5);
        t2.items.push(item);
        map.set_tile(t2);

        // Tile with teleport
        let mut t3 = Tile::new(102, 200, 7);
        t3.ground = Some(4526);
        let mut tele = MapItem::new(1387);
        tele.tele_dest = Some(TeleportDest { x: 500, y: 600, z: 7 });
        t3.items.push(tele);
        map.set_tile(t3);

        // Tile with flags (protection zone)
        let mut t4 = Tile::new(103, 200, 7);
        t4.ground = Some(4526);
        t4.flags = TileFlags { protection_zone: true, ..Default::default() };
        map.set_tile(t4);

        // House tile
        let mut t5 = Tile::new(104, 200, 7);
        t5.ground = Some(4526);
        t5.house_id = Some(42);
        let mut door = MapItem::new(6900);
        door.door_id = Some(1);
        t5.items.push(door);
        map.set_tile(t5);

        // Tile with rune charges, duration
        let mut t6 = Tile::new(105, 200, 7);
        t6.ground = Some(4526);
        let mut rune = MapItem::new(2268);
        rune.rune_charges = Some(400);
        rune.charges = Some(3);
        rune.duration = Some(60000);
        t6.items.push(rune);
        map.set_tile(t6);

        // Tile with depot
        let mut t7 = Tile::new(106, 200, 7);
        t7.ground = Some(4526);
        let mut depot = MapItem::new(2591);
        depot.depot_id = Some(1);
        t7.items.push(depot);
        map.set_tile(t7);

        // Tile with description
        let mut t8 = Tile::new(107, 200, 7);
        t8.ground = Some(4526);
        let mut sign = MapItem::new(2580);
        sign.description = Some("Sign text".to_string());
        t8.items.push(sign);
        map.set_tile(t8);

        // Town
        map.towns.push(Town {
            id: 1,
            name: "Thais".to_string(),
            position: Position { x: 100, y: 200, z: 7 },
        });

        // Waypoint
        map.waypoints.push(Waypoint {
            name: "temple".to_string(),
            position: Position { x: 100, y: 200, z: 7 },
        });

        map
    }

    #[test]
    fn roundtrip_preserves_map_metadata() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        assert_eq!(parsed.width, 2048);
        assert_eq!(parsed.height, 2048);
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.description, "Test map");
        assert_eq!(parsed.spawn_file, "spawns.xml");
        assert_eq!(parsed.house_file, "houses.xml");
    }

    #[test]
    fn roundtrip_preserves_tile_count() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();
        assert_eq!(parsed.tile_count(), map.tile_count());
    }

    #[test]
    fn roundtrip_preserves_tile_items() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(101, 200, 7).expect("tile with items");
        assert_eq!(tile.ground, Some(4526));
        assert_eq!(tile.items.len(), 1);
        assert_eq!(tile.items[0].id, 2160);
        assert_eq!(tile.items[0].action_id, Some(1001));
        assert_eq!(tile.items[0].unique_id, Some(5000));
        assert_eq!(tile.items[0].text.as_deref(), Some("Hello"));
        assert_eq!(tile.items[0].count, Some(5));
    }

    #[test]
    fn roundtrip_preserves_teleport() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(102, 200, 7).expect("teleport tile");
        let tele = &tile.items[0];
        let dest = tele.tele_dest.expect("tele_dest present");
        assert_eq!(dest.x, 500);
        assert_eq!(dest.y, 600);
        assert_eq!(dest.z, 7);
    }

    #[test]
    fn roundtrip_preserves_flags() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(103, 200, 7).expect("flagged tile");
        assert!(tile.flags.protection_zone);
        assert!(!tile.flags.no_pvp);
    }

    #[test]
    fn roundtrip_preserves_house_tile() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(104, 200, 7).expect("house tile");
        assert_eq!(tile.house_id, Some(42));
        assert_eq!(tile.items[0].door_id, Some(1));
    }

    #[test]
    fn roundtrip_preserves_rune_charges_and_duration() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(105, 200, 7).expect("rune tile");
        let item = &tile.items[0];
        assert_eq!(item.rune_charges, Some(400));
        assert_eq!(item.charges, Some(3));
        assert_eq!(item.duration, Some(60000));
    }

    #[test]
    fn roundtrip_preserves_depot() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(106, 200, 7).expect("depot tile");
        assert_eq!(tile.items[0].depot_id, Some(1));
    }

    #[test]
    fn roundtrip_preserves_description() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        let tile = parsed.get_tile(107, 200, 7).expect("sign tile");
        assert_eq!(tile.items[0].description.as_deref(), Some("Sign text"));
    }

    #[test]
    fn roundtrip_preserves_towns() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        assert_eq!(parsed.towns.len(), 1);
        assert_eq!(parsed.towns[0].name, "Thais");
        assert_eq!(parsed.towns[0].id, 1);
        assert_eq!(parsed.towns[0].position.x, 100);
    }

    #[test]
    fn roundtrip_preserves_waypoints() {
        let map = make_test_map();
        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();

        assert_eq!(parsed.waypoints.len(), 1);
        assert_eq!(parsed.waypoints[0].name, "temple");
    }

    #[test]
    fn empty_map_roundtrips() {
        let mut map = MapData::new();
        map.width = 512;
        map.height = 512;
        map.version = 2;

        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();
        assert_eq!(parsed.tile_count(), 0);
        assert_eq!(parsed.width, 512);
    }

    #[test]
    fn escape_special_bytes_in_item_ids() {
        // Item IDs containing 0xFD, 0xFE, 0xFF bytes must be escaped correctly
        let mut map = MapData::new();
        map.width = 256;
        map.height = 256;
        map.version = 2;

        // 0xFEFD = 65277, contains both ESCAPE and NODE_START bytes
        let mut tile = Tile::new(10, 10, 7);
        tile.ground = Some(0xFEFD);
        map.set_tile(tile);

        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();
        let t = parsed.get_tile(10, 10, 7).expect("tile with special-byte ID");
        assert_eq!(t.ground, Some(0xFEFD));
    }

    #[test]
    fn multi_z_level_roundtrip() {
        let mut map = MapData::new();
        map.width = 256;
        map.height = 256;
        map.version = 2;

        for z in 0..=15 {
            let mut t = Tile::new(100, 100, z);
            t.ground = Some(4526);
            map.set_tile(t);
        }

        let bytes = serialize_otbm_bytes(&map).unwrap();
        let parsed = parse_otbm_bytes(&bytes).unwrap();
        assert_eq!(parsed.tile_count(), 16);
        for z in 0..=15 {
            assert!(parsed.get_tile(100, 100, z).is_some(), "missing z={z}");
        }
    }
}
