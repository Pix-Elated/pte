//! Read and write OpenTibia `spawn.xml` files.
//!
//! Format:
//! ```xml
//! <spawns>
//!   <spawn centerx="100" centery="200" centerz="7" radius="5">
//!     <monster name="Rat" x="-2" y="1" z="7" spawntime="60" />
//!     <npc name="Oracle" x="0" y="0" z="7" spawntime="0" />
//!   </spawn>
//! </spawns>
//! ```

use std::path::Path;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

/// A single creature within a spawn area.
#[derive(Debug, Clone)]
pub struct SpawnCreature {
    pub name: String,
    /// Whether this is an NPC (true) or monster (false).
    pub is_npc: bool,
    /// Offset from spawn center (can be negative).
    pub offset_x: i32,
    pub offset_y: i32,
    pub z: u8,
    /// Respawn time in seconds (0 = no respawn, i.e. static NPC).
    pub spawn_time: u32,
}

/// A spawn area (circle of radius R centered at a position).
#[derive(Debug, Clone)]
pub struct Spawn {
    pub center_x: u16,
    pub center_y: u16,
    pub center_z: u8,
    pub radius: u8,
    pub creatures: Vec<SpawnCreature>,
}

fn attr_str(e: &BytesStart, key: &[u8]) -> String {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
        .unwrap_or_default()
}

fn attr_parse<T: std::str::FromStr>(e: &BytesStart, key: &[u8], default: T) -> T {
    attr_str(e, key).parse().unwrap_or(default)
}

/// Parse a `spawn.xml` file.
pub fn read_spawns(path: &Path) -> anyhow::Result<Vec<Spawn>> {
    let data = std::fs::read_to_string(path)?;
    let mut reader = Reader::from_str(&data);

    let mut spawns = Vec::new();
    let mut current_spawn: Option<Spawn> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = reader.decoder().decode(e.name().as_ref())
                    .unwrap_or_default()
                    .to_string();

                match tag.as_str() {
                    "spawn" => {
                        let spawn = Spawn {
                            center_x: attr_parse(e, b"centerx", 0),
                            center_y: attr_parse(e, b"centery", 0),
                            center_z: attr_parse(e, b"centerz", 7),
                            radius: attr_parse(e, b"radius", 5),
                            creatures: Vec::new(),
                        };
                        current_spawn = Some(spawn);
                    }
                    "monster" | "npc" => {
                        if let Some(ref mut spawn) = current_spawn {
                            spawn.creatures.push(SpawnCreature {
                                name: attr_str(e, b"name"),
                                is_npc: tag == "npc",
                                offset_x: attr_parse(e, b"x", 0),
                                offset_y: attr_parse(e, b"y", 0),
                                z: attr_parse(e, b"z", spawn.center_z),
                                spawn_time: attr_parse(e, b"spawntime", 60),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = reader.decoder().decode(e.name().as_ref())
                    .unwrap_or_default()
                    .to_string();
                if tag == "spawn" {
                    if let Some(spawn) = current_spawn.take() {
                        spawns.push(spawn);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {e}")),
            _ => {}
        }
    }

    Ok(spawns)
}

/// Write spawns to a `spawn.xml` file.
pub fn write_spawns(spawns: &[Spawn], path: &Path) -> anyhow::Result<()> {
    use std::fmt::Write;

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<spawns>\n");

    for spawn in spawns {
        write!(
            xml,
            "\t<spawn centerx=\"{}\" centery=\"{}\" centerz=\"{}\" radius=\"{}\">\n",
            spawn.center_x, spawn.center_y, spawn.center_z, spawn.radius
        )?;
        for c in &spawn.creatures {
            let tag = if c.is_npc { "npc" } else { "monster" };
            let safe_name = c.name.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
            write!(
                xml,
                "\t\t<{} name=\"{}\" x=\"{}\" y=\"{}\" z=\"{}\" spawntime=\"{}\" />\n",
                tag, safe_name, c.offset_x, c.offset_y, c.z, c.spawn_time
            )?;
        }
        xml.push_str("\t</spawn>\n");
    }

    xml.push_str("</spawns>\n");
    std::fs::write(path, &xml)?;
    Ok(())
}


