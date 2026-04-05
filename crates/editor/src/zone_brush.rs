//! Zone flag brush — paint/erase Protection Zone, No-PvP, PvP Zone, No-Logout flags on tiles.

use crate::state::{EditorState, UndoAction};

/// Which zone flag to paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Self::NoPvp => "No-PvP",
            Self::PvpZone => "PvP Zone",
            Self::NoLogout => "No-Logout",
        }
    }

    #[allow(dead_code)]
    pub fn short_label(self) -> &'static str {
        match self {
            Self::ProtectionZone => "PZ",
            Self::NoPvp => "NoPvP",
            Self::PvpZone => "PvP",
            Self::NoLogout => "NoLog",
        }
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            Self::ProtectionZone => egui::Color32::from_rgba_premultiplied(80, 160, 255, 60),
            Self::NoPvp => egui::Color32::from_rgba_premultiplied(80, 220, 120, 60),
            Self::PvpZone => egui::Color32::from_rgba_premultiplied(255, 80, 80, 60),
            Self::NoLogout => egui::Color32::from_rgba_premultiplied(220, 180, 60, 60),
        }
    }

    pub const ALL: &'static [ZoneFlag] = &[
        Self::ProtectionZone,
        Self::NoPvp,
        Self::PvpZone,
        Self::NoLogout,
    ];
}

/// Apply a zone flag to a tile at (x, y, z). If `erase` is true, remove the flag instead.
pub fn apply_zone(state: &mut EditorState, x: u16, y: u16, z: u8, flag: ZoneFlag, erase: bool) {
    let Some(ref mut map) = state.map_data else {
        return;
    };

    // Save tile before for undo
    let tile_before = map.get_tile(x, y, z).cloned();

    let tile = map.get_tile_mut(x, y, z);
    match flag {
        ZoneFlag::ProtectionZone => tile.flags.protection_zone = !erase,
        ZoneFlag::NoPvp => tile.flags.no_pvp = !erase,
        ZoneFlag::PvpZone => tile.flags.pvp_zone = !erase,
        ZoneFlag::NoLogout => tile.flags.no_logout = !erase,
    }

    let tile_after = map.get_tile(x, y, z).cloned();

    let action = UndoAction {
        tiles_before: vec![(x, y, z, tile_before)],
        tiles_after: vec![(x, y, z, tile_after)],
    };
    state.stroke_add(action);
}

/// Check whether a tile has a specific zone flag set.
#[allow(dead_code)]
pub fn has_zone_flag(state: &EditorState, x: u16, y: u16, z: u8, flag: ZoneFlag) -> bool {
    let Some(ref map) = state.map_data else {
        return false;
    };
    let Some(tile) = map.get_tile(x, y, z) else {
        return false;
    };
    match flag {
        ZoneFlag::ProtectionZone => tile.flags.protection_zone,
        ZoneFlag::NoPvp => tile.flags.no_pvp,
        ZoneFlag::PvpZone => tile.flags.pvp_zone,
        ZoneFlag::NoLogout => tile.flags.no_logout,
    }
}
