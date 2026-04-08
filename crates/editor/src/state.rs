//! Editor state — global application data shared across all UI panels.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Z-level constants matching Canary `map_const.hpp`.
/// Standard Tibia: z=0 is highest sky floor, z=7 is ground, z=8-15 underground.
pub const MAP_MAX_Z: u8 = 15;
pub const MAP_SURFACE_Z: u8 = 7;

use pte_appearances::LoadedAppearances;
use pte_assets::{ParsedCatalog, SpriteSheet};
use pte_otbm::{MapData, Tile};

use crate::brushes::registry::BrushRegistry;
use crate::brushes::shape::BrushShape;
use crate::brushes::BrushId;

/// Which mode the editor is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    /// Welcome/launcher — pick what to do.
    Welcome,
    /// Map editor workspace (default after loading).
    MapEditor,
}

/// Active workspace tab (shown after assets are loaded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTab {
    MapEditor,
    SpriteViewer,
}

/// Data sent from the background loading thread.
pub struct LoadedAssets {
    pub catalog: ParsedCatalog,
    pub appearances: Option<LoadedAppearances>,
    pub sheets: HashMap<String, SpriteSheet>,
    pub asset_dir: PathBuf,
    /// Map loaded on the background thread (if a map path was provided).
    pub map: Option<pte_otbm::MapData>,
    /// Path of the loaded map.
    pub map_path: Option<PathBuf>,
    /// Spawns loaded from spawn.xml alongside the map.
    pub spawns: Vec<crate::spawn_xml::Spawn>,
    /// Chunk directory path (if loaded from chunks).
    pub chunk_dir: Option<PathBuf>,
    /// Chunk manifest (if loaded from chunks) — for on-demand loading.
    pub chunk_manifest_data: Option<pte_otbm::chunk_io::ChunkManifest>,
    /// Lazy sprite sheet loader (loads sheets on-demand instead of all at startup).
    pub lazy_loader: Option<pte_assets::LazySheetLoader>,
}

/// Shared loading progress updated by the background thread.
pub struct LoadingProgress {
    pub progress: f32,
    pub message: String,
    pub stage: LoadingStage,
}

/// Granular loading stages for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingStage {
    Catalog,
    Appearances,
    SpriteSheets,
    MapParse,
    Overlays,
    Spawns,
    Finalizing,
}

/// State for the background loader.
#[allow(clippy::type_complexity)]
pub struct BackgroundLoader {
    /// Shared progress updated by the loading thread.
    pub progress: Option<std::sync::Arc<std::sync::Mutex<LoadingProgress>>>,
    /// Channel to receive loaded assets from the background thread.
    pub asset_rx: Option<std::sync::mpsc::Receiver<Result<LoadedAssets, String>>>,
    /// Channel for standalone map loading (when assets already loaded).
    pub map_rx: Option<
        std::sync::mpsc::Receiver<
            Result<
                (
                    pte_otbm::MapData,
                    std::path::PathBuf,
                    Vec<crate::spawn_xml::Spawn>,
                ),
                String,
            >,
        >,
    >,
    /// Shared progress for standalone map loading.
    pub map_progress: Option<std::sync::Arc<std::sync::Mutex<LoadingProgress>>>,
}

/// Which tool is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    Brush,
    Eraser,
    Fill,
    Select,
    Eyedropper,
    Door,
    Creature,
    Spawn,
    Waypoint,
}

/// Eraser operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraserMode {
    /// Remove the topmost item only (default click).
    TopItem,
    /// Show a picker dialog listing all items on the tile for selective removal.
    Selective,
    /// Remove the entire tile (ground + all items).
    FullTile,
}

/// State for the selective eraser dialog.
#[derive(Debug, Clone, Default)]
pub struct SelectiveEraserState {
    pub open: bool,
    pub tile_x: u16,
    pub tile_y: u16,
    pub tile_z: u8,
    /// (item_id, description) for each item on the tile, bottom-to-top.
    pub items: Vec<(u32, String)>,
    /// Whether the tile has a ground to show.
    pub has_ground: bool,
    pub ground_id: u32,
}

impl ToolType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Select => "Select",
            Self::Eyedropper => "Eyedropper",
            Self::Door => "Door",
            Self::Creature => "Creature",
            Self::Spawn => "Spawn",
            Self::Waypoint => "Waypoint",
        }
    }

    pub fn hotkey(self) -> &'static str {
        match self {
            Self::Brush => "B",
            Self::Eraser => "E",
            Self::Fill => "G",
            Self::Select => "S",
            Self::Eyedropper => "I",
            Self::Door => "D",
            Self::Creature => "C",
            Self::Spawn => "N",
            Self::Waypoint => "W",
        }
    }
}

/// Asset loading state.
#[derive(Debug)]
pub enum AssetStatus {
    NotLoaded,
    Loading { progress: f32, message: String },
    Ready,
    Error(String),
}

/// Category filter for sprite picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryFilter {
    Object,
    Outfit,
    Effect,
    Missile,
}

impl CategoryFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::Object => "Objects",
            Self::Outfit => "Outfits",
            Self::Effect => "Effects",
            Self::Missile => "Missiles",
        }
    }
}

/// Subcategory filter for objects — derived from appearance flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSubcategory {
    All,
    Ground,     // bank flag set
    Borders,    // bottom flag (rendered below objects)
    Walls,      // unpass + not bank (blocking, non-ground)
    Containers, // container flag
    Equipment,  // clothes flag
    Decoration, // market.category == DECORATION, or deco_kit, or lying_object
    Pickupable, // take flag
    Stackable,  // cumulative flag
    Hangable,   // hang flag
    Liquid,     // liquidpool flag
    Usable,     // usable or multiuse
    Weapons,    // market category in weapon types
    Light,      // light flag set
    Writable,   // write or write_once
    Corpse,     // corpse or player_corpse
    Other,      // anything not matching above
}

impl ObjectSubcategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Ground => "Ground",
            Self::Borders => "Borders",
            Self::Walls => "Walls",
            Self::Containers => "Containers",
            Self::Equipment => "Equipment",
            Self::Decoration => "Decoration",
            Self::Pickupable => "Pickupable",
            Self::Stackable => "Stackable",
            Self::Hangable => "Hangable",
            Self::Liquid => "Liquid",
            Self::Usable => "Usable",
            Self::Weapons => "Weapons",
            Self::Light => "Light",
            Self::Writable => "Writable",
            Self::Corpse => "Corpse",
            Self::Other => "Other",
        }
    }

    pub const ALL_SUBCATEGORIES: &'static [ObjectSubcategory] = &[
        Self::All,
        Self::Ground,
        Self::Borders,
        Self::Walls,
        Self::Containers,
        Self::Equipment,
        Self::Decoration,
        Self::Pickupable,
        Self::Stackable,
        Self::Hangable,
        Self::Liquid,
        Self::Usable,
        Self::Weapons,
        Self::Light,
        Self::Writable,
        Self::Corpse,
        Self::Other,
    ];
}

/// Selection rectangle (in world tiles).
#[derive(Debug, Clone, Copy)]
pub struct TileSelection {
    pub x1: u16,
    pub y1: u16,
    pub x2: u16,
    pub y2: u16,
}

/// Clipboard buffer – stores copied/cut tiles relative to the selection origin.
#[derive(Debug, Clone)]
pub struct TileClipboard {
    /// Width of the copied region.
    pub width: u16,
    /// Height of the copied region.
    pub height: u16,
    /// The z-level the copy was made from (for relative z offset).
    pub source_z: u8,
    /// Tiles stored as (dx, dy, tile) relative to the top-left of the selection.
    /// The tile's original z is preserved for multi-floor pastes.
    pub tiles: Vec<(u16, u16, Tile)>,
}

/// Undo action.
#[derive(Debug, Clone)]
pub struct UndoAction {
    pub tiles_before: Vec<(u16, u16, u8, Option<Tile>)>,
    pub tiles_after: Vec<(u16, u16, u8, Option<Tile>)>,
}

impl UndoAction {
    pub fn is_empty(&self) -> bool {
        self.tiles_before.is_empty()
    }

    /// Merge another action into this one. For tiles already present,
    /// keep the original "before" but update the "after" to the latest.
    pub fn merge(&mut self, other: UndoAction) {
        for (x, y, z, after) in other.tiles_after {
            // Update the 'after' for this position if we already have it
            if let Some(existing) = self
                .tiles_after
                .iter_mut()
                .find(|(ex, ey, ez, _)| *ex == x && *ey == y && *ez == z)
            {
                existing.3 = after;
            } else {
                // New position — find its 'before' from the other action
                let before = other
                    .tiles_before
                    .iter()
                    .find(|(bx, by, bz, _)| *bx == x && *by == y && *bz == z)
                    .cloned();
                if let Some(before) = before {
                    self.tiles_before.push(before);
                }
                self.tiles_after.push((x, y, z, after));
            }
        }
    }
}

/// Active stroke state — accumulates changes during a single mouse drag.
#[derive(Debug, Default)]
pub struct StrokeState {
    /// Whether a drag stroke is currently in progress.
    pub active: bool,
    /// Accumulated undo action for the entire stroke.
    pub undo: Option<UndoAction>,
    /// Tiles that have already been touched this stroke (prevents double-erasing).
    pub touched_tiles: HashSet<(u16, u16, u8)>,
}

/// Camera state for the viewport.
#[derive(Debug, Clone)]
pub struct Camera {
    pub center_x: f64,
    pub center_y: f64,
    pub z_level: u8,
    pub zoom: f32,
    /// Target zoom for smooth interpolation. When set, `zoom` animates toward this.
    pub zoom_target: f32,
}

/// Pre-defined zoom steps on a logarithmic scale.
/// Each step is roughly 1.5× apart, giving a smooth feel at all scales.
/// Ranges from 0.03% (whole 65k map) to 800% (pixel editing).
const ZOOM_LEVELS: &[f32] = &[
    0.0003, // 65536-tile overview
    0.0006, 0.001, 0.002, 0.004, 0.007, 0.012, 0.02, 0.035, 0.06, 0.1, 0.15, 0.25, 0.375, 0.5,
    0.67, 0.85, 1.0, // 1:1 (32 px/tile)
    1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, // max zoom
];

impl Default for Camera {
    fn default() -> Self {
        Self {
            center_x: 500.0,
            center_y: 500.0,
            z_level: MAP_SURFACE_Z,
            zoom: 1.0,
            zoom_target: 1.0,
        }
    }
}

impl Camera {
    pub fn clamp_zoom(&mut self) {
        let min = ZOOM_LEVELS[0];
        let max = *ZOOM_LEVELS.last().unwrap();
        self.zoom = self.zoom.clamp(min, max);
    }

    pub fn tile_size(&self) -> f32 {
        32.0 * self.zoom
    }

    /// Snap to the nearest zoom level from the predefined table.
    #[allow(dead_code)]
    pub fn snap_zoom(&mut self) {
        let log_z = self.zoom.ln();
        let mut best = ZOOM_LEVELS[0];
        let mut best_dist = (ZOOM_LEVELS[0].ln() - log_z).abs();
        for &level in ZOOM_LEVELS {
            let dist = (level.ln() - log_z).abs();
            if dist < best_dist {
                best_dist = dist;
                best = level;
            }
        }
        self.zoom = best;
    }

    /// Step zoom in (towards higher magnification).
    pub fn zoom_in(&mut self) {
        let cur = self.zoom_target;
        for &level in ZOOM_LEVELS.iter() {
            if level > cur * 1.01 {
                self.zoom_target = level;
                return;
            }
        }
        self.zoom_target = *ZOOM_LEVELS.last().unwrap();
    }

    /// Step zoom out (towards lower magnification).
    pub fn zoom_out(&mut self) {
        let cur = self.zoom_target;
        for &level in ZOOM_LEVELS.iter().rev() {
            if level < cur * 0.99 {
                self.zoom_target = level;
                return;
            }
        }
        self.zoom_target = ZOOM_LEVELS[0];
    }

    /// Smooth zoom by a scroll delta. Positive = zoom in.
    /// Sets zoom_target for animated interpolation; actual zoom catches up each frame.
    /// `fine` = true when Ctrl is held: 1% steps instead of 10%.
    pub fn zoom_by_scroll_fine(&mut self, scroll_delta: f32, fine: bool) {
        // Normal: ~10% per mouse tick (24px * 0.004 ≈ 0.096 log-units ≈ 10%)
        // Fine:   ~1% per mouse tick  (24px * 0.0004 ≈ 0.0096 log-units ≈ 1%)
        let sensitivity = if fine { 0.0004 } else { 0.004 };
        let log_z = self.zoom_target.ln();
        let step = scroll_delta * sensitivity;
        let min = ZOOM_LEVELS[0];
        let max = *ZOOM_LEVELS.last().unwrap();
        self.zoom_target = (log_z + step).exp().clamp(min, max);
    }

    /// Interpolate zoom towards zoom_target. Call once per frame.
    /// Returns true if still animating (needs repaint).
    pub fn animate_zoom(&mut self, dt: f32) -> bool {
        let diff = (self.zoom_target.ln() - self.zoom.ln()).abs();
        if diff < 0.001 {
            self.zoom = self.zoom_target;
            return false;
        }
        // Exponential ease — 90% of remaining distance per 80ms
        let t = 1.0 - (-dt / 0.06).exp(); // ~60ms time constant
        let log_current = self.zoom.ln();
        let log_target = self.zoom_target.ln();
        self.zoom = (log_current + (log_target - log_current) * t).exp();
        self.clamp_zoom();
        true
    }
}

/// The complete editor state.
pub struct EditorState {
    // Mode
    pub mode: EditorMode,
    /// Active workspace tab (Map Editor vs Sprite Viewer).
    pub active_tab: WorkspaceTab,

    // Background loading
    pub loader: BackgroundLoader,

    // Assets
    pub asset_status: AssetStatus,
    pub asset_dir: Option<PathBuf>,
    pub catalog: Option<ParsedCatalog>,
    pub appearances: Option<LoadedAppearances>,
    pub sprite_sheets: HashMap<String, SpriteSheet>,
    /// Lazy sprite sheet loader — loads sheets on-demand instead of all at startup.
    pub lazy_sheets: Option<pte_assets::LazySheetLoader>,
    /// Visual metadata: item ID → visual category string (e.g. "ground/grass", "wall/stone").
    /// Loaded from visual_classifications.json alongside appearances.
    pub visual_metadata: HashMap<u32, String>,
    /// Sorted unique top-level visual categories for the filter dropdown.
    pub visual_category_groups: Vec<String>,
    /// Currently selected visual category filter (empty = show all).
    pub visual_category_filter: String,

    // Map
    pub map_data: Option<MapData>,
    /// Chunk cache for viewport-driven lazy loading (None for single-file maps).
    pub chunk_cache: Option<crate::chunk_cache::ChunkCache>,
    pub map_path: Option<PathBuf>,
    /// True while a standalone map load is in progress.
    pub map_loading: bool,
    pub map_loading_message: String,

    // Camera
    pub camera: Camera,

    // Tools
    pub active_tool: ToolType,
    pub selected_item_id: Option<u32>,
    /// Separate selection for the sprite viewer tab (doesn't affect map editor brush).
    pub viewer_selected_id: Option<u32>,
    pub brush_size: u32,
    pub brush_shape: BrushShape,

    // Tool-specific configuration
    pub door_variant: crate::brushes::door::DoorVariant,
    pub spawn_radius: u8,
    pub waypoint_name: String,

    // Brush system
    pub brush_registry: BrushRegistry,
    pub active_brush: Option<BrushId>,
    pub brush_palette_filter: Option<crate::brushes::BrushType>,

    // Sprite picker
    pub sprite_search: String,
    pub sprite_category: CategoryFilter,
    pub sprite_page: usize,
    pub sprite_preview_direction: usize,

    // Selection
    pub selection: Option<TileSelection>,
    pub select_start: Option<(u16, u16)>,

    // Clipboard
    pub clipboard: Option<TileClipboard>,

    // Undo/Redo
    pub undo_stack: Vec<UndoAction>,
    pub undo_cursor: usize,

    /// Active stroke — batches all changes from a single drag.
    pub stroke: StrokeState,

    // Hover info
    pub hover_tile: Option<(u16, u16)>,

    // Sprite textures cached for egui
    pub sprite_textures: HashMap<u32, egui::TextureHandle>,
    /// Generation counter for O(1) LRU texture eviction.
    pub texture_lru_gen: HashMap<u32, u64>,
    pub texture_lru_counter: u64,
    /// Maximum number of GPU textures to keep loaded.
    pub texture_cache_limit: usize,

    // Performance: debounce refresh_cache
    pub last_cache_refresh: std::time::Instant,

    /// Anchored position for right-click context menu.
    pub context_menu_pos: Option<egui::Pos2>,

    // Animation
    /// Time accumulator for animated sprites (seconds since start).
    pub anim_time: f64,
    /// Whether to play animations in the viewport.
    pub animate_sprites: bool,

    // Z-level range detected from the loaded map (or defaults)
    /// Minimum z displayed in the layers panel.
    pub z_min: u8,
    /// Maximum z displayed in the layers panel.
    pub z_max: u8,
    /// Detected surface z (largest cluster of tiles, or MAP_SURFACE_Z default).
    pub z_surface: u8,

    // Sprite editor
    pub sprite_editor: crate::sprite_editor::SpriteEditorState,

    // Auto-updater
    pub updater: crate::updater::UpdaterState,

    // Loading tasks
    pub pending_asset_load: Option<PathBuf>,
    pub pending_map_load: Option<PathBuf>,
    /// Custom overlay maps to merge after the main map loads
    pub pending_custom_maps: Vec<PathBuf>,

    // Dialogs
    pub show_goto_dialog: bool,
    pub goto_x: String,
    pub goto_y: String,
    pub goto_z: String,

    pub show_find_dialog: bool,
    pub find_item_id: String,
    pub replace_item_id: String,
    pub find_results: Vec<(u16, u16, u8)>,
    pub find_result_cursor: usize,

    pub show_stats_dialog: bool,
    pub stats_cache: Option<crate::map_stats::MapStatsCache>,

    // View toggles
    pub show_items: bool,
    pub show_ground: bool,
    pub show_creatures: bool,
    pub show_spawns: bool,
    pub show_zone_overlays: bool,
    pub show_minimap: bool,

    // Multi-z ghost rendering
    /// Show ghost transparency of floors above/below current z.
    pub show_ghost_floors: bool,

    // Object subcategory filter in sprite picker
    pub object_subcategory: ObjectSubcategory,

    // Paste preview (ghost position for paste stamp)
    pub paste_preview: bool,

    // Town editor
    pub show_town_editor: bool,
    pub town_new_name: String,

    // Waypoint palette
    pub show_waypoint_palette: bool,
    pub waypoint_filter: String,

    // Zone brush
    pub active_zone_flag: Option<crate::zone_brush::ZoneFlag>,

    // House system
    pub show_house_palette: bool,
    pub house_new_name: String,
    pub house_filter: String,
    pub active_house_id: Option<u32>,
    pub show_house_overlay: bool,

    // Item properties editor
    pub show_item_props: bool,

    // Navigation history
    pub nav_history: crate::nav_history::NavHistory,

    // Map cleanup
    pub show_cleanup_dialog: bool,
    pub cleanup_last_result: Option<String>,

    // Selection drag-move
    pub selection_drag_origin: Option<(u16, u16)>,
    pub selection_drag_offset: Option<(i32, i32)>,

    // Recent files
    pub recent_files: Vec<std::path::PathBuf>,

    // Autosave
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u32,
    pub last_autosave: std::time::Instant,

    // Asset scanner
    pub scanner: crate::asset_scanner::ScannerState,

    /// The active project (set when loaded from the scanner).
    pub active_project: Option<crate::asset_scanner::DiscoveredProject>,
    /// Show the map switcher panel.
    pub show_map_switcher: bool,

    // Map properties dialog
    pub show_map_props: bool,

    // Minimap export
    pub show_minimap_export: bool,
    pub minimap_export_z: u8,
    pub minimap_export_scale: u8,
    pub minimap_export_result: Option<String>,

    // New map dialog
    pub show_new_map_dialog: bool,
    pub new_map_w: u32,
    pub new_map_h: u32,
    pub new_map_desc: String,

    // New project wizard
    pub new_project_wizard: crate::new_project::NewProjectWizard,

    // View overlays
    pub show_grid: bool,
    pub show_client_box: bool,
    pub show_light_overlay: bool,
    pub show_shade: bool,
    pub show_tooltips: bool,

    // Item type highlights
    pub highlight_pickupable: bool,
    pub highlight_moveable: bool,
    pub highlight_blocking: bool,
    pub highlight_hooks: bool,

    // Hotkeys (10 slots)
    pub hotkeys: Vec<crate::hotkeys::HotkeySlot>,
    pub show_hotkey_editor: bool,

    // Tile stack dialog
    pub show_tile_stack: bool,

    // Preferences dialog
    pub show_preferences: bool,
    pub undo_limit: usize,

    // Shortcuts panel
    pub show_shortcuts: bool,

    // Performance monitor
    pub show_perf_monitor: bool,
    pub perf: crate::perf_monitor::PerfStats,

    // Eraser flags mode: only erase zone/house markers, not tile contents
    pub eraser_flags_only: bool,
    /// Active eraser mode (top-item, selective picker, or full-tile).
    pub eraser_mode: EraserMode,
    /// Selective eraser dialog state — populated when eraser_mode == Selective.
    pub selective_eraser: SelectiveEraserState,

    // Dirty tracking — save_version tracks the undo_cursor value at last save.
    // If undo_cursor != save_version, the map has unsaved changes.
    pub save_version: usize,

    // Close confirmation
    pub show_close_confirm: bool,
    /// When true, the user confirmed they want to close without saving.
    pub close_confirmed: bool,

    // Creature palette
    pub show_creature_palette: bool,
    pub creature_filter: String,
    pub creature_list: Vec<crate::creature_palette::CreatureEntry>,

    // Spawn data (loaded from spawn.xml alongside the map)
    pub spawns: Vec<crate::spawn_xml::Spawn>,

    // Find by name mode
    pub find_by_name: bool,
    pub find_name_query: String,

    // Find by property (action_id / unique_id filter)
    pub find_action_id: String,
    pub find_unique_id: String,
    /// When true, restrict find to current z-level only.
    pub find_current_z_only: bool,

    // Map import
    pub show_import_dialog: bool,
    pub import_offset_x: i32,
    pub import_offset_y: i32,

    // Deferred save actions (can't call save_map from within a menu)
    pub pending_quick_save: bool,
    pub pending_save_as: bool,

    // Background save state
    pub map_saving: bool,
    pub map_saving_message: String,
    pub save_result_rx: Option<std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>>,

    // Cached map statistics (avoid recomputing every frame)
    pub cached_tile_count: usize,
    pub cached_z_levels: Vec<u8>,
    /// Per-z-level tile counts, keyed by z.
    pub cached_z_tile_counts: HashMap<u8, usize>,
    pub cache_dirty: bool,

    // Pending selection nudge from arrow keys
    pub pending_selection_nudge: Option<(i32, i32)>,

    // Chunk-based map system
    pub chunk_dir: Option<String>,
    pub chunk_manifest: Option<pte_otbm::chunk_io::ChunkManifest>,
    pub pending_chunk_dir_load: Option<std::path::PathBuf>,

    // "Load Assets Only" flow — after assets load, switch to SpriteViewer tab
    pub pending_sprite_viewer_after_load: bool,
    /// True when loading assets without a map — hides map stages in loading overlay
    pub loading_assets_only: bool,

    // API-triggered asset loading (the API handler only has &mut EditorState, not &mut App)
    pub pending_api_asset_load: Option<std::path::PathBuf>,
}

impl EditorState {
    /// Get the effective selected item ID based on the active tab.
    /// Sprite Viewer uses its own selection; Map Editor uses the brush selection.
    pub fn effective_selected_id(&self) -> Option<u32> {
        match self.active_tab {
            WorkspaceTab::SpriteViewer => self.viewer_selected_id,
            WorkspaceTab::MapEditor => self.selected_item_id,
        }
    }

    pub fn new() -> Self {
        Self {
            mode: EditorMode::Welcome,
            active_tab: WorkspaceTab::MapEditor,
            loader: BackgroundLoader {
                progress: None,
                asset_rx: None,
                map_rx: None,
                map_progress: None,
            },
            asset_status: AssetStatus::NotLoaded,
            asset_dir: None,
            catalog: None,
            appearances: None,
            sprite_sheets: HashMap::new(),
            lazy_sheets: None,
            visual_metadata: HashMap::new(),
            visual_category_groups: Vec::new(),
            visual_category_filter: String::new(),
            map_data: None,
            chunk_cache: None,
            map_path: None,
            map_loading: false,
            map_loading_message: String::new(),
            camera: Camera::default(),
            active_tool: ToolType::Brush,
            selected_item_id: None,
            viewer_selected_id: None,
            brush_size: 1,
            brush_shape: BrushShape::Square,
            door_variant: crate::brushes::door::DoorVariant::Normal,
            spawn_radius: 5,
            waypoint_name: String::new(),
            brush_registry: BrushRegistry::new(),
            active_brush: None,
            brush_palette_filter: None,
            sprite_search: String::new(),
            sprite_category: CategoryFilter::Object,
            sprite_page: 0,
            sprite_preview_direction: 2, // south-facing (front view)
            selection: None,
            select_start: None,
            undo_stack: Vec::new(),
            undo_cursor: 0,
            stroke: StrokeState::default(),
            hover_tile: None,
            sprite_textures: HashMap::new(),
            texture_lru_gen: HashMap::new(),
            texture_lru_counter: 0,
            texture_cache_limit: 8192,
            last_cache_refresh: std::time::Instant::now(),
            context_menu_pos: None,
            anim_time: 0.0,
            animate_sprites: true,
            sprite_editor: Default::default(),
            updater: Default::default(),
            z_min: 0,
            z_max: MAP_MAX_Z,
            z_surface: MAP_SURFACE_Z,
            pending_asset_load: None,
            pending_map_load: None,
            pending_custom_maps: Vec::new(),
            clipboard: None,
            show_goto_dialog: false,
            goto_x: String::new(),
            goto_y: String::new(),
            goto_z: String::new(),
            show_find_dialog: false,
            find_item_id: String::new(),
            replace_item_id: String::new(),
            find_results: Vec::new(),
            find_result_cursor: 0,
            show_stats_dialog: false,
            stats_cache: None,
            show_items: true,
            show_ground: true,
            show_creatures: true,
            show_spawns: true,
            show_zone_overlays: true,
            show_minimap: false,
            show_ghost_floors: true,
            object_subcategory: ObjectSubcategory::All,
            paste_preview: false,
            show_town_editor: false,
            town_new_name: String::new(),
            show_waypoint_palette: false,
            waypoint_filter: String::new(),
            active_zone_flag: None,
            show_house_palette: false,
            house_new_name: String::new(),
            house_filter: String::new(),
            active_house_id: None,
            show_house_overlay: true,
            show_item_props: false,
            nav_history: Default::default(),
            show_cleanup_dialog: false,
            cleanup_last_result: None,
            selection_drag_origin: None,
            selection_drag_offset: None,
            recent_files: Vec::new(),
            autosave_enabled: true,
            autosave_interval_secs: 300, // 5 minutes
            last_autosave: std::time::Instant::now(),
            scanner: Default::default(),
            active_project: None,
            show_map_switcher: false,

            // New features
            show_map_props: false,
            show_minimap_export: false,
            minimap_export_z: MAP_SURFACE_Z,
            minimap_export_scale: 1,
            minimap_export_result: None,
            show_new_map_dialog: false,
            new_map_w: 1024,
            new_map_h: 1024,
            new_map_desc: String::new(),
            new_project_wizard: crate::new_project::NewProjectWizard::default(),
            show_grid: true,
            show_client_box: false,
            show_light_overlay: false,
            show_shade: false,
            show_tooltips: false,
            highlight_pickupable: false,
            highlight_moveable: false,
            highlight_blocking: false,
            highlight_hooks: false,
            hotkeys: (0..10)
                .map(|_| crate::hotkeys::HotkeySlot::default())
                .collect(),
            show_hotkey_editor: false,
            show_tile_stack: false,
            show_preferences: false,
            undo_limit: 100,
            show_shortcuts: false,
            show_perf_monitor: false,
            perf: Default::default(),
            eraser_flags_only: false,
            eraser_mode: EraserMode::TopItem,
            selective_eraser: SelectiveEraserState::default(),

            save_version: 0,
            show_close_confirm: false,
            close_confirmed: false,
            show_creature_palette: false,
            creature_filter: String::new(),
            creature_list: Vec::new(),
            spawns: Vec::new(),
            find_by_name: false,
            find_name_query: String::new(),
            find_action_id: String::new(),
            find_unique_id: String::new(),
            find_current_z_only: false,
            show_import_dialog: false,
            import_offset_x: 0,
            import_offset_y: 0,
            pending_quick_save: false,
            pending_save_as: false,
            map_saving: false,
            map_saving_message: String::new(),
            save_result_rx: None,
            cached_tile_count: 0,
            cached_z_levels: Vec::new(),
            cached_z_tile_counts: HashMap::new(),
            cache_dirty: true,
            pending_selection_nudge: None,
            chunk_dir: None,
            chunk_manifest: None,
            pending_chunk_dir_load: None,
            pending_sprite_viewer_after_load: false,
            loading_assets_only: false,
            pending_api_asset_load: None,
        }
    }

    /// Push an undo action.
    pub fn push_undo(&mut self, action: UndoAction) {
        if action.is_empty() {
            return;
        }
        // Truncate any redo actions
        self.undo_stack.truncate(self.undo_cursor);
        self.undo_stack.push(action);
        self.undo_cursor = self.undo_stack.len();

        // Limit undo stack
        let limit = self.undo_limit;
        if self.undo_stack.len() > limit {
            self.undo_stack.remove(0);
            self.undo_cursor = self.undo_stack.len();
        }

        self.cache_dirty = true;
    }

    /// Accumulate an undo action into the current stroke (batches during drag).
    pub fn stroke_add(&mut self, action: UndoAction) {
        if action.is_empty() {
            return;
        }
        self.stroke.active = true;
        match self.stroke.undo {
            Some(ref mut existing) => existing.merge(action),
            None => self.stroke.undo = Some(action),
        }
    }

    /// Check if a tile was already touched in this stroke (for eraser dedup).
    pub fn stroke_touched(&mut self, x: u16, y: u16, z: u8) -> bool {
        !self.stroke.touched_tiles.insert((x, y, z))
    }

    /// Finish the current stroke — push the accumulated undo action.
    pub fn stroke_commit(&mut self) {
        if let Some(action) = self.stroke.undo.take() {
            self.push_undo(action);
        }
        self.stroke.active = false;
        self.stroke.touched_tiles.clear();
    }

    /// Returns true if the map has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.map_data.is_some() && self.undo_cursor != self.save_version
    }

    /// Mark the current state as saved (resets dirty flag).
    pub fn mark_saved(&mut self) {
        self.save_version = self.undo_cursor;
    }

    /// Undo the last action.
    pub fn undo(&mut self) {
        if self.undo_cursor == 0 {
            return;
        }
        self.undo_cursor -= 1;
        let action = &self.undo_stack[self.undo_cursor];
        if let Some(ref mut map) = self.map_data {
            for (x, y, z, tile_opt) in &action.tiles_before {
                match tile_opt {
                    Some(tile) => map.set_tile(tile.clone()),
                    None => map.remove_tile(*x, *y, *z),
                }
            }
        }
        self.cache_dirty = true;
    }

    /// Redo the last undone action.
    pub fn redo(&mut self) {
        if self.undo_cursor >= self.undo_stack.len() {
            return;
        }
        let action = &self.undo_stack[self.undo_cursor];
        if let Some(ref mut map) = self.map_data {
            for (x, y, z, tile_opt) in &action.tiles_after {
                match tile_opt {
                    Some(tile) => map.set_tile(tile.clone()),
                    None => map.remove_tile(*x, *y, *z),
                }
            }
        }
        self.cache_dirty = true;
        self.undo_cursor += 1;
    }

    pub fn can_undo(&self) -> bool {
        self.undo_cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.undo_cursor < self.undo_stack.len()
    }

    pub fn assets_ready(&self) -> bool {
        matches!(self.asset_status, AssetStatus::Ready)
    }

    /// Evict least-recently-used textures when over the cache limit.
    /// Call periodically (e.g. once per frame or after bulk loads).
    pub fn evict_textures(&mut self) {
        if self.sprite_textures.len() <= self.texture_cache_limit { return; }
        let to_remove = self.sprite_textures.len() - self.texture_cache_limit;
        let mut entries: Vec<(u32, u64)> = self.texture_lru_gen.iter().map(|(&k, &v)| (k, v)).collect();
        entries.sort_by_key(|&(_, gen)| gen);
        for (id, _) in entries.into_iter().take(to_remove) {
            self.sprite_textures.remove(&id);
            self.texture_lru_gen.remove(&id);
        }
    }

    /// Recompute cached tile count, z-levels, and per-z counts from map_data.
    /// Only call when `cache_dirty` is true. Debounced to at most once per 200ms.
    pub fn refresh_cache(&mut self) {
        if !self.cache_dirty {
            return;
        }
        if self.last_cache_refresh.elapsed() < std::time::Duration::from_millis(200) {
            return;
        }
        if let Some(ref map) = self.map_data {
            self.cached_tile_count = map.tile_count();
            self.cached_z_levels = map.occupied_z_levels();
            self.cached_z_tile_counts.clear();
            for (key, chunk) in &map.chunks {
                *self.cached_z_tile_counts.entry(key.z).or_default() += chunk.len();
            }
        } else {
            self.cached_tile_count = 0;
            self.cached_z_levels.clear();
            self.cached_z_tile_counts.clear();
        }
        self.last_cache_refresh = std::time::Instant::now();
        self.cache_dirty = false;
    }

    /// Detect z-level range from the loaded map.
    /// Call this after loading or significantly modifying the map.
    pub fn update_z_range(&mut self) {
        let Some(ref map) = self.map_data else { return };

        if let Some((min_z, max_z)) = map.z_range() {
            self.z_min = min_z;
            self.z_max = max_z;

            // Detect surface level: the z-level with the most tiles is likely ground.
            // Count tiles per z-level from the chunk keys.
            let mut counts: HashMap<u8, usize> = HashMap::new();
            for (key, chunk) in &map.chunks {
                *counts.entry(key.z).or_default() += chunk.len();
            }
            // The z with the most tiles is our best guess at surface level.
            if let Some((&best_z, _)) = counts.iter().max_by_key(|(_, &count)| count) {
                self.z_surface = best_z;
            }

            // Clamp the camera z to the detected range
            self.camera.z_level = self.camera.z_level.clamp(self.z_min, self.z_max);
        }
    }
}
