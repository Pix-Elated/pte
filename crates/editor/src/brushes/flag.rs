//! Flag brush — paints zone flags (PZ, PvP, no-logout, etc.) onto tiles.

use pte_otbm::{MapData, TileFlags};
use crate::state::UndoAction;
use super::{Brush, BrushId, BrushStroke, BrushType};

/// Zone flag types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ZoneFlag {
    ProtectionZone,
    NoPvp,
    PvpZone,
    NoLogout,
}

impl ZoneFlag {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProtectionZone => "Protection Zone",
            Self::NoPvp => "No-PvP Zone",
            Self::PvpZone => "PvP Zone",
            Self::NoLogout => "No-Logout Zone",
        }
    }
}

/// Flag brush paints zone attributes onto tiles.
#[allow(dead_code)]
pub struct FlagBrush {
    pub brush_id: BrushId,
    pub flag: ZoneFlag,
}

#[allow(dead_code)]
impl FlagBrush {
    pub fn new(id: BrushId, flag: ZoneFlag) -> Self {
        Self { brush_id: id, flag }
    }

    fn apply_flag(&self, flags: &mut TileFlags) {
        match self.flag {
            ZoneFlag::ProtectionZone => flags.protection_zone = true,
            ZoneFlag::NoPvp => flags.no_pvp = true,
            ZoneFlag::PvpZone => flags.pvp_zone = true,
            ZoneFlag::NoLogout => flags.no_logout = true,
        }
    }

    fn remove_flag(&self, flags: &mut TileFlags) {
        match self.flag {
            ZoneFlag::ProtectionZone => flags.protection_zone = false,
            ZoneFlag::NoPvp => flags.no_pvp = false,
            ZoneFlag::PvpZone => flags.pvp_zone = false,
            ZoneFlag::NoLogout => flags.no_logout = false,
        }
    }
}

impl Brush for FlagBrush {
    fn id(&self) -> BrushId { self.brush_id }
    fn name(&self) -> &str { self.flag.label() }
    fn brush_type(&self) -> BrushType { BrushType::Flag }
    fn look_id(&self) -> u16 { 0 }

    fn draw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            let tile = map.get_tile_mut(x, y, z);
            self.apply_flag(&mut tile.flags);
            tiles_after.push((x, y, z, Some(tile.clone())));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }

    fn undraw(&self, map: &mut MapData, positions: &[(u16, u16, u8)]) -> BrushStroke {
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();

        for &(x, y, z) in positions {
            let before = map.get_tile(x, y, z).cloned();
            tiles_before.push((x, y, z, before));

            let tile = map.get_tile_mut(x, y, z);
            self.remove_flag(&mut tile.flags);
            tiles_after.push((x, y, z, Some(tile.clone())));
        }

        BrushStroke {
            undo: UndoAction { tiles_before, tiles_after },
            dirty_positions: vec![],
        }
    }
}
