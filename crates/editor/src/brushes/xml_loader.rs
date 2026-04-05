//! XML brush data loader — parses RME-format materials XML files.
//!
//! Loads ground brushes, wall brushes, doodad brushes, table/carpet brushes,
//! and tilesets from the standard RME data/ directory structure.

use std::path::Path;
use std::sync::Arc;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tracing::{debug, warn};

use super::border::AutoBorder;
use super::carpet::{CarpetAlignment, CarpetBrush};
use super::creature::{CreatureBrush, CreatureDef, SpawnBrush, WaypointBrush};
use super::doodad::{Composite, CompositeEntry, DoodadBrush};
use super::door::{DoorBrush, DoorItem, DoorOrientation, DoorVariant};
use super::flag::{FlagBrush, ZoneFlag};
use super::ground::GroundBrush;
use super::raw::RawBrush;
use super::registry::{BrushPalette, BrushRegistry, Tileset};
use super::table::{TableAlignment, TableBrush};
use super::wall::{WallAlignment, WallBrush};

/// Load all materials from an RME-format materials directory.
/// Expects to find files like `grounds.xml`, `walls.xml`, `doodads.xml`, `tilesets.xml`.
pub fn load_materials_dir(
    dir: &Path,
    registry: &mut BrushRegistry,
) -> Result<usize, anyhow::Error> {
    let mut count = 0;

    // Load individual material files
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "xml") {
            match load_materials_file(&path, registry) {
                Ok(n) => {
                    debug!("Loaded {} brushes from {}", n, path.display());
                    count += n;
                }
                Err(e) => {
                    warn!("Failed to load {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(count)
}

/// Load brushes from a single XML materials file.
pub fn load_materials_file(
    path: &Path,
    registry: &mut BrushRegistry,
) -> Result<usize, anyhow::Error> {
    let xml_data = std::fs::read_to_string(path)?;
    load_materials_xml(&xml_data, registry)
}

/// Parse brushes from XML string.
pub fn load_materials_xml(xml: &str, registry: &mut BrushRegistry) -> Result<usize, anyhow::Error> {
    let mut reader = Reader::from_str(xml);
    let mut count = 0;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"brush" => {
                let attrs = parse_brush_attrs(e)?;
                match attrs.brush_type.as_deref() {
                    Some("ground") => {
                        if parse_ground_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("wall") => {
                        if parse_wall_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("table") => {
                        if parse_table_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("carpet") => {
                        if parse_carpet_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("doodad") => {
                        if parse_doodad_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("door") => {
                        if parse_door_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("creature") => {
                        if parse_creature_brush(&mut reader, &attrs, registry)?.is_some() {
                            count += 1;
                        }
                    }
                    Some("spawn") => {
                        if parse_spawn_brush(&attrs, registry).is_some() {
                            count += 1;
                        }
                        // spawn is typically an empty-element brush
                        skip_to_end(&mut reader, b"brush").ok();
                    }
                    Some("waypoint") => {
                        if parse_waypoint_brush(&attrs, registry).is_some() {
                            count += 1;
                        }
                        skip_to_end(&mut reader, b"brush").ok();
                    }
                    Some("raw") => {
                        if parse_raw_brush(&attrs, registry).is_some() {
                            count += 1;
                        }
                        skip_to_end(&mut reader, b"brush").ok();
                    }
                    Some("flag") => {
                        if parse_flag_brush(&attrs, registry).is_some() {
                            count += 1;
                        }
                        skip_to_end(&mut reader, b"brush").ok();
                    }
                    _ => {
                        // Unknown or raw brush type — skip to end
                        if matches!(reader.read_event_into(&mut buf), Ok(Event::Start(_))) {
                            skip_to_end(&mut reader, b"brush")?;
                        }
                    }
                }
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"tileset" => {
                parse_tileset(&mut reader, e, registry)?;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
        }
        buf.clear();
    }

    Ok(count)
}

/// Parsed brush attributes from the <brush> tag.
struct BrushAttrs {
    name: Option<String>,
    brush_type: Option<String>,
    look_id: Option<u16>,
    z_order: Option<i32>,
    draggable: bool,
    on_blocking: bool,
    redo_borders: bool,
}

fn parse_brush_attrs(e: &quick_xml::events::BytesStart) -> Result<BrushAttrs, anyhow::Error> {
    let mut attrs = BrushAttrs {
        name: None,
        brush_type: None,
        look_id: None,
        z_order: None,
        draggable: false,
        on_blocking: false,
        redo_borders: false,
    };

    for attr in e.attributes().flatten() {
        let key = std::str::from_utf8(attr.key.as_ref())?;
        let val = std::str::from_utf8(&attr.value)?;
        match key {
            "name" => attrs.name = Some(val.to_string()),
            "type" => attrs.brush_type = Some(val.to_string()),
            "lookid" => attrs.look_id = val.parse().ok(),
            "z-order" => attrs.z_order = val.parse().ok(),
            "draggable" => attrs.draggable = val == "true",
            "on_blocking" => attrs.on_blocking = val == "true",
            "redo_borders" => attrs.redo_borders = val == "true",
            _ => {}
        }
    }

    Ok(attrs)
}

fn skip_to_end(reader: &mut Reader<&[u8]>, tag: &[u8]) -> Result<(), anyhow::Error> {
    let mut buf = Vec::new();
    let mut depth = 1u32;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == tag => depth += 1,
            Ok(Event::End(ref e)) if e.name().as_ref() == tag => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("XML skip error: {}", e)),
        }
        buf.clear();
    }
}

// ── Ground brush parsing ──

fn parse_ground_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let z_order = attrs.z_order.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let mut brush = GroundBrush::new(brush_id, name, look_id, z_order);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"item" => {
                        let (id, chance) = parse_item_attrs(e)?;
                        if let Some(id) = id {
                            brush.add_item(id, chance.unwrap_or(1));
                            registry.register_ground_item(id, brush_id);
                        }
                    }
                    b"border" => {
                        let border = parse_border_element(e)?;
                        let align = attr_str(e, "align").unwrap_or_default();
                        if align == "outer" {
                            brush.outer_border = Some(border);
                        } else if align == "inner" {
                            brush.inner_border = Some(border);
                        }
                        // Skip inner content if it's a start tag
                        if matches!(e.name().as_ref(), b"border") && !is_empty_event(e) {
                            skip_to_end(reader, b"border")?;
                        }
                    }
                    b"optional" => {
                        let border = parse_border_element(e)?;
                        brush.optional_border = Some(border);
                    }
                    b"friend" => {
                        if let Some(fname) = attr_str(e, "name") {
                            if fname == "all" {
                                brush.friend_all = true;
                            } else {
                                brush.friends.push(fname);
                            }
                        }
                    }
                    b"enemy" => {
                        if let Some(ename) = attr_str(e, "name") {
                            if ename == "all" {
                                brush.enemy_all = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"brush" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Ground brush parse error: {}", e)),
        }
        buf.clear();
    }

    // Also register look_id
    if look_id != 0 {
        registry.register_ground_item(look_id, brush_id);
    }

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

// ── Wall brush parsing ──

fn parse_wall_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let mut brush = WallBrush::new(brush_id, name, look_id);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"wall" => {
                        if let (Some(wall_type), Some(id)) =
                            (attr_str(e, "type"), attr_u16(e, "id"))
                        {
                            let alignment = match wall_type.as_str() {
                                "pole" => WallAlignment::Pole,
                                "horizontal" => WallAlignment::Horizontal,
                                "vertical" => WallAlignment::Vertical,
                                "north_end" => WallAlignment::NorthEnd,
                                "east_end" => WallAlignment::EastEnd,
                                "south_end" => WallAlignment::SouthEnd,
                                "west_end" => WallAlignment::WestEnd,
                                "north_t" => WallAlignment::NorthT,
                                "east_t" => WallAlignment::EastT,
                                "south_t" => WallAlignment::SouthT,
                                "west_t" => WallAlignment::WestT,
                                "northwest_diagonal" => WallAlignment::NorthWestDiag,
                                "northeast_diagonal" => WallAlignment::NorthEastDiag,
                                "southwest_diagonal" => WallAlignment::SouthWestDiag,
                                "southeast_diagonal" => WallAlignment::SouthEastDiag,
                                "intersection" => WallAlignment::Intersection,
                                _ => WallAlignment::Pole,
                            };
                            brush.set_alignment_item(alignment, id);
                            registry.register_wall_item(id, brush_id);
                        }
                    }
                    b"door" => {
                        if let Some(id) = attr_u16(e, "id") {
                            let door_type = attr_str(e, "type").unwrap_or_default();
                            let open = attr_str(e, "open").unwrap_or_default() == "true";
                            let _ = door_type;
                            let _ = open;
                            // Door entries stored on wall brush for reference
                            use super::wall::DoorEntry;
                            brush.doors.push(DoorEntry {
                                item_id: id,
                                door_type: match attr_str(e, "type").unwrap_or_default().as_str() {
                                    "locked" => super::wall::DoorType::Locked,
                                    "quest" => super::wall::DoorType::Quest,
                                    "magic" => super::wall::DoorType::Magic,
                                    "window" => super::wall::DoorType::Window,
                                    "hatch_window" => super::wall::DoorType::HatchWindow,
                                    _ => super::wall::DoorType::Normal,
                                },
                            });
                        }
                    }
                    b"friend" => {
                        if let Some(fname) = attr_str(e, "name") {
                            brush.friends.push(fname);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"brush" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Wall brush parse error: {}", e)),
        }
        buf.clear();
    }

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

// ── Table brush parsing ──

fn parse_table_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let mut brush = TableBrush::new(brush_id, name, look_id);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"item" => {
                if let (Some(id), Some(align_str)) = (attr_u16(e, "id"), attr_str(e, "type")) {
                    let alignment = match align_str.as_str() {
                        "alone" => TableAlignment::Alone,
                        "north_end" => TableAlignment::NorthEnd,
                        "east_end" => TableAlignment::EastEnd,
                        "south_end" => TableAlignment::SouthEnd,
                        "west_end" => TableAlignment::WestEnd,
                        "horizontal" => TableAlignment::Horizontal,
                        "vertical" => TableAlignment::Vertical,
                        _ => TableAlignment::Alone,
                    };
                    brush.set_item(alignment, id);
                    registry.register_table_item(id, brush_id);
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"brush" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Table brush parse error: {}", e)),
        }
        buf.clear();
    }

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

// ── Carpet brush parsing ──

fn parse_carpet_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let mut brush = CarpetBrush::new(brush_id, name, look_id);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"item" => {
                if let (Some(id), Some(align_str)) = (attr_u16(e, "id"), attr_str(e, "type")) {
                    let alignment = match align_str.as_str() {
                        "center" => CarpetAlignment::Center,
                        "north" => CarpetAlignment::North,
                        "east" => CarpetAlignment::East,
                        "south" => CarpetAlignment::South,
                        "west" => CarpetAlignment::West,
                        "northwest" => CarpetAlignment::NorthWest,
                        "northeast" => CarpetAlignment::NorthEast,
                        "southwest" => CarpetAlignment::SouthWest,
                        "southeast" => CarpetAlignment::SouthEast,
                        "full" => CarpetAlignment::Full,
                        "alone" => CarpetAlignment::Alone,
                        "horizontal" => CarpetAlignment::Horizontal,
                        "vertical" => CarpetAlignment::Vertical,
                        _ => CarpetAlignment::Alone,
                    };
                    brush.set_item(alignment, id);
                    registry.register_carpet_item(id, brush_id);
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"brush" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Carpet brush parse error: {}", e)),
        }
        buf.clear();
    }

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

// ── Doodad brush parsing ──

fn parse_doodad_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let mut brush = DoodadBrush::new(brush_id, name, look_id);
    brush.draggable = attrs.draggable;
    brush.on_blocking = attrs.on_blocking;
    brush.redo_borders = attrs.redo_borders;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"composite" => {
                let chance = attr_u32(e, "chance").unwrap_or(1);
                let composite = parse_composite(reader, chance)?;
                brush.add_composite(composite);
            }
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"item" => {
                let (id, chance) = parse_item_attrs(e)?;
                if let Some(id) = id {
                    brush.add_single_item(id, chance.unwrap_or(1));
                    brush.one_size = true;
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"brush" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Doodad brush parse error: {}", e)),
        }
        buf.clear();
    }

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

fn parse_composite(reader: &mut Reader<&[u8]>, chance: u32) -> Result<Composite, anyhow::Error> {
    let mut entries = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"tile" => {
                let dx = attr_i32(e, "x").unwrap_or(0);
                let dy = attr_i32(e, "y").unwrap_or(0);

                // Items inside the tile
                if !is_empty_event(e) {
                    let mut inner_buf = Vec::new();
                    loop {
                        match reader.read_event_into(&mut inner_buf) {
                            Ok(Event::Start(ref ie)) | Ok(Event::Empty(ref ie))
                                if ie.name().as_ref() == b"item" =>
                            {
                                if let Some(id) = attr_u16(ie, "id") {
                                    entries.push(CompositeEntry {
                                        dx,
                                        dy,
                                        item_id: id,
                                    });
                                }
                            }
                            Ok(Event::End(ref ie)) if ie.name().as_ref() == b"tile" => break,
                            Ok(Event::Eof) => break,
                            Ok(_) => {}
                            Err(e) => return Err(anyhow::anyhow!("Composite tile error: {}", e)),
                        }
                        inner_buf.clear();
                    }
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"composite" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Composite parse error: {}", e)),
        }
        buf.clear();
    }

    Ok(Composite { entries, chance })
}

// ── Tileset parsing ──

fn parse_tileset(
    reader: &mut Reader<&[u8]>,
    e: &quick_xml::events::BytesStart,
    registry: &mut BrushRegistry,
) -> Result<(), anyhow::Error> {
    let name = attr_str(e, "name").unwrap_or_else(|| "Unknown".to_string());
    let mut tileset = Tileset {
        name,
        palettes: Vec::new(),
    };

    let mut buf = Vec::new();
    let mut current_palette: Option<BrushPalette> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"palette" => {
                let pname = attr_str(e, "name").unwrap_or_else(|| "Default".to_string());
                current_palette = Some(BrushPalette {
                    name: pname,
                    brushes: Vec::new(),
                });
            }
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"brush" => {
                let bname = attr_str(e, "name");
                if let (Some(palette), Some(bname)) = (&mut current_palette, bname) {
                    if let Some(brush) = registry.get_by_name(&bname) {
                        palette.brushes.push(brush.id());
                    }
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"palette" => {
                if let Some(p) = current_palette.take() {
                    tileset.palettes.push(p);
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"tileset" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Tileset parse error: {}", e)),
        }
        buf.clear();
    }

    registry.add_tileset(tileset);
    Ok(())
}

// ── Door brush parsing ──

fn parse_door_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let mut brush = DoorBrush::new(brush_id, name, look_id);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) if e.name().as_ref() == b"door" => {
                let open_id = attr_u16(e, "open").unwrap_or(0);
                let closed_id = attr_u16(e, "closed").unwrap_or(0);
                let variant = match attr_str(e, "type").unwrap_or_default().as_str() {
                    "locked" => DoorVariant::Locked,
                    "quest" => DoorVariant::Quest,
                    "magic" => DoorVariant::Magic,
                    _ => DoorVariant::Normal,
                };
                let orientation = match attr_str(e, "orientation").unwrap_or_default().as_str() {
                    "vertical" => DoorOrientation::Vertical,
                    _ => DoorOrientation::Horizontal,
                };
                brush.add_door(DoorItem {
                    variant,
                    orientation,
                    open_id,
                    closed_id,
                });
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"brush" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("Door brush parse error: {}", e)),
        }
        buf.clear();
    }

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

// ── Creature brush parsing ──

fn parse_creature_brush(
    reader: &mut Reader<&[u8]>,
    attrs: &BrushAttrs,
    registry: &mut BrushRegistry,
) -> Result<Option<()>, anyhow::Error> {
    let name = match &attrs.name {
        Some(n) => n.clone(),
        None => return Ok(None),
    };
    let look_id = attrs.look_id.unwrap_or(0);
    let brush_id = registry.next_brush_id();

    let creature = CreatureDef {
        name: name.clone(),
        look_id,
        spawn_time: 60,
    };
    let brush = CreatureBrush::new(brush_id, creature);
    skip_to_end(reader, b"brush")?;

    registry.register(Arc::new(brush));
    Ok(Some(()))
}

// ── Spawn brush ──

fn parse_spawn_brush(attrs: &BrushAttrs, registry: &mut BrushRegistry) -> Option<()> {
    let brush_id = registry.next_brush_id();
    let brush = SpawnBrush::new(brush_id);
    registry.register(Arc::new(brush));
    let _ = attrs;
    Some(())
}

// ── Waypoint brush ──

fn parse_waypoint_brush(attrs: &BrushAttrs, registry: &mut BrushRegistry) -> Option<()> {
    let name = attrs.name.clone().unwrap_or_else(|| "waypoint".to_string());
    let brush_id = registry.next_brush_id();
    let brush = WaypointBrush::new(brush_id, name);
    registry.register(Arc::new(brush));
    Some(())
}

// ── Raw brush ──

fn parse_raw_brush(attrs: &BrushAttrs, registry: &mut BrushRegistry) -> Option<()> {
    let name = attrs.name.clone()?;
    let item_id = attrs.look_id?;
    let brush_id = registry.next_brush_id();
    let brush = RawBrush::new(brush_id, name, item_id, false);
    registry.register(Arc::new(brush));
    Some(())
}

// ── Flag brush ──

fn parse_flag_brush(attrs: &BrushAttrs, registry: &mut BrushRegistry) -> Option<()> {
    let flag_name = attrs.name.as_deref()?;
    let flag = match flag_name {
        "pvp" | "pvp_zone" => ZoneFlag::PvpZone,
        "nopvp" | "no_pvp" => ZoneFlag::NoPvp,
        "nologout" | "no_logout" => ZoneFlag::NoLogout,
        "protection" | "protection_zone" => ZoneFlag::ProtectionZone,
        _ => ZoneFlag::ProtectionZone,
    };
    let brush_id = registry.next_brush_id();
    let brush = FlagBrush::new(brush_id, flag);
    registry.register(Arc::new(brush));
    Some(())
}

#[allow(dead_code)]
fn attr_u32_from_attrs(_attrs: &BrushAttrs, _name: &str) -> Option<u32> {
    // BrushAttrs doesn't store arbitrary attrs; this is a placeholder
    None
}

// ── Helper attribute parsers ──

fn parse_item_attrs(
    e: &quick_xml::events::BytesStart,
) -> Result<(Option<u16>, Option<u32>), anyhow::Error> {
    let id = attr_u16(e, "id");
    let chance = attr_u32(e, "chance");
    Ok((id, chance))
}

fn parse_border_element(e: &quick_xml::events::BytesStart) -> Result<AutoBorder, anyhow::Error> {
    let id = attr_u16(e, "id").unwrap_or(0);
    Ok(AutoBorder::new(id))
}

fn attr_str(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.as_ref() == name.as_bytes() {
            std::str::from_utf8(&a.value).ok().map(|s| s.to_string())
        } else {
            None
        }
    })
}

fn attr_u16(e: &quick_xml::events::BytesStart, name: &str) -> Option<u16> {
    attr_str(e, name).and_then(|s| s.parse().ok())
}

fn attr_u32(e: &quick_xml::events::BytesStart, name: &str) -> Option<u32> {
    attr_str(e, name).and_then(|s| s.parse().ok())
}

fn attr_i32(e: &quick_xml::events::BytesStart, name: &str) -> Option<i32> {
    attr_str(e, name).and_then(|s| s.parse().ok())
}

fn is_empty_event(_e: &quick_xml::events::BytesStart) -> bool {
    // quick-xml doesn't expose this on BytesStart directly;
    // Empty events are delivered as Event::Empty, not Event::Start,
    // so if we're in a Start handler, it's not empty.
    false
}
