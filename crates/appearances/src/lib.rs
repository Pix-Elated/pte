//! Appearances parser — loads appearances.dat (protobuf) into categorized appearance data.

use anyhow::{Context, Result};
use prost::Message;
use std::collections::HashMap;
use std::path::Path;

/// Generated protobuf types.
pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        "/otclient.protobuf.appearances.rs"
    ));
}

/// Re-export key types.
pub use proto::{
    Appearance, AppearanceFlags, Appearances, FrameGroup, SpriteAnimation, SpriteInfo, SpritePhase,
};

/// Categories of appearances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Object,
    Outfit,
    Effect,
    Missile,
}

/// All loaded appearances, indexed by category and ID.
#[derive(Debug, Default)]
pub struct LoadedAppearances {
    pub objects: HashMap<u32, Appearance>,
    pub outfits: HashMap<u32, Appearance>,
    pub effects: HashMap<u32, Appearance>,
    pub missiles: HashMap<u32, Appearance>,
}

impl LoadedAppearances {
    /// Get an appearance by category and ID.
    pub fn get(&self, category: Category, id: u32) -> Option<&Appearance> {
        match category {
            Category::Object => self.objects.get(&id),
            Category::Outfit => self.outfits.get(&id),
            Category::Effect => self.effects.get(&id),
            Category::Missile => self.missiles.get(&id),
        }
    }

    /// Get all appearances for a category, sorted by ID.
    pub fn get_all(&self, category: Category) -> Vec<&Appearance> {
        let map = match category {
            Category::Object => &self.objects,
            Category::Outfit => &self.outfits,
            Category::Effect => &self.effects,
            Category::Missile => &self.missiles,
        };
        let mut items: Vec<_> = map.values().collect();
        items.sort_by_key(|a| a.id());
        items
    }

    /// Iterate all appearances across all categories.
    pub fn iter_all(&self) -> impl Iterator<Item = (Category, &Appearance)> {
        self.objects
            .values()
            .map(|a| (Category::Object, a))
            .chain(self.outfits.values().map(|a| (Category::Outfit, a)))
            .chain(self.effects.values().map(|a| (Category::Effect, a)))
            .chain(self.missiles.values().map(|a| (Category::Missile, a)))
    }

    /// Total count across all categories.
    pub fn total_count(&self) -> usize {
        self.objects.len() + self.outfits.len() + self.effects.len() + self.missiles.len()
    }
}

/// Load appearances from a .dat file (protobuf binary).
pub fn load_appearances(path: &Path) -> Result<LoadedAppearances> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_appearances(&data)
}

/// Parse appearances from raw protobuf bytes.
pub fn parse_appearances(data: &[u8]) -> Result<LoadedAppearances> {
    let appearances = Appearances::decode(data).context("decoding appearances protobuf")?;

    let mut loaded = LoadedAppearances::default();

    for a in appearances.object {
        if let Some(id) = a.id {
            loaded.objects.insert(id, a);
        }
    }
    for a in appearances.outfit {
        if let Some(id) = a.id {
            loaded.outfits.insert(id, a);
        }
    }
    for a in appearances.effect {
        if let Some(id) = a.id {
            loaded.effects.insert(id, a);
        }
    }
    for a in appearances.missile {
        if let Some(id) = a.id {
            loaded.missiles.insert(id, a);
        }
    }

    tracing::info!(
        objects = loaded.objects.len(),
        outfits = loaded.outfits.len(),
        effects = loaded.effects.len(),
        missiles = loaded.missiles.len(),
        "Loaded appearances"
    );

    Ok(loaded)
}

/// Helper to get the first sprite ID from an appearance.
pub fn first_sprite_id(appearance: &Appearance) -> Option<u32> {
    appearance
        .frame_group
        .first()
        .and_then(|fg| fg.sprite_info.as_ref())
        .and_then(|si| si.sprite_id.first())
        .copied()
}

/// Serialize all appearances back to protobuf bytes.
pub fn serialize_appearances(loaded: &LoadedAppearances) -> Result<Vec<u8>> {
    let mut objects: Vec<_> = loaded.objects.values().cloned().collect();
    objects.sort_by_key(|a| a.id());
    let mut outfits: Vec<_> = loaded.outfits.values().cloned().collect();
    outfits.sort_by_key(|a| a.id());
    let mut effects: Vec<_> = loaded.effects.values().cloned().collect();
    effects.sort_by_key(|a| a.id());
    let mut missiles: Vec<_> = loaded.missiles.values().cloned().collect();
    missiles.sort_by_key(|a| a.id());

    let appearances = Appearances {
        object: objects,
        outfit: outfits,
        effect: effects,
        missile: missiles,
        special_meaning_appearance_ids: None,
    };

    let mut buf = Vec::with_capacity(appearances.encoded_len());
    appearances
        .encode(&mut buf)
        .context("encoding appearances protobuf")?;
    Ok(buf)
}

/// Write appearances to a file.
pub fn save_appearances(loaded: &LoadedAppearances, path: &Path) -> Result<()> {
    let data = serialize_appearances(loaded)?;
    std::fs::write(path, &data).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Add or replace an appearance in the loaded set.
pub fn upsert_appearance(
    loaded: &mut LoadedAppearances,
    category: Category,
    appearance: Appearance,
) {
    let id = appearance.id();
    let map = match category {
        Category::Object => &mut loaded.objects,
        Category::Outfit => &mut loaded.outfits,
        Category::Effect => &mut loaded.effects,
        Category::Missile => &mut loaded.missiles,
    };
    map.insert(id, appearance);
}

/// Remove an appearance from the loaded set.
pub fn remove_appearance(loaded: &mut LoadedAppearances, category: Category, id: u32) -> bool {
    let map = match category {
        Category::Object => &mut loaded.objects,
        Category::Outfit => &mut loaded.outfits,
        Category::Effect => &mut loaded.effects,
        Category::Missile => &mut loaded.missiles,
    };
    map.remove(&id).is_some()
}
