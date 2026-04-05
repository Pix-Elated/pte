//! Common types for SPR/DAT data.

/// A single 32×32 sprite as RGBA pixels (4096 bytes).
#[derive(Debug, Clone)]
pub struct Sprite {
    /// RGBA pixel data, 32×32×4 = 4096 bytes, row-major top-down.
    pub pixels: Vec<u8>,
}

impl Sprite {
    pub const WIDTH: u32 = 32;
    pub const HEIGHT: u32 = 32;
    pub const PIXEL_COUNT: usize = (Self::WIDTH * Self::HEIGHT) as usize;
    pub const BYTE_COUNT: usize = Self::PIXEL_COUNT * 4;

    pub fn new_transparent() -> Self {
        Self {
            pixels: vec![0u8; Self::BYTE_COUNT],
        }
    }

    pub fn is_blank(&self) -> bool {
        self.pixels.chunks_exact(4).all(|px| px[3] == 0)
    }
}

/// Complete sprite file data.
#[derive(Debug)]
pub struct SprFile {
    pub signature: u32,
    /// Sprites indexed by ID (1-based in file, 0-based in this vec).
    pub sprites: Vec<Sprite>,
    /// Whether to use extended (u32) sprite count header (>= v960).
    pub extended: bool,
}

impl SprFile {
    pub fn sprite_count(&self) -> u32 {
        self.sprites.len() as u32
    }

    /// Get sprite by 1-based ID (as used in DAT references).
    pub fn get_sprite(&self, id: u32) -> Option<&Sprite> {
        if id == 0 {
            return None;
        }
        self.sprites.get((id - 1) as usize)
    }

    /// Get mutable sprite by 1-based ID.
    pub fn get_sprite_mut(&mut self, id: u32) -> Option<&mut Sprite> {
        if id == 0 {
            return None;
        }
        self.sprites.get_mut((id - 1) as usize)
    }

    /// Add a new sprite, returns its 1-based ID.
    pub fn add_sprite(&mut self, sprite: Sprite) -> u32 {
        self.sprites.push(sprite);
        self.sprites.len() as u32
    }

    /// Replace a sprite at 1-based ID.
    pub fn replace_sprite(&mut self, id: u32, sprite: Sprite) -> bool {
        if id == 0 || id as usize > self.sprites.len() {
            return false;
        }
        self.sprites[(id - 1) as usize] = sprite;
        true
    }

    /// Delete a sprite by 1-based ID (replaces with transparent).
    /// Cannot truly remove without breaking all DAT references.
    pub fn delete_sprite(&mut self, id: u32) -> bool {
        if id == 0 || id as usize > self.sprites.len() {
            return false;
        }
        self.sprites[(id - 1) as usize] = Sprite::new_transparent();
        true
    }
}

/// Metadata category — matches Tibia's internal grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatCategory {
    Item,
    Outfit,
    Effect,
    Missile,
}

/// Sprite layout dimensions from DAT.
#[derive(Debug, Clone)]
pub struct SpriteLayout {
    pub width: u8,
    pub height: u8,
    pub exact_size: Option<u8>,
    pub layers: u8,
    pub pattern_x: u8,
    pub pattern_y: u8,
    pub pattern_z: u8,
    pub frames: u8,
    pub sprite_ids: Vec<u32>,
}

/// A single DAT entry (item, outfit, effect, or missile).
#[derive(Debug, Clone)]
pub struct DatEntry {
    pub id: u32,
    pub category: DatCategory,
    pub flags: DatFlags,
    pub sprite_layout: SpriteLayout,
}

/// Attribute flags from DAT format.
/// These use byte markers: the attribute byte indicates what follows.
#[derive(Debug, Clone, Default)]
pub struct DatFlags {
    pub is_ground: Option<u16>, // speed
    pub is_ground_border: bool,
    pub is_on_bottom: bool,
    pub is_on_top: bool,
    pub is_container: bool,
    pub is_stackable: bool,
    pub is_force_use: bool,
    pub is_multi_use: bool,
    pub is_writable: Option<u16>, // max text length
    pub is_writable_once: Option<u16>,
    pub is_fluid_container: bool,
    pub is_splash: bool,
    pub is_not_walkable: bool,
    pub is_not_moveable: bool,
    pub blocks_projectile: bool,
    pub is_not_pathable: bool,
    pub is_pickupable: bool,
    pub is_hangable: bool,
    pub hook_south: bool,
    pub hook_east: bool,
    pub is_rotatable: bool,
    pub has_light: Option<(u16, u16)>, // intensity, color
    pub dont_hide: bool,
    pub is_translucent: bool,
    pub has_offset: Option<(u16, u16)>, // x, y
    pub has_elevation: Option<u16>,
    pub is_lying_object: bool,
    pub animate_always: bool,
    pub has_minimap_color: Option<u16>,
    pub has_lens_help: Option<u16>,
    pub is_full_ground: bool,
    pub ignore_look: bool,
    pub is_cloth: Option<u16>, // slot
    pub is_market_item: Option<MarketData>,
    pub has_default_action: Option<u16>,
    pub is_usable: bool,
    pub raw_attrs: Vec<(u8, Vec<u8>)>, // unknown attributes preserved for round-trip
}

/// Market item data.
#[derive(Debug, Clone)]
pub struct MarketData {
    pub category: u16,
    pub trade_as_id: u16,
    pub show_as_id: u16,
    pub name: String,
    pub restrict_profession: u16,
    pub restrict_level: u16,
}

/// Complete DAT file.
#[derive(Debug)]
pub struct DatFile {
    pub signature: u32,
    pub items: Vec<DatEntry>,
    pub outfits: Vec<DatEntry>,
    pub effects: Vec<DatEntry>,
    pub missiles: Vec<DatEntry>,
}

impl DatFile {
    /// Start item ID (items start at 100 in most versions).
    pub const ITEM_START_ID: u32 = 100;

    pub fn item_count(&self) -> u16 {
        self.items.len() as u16
    }

    pub fn outfit_count(&self) -> u16 {
        self.outfits.len() as u16
    }

    pub fn effect_count(&self) -> u16 {
        self.effects.len() as u16
    }

    pub fn missile_count(&self) -> u16 {
        self.missiles.len() as u16
    }

    pub fn get_entry(&self, category: DatCategory, id: u32) -> Option<&DatEntry> {
        let list = match category {
            DatCategory::Item => &self.items,
            DatCategory::Outfit => &self.outfits,
            DatCategory::Effect => &self.effects,
            DatCategory::Missile => &self.missiles,
        };
        list.iter().find(|e| e.id == id)
    }

    pub fn get_entry_mut(&mut self, category: DatCategory, id: u32) -> Option<&mut DatEntry> {
        let list = match category {
            DatCategory::Item => &mut self.items,
            DatCategory::Outfit => &mut self.outfits,
            DatCategory::Effect => &mut self.effects,
            DatCategory::Missile => &mut self.missiles,
        };
        list.iter_mut().find(|e| e.id == id)
    }

    /// Add a new entry at the end of its category. Returns assigned ID.
    pub fn add_entry(&mut self, mut entry: DatEntry) -> u32 {
        let list = match entry.category {
            DatCategory::Item => &mut self.items,
            DatCategory::Outfit => &mut self.outfits,
            DatCategory::Effect => &mut self.effects,
            DatCategory::Missile => &mut self.missiles,
        };
        let next_id = if entry.category == DatCategory::Item {
            Self::ITEM_START_ID + list.len() as u32
        } else {
            1 + list.len() as u32
        };
        entry.id = next_id;
        list.push(entry);
        next_id
    }

    /// Remove an entry. Returns true if found & removed.
    pub fn remove_entry(&mut self, category: DatCategory, id: u32) -> bool {
        let list = match category {
            DatCategory::Item => &mut self.items,
            DatCategory::Outfit => &mut self.outfits,
            DatCategory::Effect => &mut self.effects,
            DatCategory::Missile => &mut self.missiles,
        };
        if let Some(pos) = list.iter().position(|e| e.id == id) {
            list.remove(pos);
            true
        } else {
            false
        }
    }
}
