//! OTBM binary tree parser.
//!
//! OTBM format uses escape-coded binary trees:
//! - 0xFE = node start (followed by node type byte)
//! - 0xFF = node end
//! - 0xFD = escape prefix (next byte is literal data, not a control byte)

use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::types::*;

/// Escape-coded byte values.
const NODE_START: u8 = 0xFE;
const NODE_END: u8 = 0xFF;
const ESCAPE: u8 = 0xFD;

/// A raw parsed OTBM node before interpretation.
struct RawNode {
    node_type: u8,
    data: Vec<u8>,
    children: Vec<RawNode>,
}

/// Parse an OTBM file from disk.
pub fn parse_otbm(path: &Path) -> Result<MapData> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_otbm_bytes(&data)
}

/// Parse OTBM from bytes.
pub fn parse_otbm_bytes(data: &[u8]) -> Result<MapData> {
    // Skip the 4-byte identifier/version header
    if data.len() < 4 {
        bail!("OTBM file too small");
    }

    let mut pos = 4; // skip OTBI header
    let root = parse_node(data, &mut pos).context("parsing root node")?;

    interpret_root(&root)
}

/// Parse a single node and its children from the escape-coded stream.
fn parse_node(data: &[u8], pos: &mut usize) -> Result<RawNode> {
    if *pos >= data.len() || data[*pos] != NODE_START {
        bail!("Expected NODE_START at position {}", *pos);
    }
    *pos += 1; // skip 0xFE

    if *pos >= data.len() {
        bail!("Unexpected end of data after NODE_START");
    }
    let node_type = data[*pos];
    *pos += 1;

    // Read node data until we hit NODE_START (child), NODE_END (close), or end
    let mut node_data = Vec::new();
    let mut children = Vec::new();

    while *pos < data.len() {
        match data[*pos] {
            NODE_START => {
                // Child node
                let child = parse_node(data, pos)?;
                children.push(child);
            }
            NODE_END => {
                *pos += 1; // consume the 0xFF
                break;
            }
            ESCAPE => {
                *pos += 1;
                if *pos >= data.len() {
                    bail!("Unexpected end after ESCAPE at {}", *pos - 1);
                }
                node_data.push(data[*pos]);
                *pos += 1;
            }
            b => {
                node_data.push(b);
                *pos += 1;
            }
        }
    }

    Ok(RawNode {
        node_type,
        data: node_data,
        children,
    })
}

/// Interpret the root node into a MapData structure.
fn interpret_root(root: &RawNode) -> Result<MapData> {
    let mut map = MapData::new();

    // Root node data: version (u32), width (u16), height (u16), item versions
    let mut r = BufReader::new(&root.data);
    map.version = r.read_u32()?;
    map.width = r.read_u16()?;
    map.height = r.read_u16()?;
    map.item_major_version = r.read_u32()?;
    map.item_minor_version = r.read_u32()?;

    // First child should be MAP_DATA
    for child in &root.children {
        if child.node_type == node_type::MAP_DATA {
            interpret_map_data(child, &mut map)?;
        }
    }

    tracing::info!(
        tiles = map.tile_count(),
        towns = map.towns.len(),
        waypoints = map.waypoints.len(),
        "Parsed OTBM map"
    );

    Ok(map)
}

fn interpret_map_data(node: &RawNode, map: &mut MapData) -> Result<()> {
    // Parse map-level attributes from node data
    let mut r = BufReader::new(&node.data);
    while r.remaining() > 0 {
        let attr_type = r.read_u8()?;
        match attr_type {
            attr::DESCRIPTION => {
                let len = r.read_u16()? as usize;
                map.description = r.read_string(len)?;
            }
            attr::EXT_SPAWN_FILE => {
                let len = r.read_u16()? as usize;
                map.spawn_file = r.read_string(len)?;
            }
            attr::EXT_HOUSE_FILE => {
                let len = r.read_u16()? as usize;
                map.house_file = r.read_string(len)?;
            }
            other => {
                // Unknown map-level attribute — store remaining bytes and stop parsing attrs
                tracing::warn!("Unknown map-level attribute {other}, preserving remaining data");
                break;
            }
        }
    }

    for child in &node.children {
        match child.node_type {
            node_type::TILE_AREA => interpret_tile_area(child, map)?,
            node_type::TOWNS => interpret_towns(child, map)?,
            node_type::WAYPOINTS => interpret_waypoints(child, map)?,
            _ => {} // ignore unknown
        }
    }

    Ok(())
}

fn interpret_tile_area(node: &RawNode, map: &mut MapData) -> Result<()> {
    let mut r = BufReader::new(&node.data);
    let base_x = r.read_u16()?;
    let base_y = r.read_u16()?;
    let base_z = r.read_u8()?;

    for child in &node.children {
        if child.node_type == node_type::TILE || child.node_type == node_type::HOUSE_TILE {
            interpret_tile(child, base_x, base_y, base_z, map)?;
        }
    }

    Ok(())
}

fn interpret_tile(
    node: &RawNode,
    base_x: u16,
    base_y: u16,
    base_z: u8,
    map: &mut MapData,
) -> Result<()> {
    let mut r = BufReader::new(&node.data);
    let offset_x = r.read_u8()? as u16;
    let offset_y = r.read_u8()? as u16;

    let x = base_x + offset_x;
    let y = base_y + offset_y;
    let mut tile = Tile::new(x, y, base_z);

    // House tile has house_id as u32
    if node.node_type == node_type::HOUSE_TILE {
        tile.house_id = Some(r.read_u32()?);
    }

    // Parse tile attributes
    while r.remaining() > 0 {
        let attr_type = r.read_u8()?;
        match attr_type {
            attr::TILE_FLAGS => {
                tile.flags = TileFlags::from_u32(r.read_u32()?);
            }
            attr::ITEM => {
                let item_id = r.read_u16()?;
                if tile.ground.is_none() {
                    tile.ground = Some(item_id);
                } else {
                    tile.items.push(MapItem::new(item_id));
                }
            }
            other => {
                tracing::debug!("Unknown tile attribute {other} at ({x},{y},{base_z}), skipping remaining tile attrs");
                break;
            }
        }
    }

    // Child nodes are items
    for child in &node.children {
        if child.node_type == node_type::ITEM {
            let item = interpret_item(child)?;
            tile.items.push(item);
        }
    }

    map.set_tile(tile);
    Ok(())
}

fn interpret_item(node: &RawNode) -> Result<MapItem> {
    let mut r = BufReader::new(&node.data);
    let id = r.read_u16()?;
    let mut item = MapItem::new(id);

    while r.remaining() > 0 {
        let attr_type = r.read_u8()?;
        match attr_type {
            attr::ACTION_ID => item.action_id = Some(r.read_u16()?),
            attr::UNIQUE_ID => item.unique_id = Some(r.read_u16()?),
            attr::TEXT => {
                let len = r.read_u16()? as usize;
                item.text = Some(r.read_string(len)?);
            }
            attr::DESC => {
                let len = r.read_u16()? as usize;
                item.description = Some(r.read_string(len)?);
            }
            attr::TELE_DEST => {
                item.tele_dest = Some(TeleportDest {
                    x: r.read_u16()?,
                    y: r.read_u16()?,
                    z: r.read_u8()?,
                });
            }
            attr::DEPOT_ID => item.depot_id = Some(r.read_u16()?),
            attr::HOUSEDOOR_ID => item.door_id = Some(r.read_u8()?),
            attr::COUNT => item.count = Some(r.read_u8()?),
            attr::RUNE_CHARGES => item.rune_charges = Some(r.read_u16()?),
            attr::CHARGES => item.charges = Some(r.read_u16()?),
            attr::DURATION => item.duration = Some(r.read_u32()?),
            attr::DECAYING_STATE => {
                // Known fixed-size attr (1 byte) — skip but preserve
                let val = r.read_u8()?;
                item.unknown_attrs.push((attr_type, vec![val]));
            }
            attr::ATTRIBUTE_MAP => {
                // Variable-length — consume all remaining bytes as raw data
                let remaining = r.read_remaining();
                item.unknown_attrs.push((attr_type, remaining));
            }
            other => {
                // Truly unknown attr — preserve the attr type byte + all remaining data
                // We can't know the size, so we must stop parsing and keep the rest
                let mut blob = vec![other];
                blob.extend_from_slice(&r.read_remaining());
                if !blob.is_empty() {
                    // Store under a sentinel key (255) to indicate raw trailing data
                    item.unknown_attrs.push((255, blob));
                }
                break;
            }
        }
    }

    Ok(item)
}

fn interpret_towns(node: &RawNode, map: &mut MapData) -> Result<()> {
    for child in &node.children {
        if child.node_type == node_type::TOWN {
            let mut r = BufReader::new(&child.data);
            let id = r.read_u32()?;
            let name_len = r.read_u16()? as usize;
            let name = r.read_string(name_len)?;
            let position = Position {
                x: r.read_u16()?,
                y: r.read_u16()?,
                z: r.read_u8()?,
            };
            map.towns.push(Town { id, name, position });
        }
    }
    Ok(())
}

fn interpret_waypoints(node: &RawNode, map: &mut MapData) -> Result<()> {
    for child in &node.children {
        if child.node_type == node_type::WAYPOINT {
            let mut r = BufReader::new(&child.data);
            let name_len = r.read_u16()? as usize;
            let name = r.read_string(name_len)?;
            let position = Position {
                x: r.read_u16()?,
                y: r.read_u16()?,
                z: r.read_u8()?,
            };
            map.waypoints.push(Waypoint { name, position });
        }
    }
    Ok(())
}

/// Simple byte buffer reader for parsing node data.
struct BufReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BufReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.pos >= self.data.len() {
            bail!("Unexpected end of node data");
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16(&mut self) -> Result<u16> {
        if self.pos + 2 > self.data.len() {
            bail!("Unexpected end reading u16");
        }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32> {
        if self.pos + 4 > self.data.len() {
            bail!("Unexpected end reading u32");
        }
        let v = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_string(&mut self, len: usize) -> Result<String> {
        if self.pos + len > self.data.len() {
            bail!("Unexpected end reading string of len {}", len);
        }
        let s = String::from_utf8_lossy(&self.data[self.pos..self.pos + len]).into_owned();
        self.pos += len;
        Ok(s)
    }

    fn read_remaining(&mut self) -> Vec<u8> {
        let rest = self.data[self.pos..].to_vec();
        self.pos = self.data.len();
        rest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_small() {
        assert!(parse_otbm_bytes(&[]).is_err());
        assert!(parse_otbm_bytes(&[0, 0, 0]).is_err());
    }

    #[test]
    fn test_minimal_valid_otbm() {
        // Build a minimal valid OTBM:
        // 4-byte header + root node with version, width, height, item versions
        let mut data = vec![0u8; 4]; // OTBI header

        // NODE_START, type=ROOT(0)
        data.push(NODE_START);
        data.push(0); // node type = ROOT

        // Root node data: version u32, width u16, height u16, major u32, minor u32
        data.extend_from_slice(&2u32.to_le_bytes()); // version = 2
        data.extend_from_slice(&256u16.to_le_bytes()); // width
        data.extend_from_slice(&256u16.to_le_bytes()); // height
        data.extend_from_slice(&0u32.to_le_bytes()); // item major
        data.extend_from_slice(&0u32.to_le_bytes()); // item minor

        // Child: MAP_DATA node with no attributes
        data.push(NODE_START);
        data.push(node_type::MAP_DATA);
        data.push(NODE_END); // close MAP_DATA

        data.push(NODE_END); // close ROOT

        let map = parse_otbm_bytes(&data).unwrap();
        assert_eq!(map.version, 2);
        assert_eq!(map.width, 256);
        assert_eq!(map.height, 256);
        assert_eq!(map.tile_count(), 0);
    }

    #[test]
    fn test_escape_codes() {
        // Test that 0xFD escape prefix works correctly
        let mut data = vec![0u8; 4]; // OTBI header

        data.push(NODE_START);
        data.push(0); // ROOT

        // Write version = 2 with an escaped byte (0xFE needs escaping)
        data.extend_from_slice(&2u32.to_le_bytes()); // version
        data.extend_from_slice(&256u16.to_le_bytes()); // width
        data.extend_from_slice(&256u16.to_le_bytes()); // height
        data.extend_from_slice(&0u32.to_le_bytes()); // major
        data.extend_from_slice(&0u32.to_le_bytes()); // minor

        data.push(NODE_END);

        let map = parse_otbm_bytes(&data).unwrap();
        assert_eq!(map.version, 2);
    }

    /// Test against the real xtrails.otbm map file.
    #[test]
    fn test_real_otbm() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../canary/data-otservbr-global/world/xtrails.otbm");
        if !path.exists() {
            eprintln!("Skipping real OTBM test — file not found at {}", path.display());
            return;
        }

        let map = parse_otbm(&path).unwrap();
        assert!(map.width > 0);
        assert!(map.height > 0);
        assert!(map.tile_count() > 0, "Map should have tiles");

        // Dump bounds
        let mut min_x = u16::MAX;
        let mut max_x = 0u16;
        let mut min_y = u16::MAX;
        let mut max_y = 0u16;
        for chunk in map.chunks.values() {
            for tile in chunk.values() {
                min_x = min_x.min(tile.x);
                max_x = max_x.max(tile.x);
                min_y = min_y.min(tile.y);
                max_y = max_y.max(tile.y);
            }
        }
        eprintln!("Tile bounds: X={}..{}, Y={}..{}", min_x, max_x, min_y, max_y);
        eprintln!("Center: ({}, {})", (min_x as u32 + max_x as u32) / 2, (min_y as u32 + max_y as u32) / 2);

        eprintln!(
            "Parsed OTBM: {}x{}, {} tiles, {} towns, {} waypoints",
            map.width,
            map.height,
            map.tile_count(),
            map.towns.len(),
            map.waypoints.len()
        );

        // Verify we can round-trip: serialize and re-parse
        let serialized = crate::writer::serialize_otbm_bytes(&map).unwrap();
        let reparsed = parse_otbm_bytes(&serialized).unwrap();
        assert_eq!(reparsed.tile_count(), map.tile_count());
        assert_eq!(reparsed.width, map.width);
        assert_eq!(reparsed.height, map.height);
        assert_eq!(reparsed.towns.len(), map.towns.len());
    }
}
