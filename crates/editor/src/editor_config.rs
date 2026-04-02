//! Editor configuration — persists preferences and recent files to disk.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persisted editor configuration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    // General
    pub undo_limit: usize,
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u32,

    // View
    pub animate_sprites: bool,
    pub show_ghost_floors: bool,
    pub show_grid: bool,
    pub show_tooltips: bool,
    pub show_client_box: bool,
    pub show_light_overlay: bool,

    // Highlights
    pub highlight_pickupable: bool,
    pub highlight_moveable: bool,
    pub highlight_blocking: bool,
    pub highlight_hooks: bool,

    // Recent files (up to 10)
    pub recent_files: Vec<PathBuf>,

    // Last known paths (for auto-discover on startup)
    pub last_asset_dir: Option<PathBuf>,
    pub last_scan_root: Option<PathBuf>,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            undo_limit: 100,
            autosave_enabled: true,
            autosave_interval_secs: 300,
            animate_sprites: true,
            show_ghost_floors: true,
            show_grid: true,
            show_tooltips: false,
            show_client_box: false,
            show_light_overlay: false,
            highlight_pickupable: false,
            highlight_moveable: false,
            highlight_blocking: false,
            highlight_hooks: false,
            recent_files: Vec::new(),
            last_asset_dir: None,
            last_scan_root: None,
        }
    }
}

/// Return the path to the config file (next to the executable).
fn config_path() -> Option<PathBuf> {
    std::env::current_exe().ok().map(|p| p.with_file_name("editor_config.json"))
}

/// Load config from disk. Returns defaults if file doesn't exist or is invalid.
pub fn load() -> EditorConfig {
    let Some(path) = config_path() else { return EditorConfig::default() };
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => EditorConfig::default(),
    }
}

/// Save config to disk.
pub fn save(config: &EditorConfig) {
    let Some(path) = config_path() else { return };
    match serde_json::to_string_pretty(config) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                tracing::warn!("Failed to save config: {e}");
            }
        }
        Err(e) => tracing::warn!("Failed to serialize config: {e}"),
    }
}

/// Extract current preferences from EditorState into a config struct.
pub fn from_state(state: &crate::state::EditorState) -> EditorConfig {
    EditorConfig {
        undo_limit: state.undo_limit,
        autosave_enabled: state.autosave_enabled,
        autosave_interval_secs: state.autosave_interval_secs,
        animate_sprites: state.animate_sprites,
        show_ghost_floors: state.show_ghost_floors,
        show_grid: state.show_grid,
        show_tooltips: state.show_tooltips,
        show_client_box: state.show_client_box,
        show_light_overlay: state.show_light_overlay,
        highlight_pickupable: state.highlight_pickupable,
        highlight_moveable: state.highlight_moveable,
        highlight_blocking: state.highlight_blocking,
        highlight_hooks: state.highlight_hooks,
        recent_files: state.recent_files.clone(),
        last_asset_dir: state.asset_dir.clone(),
        last_scan_root: state.scanner.scan_root.clone(),
    }
}

/// Apply a loaded config to an EditorState.
pub fn apply_to_state(config: &EditorConfig, state: &mut crate::state::EditorState) {
    state.undo_limit = config.undo_limit;
    state.autosave_enabled = config.autosave_enabled;
    state.autosave_interval_secs = config.autosave_interval_secs;
    state.animate_sprites = config.animate_sprites;
    state.show_ghost_floors = config.show_ghost_floors;
    state.show_grid = config.show_grid;
    state.show_tooltips = config.show_tooltips;
    state.show_client_box = config.show_client_box;
    state.show_light_overlay = config.show_light_overlay;
    state.highlight_pickupable = config.highlight_pickupable;
    state.highlight_moveable = config.highlight_moveable;
    state.highlight_blocking = config.highlight_blocking;
    state.highlight_hooks = config.highlight_hooks;
    state.recent_files = config.recent_files.clone();
}
