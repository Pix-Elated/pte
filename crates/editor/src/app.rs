//! Main eframe application — assembles all panels into the editor layout.

use egui::{Color32, Stroke};

use crate::state::{AssetStatus, EditorMode, EditorState, ToolType};

pub struct MapEditorApp {
    pub state: EditorState,
    api_cmd_rx: Option<std::sync::mpsc::Receiver<crate::api_server::ApiCommand>>,
}

impl MapEditorApp {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        // Start the embedded API server on a background thread
        let api_cmd_rx = crate::api_server::start(crate::api_server::DEFAULT_PORT);

        let mut app = Self {
            state: EditorState::new(),
            api_cmd_rx: Some(api_cmd_rx),
        };

        // Load persisted preferences and recent files
        let config = crate::editor_config::load();
        crate::editor_config::apply_to_state(&config, &mut app.state);

        // Auto-discover: scan for project assets on startup
        // Priority: last known scan root → exe directory → working directory
        let scan_root = config.last_scan_root
            .as_ref()
            .filter(|p| p.is_dir())
            .cloned()
            .or_else(|| {
                // Walk up from exe to find a directory containing client/ or data/
                let exe = std::env::current_exe().ok()?;
                let mut dir = exe.parent()?;
                for _ in 0..6 {
                    if dir.join("client").is_dir() || dir.join("canary").is_dir() || dir.join("data").is_dir() {
                        return Some(dir.to_path_buf());
                    }
                    dir = dir.parent()?;
                }
                None
            })
            .or_else(|| std::env::current_dir().ok());

        if let Some(root) = scan_root {
            tracing::info!("Auto-discover: scanning {}", root.display());
            let result = crate::asset_scanner::scan_directory(&root, 8);
            if !result.is_empty() {
                tracing::info!("Auto-discover: found {} project(s)", result.projects.len());
                app.state.scanner.scan_root = Some(root);
                app.state.scanner.scan_result = Some(result);
            }
        }

        app
    }

    /// Process any pending async-like work (file loads triggered from UI).
    fn process_pending(&mut self, ctx: &egui::Context) {
        // Asset loading
        if let Some(dir) = self.state.pending_asset_load.take() {
            self.load_assets_from_dir(&dir, ctx);
        }

        // Map loading
        if let Some(path) = self.state.pending_map_load.take() {
            let custom_maps = std::mem::take(&mut self.state.pending_custom_maps);
            self.load_map(&path);
            // Merge custom overlay maps into the loaded main map
            if !custom_maps.is_empty() && self.state.map_data.is_some() {
                self.merge_overlay_maps(&custom_maps);
            }
        }

        // Save actions (deferred from menu/hotkey to avoid borrow issues)
        if self.state.pending_quick_save {
            self.state.pending_quick_save = false;
            self.quick_save();
        }
        if self.state.pending_save_as {
            self.state.pending_save_as = false;
            self.save_map_as();
        }

        // Selection nudge (arrow keys)
        if let Some((dx, dy)) = self.state.pending_selection_nudge.take() {
            self.nudge_selection(dx, dy);
        }

        // Update window title with filename + dirty indicator
        self.update_title(ctx);
    }

    fn update_title(&self, ctx: &egui::Context) {
        let title = match &self.state.map_path {
            Some(path) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("map.otbm");
                if self.state.is_dirty() {
                    format!("*{} — Pixelated's Tibia Editor", name)
                } else {
                    format!("{} — Pixelated's Tibia Editor", name)
                }
            }
            None => "Pixelated's Tibia Editor".to_string(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    fn load_assets_from_dir(&mut self, dir: &std::path::Path, ctx: &egui::Context) {
        self.state.asset_status = AssetStatus::Loading {
            progress: 0.0,
            message: "Parsing catalog...".to_string(),
        };

        let catalog_path = dir.join("catalog-content.json");
        let catalog = match pte_assets::load_catalog(&catalog_path) {
            Ok(c) => c,
            Err(e) => {
                self.state.asset_status = AssetStatus::Error(format!("Catalog: {e:#}"));
                return;
            }
        };

        // Load appearances if available
        if let Some(ref app_file) = catalog.appearances_file {
            let app_path = dir.join(app_file);
            match pte_appearances::load_appearances(&app_path) {
                Ok(apps) => {
                    tracing::info!("Loaded {} appearances", apps.total_count());
                    self.state.appearances = Some(apps);
                }
                Err(e) => {
                    tracing::warn!("Failed to load appearances: {e:#}");
                }
            }
        }

        // Load sprite sheets in parallel
        self.state.asset_status = AssetStatus::Loading {
            progress: 0.1,
            message: "Decompressing sprite sheets...".to_string(),
        };

        let sheets = pte_assets::load_all_sheets(dir, &catalog, |done, total| {
            tracing::debug!("Sheet {}/{}", done, total);
        });

        tracing::info!("Loaded {} sprite sheets", sheets.len());

        // Upload sprite textures to GPU
        self.state.asset_status = AssetStatus::Loading {
            progress: 0.8,
            message: "Uploading textures...".to_string(),
        };

        for sheet in sheets.values() {
            for sid in sheet.first_sprite_id..=sheet.last_sprite_id {
                if let Some(pixels) = sheet.get_sprite(sid) {
                    let (w, h) = sheet.sprite_dimensions();
                    let tex = crate::sprite_picker::upload_sprite_texture(ctx, sid, &pixels, w, h);
                    self.state.sprite_textures.insert(sid, tex);
                }
            }
        }

        self.state.sprite_sheets = sheets;
        self.state.catalog = Some(catalog);
        self.state.asset_dir = Some(dir.to_path_buf());
        self.state.asset_status = AssetStatus::Ready;
    }

    fn load_map(&mut self, path: &std::path::Path) {
        match pte_otbm::parse_otbm(path) {
            Ok(map) => {
                tracing::info!("Loaded map: {} tiles", map.tile_count());
                self.state.map_data = Some(map);
                self.state.map_path = Some(path.to_path_buf());

                // Reset dirty tracking for freshly loaded map
                self.state.save_version = 0;
                self.state.undo_stack.clear();
                self.state.undo_cursor = 0;

                // Load spawns from spawn.xml next to the map
                let spawn_file = if self.state.map_data.as_ref().map_or(true, |m| m.spawn_file.is_empty()) {
                    "spawn.xml".to_string()
                } else {
                    self.state.map_data.as_ref().unwrap().spawn_file.clone()
                };
                let spawn_path = path.with_file_name(&spawn_file);
                if spawn_path.exists() {
                    match crate::spawn_xml::read_spawns(&spawn_path) {
                        Ok(spawns) => {
                            tracing::info!("Loaded {} spawns from {}", spawns.len(), spawn_path.display());
                            self.state.spawns = spawns;
                        }
                        Err(e) => tracing::warn!("Failed to load spawns: {e:#}"),
                    }
                } else {
                    self.state.spawns.clear();
                }
                crate::creature_palette::rebuild_creature_list(&mut self.state);

                // Add to recent files (deduplicated, max 10)
                let pb = path.to_path_buf();
                self.state.recent_files.retain(|p| p != &pb);
                self.state.recent_files.insert(0, pb);
                self.state.recent_files.truncate(10);
                self.state.last_autosave = std::time::Instant::now();
                self.state.update_z_range();
                tracing::info!(
                    "Detected z range: {}–{}, surface: {}",
                    self.state.z_min,
                    self.state.z_max,
                    self.state.z_surface
                );
                // Center the camera on the map
                self.state.camera.z_level = self.state.z_surface;
                if let Some(ref map) = self.state.map_data {
                    if let Some((min_x, min_y, max_x, max_y)) = map.xy_extents(self.state.z_surface) {
                        let cx = (min_x as f64 + max_x as f64) / 2.0;
                        let cy = (min_y as f64 + max_y as f64) / 2.0;
                        self.state.camera.center_x = cx;
                        self.state.camera.center_y = cy;
                        tracing::info!(
                            "Centered camera on ({}, {}) — tile extent x={}..{} y={}..{}",
                            cx, cy, min_x, max_x, min_y, max_y
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to load map: {e:#}");
                // Show error somewhere visible
            }
        }
    }

    /// Merge overlay .otbm files (custom, quest, etc.) into the currently loaded map.
    /// Overlay tiles overwrite existing tiles at matching positions.
    fn merge_overlay_maps(&mut self, overlay_paths: &[std::path::PathBuf]) {
        let Some(ref mut map) = self.state.map_data else { return };

        let mut total_merged = 0usize;
        for path in overlay_paths {
            match pte_otbm::parse_otbm(path) {
                Ok(overlay) => {
                    let mut count = 0;
                    // Merge tiles: overlay overwrites existing
                    for chunk in overlay.chunks.values() {
                        for tile in chunk.values() {
                            map.set_tile(tile.clone());
                            count += 1;
                        }
                    }
                    // Merge towns (deduplicate by id)
                    for town in &overlay.towns {
                        if !map.towns.iter().any(|t| t.id == town.id) {
                            map.towns.push(town.clone());
                        }
                    }
                    // Merge waypoints (deduplicate by name)
                    for wp in &overlay.waypoints {
                        if !map.waypoints.iter().any(|w| w.name == wp.name) {
                            map.waypoints.push(wp.clone());
                        }
                    }
                    // Merge houses (deduplicate by id)
                    for house in &overlay.houses {
                        if !map.houses.iter().any(|h| h.id == house.id) {
                            map.houses.push(house.clone());
                        }
                    }
                    tracing::info!(
                        "Merged overlay {} — {} tiles",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                        count,
                    );
                    total_merged += count;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load overlay {}: {e:#}",
                        path.display(),
                    );
                }
            }
        }

        if total_merged > 0 {
            tracing::info!(
                "Merged {} overlay map(s), {} total tiles applied",
                overlay_paths.len(),
                total_merged,
            );
            self.state.update_z_range();
        }
    }

    fn autosave(&self) {
        let Some(ref map) = self.state.map_data else { return };
        let Some(ref original) = self.state.map_path else { return };

        // Write to a .autosave sibling file, not the original
        let mut autosave_path = original.clone();
        let ext = autosave_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("otbm")
            .to_string();
        autosave_path.set_extension(format!("{}.autosave", ext));

        match pte_otbm::serialize_otbm(map, &autosave_path) {
            Ok(()) => tracing::info!("Autosaved to {}", autosave_path.display()),
            Err(e) => tracing::warn!("Autosave failed: {e:#}"),
        }
    }

    fn save_map_as(&mut self) {
        let map = match &self.state.map_data {
            Some(m) => m,
            None => return,
        };

        let default_name = self.state.map_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("map.otbm");

        let mut dialog = rfd::FileDialog::new()
            .add_filter("OTBM Map", &["otbm"])
            .set_file_name(default_name);

        if let Some(dir) = self.state.map_path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }

        if let Some(path) = dialog.save_file() {
            match pte_otbm::serialize_otbm(map, &path) {
                Ok(()) => {
                    tracing::info!("Saved map to {}", path.display());
                    self.state.map_path = Some(path);
                    self.state.mark_saved();
                    self.save_spawns();
                }
                Err(e) => tracing::error!("Failed to save map: {e:#}"),
            }
        }
    }

    /// Quick save — writes to the current map_path without dialog.
    /// Falls back to save-as if no path is set.
    fn quick_save(&mut self) {
        let Some(ref path) = self.state.map_path else {
            self.save_map_as();
            return;
        };
        let path = path.clone();
        let Some(ref map) = self.state.map_data else { return };

        // Create .bak backup of the existing file before overwriting
        if path.exists() {
            let bak = path.with_extension("otbm.bak");
            if let Err(e) = std::fs::copy(&path, &bak) {
                tracing::warn!("Failed to create backup {}: {e:#}", bak.display());
            }
        }

        match pte_otbm::serialize_otbm(map, &path) {
            Ok(()) => {
                tracing::info!("Saved map to {}", path.display());
                self.state.mark_saved();
                self.save_spawns();
            }
            Err(e) => tracing::error!("Failed to save map: {e:#}"),
        }
    }

    /// Save spawns to spawn.xml next to the map file.
    fn save_spawns(&self) {
        if self.state.spawns.is_empty() { return; }
        if let Some(ref map_path) = self.state.map_path {
            let spawn_path = map_path.with_file_name(
                if self.state.map_data.as_ref().map_or(true, |m| m.spawn_file.is_empty()) {
                    "spawn.xml".to_string()
                } else {
                    self.state.map_data.as_ref().unwrap().spawn_file.clone()
                }
            );
            if let Err(e) = crate::spawn_xml::write_spawns(&self.state.spawns, &spawn_path) {
                tracing::error!("Failed to save spawns: {e:#}");
            }
        }
    }

    /// Move selected tiles by (dx, dy) — arrow key nudge.
    fn nudge_selection(&mut self, dx: i32, dy: i32) {
        let Some(sel) = self.state.selection else { return };
        let Some(ref mut map) = self.state.map_data else { return };
        let z = self.state.camera.z_level;

        let mut undo_before = Vec::new();
        let mut undo_after = Vec::new();
        let mut tiles_to_move = Vec::new();

        // Collect tiles in selection
        for ty in sel.y1..=sel.y2 {
            for tx in sel.x1..=sel.x2 {
                if let Some(tile) = map.get_tile(tx, ty, z) {
                    tiles_to_move.push(tile.clone());
                }
            }
        }

        // Save originals and clear source tiles
        for ty in sel.y1..=sel.y2 {
            for tx in sel.x1..=sel.x2 {
                undo_before.push((tx, ty, z, map.get_tile(tx, ty, z).cloned()));
                map.remove_tile(tx, ty, z);
                undo_after.push((tx, ty, z, None));
            }
        }

        // Place tiles at new positions
        for tile in tiles_to_move {
            let nx = (tile.x as i32 + dx).max(0) as u16;
            let ny = (tile.y as i32 + dy).max(0) as u16;
            undo_before.push((nx, ny, z, map.get_tile(nx, ny, z).cloned()));
            let mut new_tile = tile;
            new_tile.x = nx;
            new_tile.y = ny;
            map.set_tile(new_tile);
            undo_after.push((nx, ny, z, map.get_tile(nx, ny, z).cloned()));
        }

        if !undo_before.is_empty() {
            self.state.push_undo(crate::state::UndoAction {
                tiles_before: undo_before,
                tiles_after: undo_after,
            });
        }

        // Move the selection rectangle
        if let Some(ref mut sel) = self.state.selection {
            sel.x1 = (sel.x1 as i32 + dx).max(0) as u16;
            sel.y1 = (sel.y1 as i32 + dy).max(0) as u16;
            sel.x2 = (sel.x2 as i32 + dx).max(0) as u16;
            sel.y2 = (sel.y2 as i32 + dy).max(0) as u16;
        }
    }

    fn load_materials(&mut self, dir: &std::path::Path) {
        match crate::brushes::xml_loader::load_materials_dir(dir, &mut self.state.brush_registry) {
            Ok(count) => {
                tracing::info!("Loaded {} brushes from materials", count);
            }
            Err(e) => {
                tracing::error!("Failed to load materials: {e:#}");
            }
        }
    }

    fn show_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open Assets Folder...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Select Asset Folder")
                        .pick_folder()
                    {
                        self.state.pending_asset_load = Some(dir);
                    }
                    ui.close_menu();
                }

                if ui.button("🔍 Scan Project…").clicked() {
                    self.state.scanner.open = true;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("New Map...").clicked() {
                    self.state.show_new_map_dialog = true;
                    ui.close_menu();
                }

                if ui.button("Open Map...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("OTBM Map", &["otbm"])
                        .pick_file()
                    {
                        self.state.pending_map_load = Some(path);
                    }
                    ui.close_menu();
                }

                // Recent files
                if !self.state.recent_files.is_empty() {
                    ui.menu_button("Recent Maps", |ui| {
                        let mut load_path = None;
                        for path in &self.state.recent_files {
                            let label = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("???");
                            if ui.button(label)
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                load_path = Some(path.clone());
                                ui.close_menu();
                            }
                        }
                        if let Some(p) = load_path {
                            self.state.pending_map_load = Some(p);
                        }
                    });
                }

                let save_enabled = self.state.map_data.is_some();
                if ui
                    .add_enabled(save_enabled, egui::Button::new("Save  (Ctrl+S)"))
                    .clicked()
                {
                    // Deferred — handled after menu closes
                    self.state.pending_quick_save = true;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(save_enabled, egui::Button::new("Save As...  (Ctrl+Shift+S)"))
                    .clicked()
                {
                    self.state.pending_save_as = true;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Load Materials XML...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.load_materials(&dir);
                    }
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Back to Launcher").clicked() {
                    self.state.mode = EditorMode::Welcome;
                    ui.close_menu();
                }

                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(self.state.can_undo(), egui::Button::new("Undo  (Ctrl+Z)"))
                    .clicked()
                {
                    self.state.undo();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(self.state.can_redo(), egui::Button::new("Redo  (Ctrl+Y)"))
                    .clicked()
                {
                    self.state.redo();
                    ui.close_menu();
                }

                ui.separator();

                let has_sel = crate::clipboard::has_selection(&self.state);
                let has_clip = crate::clipboard::has_clipboard(&self.state);

                if ui.add_enabled(has_sel, egui::Button::new("Cut  (Ctrl+X)")).clicked() {
                    crate::clipboard::cut_selection(&mut self.state);
                    ui.close_menu();
                }
                if ui.add_enabled(has_sel, egui::Button::new("Copy  (Ctrl+C)")).clicked() {
                    crate::clipboard::copy_selection(&mut self.state);
                    ui.close_menu();
                }
                if ui.add_enabled(has_clip, egui::Button::new("Paste  (Ctrl+V)")).clicked() {
                    self.state.paste_preview = true;
                    ui.close_menu();
                }
                if ui.add_enabled(has_sel, egui::Button::new("Delete  (Del)")).clicked() {
                    crate::clipboard::delete_selection(&mut self.state);
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Find / Replace...  (Ctrl+F)").clicked() {
                    self.state.show_find_dialog = true;
                    ui.close_menu();
                }
                if ui.button("Go To Position...  (Ctrl+G)").clicked() {
                    self.state.show_goto_dialog = true;
                    ui.close_menu();
                }

                ui.separator();

                // Selection operations
                if ui.add_enabled(has_sel, egui::Button::new("Randomize Selection")).clicked() {
                    crate::selection_ops::randomize_selection(&mut self.state);
                    ui.close_menu();
                }
                if ui.add_enabled(has_sel, egui::Button::new("Borderize Selection")).clicked() {
                    crate::selection_ops::borderize_selection(&mut self.state);
                    ui.close_menu();
                }

                ui.separator();

                // Eraser flags mode toggle
                let flags_label = if self.state.eraser_flags_only { "✓ Eraser: Flags Only" } else { "  Eraser: Flags Only" };
                if ui.button(flags_label).clicked() {
                    self.state.eraser_flags_only = !self.state.eraser_flags_only;
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("Zoom In").clicked() {
                    self.state.camera.zoom *= 1.5;
                    self.state.camera.clamp_zoom();
                    ui.close_menu();
                }
                if ui.button("Zoom Out").clicked() {
                    self.state.camera.zoom /= 1.5;
                    self.state.camera.clamp_zoom();
                    ui.close_menu();
                }
                if ui.button("Reset Zoom").clicked() {
                    self.state.camera.zoom = 1.0;
                    ui.close_menu();
                }

                ui.separator();

                // Layer visibility toggles
                ui.checkbox(&mut self.state.show_ground, "Show Ground");
                ui.checkbox(&mut self.state.show_items, "Show Items");
                ui.checkbox(&mut self.state.show_zone_overlays, "Show Zone Overlays");
                ui.checkbox(&mut self.state.show_creatures, "Show Creatures");
                ui.checkbox(&mut self.state.show_spawns, "Show Spawns");
                ui.checkbox(&mut self.state.show_ghost_floors, "Ghost Floors");

                ui.separator();

                let minimap_label = if self.state.show_minimap { "✓ Minimap" } else { "  Minimap" };
                if ui.button(minimap_label).clicked() {
                    self.state.show_minimap = !self.state.show_minimap;
                    ui.close_menu();
                }

                let anim_label = if self.state.animate_sprites { "⏸ Pause Animations" } else { "▶ Play Animations" };
                if ui.button(anim_label).clicked() {
                    self.state.animate_sprites = !self.state.animate_sprites;
                    ui.close_menu();
                }

                if ui.button("Map Statistics...").clicked() {
                    self.state.show_stats_dialog = true;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Town Editor...").clicked() {
                    self.state.show_town_editor = true;
                    ui.close_menu();
                }
                if ui.button("Waypoint Palette...").clicked() {
                    self.state.show_waypoint_palette = true;
                    ui.close_menu();
                }
                if ui.button("House Palette...").clicked() {
                    self.state.show_house_palette = true;
                    ui.close_menu();
                }
                if ui.button("Creature Palette...").clicked() {
                    self.state.show_creature_palette = true;
                    ui.close_menu();
                }
                if ui.button("Item Properties...").clicked() {
                    self.state.show_item_props = true;
                    ui.close_menu();
                }

                ui.separator();

                ui.checkbox(&mut self.state.show_house_overlay, "Show House Overlay");
                ui.checkbox(&mut self.state.show_grid, "Show Grid");
                ui.checkbox(&mut self.state.show_client_box, "Show Client Box");
                ui.checkbox(&mut self.state.show_light_overlay, "Light Sources");
                ui.checkbox(&mut self.state.show_shade, "Shade Non-Selected");
                ui.checkbox(&mut self.state.show_tooltips, "Item Tooltips");

                ui.separator();

                // Item type highlights sub-menu
                ui.menu_button("Highlight", |ui| {
                    ui.checkbox(&mut self.state.highlight_pickupable, "Pickupable");
                    ui.checkbox(&mut self.state.highlight_moveable, "Moveable");
                    ui.checkbox(&mut self.state.highlight_blocking, "Blocking");
                    ui.checkbox(&mut self.state.highlight_hooks, "Wall Hooks");
                });

                ui.separator();

                if ui.button("Tile Stack...").clicked() {
                    self.state.show_tile_stack = true;
                    ui.close_menu();
                }
                if ui.button("Hotkeys...").clicked() {
                    self.state.show_hotkey_editor = true;
                    ui.close_menu();
                }
                if ui.button("Preferences...").clicked() {
                    self.state.show_preferences = true;
                    ui.close_menu();
                }
                if ui.button("Keyboard Shortcuts  (?)").clicked() {
                    self.state.show_shortcuts = true;
                    ui.close_menu();
                }

                let perf_label = if self.state.show_perf_monitor { "✓ Performance Monitor" } else { "  Performance Monitor" };
                if ui.button(perf_label).clicked() {
                    self.state.show_perf_monitor = !self.state.show_perf_monitor;
                    ui.close_menu();
                }

                ui.separator();

                // Nav history
                let can_back = self.state.nav_history.can_go_back();
                let can_fwd = self.state.nav_history.can_go_forward();
                if ui.add_enabled(can_back, egui::Button::new("← Go Back")).clicked() {
                    crate::nav_history::go_back(&mut self.state);
                    ui.close_menu();
                }
                if ui.add_enabled(can_fwd, egui::Button::new("→ Go Forward")).clicked() {
                    crate::nav_history::go_forward(&mut self.state);
                    ui.close_menu();
                }
            });

            ui.menu_button("Map", |ui| {
                let has_map = self.state.map_data.is_some();

                if ui.add_enabled(has_map, egui::Button::new("Map Properties...")).clicked() {
                    self.state.show_map_props = true;
                    ui.close_menu();
                }
                if ui.add_enabled(has_map, egui::Button::new("Map Cleanup...")).clicked() {
                    self.state.show_cleanup_dialog = true;
                    ui.close_menu();
                }
                if ui.add_enabled(has_map, egui::Button::new("Export Minimap PNG...")).clicked() {
                    self.state.show_minimap_export = true;
                    ui.close_menu();
                }
                if ui.add_enabled(has_map, egui::Button::new("Import / Merge Map...")).clicked() {
                    self.state.show_import_dialog = true;
                    ui.close_menu();
                }

                ui.separator();

                // Zone flag brushes
                ui.label(egui::RichText::new("Zone Brush").size(10.0).color(crate::theme::TEXT_MUTED));
                for &flag in crate::zone_brush::ZoneFlag::ALL {
                    let active = self.state.active_zone_flag == Some(flag);
                    let label = if active {
                        format!("✓ {}", flag.label())
                    } else {
                        format!("  {}", flag.label())
                    };
                    if ui.add_enabled(has_map, egui::Button::new(label)).clicked() {
                        if active {
                            self.state.active_zone_flag = None;
                        } else {
                            self.state.active_zone_flag = Some(flag);
                            self.state.active_tool = ToolType::Brush;
                        }
                        ui.close_menu();
                    }
                }
                if self.state.active_zone_flag.is_some()
                    && ui.button("Clear Zone Brush").clicked()
                {
                    self.state.active_zone_flag = None;
                    ui.close_menu();
                }
            });
        });
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Ctrl+S quick save
            if i.modifiers.ctrl && i.key_pressed(egui::Key::S) {
                if i.modifiers.shift {
                    self.state.pending_save_as = true;
                } else {
                    self.state.pending_quick_save = true;
                }
            }

            // Undo/Redo
            if i.modifiers.ctrl && i.key_pressed(egui::Key::Z) {
                if i.modifiers.shift {
                    self.state.redo();
                } else {
                    self.state.undo();
                }
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::Y) {
                self.state.redo();
            }

            // Clipboard
            if i.modifiers.ctrl && i.key_pressed(egui::Key::C) {
                crate::clipboard::copy_selection(&mut self.state);
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::X) {
                crate::clipboard::cut_selection(&mut self.state);
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::V)
                && crate::clipboard::has_clipboard(&self.state) {
                    self.state.paste_preview = true;
                }
            if i.key_pressed(egui::Key::Delete) {
                crate::clipboard::delete_selection(&mut self.state);
            }

            // Dialogs
            if i.modifiers.ctrl && i.key_pressed(egui::Key::G) {
                self.state.show_goto_dialog = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::F) {
                self.state.show_find_dialog = true;
            }
            if i.key_pressed(egui::Key::M) && !i.modifiers.ctrl {
                self.state.show_minimap = !self.state.show_minimap;
            }
            if i.key_pressed(egui::Key::Escape) {
                // Cancel paste preview or deselect
                if self.state.paste_preview {
                    self.state.paste_preview = false;
                } else if self.state.selection.is_some() {
                    self.state.selection = None;
                }
            }

            // Tool hotkeys
            if i.key_pressed(egui::Key::B) { self.state.active_tool = ToolType::Brush; }
            if i.key_pressed(egui::Key::E) { self.state.active_tool = ToolType::Eraser; }
            if i.key_pressed(egui::Key::G) { self.state.active_tool = ToolType::Fill; }
            if i.key_pressed(egui::Key::S) && !i.modifiers.ctrl && !i.modifiers.shift { self.state.active_tool = ToolType::Select; }
            if i.key_pressed(egui::Key::I) { self.state.active_tool = ToolType::Eyedropper; }
            if i.key_pressed(egui::Key::D) { self.state.active_tool = ToolType::Door; }
            if i.key_pressed(egui::Key::C) { self.state.active_tool = ToolType::Creature; }
            if i.key_pressed(egui::Key::N) { self.state.active_tool = ToolType::Spawn; }
            if i.key_pressed(egui::Key::W) && !i.modifiers.ctrl { self.state.active_tool = ToolType::Waypoint; }

            // Z-level (clamped to detected map range)
            if i.key_pressed(egui::Key::PageUp) {
                self.state.camera.z_level = self.state.camera.z_level
                    .saturating_sub(1)
                    .max(self.state.z_min);
            }
            if i.key_pressed(egui::Key::PageDown) {
                self.state.camera.z_level = (self.state.camera.z_level + 1)
                    .min(self.state.z_max);
            }

            // Navigation history: Alt+Left / Alt+Right
            if i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft) {
                crate::nav_history::go_back(&mut self.state);
            }
            if i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight) {
                crate::nav_history::go_forward(&mut self.state);
            }

            // Arrow key selection nudge (when selection exists, no modifiers)
            if self.state.selection.is_some() && !i.modifiers.alt && !i.modifiers.ctrl {
                let mut dx = 0i32;
                let mut dy = 0i32;
                if i.key_pressed(egui::Key::ArrowLeft) { dx = -1; }
                if i.key_pressed(egui::Key::ArrowRight) { dx = 1; }
                if i.key_pressed(egui::Key::ArrowUp) { dy = -1; }
                if i.key_pressed(egui::Key::ArrowDown) { dy = 1; }

                if dx != 0 || dy != 0 {
                    self.state.pending_selection_nudge = Some((dx, dy));
                }
            }

            // Arrow key camera pan (when no selection, no modifiers)
            if self.state.selection.is_none() && !i.modifiers.alt && !i.modifiers.ctrl {
                let pan_speed = 4.0 / self.state.camera.zoom as f64;
                if i.key_pressed(egui::Key::ArrowLeft)  { self.state.camera.center_x -= pan_speed; }
                if i.key_pressed(egui::Key::ArrowRight) { self.state.camera.center_x += pan_speed; }
                if i.key_pressed(egui::Key::ArrowUp)    { self.state.camera.center_y -= pan_speed; }
                if i.key_pressed(egui::Key::ArrowDown)  { self.state.camera.center_y += pan_speed; }
            }

            // Keyboard zoom: + / - (also = for numpad-less keyboards)
            if !i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                    self.state.camera.zoom_in();
                }
                if i.key_pressed(egui::Key::Minus) {
                    self.state.camera.zoom_out();
                }
            }

            // Item properties: Ctrl+P
            if i.modifiers.ctrl && i.key_pressed(egui::Key::P) {
                self.state.show_item_props = !self.state.show_item_props;
            }

            // House palette: Ctrl+H
            if i.modifiers.ctrl && i.key_pressed(egui::Key::H) {
                self.state.show_house_palette = !self.state.show_house_palette;
            }

            // Shortcuts panel: Shift+/ (i.e. ?)
            if i.modifiers.shift && i.key_pressed(egui::Key::Slash) {
                self.state.show_shortcuts = !self.state.show_shortcuts;
            }

            // Select all: Ctrl+A
            if i.modifiers.ctrl && i.key_pressed(egui::Key::A) {
                if let Some(ref map) = self.state.map_data {
                    let z = self.state.camera.z_level;
                    if let Some((min_x, min_y, max_x, max_y)) = map.xy_extents(z) {
                        self.state.selection = Some(crate::state::TileSelection {
                            x1: min_x, y1: min_y, x2: max_x, y2: max_y,
                        });
                    }
                }
            }

            // Hotkey recall: F1-F10
            let fkeys = [
                egui::Key::F1, egui::Key::F2, egui::Key::F3, egui::Key::F4, egui::Key::F5,
                egui::Key::F6, egui::Key::F7, egui::Key::F8, egui::Key::F9, egui::Key::F10,
            ];
            for (idx, &fkey) in fkeys.iter().enumerate() {
                if i.key_pressed(fkey) {
                    if i.modifiers.shift {
                        crate::hotkeys::save_hotkey(&mut self.state, idx);
                    } else {
                        crate::hotkeys::recall_hotkey(&mut self.state, idx);
                    }
                }
            }
        });
    }
}

/// Section header label for sidebar panels.
fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.0)
            .color(crate::theme::TEXT_MUTED)
            .strong(),
    );
    ui.add_space(4.0);
}

/// Subtle horizontal separator line.
fn separator_line(ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right(), rect.top()),
        ],
        Stroke::new(0.5, crate::theme::BORDER),
    );
    ui.add_space(1.0);
}

/// A clickable card for the welcome screen mode selector.
fn mode_card(ui: &mut egui::Ui, title: &str, subtitle: &str, width: f32) -> bool {
    use crate::theme;
    let height = 72.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, height),
        egui::Sense::click(),
    );

    let hovered = response.hovered();
    let bg = if hovered { theme::BG_RAISED } else { theme::BG_SURFACE };
    let border = if hovered { theme::ACCENT_HOVER } else { theme::BORDER };

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        bg,
        Stroke::new(if hovered { 1.5 } else { 0.5 }, border),
        egui::StrokeKind::Outside,
    );

    let title_pos = rect.left_top() + egui::vec2(16.0, 20.0);
    ui.painter().text(
        title_pos,
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(15.0),
        if hovered { Color32::WHITE } else { theme::TEXT_PRIMARY },
    );

    let sub_pos = rect.left_top() + egui::vec2(16.0, 42.0);
    ui.painter().text(
        sub_pos,
        egui::Align2::LEFT_TOP,
        subtitle,
        egui::FontId::proportional(11.0),
        theme::TEXT_MUTED,
    );

    response.clicked()
}

impl eframe::App for MapEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_pending(ctx);

        // Handle close confirmation for unsaved changes
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.state.is_dirty() && !self.state.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.state.show_close_confirm = true;
            }
        }

        // Close confirmation dialog
        if self.state.show_close_confirm {
            let mut open = true;
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes. Save before closing?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save & Close").clicked() {
                            self.state.pending_quick_save = true;
                            self.state.close_confirmed = true;
                            open = false;
                        }
                        if ui.button("Discard & Close").clicked() {
                            self.state.close_confirmed = true;
                            open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            open = false;
                        }
                    });
                });
            if !open {
                self.state.show_close_confirm = false;
                if self.state.close_confirmed {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // Process API commands from the embedded HTTP server
        if let Some(ref rx) = self.api_cmd_rx {
            crate::api_handler::process_commands(&mut self.state, rx);
        }

        // Autosave check
        if self.state.autosave_enabled
            && self.state.map_data.is_some()
            && self.state.map_path.is_some()
        {
            let elapsed = self.state.last_autosave.elapsed();
            if elapsed.as_secs() >= self.state.autosave_interval_secs as u64 {
                self.autosave();
                self.state.last_autosave = std::time::Instant::now();
            }
        }

        // Show loading overlay if assets are loading
        if let AssetStatus::Loading { ref message, progress } = self.state.asset_status {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(crate::theme::BG_BASE))
                .show(ctx, |ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(message)
                                .size(14.0)
                                .color(crate::theme::TEXT_SECONDARY),
                        );
                        ui.add_space(8.0);
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .animate(true)
                                .desired_width(260.0),
                        );
                    });
                });
            return;
        }

        // Dispatch to the active mode
        match self.state.mode {
            EditorMode::Welcome => self.show_welcome(ctx),
            EditorMode::MapEditor => self.show_map_editor(ctx),
            EditorMode::SpriteViewer => self.show_sprite_viewer(ctx),
        }

        // Asset scanner dialog (can be opened from any mode)
        match crate::asset_scanner::show(ctx, &mut self.state.scanner) {
            crate::asset_scanner::ScannerAction::LoadProject { asset_dir, main_map, custom_maps } => {
                self.state.pending_asset_load = Some(asset_dir);
                self.state.pending_map_load = Some(main_map);
                self.state.pending_custom_maps = custom_maps;
            }
            crate::asset_scanner::ScannerAction::None => {}
        }
    }

    fn on_exit(&mut self) {
        // Persist preferences and recent files
        let config = crate::editor_config::from_state(&self.state);
        crate::editor_config::save(&config);
    }
}

// ---- Mode UIs ----

impl MapEditorApp {
    /// Welcome / launcher screen.
    fn show_welcome(&mut self, ctx: &egui::Context) {
        use crate::theme;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::BG_BASE))
            .show(ctx, |ui| {
                let avail = ui.available_size();

                // Vertical centering
                ui.add_space((avail.y * 0.15).max(30.0));

                ui.vertical_centered(|ui| {
                    // Title
                    ui.label(
                        egui::RichText::new("Pixelated's Tibia Editor")
                            .size(28.0)
                            .color(theme::TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("Map Editor  ·  Sprite Viewer")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );

                    ui.add_space(32.0);

                    // ── Primary action: Open Project ──
                    let btn = egui::Button::new(
                        egui::RichText::new("📂  Open Project Folder…")
                            .size(14.0)
                            .color(Color32::WHITE),
                    )
                    .fill(theme::ACCENT)
                    .stroke(Stroke::new(1.0, theme::ACCENT_HOVER))
                    .corner_radius(egui::CornerRadius::same(6))
                    .min_size(egui::vec2(240.0, 40.0));

                    if ui.add(btn).clicked() {
                        if let Some(dir) = rfd::FileDialog::new()
                            .set_title("Select OT Project Root")
                            .pick_folder()
                        {
                            let result = crate::asset_scanner::scan_directory(&dir, 8);
                            self.state.scanner.scan_root = Some(dir);
                            self.state.scanner.scan_result = Some(result);
                            self.state.scanner.expanded_project = None;
                            self.state.scanner.open = true;
                        }
                    }

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Scans for client assets, maps, and server configs")
                            .size(10.5)
                            .color(theme::TEXT_MUTED),
                    );

                    // ── Error display ──
                    if let AssetStatus::Error(ref msg) = self.state.asset_status {
                        ui.add_space(12.0);
                        ui.colored_label(theme::ERROR, msg);
                    }

                    // ── Recent files ──
                    if !self.state.recent_files.is_empty() {
                        ui.add_space(28.0);

                        // Subtle divider
                        let rect = ui.available_rect_before_wrap();
                        let center_x = rect.center().x;
                        ui.painter().line_segment(
                            [
                                egui::pos2(center_x - 120.0, rect.top()),
                                egui::pos2(center_x + 120.0, rect.top()),
                            ],
                            Stroke::new(0.5, theme::BORDER),
                        );
                        ui.add_space(16.0);

                        ui.label(
                            egui::RichText::new("Recent files")
                                .size(12.0)
                                .color(theme::TEXT_SECONDARY),
                        );
                        ui.add_space(8.0);

                        let mut clicked_path = None;
                        for path in &self.state.recent_files {
                            let display = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("?");
                            let parent = path.parent()
                                .and_then(|p| p.file_name())
                                .and_then(|n| n.to_str())
                                .unwrap_or("");
                            let label = if parent.is_empty() {
                                display.to_string()
                            } else {
                                format!("{}  ·  {}", display, parent)
                            };

                            if ui.add(
                                egui::Button::new(
                                    egui::RichText::new(&label)
                                        .size(11.0)
                                        .color(theme::TEXT_PRIMARY),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(egui::vec2(240.0, 22.0)),
                            ).clicked() {
                                clicked_path = Some(path.clone());
                            }
                        }

                        // Open a recent map directly (needs assets first)
                        if let Some(path) = clicked_path {
                            if self.state.assets_ready() {
                                self.state.pending_map_load = Some(path);
                            } else {
                                // Try to auto-discover assets from the map file's directory tree
                                if let Some(parent) = path.parent() {
                                    let result = crate::asset_scanner::scan_directory(parent, 6);
                                    if let Some(project) = result.projects.first() {
                                        self.state.pending_asset_load = Some(project.catalog_dir.clone());
                                    }
                                }
                                self.state.pending_map_load = Some(path);
                            }
                        }
                    }

                    // ── If assets are loaded, show mode selection ──
                    if self.state.assets_ready() {
                        ui.add_space(28.0);

                        let rect = ui.available_rect_before_wrap();
                        let center_x = rect.center().x;
                        ui.painter().line_segment(
                            [
                                egui::pos2(center_x - 120.0, rect.top()),
                                egui::pos2(center_x + 120.0, rect.top()),
                            ],
                            Stroke::new(0.5, theme::BORDER),
                        );
                        ui.add_space(16.0);

                        ui.label(
                            egui::RichText::new("Choose a workspace")
                                .size(13.0)
                                .color(theme::TEXT_SECONDARY),
                        );
                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            let card_w = 200.0;
                            let gap = 16.0;
                            let total = card_w * 2.0 + gap;
                            let avail_w = ui.available_width();
                            if avail_w > total {
                                ui.add_space((avail_w - total) / 2.0);
                            }

                            if mode_card(ui, "Map Editor", "Edit .otbm map files", card_w) {
                                self.state.mode = EditorMode::MapEditor;
                            }

                            ui.add_space(gap);

                            if mode_card(ui, "Sprite Viewer", "Browse and edit sprites", card_w) {
                                self.state.mode = EditorMode::SpriteViewer;
                            }
                        });
                    }
                });
            });
    }

    /// Full map editor workspace.
    fn show_map_editor(&mut self, ctx: &egui::Context) {
        self.handle_hotkeys(ctx);

        // Handle sprite editor overlay
        if crate::sprite_editor::show(ctx, &mut self.state.sprite_editor) {
            self.save_edited_sprite(ctx);
        }

        // Floating dialogs & overlays
        crate::goto_dialog::show(ctx, &mut self.state);
        crate::find_replace::show(ctx, &mut self.state);
        crate::map_stats::show(ctx, &mut self.state);
        crate::minimap::show(ctx, &mut self.state);
        crate::context_menu::show(ctx, &mut self.state);
        crate::item_properties::show(ctx, &mut self.state);
        crate::map_cleanup::show(ctx, &mut self.state);

        // New dialogs
        crate::map_properties::show(ctx, &mut self.state);
        crate::minimap_export::show_export_dialog(ctx, &mut self.state);
        crate::new_map::show(ctx, &mut self.state);
        crate::hotkeys::show(ctx, &mut self.state);
        crate::tile_stack::show(ctx, &mut self.state);
        crate::preferences::show(ctx, &mut self.state);
        crate::shortcuts_panel::show(ctx, &mut self.state);
        crate::perf_monitor::show_overlay(ctx, &self.state);
        crate::view_overlays::draw_tooltip(ctx, &self.state);
        crate::creature_palette::show(ctx, &mut self.state);
        crate::map_import::show(ctx, &mut self.state);

        // Town editor
        match crate::town_editor::show(ctx, &mut self.state) {
            crate::town_editor::TownAction::GoTo { x, y, z } => {
                crate::nav_history::record(&mut self.state);
                self.state.camera.center_x = x as f64;
                self.state.camera.center_y = y as f64;
                self.state.camera.z_level = z;
            }
            crate::town_editor::TownAction::None => {}
        }

        // Waypoint palette
        match crate::waypoint_palette::show(ctx, &mut self.state) {
            crate::waypoint_palette::WaypointAction::GoTo { x, y, z } => {
                crate::nav_history::record(&mut self.state);
                self.state.camera.center_x = x as f64;
                self.state.camera.center_y = y as f64;
                self.state.camera.z_level = z;
            }
            crate::waypoint_palette::WaypointAction::None => {}
        }

        // House palette
        match crate::house_palette::show(ctx, &mut self.state) {
            crate::house_palette::HouseAction::GoTo { x, y, z } => {
                crate::nav_history::record(&mut self.state);
                self.state.camera.center_x = x as f64;
                self.state.camera.center_y = y as f64;
                self.state.camera.z_level = z;
            }
            crate::house_palette::HouseAction::None => {}
        }

        let panel_frame = egui::Frame::NONE
            .fill(crate::theme::BG_PANEL)
            .inner_margin(egui::Margin::same(0));

        // Menu bar
        egui::TopBottomPanel::top("menu_bar")
            .frame(panel_frame)
            .show(ctx, |ui| {
                self.show_menu_bar(ui);
            });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar")
            .max_height(22.0)
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_BASE)
                .inner_margin(egui::Margin::symmetric(8, 2)))
            .show(ctx, |ui| {
                crate::status_bar::show(ui, &self.state);
            });

        // Toolbar
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_SURFACE)
                .inner_margin(egui::Margin::symmetric(6, 3)))
            .show(ctx, |ui| {
                let action = crate::toolbar::show(ui, &mut self.state);
                match action {
                    crate::toolbar::ToolbarAction::None => {}
                    crate::toolbar::ToolbarAction::Undo => { self.state.undo(); }
                    crate::toolbar::ToolbarAction::Redo => { self.state.redo(); }
                    crate::toolbar::ToolbarAction::ZoomIn => {
                        self.state.camera.zoom_in();
                    }
                    crate::toolbar::ToolbarAction::ZoomOut => {
                        self.state.camera.zoom_out();
                    }
                    crate::toolbar::ToolbarAction::ZoomReset => {
                        self.state.camera.zoom = 1.0;
                    }
                    crate::toolbar::ToolbarAction::FitToMap => {
                        if let Some(ref map) = self.state.map_data {
                            let z = self.state.camera.z_level;
                            if let Some((x1, y1, x2, y2)) = map.xy_extents(z) {
                                self.state.camera.center_x = (x1 as f64 + x2 as f64) / 2.0;
                                self.state.camera.center_y = (y1 as f64 + y2 as f64) / 2.0;
                                // Leave zoom as-is for now; a proper fit would size to viewport
                            }
                        }
                    }
                }
            });

        // Left sidebar: brushes + sprites
        egui::SidePanel::left("sprite_picker")
            .default_width(260.0)
            .min_width(200.0)
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_PANEL)
                .inner_margin(egui::Margin::same(6)))
            .show(ctx, |ui| {
                if self.state.brush_registry.count() > 0 {
                    section_header(ui, "Brushes");
                    crate::brush_palette::show(ui, &mut self.state);
                    ui.add_space(8.0);
                    separator_line(ui);
                    ui.add_space(4.0);
                }
                section_header(ui, "Sprites");
                crate::sprite_picker::show(ui, &mut self.state);
            });

        // Right sidebar: layers + properties + sprite details
        egui::SidePanel::right("right_panel")
            .default_width(220.0)
            .min_width(160.0)
            .max_width(320.0)
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_PANEL)
                .inner_margin(egui::Margin::same(6)))
            .show(ctx, |ui| {
                section_header(ui, "Layers");
                crate::layers::show(ui, &mut self.state);
                ui.add_space(8.0);
                separator_line(ui);
                ui.add_space(4.0);
                section_header(ui, "Properties");
                let prop_action = crate::properties::show(ui, &mut self.state);
                match prop_action {
                    crate::properties::PropertiesAction::DeleteItem { x, y, z, index } => {
                        if let Some(ref mut map) = self.state.map_data {
                            // Only modify if the tile already exists
                            let key = pte_otbm::ChunkKey {
                                cx: x as i32 / pte_otbm::CHUNK_SIZE,
                                cy: y as i32 / pte_otbm::CHUNK_SIZE,
                                z,
                            };
                            let local = ((x as i32 % pte_otbm::CHUNK_SIZE) as u8,
                                         (y as i32 % pte_otbm::CHUNK_SIZE) as u8);
                            if let Some(chunk) = map.chunks.get_mut(&key) {
                                if let Some(tile) = chunk.get_mut(&local) {
                                    if index == usize::MAX {
                                        tile.ground = None;
                                    } else if index < tile.items.len() {
                                        tile.items.remove(index);
                                    }
                                }
                            }
                        }
                    }
                    crate::properties::PropertiesAction::SelectItem { item_id } => {
                        self.state.selected_item_id = Some(item_id);
                    }
                    crate::properties::PropertiesAction::None => {}
                }

                // Show sprite detail panel when a sprite is selected
                if self.state.selected_item_id.is_some() {
                    ui.add_space(8.0);
                    separator_line(ui);
                    ui.add_space(4.0);
                    section_header(ui, "Sprite Details");
                    let action = crate::sprite_detail::show(ui, &mut self.state);
                    self.handle_detail_action(action, ctx);
                }
            });

        // Central panel: map viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(crate::theme::BG_BASE))
            .show(ctx, |ui| {
                crate::viewport::show(ui, &mut self.state);
            });
    }

    /// Standalone sprite viewer mode with CRUD and pixel editor.
    fn show_sprite_viewer(&mut self, ctx: &egui::Context) {
        // Handle sprite editor save
        if crate::sprite_editor::show(ctx, &mut self.state.sprite_editor) {
            self.save_edited_sprite(ctx);
        }

        // Menu bar
        egui::TopBottomPanel::top("sprite_menu_bar")
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_PANEL)
                .inner_margin(egui::Margin::same(0)))
            .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Change Assets Folder…").clicked() {
                        if let Some(dir) = rfd::FileDialog::new()
                            .set_title("Select Asset Folder")
                            .pick_folder()
                        {
                            self.state.pending_asset_load = Some(dir);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save All to Disk").clicked() {
                        self.save_all_sprite_sheets();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Back to Launcher").clicked() {
                        self.state.mode = EditorMode::Welcome;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Sprite", |ui| {
                    let has_selection = self.state.selected_item_id.is_some();
                    if ui.add_enabled(has_selection, egui::Button::new("Edit Selected")).clicked() {
                        self.open_sprite_editor(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Duplicate")).clicked() {
                        self.duplicate_sprite();
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Delete")).clicked() {
                        self.delete_sprite();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("New Blank Sprite").clicked() {
                        self.add_blank_sprite();
                        ui.close_menu();
                    }
                    if ui.button("Import from PNG…").clicked() {
                        self.import_sprite_from_png(ctx);
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Export to PNG…")).clicked() {
                        self.export_sprite_to_png();
                        ui.close_menu();
                    }
                });
            });
        });

        // Action bar
        egui::TopBottomPanel::top("sprite_actions")
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_SURFACE)
                .inner_margin(egui::Margin::symmetric(6, 3)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let has_selection = self.state.selected_item_id.is_some();

                let action_btn = |ui: &mut egui::Ui, label: &str, enabled: bool| -> bool {
                    let btn = egui::Button::new(
                        egui::RichText::new(label).size(11.0),
                    ).min_size(egui::vec2(0.0, 22.0));
                    ui.add_enabled(enabled, btn).clicked()
                };

                if action_btn(ui, "New", true) {
                    self.add_blank_sprite();
                }
                if action_btn(ui, "Edit", has_selection) {
                    self.open_sprite_editor(ctx);
                }
                if action_btn(ui, "Duplicate", has_selection) {
                    self.duplicate_sprite();
                }
                if action_btn(ui, "Delete", has_selection) {
                    self.delete_sprite();
                }

                ui.add_space(4.0);

                if action_btn(ui, "Import", true) {
                    self.import_sprite_from_png(ctx);
                }
                if action_btn(ui, "Export", has_selection) {
                    self.export_sprite_to_png();
                }

                ui.add_space(4.0);

                if action_btn(ui, "Save All", true) {
                    self.save_all_sprite_sheets();
                }

                // Selected info (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(id) = self.state.selected_item_id {
                        let mut label = format!("#{}", id);
                        if let Some(ref apps) = self.state.appearances {
                            if let Some(app) = apps.get(pte_appearances::Category::Object, id) {
                                if let Some(ref name) = app.name {
                                    label = format!("{} — #{}", name, id);
                                }
                            }
                        }
                        ui.label(
                            egui::RichText::new(label)
                                .size(11.0)
                                .color(crate::theme::TEXT_SECONDARY),
                        );
                    }
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("sprite_status")
            .max_height(22.0)
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_BASE)
                .inner_margin(egui::Margin::symmetric(8, 2)))
            .show(ctx, |ui| {
                let text = |s: &str| egui::RichText::new(s).size(10.5).color(crate::theme::TEXT_MUTED);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if let Some(ref apps) = self.state.appearances {
                        ui.label(text(&format!("{} appearances", apps.total_count())));
                    }
                    ui.label(text(&format!("{} textures", self.state.sprite_textures.len())));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(
                                egui::RichText::new("Back to Launcher").size(10.0)
                            ).min_size(egui::vec2(0.0, 18.0))
                        ).clicked() {
                            self.state.mode = EditorMode::Welcome;
                        }
                    });
                });
            });

        // Full-width sprite browser with detail panel
        egui::SidePanel::right("sprite_detail_panel")
            .default_width(240.0)
            .min_width(200.0)
            .max_width(360.0)
            .resizable(true)
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_PANEL)
                .inner_margin(egui::Margin::same(6)))
            .show(ctx, |ui| {
                section_header(ui, "Details");
                let action = crate::sprite_detail::show(ui, &mut self.state);
                self.handle_detail_action(action, ctx);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE
                .fill(crate::theme::BG_PANEL)
                .inner_margin(egui::Margin::same(6)))
            .show(ctx, |ui| {
            crate::sprite_picker::show(ui, &mut self.state);
        });
    }

    /// Handle actions from the sprite detail panel.
    fn handle_detail_action(&mut self, action: crate::sprite_detail::DetailAction, ctx: &egui::Context) {
        use crate::sprite_detail::DetailAction;
        match action {
            DetailAction::None => {}
            DetailAction::EditSprite => self.open_sprite_editor(ctx),
            DetailAction::Duplicate => self.duplicate_sprite(),
            DetailAction::Delete => self.delete_sprite(),
            DetailAction::ExportPng => self.export_sprite_to_png(),
            DetailAction::NewBlank => self.add_blank_sprite(),
        }
    }

    // ── Sprite CRUD operations ──

    /// Open the pixel editor for the currently selected sprite.
    fn open_sprite_editor(&mut self, _ctx: &egui::Context) {
        let Some(item_id) = self.state.selected_item_id else { return };

        // Resolve item_id → sprite_id via appearances
        let sprite_id = if let Some(ref apps) = self.state.appearances {
            pte_appearances::first_sprite_id(
                apps.get(pte_appearances::Category::Object, item_id)
                    .unwrap_or_else(|| {
                        // fallback: dummy
                        apps.objects.values().next().unwrap()
                    }),
            )
        } else {
            Some(item_id)
        };

        let Some(sid) = sprite_id else { return };

        // Find the sheet containing this sprite and extract pixel data
        for sheet in self.state.sprite_sheets.values() {
            if sid >= sheet.first_sprite_id && sid <= sheet.last_sprite_id {
                if let Some(pixels) = sheet.get_sprite(sid) {
                    let (w, h) = sheet.sprite_dimensions();
                    self.state.sprite_editor.load_sprite(sid, pixels, w, h);
                    return;
                }
            }
        }
    }

    /// Save edited sprite pixels back to the sprite sheet + refresh the GPU texture.
    fn save_edited_sprite(&mut self, ctx: &egui::Context) {
        let editor = &self.state.sprite_editor;
        let Some(sid) = editor.editing_sprite_id else { return };
        let pixels = editor.pixels.clone();
        let w = editor.sprite_w;
        let h = editor.sprite_h;

        // Write back to the in-memory sprite sheet
        for sheet in self.state.sprite_sheets.values_mut() {
            if sid >= sheet.first_sprite_id && sid <= sheet.last_sprite_id {
                sheet.set_sprite(sid, &pixels);
                break;
            }
        }

        // Refresh the GPU texture
        let tex = crate::sprite_picker::upload_sprite_texture(ctx, sid, &pixels, w, h);
        self.state.sprite_textures.insert(sid, tex);

        tracing::info!("Saved sprite #{} back to sheet", sid);
    }

    /// Save all modified sprite sheets to disk.
    fn save_all_sprite_sheets(&self) {
        let Some(ref asset_dir) = self.state.asset_dir else {
            tracing::error!("No asset directory set");
            return;
        };

        for (filename, sheet) in &self.state.sprite_sheets {
            let path = asset_dir.join(filename);
            match pte_assets::save_sprite_sheet(sheet, &path) {
                Ok(()) => tracing::info!("Saved sheet {}", filename),
                Err(e) => tracing::error!("Failed to save {}: {e:#}", filename),
            }
        }

        // Also save appearances if loaded
        if let Some(ref apps) = self.state.appearances {
            if let Some(ref catalog) = self.state.catalog {
                if let Some(ref app_file) = catalog.appearances_file {
                    let path = asset_dir.join(app_file);
                    match pte_appearances::save_appearances(apps, &path) {
                        Ok(()) => tracing::info!("Saved appearances"),
                        Err(e) => tracing::error!("Failed to save appearances: {e:#}"),
                    }
                }
            }
        }
    }

    /// Add a new blank object appearance (32×32 transparent).
    fn add_blank_sprite(&mut self) {
        let Some(ref mut apps) = self.state.appearances else { return };

        // Find the next available object ID
        let next_id = apps.objects.keys().copied().max().unwrap_or(0) + 1;

        // For now, just add a blank appearance entry — no sprite_id allocation.
        // The user can edit it with the pixel editor.
        let new_app = pte_appearances::Appearance {
            id: Some(next_id),
            frame_group: vec![],
            flags: None,
            name: Some(format!("New Sprite #{}", next_id)),
            description: None,
        };
        pte_appearances::upsert_appearance(apps, pte_appearances::Category::Object, new_app);
        self.state.selected_item_id = Some(next_id);
        tracing::info!("Created blank appearance #{}", next_id);
    }

    /// Duplicate the selected appearance.
    fn duplicate_sprite(&mut self) {
        let Some(item_id) = self.state.selected_item_id else { return };
        let Some(ref mut apps) = self.state.appearances else { return };

        let source = match apps.get(pte_appearances::Category::Object, item_id) {
            Some(a) => a.clone(),
            None => return,
        };

        let next_id = apps.objects.keys().copied().max().unwrap_or(0) + 1;
        let mut new_app = source;
        new_app.id = Some(next_id);
        new_app.name = Some(format!("Copy of #{}", item_id));
        pte_appearances::upsert_appearance(apps, pte_appearances::Category::Object, new_app);
        self.state.selected_item_id = Some(next_id);
        tracing::info!("Duplicated #{} → #{}", item_id, next_id);
    }

    /// Delete the selected appearance.
    fn delete_sprite(&mut self) {
        let Some(item_id) = self.state.selected_item_id else { return };
        let Some(ref mut apps) = self.state.appearances else { return };

        if pte_appearances::remove_appearance(apps, pte_appearances::Category::Object, item_id) {
            self.state.selected_item_id = None;
            tracing::info!("Deleted appearance #{}", item_id);
        }
    }

    /// Import a PNG file as a new sprite.
    fn import_sprite_from_png(&mut self, _ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_title("Import Sprite from PNG")
            .pick_file()
        else {
            return;
        };

        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let w = rgba.width();
                let h = rgba.height();
                let pixels = rgba.into_raw();

                // Open in the sprite editor for the user to crop/adjust
                let fake_sid = 0; // Will be assigned when saved
                self.state.sprite_editor.load_sprite(fake_sid, pixels, w, h);
                tracing::info!("Imported PNG {}×{} from {}", w, h, path.display());
            }
            Err(e) => {
                tracing::error!("Failed to import PNG: {e:#}");
            }
        }
    }

    /// Export the selected sprite to a PNG file.
    fn export_sprite_to_png(&self) {
        let Some(item_id) = self.state.selected_item_id else { return };

        // Resolve to sprite_id
        let sprite_id = if let Some(ref apps) = self.state.appearances {
            apps.get(pte_appearances::Category::Object, item_id)
                .and_then(pte_appearances::first_sprite_id)
        } else {
            Some(item_id)
        };

        let Some(sid) = sprite_id else { return };

        // Find sprite pixel data
        for sheet in self.state.sprite_sheets.values() {
            if sid >= sheet.first_sprite_id && sid <= sheet.last_sprite_id {
                if let Some(pixels) = sheet.get_sprite(sid) {
                    let (w, h) = sheet.sprite_dimensions();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG Image", &["png"])
                        .set_file_name(format!("sprite_{}.png", sid))
                        .save_file()
                    {
                        let img = image::RgbaImage::from_raw(w, h, pixels)
                            .expect("pixel buffer size mismatch");
                        match img.save(&path) {
                            Ok(()) => tracing::info!("Exported sprite #{} to {}", sid, path.display()),
                            Err(e) => tracing::error!("Failed to export PNG: {e:#}"),
                        }
                    }
                    return;
                }
            }
        }
    }
}
