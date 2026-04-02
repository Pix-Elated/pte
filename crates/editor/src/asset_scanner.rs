//! Project scanner — discovers OT assets, catalogs, and maps in a directory tree.
//!
//! Scans for catalog-content.json files (sprite catalogs with version info),
//! config.lua (canary server config with mapName), and groups all associated
//! .otbm map files by category (main, custom, quest, world_changes, events).
//!
//! The UI presents discovered projects as version cards the user can open.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::theme;
use egui::Color32;

// ── Data model ──

/// A discovered OT project with assets and maps grouped together.
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    /// Path to the catalog directory (containing catalog-content.json)
    pub catalog_dir: PathBuf,
    /// Version label (from folder name, e.g. "1310", "1500")
    pub version: String,
    /// Catalog file size
    pub catalog_size: u64,
    /// The world/ directory containing maps (if found)
    pub world_dir: Option<PathBuf>,
    /// Main map file (e.g. xtrails.otbm) — the primary editable map
    pub main_map: Option<MapEntry>,
    /// Custom overlay maps (world/custom/*.otbm) — loaded at startup
    pub custom_maps: Vec<MapEntry>,
    /// Quest-triggered maps (world/quest/**/*.otbm) — dynamic overlays
    pub quest_maps: Vec<MapEntry>,
    /// World change maps (world/world_changes/**/*.otbm) — dynamic overlays
    pub world_change_maps: Vec<MapEntry>,
    /// Event maps (world/annual_events/**/*.otbm) — dynamic overlays
    pub event_maps: Vec<MapEntry>,
    /// Other .otbm files that don't fit the above categories
    pub other_maps: Vec<MapEntry>,
    /// Whether legacy .spr/.dat files were found nearby
    pub has_legacy_sprites: bool,
    /// mapName from config.lua (if found)
    pub config_map_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub path: PathBuf,
    /// Display label relative to the world/ dir
    pub label: String,
    pub size: u64,
}

impl DiscoveredProject {
    pub fn total_map_count(&self) -> usize {
        self.main_map.iter().count()
            + self.custom_maps.len()
            + self.quest_maps.len()
            + self.world_change_maps.len()
            + self.event_maps.len()
            + self.other_maps.len()
    }

    pub fn total_map_size(&self) -> u64 {
        let main = self.main_map.as_ref().map_or(0, |m| m.size);
        let sum = |maps: &[MapEntry]| maps.iter().map(|m| m.size).sum::<u64>();
        main + sum(&self.custom_maps)
            + sum(&self.quest_maps)
            + sum(&self.world_change_maps)
            + sum(&self.event_maps)
            + sum(&self.other_maps)
    }
}

/// Full scan result.
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub projects: Vec<DiscoveredProject>,
}

impl ScanResult {
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }
}

// ── Scanning logic ──

/// Scan a directory tree for OT projects.
pub fn scan_directory(root: &Path, max_depth: usize) -> ScanResult {
    let mut catalogs: Vec<(PathBuf, String, u64)> = Vec::new();
    let mut config_map_names: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut world_dirs: Vec<PathBuf> = Vec::new();

    // Phase 1: Find catalog-content.json, config.lua, and .otbm files
    find_files_recursive(root, 0, max_depth, &mut |path, size| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name_lower = name.to_lowercase();

        if name_lower == "catalog-content.json" {
            let dir = path.parent().unwrap_or(path).to_path_buf();
            let version = dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            catalogs.push((dir, version, size));
        } else if name_lower == "config.lua" {
            // Try to extract mapName from canary config
            if let Ok(contents) = std::fs::read_to_string(path) {
                if let Some(map_name) = parse_config_map_name(&contents) {
                    let dir = path.parent().unwrap_or(path).to_path_buf();
                    config_map_names.insert(dir, map_name);
                }
            }
        } else if name_lower.ends_with(".otbm") && size > 0 {
            // Track directories that contain .otbm files — these are world dirs
            if let Some(parent) = path.parent() {
                let world_candidate = if parent.file_name().and_then(|n| n.to_str()) == Some("world") {
                    parent.to_path_buf()
                } else {
                    // Walk up to find the nearest ancestor named "world"
                    let mut p = parent;
                    loop {
                        if p.file_name().and_then(|n| n.to_str()) == Some("world") {
                            break p.to_path_buf();
                        }
                        match p.parent() {
                            Some(pp) if pp != root => p = pp,
                            _ => { break parent.to_path_buf(); }
                        }
                    }
                };
                if !world_dirs.contains(&world_candidate) {
                    world_dirs.push(world_candidate);
                }
            }
        }
    });

    tracing::info!(
        "Phase 1 complete: {} catalog(s), {} config(s), {} world dir(s)",
        catalogs.len(), config_map_names.len(), world_dirs.len(),
    );
    for wd in &world_dirs {
        tracing::info!("  discovered world dir: {}", wd.display());
    }

    // Phase 2: For each catalog, find the nearest world/ directory and group maps
    let mut projects = Vec::new();
    for (catalog_dir, version, catalog_size) in catalogs {
        let project = build_project(root, &catalog_dir, &version, catalog_size, &config_map_names, &world_dirs);
        projects.push(project);
    }

    // Sort by version (numeric-aware)
    projects.sort_by(|a, b| {
        let a_num: u64 = a.version.parse().unwrap_or(0);
        let b_num: u64 = b.version.parse().unwrap_or(0);
        b_num.cmp(&a_num) // Newest first
    });

    ScanResult { projects }
}

/// Build a DiscoveredProject by finding the associated world/ directory.
fn build_project(
    _scan_root: &Path,
    catalog_dir: &Path,
    version: &str,
    catalog_size: u64,
    config_map_names: &BTreeMap<PathBuf, String>,
    discovered_world_dirs: &[PathBuf],
) -> DiscoveredProject {
    let mut project = DiscoveredProject {
        catalog_dir: catalog_dir.to_path_buf(),
        version: version.to_string(),
        catalog_size,
        world_dir: None,
        main_map: None,
        custom_maps: Vec::new(),
        quest_maps: Vec::new(),
        world_change_maps: Vec::new(),
        event_maps: Vec::new(),
        other_maps: Vec::new(),
        has_legacy_sprites: false,
        config_map_name: None,
    };

    // Check for legacy .spr files nearby
    if let Ok(entries) = std::fs::read_dir(catalog_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".spr") {
                project.has_legacy_sprites = true;
                break;
            }
        }
    }

    // Look for world/ directory: first check pre-discovered world dirs from the scan,
    // then fall back to walking up from the catalog dir
    let world_dir = find_world_dir(catalog_dir, config_map_names, discovered_world_dirs);
    let Some(world_dir) = world_dir else {
        tracing::warn!("build_project: no world/ dir found for catalog {}", catalog_dir.display());
        return project;
    };
    tracing::info!("build_project: using world dir {}", world_dir.display());

    // Get map name from the config.lua nearest to this world dir
    let map_name = find_map_name_for_world(&world_dir, config_map_names);
    project.config_map_name = map_name.clone();
    project.world_dir = Some(world_dir.clone());

    // Collect all .otbm files under world/
    let mut all_maps: Vec<(PathBuf, u64)> = Vec::new();
    find_files_recursive(&world_dir, 0, 6, &mut |path, size| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.to_lowercase().ends_with(".otbm") {
            all_maps.push((path.to_path_buf(), size));
        }
    });

    // Categorize maps based on their path relative to world/
    for (map_path, size) in all_maps {
        let rel = map_path.strip_prefix(&world_dir).unwrap_or(&map_path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let label = rel_str.clone();

        let entry = MapEntry {
            path: map_path.clone(),
            label,
            size,
        };

        // Is this the main map?
        let stem = map_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let is_main = map_name.as_deref().map_or(false, |mn| stem == mn);
        // Also treat top-level .otbm files as potential mains
        let is_top_level = rel.parent().map_or(true, |p| p == Path::new(""));

        if is_main {
            project.main_map = Some(entry);
        } else if rel_str.starts_with("custom/") {
            project.custom_maps.push(entry);
        } else if rel_str.starts_with("quest/") || rel_str.starts_with("quests/") {
            project.quest_maps.push(entry);
        } else if rel_str.starts_with("world_changes/") {
            project.world_change_maps.push(entry);
        } else if rel_str.starts_with("annual_events/") || rel_str.starts_with("events/") {
            project.event_maps.push(entry);
        } else if is_top_level && project.main_map.is_none() {
            // If no mapName configured, the largest top-level .otbm is probably the main map
            if project.main_map.as_ref().map_or(true, |m| size > m.size) {
                // Swap: current main becomes other, this becomes main
                if let Some(prev) = project.main_map.take() {
                    project.other_maps.push(prev);
                }
                project.main_map = Some(entry);
            } else {
                project.other_maps.push(entry);
            }
        } else {
            project.other_maps.push(entry);
        }
    }

    project
}

/// Find the world/ directory associated with a catalog dir.
/// Strategy:
///   1. Check pre-discovered world dirs (found via .otbm files during the scan)
///   2. Walk up from the catalog dir checking each ancestor
fn find_world_dir(
    catalog_dir: &Path,
    config_map_names: &BTreeMap<PathBuf, String>,
    discovered_world_dirs: &[PathBuf],
) -> Option<PathBuf> {
    tracing::info!("find_world_dir: starting from {}", catalog_dir.display());

    // Strategy 1: Use pre-discovered world dirs from .otbm file scanning.
    // Pick the one that shares the most common ancestor with the catalog.
    if !discovered_world_dirs.is_empty() {
        // Find the common root between catalog and each world dir
        let mut best: Option<(usize, &PathBuf)> = None;
        for wd in discovered_world_dirs {
            let common = common_ancestor_depth(catalog_dir, wd);
            if let Some((best_depth, _)) = best {
                if common > best_depth {
                    best = Some((common, wd));
                }
            } else {
                best = Some((common, wd));
            }
        }
        if let Some((depth, wd)) = best {
            if depth >= 1 {
                tracing::info!(
                    "find_world_dir: using pre-discovered world dir {} (common depth {})",
                    wd.display(), depth,
                );
                return Some(wd.clone());
            }
        }
    }

    // Strategy 2: Walk up from catalog dir
    let mut search = catalog_dir.to_path_buf();
    for level in 0..8 {
        tracing::debug!("find_world_dir: level {} checking {}", level, search.display());
        // Check if current dir has a world/ subfolder
        let candidate = search.join("world");
        if candidate.is_dir() {
            tracing::info!("find_world_dir: found direct world/ at {}", candidate.display());
            return Some(candidate);
        }

        // Check direct children for data pack dirs with world/
        if let Ok(entries) = std::fs::read_dir(&search) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Direct data-*/world pattern
                    if name.starts_with("data-") || name == "data" {
                        let world = path.join("world");
                        if world.is_dir() {
                            return Some(world);
                        }
                    }

                    // One level deeper: sibling/data-*/world (e.g. canary/data-otservbr-global/world)
                    if let Ok(sub_entries) = std::fs::read_dir(&path) {
                        for sub in sub_entries.flatten() {
                            if sub.path().is_dir() {
                                let sub_name = sub.file_name().to_string_lossy().to_string();
                                if sub_name.starts_with("data-") || sub_name == "data" {
                                    let world = sub.path().join("world");
                                    tracing::debug!("find_world_dir: grandchild check {} -> exists={}", world.display(), world.is_dir());
                                    if world.is_dir() {
                                        tracing::info!("find_world_dir: found grandchild world/ at {}", world.display());
                                        return Some(world);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check if there's a config.lua at this level pointing us to a data pack
        if config_map_names.contains_key(&search) {
            if let Ok(entries) = std::fs::read_dir(&search) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let world = path.join("world");
                        if world.is_dir() {
                            return Some(world);
                        }
                    }
                }
            }
        }

        match search.parent() {
            Some(p) => search = p.to_path_buf(),
            None => break,
        }
    }
    None
}

/// Count how many path components two paths share from the root.
fn common_ancestor_depth(a: &Path, b: &Path) -> usize {
    a.components()
        .zip(b.components())
        .take_while(|(ca, cb)| ca == cb)
        .count()
}

/// Find the mapName from the nearest config.lua to a given world/ dir.
fn find_map_name_for_world(world_dir: &Path, config_map_names: &BTreeMap<PathBuf, String>) -> Option<String> {
    // Walk up from world dir's parent, looking for a config.lua
    let mut search = world_dir.parent()?.to_path_buf();
    for _ in 0..5 {
        if let Some(name) = config_map_names.get(&search) {
            return Some(name.clone());
        }
        match search.parent() {
            Some(p) => search = p.to_path_buf(),
            None => break,
        }
    }
    None
}

/// Parse `mapName = "..."` from a config.lua contents.
fn parse_config_map_name(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("mapName") {
            // mapName = "xtrails"
            if let Some(val) = trimmed.split('=').nth(1) {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Recursively find files, calling handler for each regular file found.
fn find_files_recursive(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    handler: &mut dyn FnMut(&Path, u64),
) {
    if depth > max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    // Skip directories that never contain OT assets
    if matches!(
        dir_name,
        ".git" | ".github" | ".svn" | ".claude" | ".vscode"
            | "node_modules" | "target" | "__pycache__"
            | "build" | "build-static" | "cmake"
            | "Release" | "RelWithDebInfo" | "Debug" | "MinSizeRel"
            | "vcproj" | "docker" | "metrics"
    ) {
        return;
    }

    // Skip hidden directories
    if dir_name.starts_with('.') && dir_name != "." {
        return;
    }

    // Skip C/C++ source directories (contain .cpp/.h files directly)
    // This avoids scanning huge source trees while still entering data dirs
    if dir_name == "src" {
        let has_cpp = std::fs::read_dir(dir)
            .ok()
            .map(|entries| {
                entries.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    n.ends_with(".cpp") || n.ends_with(".h") || n.ends_with(".cc") || n.ends_with(".cxx")
                })
            })
            .unwrap_or(false);
        if has_cpp {
            return;
        }
    }

    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            handler(&path, size);
        }
    }

    for subdir in subdirs {
        find_files_recursive(&subdir, depth + 1, max_depth, handler);
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

// ── Scanner state + UI ──

/// Scanner state stored in EditorState.
#[derive(Debug, Clone, Default)]
pub struct ScannerState {
    pub scan_root: Option<PathBuf>,
    pub scan_result: Option<ScanResult>,
    /// Which project the user is hovering/expanding
    pub expanded_project: Option<usize>,
    pub open: bool,
}

/// Action from the scanner dialog.
#[derive(Debug, Clone)]
pub enum ScannerAction {
    None,
    /// Load a complete project: assets + main map + custom overlays
    LoadProject {
        asset_dir: PathBuf,
        main_map: PathBuf,
        custom_maps: Vec<PathBuf>,
    },
}

/// Show the scanner UI. Called from the welcome screen or as a dialog.
pub fn show(ctx: &egui::Context, scanner: &mut ScannerState) -> ScannerAction {
    if !scanner.open {
        return ScannerAction::None;
    }

    // Use a shared action slot that the card callbacks can write to
    let mut pending_action: Option<ScannerAction> = None;

    egui::Window::new("Open Project")
        .default_size([600.0, 500.0])
        .resizable(true)
        .collapsible(false)
        .open(&mut scanner.open)
        .show(ctx, |ui| {
            // Scan button + path
            ui.horizontal(|ui| {
                if ui.button("📂 Choose Folder…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Select OT Project Root")
                        .pick_folder()
                    {
                        let result = scan_directory(&dir, 8);
                        scanner.scan_root = Some(dir);
                        scanner.scan_result = Some(result);
                        scanner.expanded_project = None;
                    }
                }

                if let Some(ref root) = scanner.scan_root {
                    ui.label(
                        egui::RichText::new(root.display().to_string())
                            .size(10.0)
                            .color(theme::TEXT_MUTED)
                            .monospace(),
                    );
                }
            });

            ui.add_space(8.0);

            let Some(ref result) = scanner.scan_result else {
                ui.label(
                    egui::RichText::new("Select a project folder to scan for OT assets")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                return;
            };

            if result.is_empty() {
                ui.colored_label(theme::WARNING, "No catalog-content.json files found in this directory tree.");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Make sure the folder contains client asset files.")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                return;
            }

            ui.label(
                egui::RichText::new(format!("Found {} asset version(s)", result.projects.len()))
                    .size(11.0)
                    .color(theme::SUCCESS),
            );
            ui.add_space(8.0);

            // Project cards
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (idx, project) in result.projects.iter().enumerate() {
                        let expanded = scanner.expanded_project == Some(idx);
                        if let Some(act) = show_project_card(ui, project, idx, expanded, &mut scanner.expanded_project) {
                            pending_action = Some(act);
                        }
                        ui.add_space(6.0);
                    }
                });
        });

    if let Some(ref _act) = pending_action {
        scanner.open = false;
    }
    pending_action.unwrap_or(ScannerAction::None)
}

/// Draw a single project version card. Returns Some(action) if user clicked Open.
fn show_project_card(
    ui: &mut egui::Ui,
    project: &DiscoveredProject,
    idx: usize,
    expanded: bool,
    expanded_state: &mut Option<usize>,
) -> Option<ScannerAction> {
    let mut action = None;
    let frame_bg = if expanded { theme::BG_RAISED } else { theme::BG_SURFACE };

    egui::Frame::NONE
        .fill(frame_bg)
        .corner_radius(egui::CornerRadius::same(6))
        .stroke(egui::Stroke::new(
            if expanded { 1.0 } else { 0.5 },
            if expanded { theme::ACCENT } else { theme::BORDER },
        ))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // ── Header row: version badge + catalog path + Open button ──
            ui.horizontal(|ui| {
                // Version badge
                let badge_text = format!("v{}", project.version);
                let badge_rect = ui.allocate_exact_size(egui::vec2(56.0, 22.0), egui::Sense::hover()).0;
                ui.painter().rect_filled(badge_rect, 4.0, theme::ACCENT);
                ui.painter().text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &badge_text,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );

                ui.add_space(8.0);

                // Info column
                ui.vertical(|ui| {
                    // Catalog location
                    let rel_path = project.catalog_dir.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?");
                    ui.label(
                        egui::RichText::new(format!("📁 {}", rel_path))
                            .size(12.0)
                            .color(theme::TEXT_PRIMARY),
                    );

                    // Map summary
                    let map_count = project.total_map_count();
                    let map_label = if let Some(ref main) = project.main_map {
                        let stem = main.path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                        format!(
                            "🗺 {} ({}) + {} overlay(s)",
                            stem,
                            format_size(main.size),
                            map_count.saturating_sub(1),
                        )
                    } else {
                        format!("🗺 {} map file(s)", map_count)
                    };
                    ui.label(
                        egui::RichText::new(map_label)
                            .size(10.5)
                            .color(theme::TEXT_SECONDARY),
                    );
                });

                // Push button to right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Open button (only if there's a main map)
                    let can_open = project.main_map.is_some();
                    let btn = egui::Button::new(
                        egui::RichText::new("Open")
                            .size(12.0)
                            .color(if can_open { Color32::WHITE } else { theme::TEXT_MUTED }),
                    )
                    .fill(if can_open { theme::ACCENT } else { theme::BG_RAISED })
                    .corner_radius(egui::CornerRadius::same(4))
                    .min_size(egui::vec2(64.0, 26.0));

                    if ui.add_enabled(can_open, btn).clicked() {
                        if let Some(ref main) = project.main_map {
                            action = Some(ScannerAction::LoadProject {
                                asset_dir: project.catalog_dir.clone(),
                                main_map: main.path.clone(),
                                custom_maps: project.custom_maps.iter().map(|m| m.path.clone()).collect(),
                            });
                        }
                    }

                    // Expand toggle
                    let toggle_text = if expanded { "▾ Details" } else { "▸ Details" };
                    if ui.small_button(toggle_text).clicked() {
                        if expanded {
                            *expanded_state = None;
                        } else {
                            *expanded_state = Some(idx);
                        }
                    }
                });
            });

            // ── Expanded details ──
            if expanded {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                let section = |ui: &mut egui::Ui, label: &str, maps: &[MapEntry], color: Color32| {
                    if maps.is_empty() { return; }
                    ui.horizontal(|ui| {
                        ui.colored_label(color, "●");
                        ui.label(
                            egui::RichText::new(format!("{} ({})", label, maps.len()))
                                .size(10.5)
                                .color(theme::TEXT_SECONDARY)
                                .strong(),
                        );
                    });
                    for map in maps {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                egui::RichText::new(&map.label)
                                    .size(10.0)
                                    .color(theme::TEXT_MUTED)
                                    .monospace(),
                            );
                            ui.label(
                                egui::RichText::new(format_size(map.size))
                                    .size(9.5)
                                    .color(theme::TEXT_MUTED),
                            );
                        });
                    }
                    ui.add_space(2.0);
                };

                if let Some(ref main) = project.main_map {
                    ui.horizontal(|ui| {
                        ui.colored_label(theme::SUCCESS, "●");
                        ui.label(
                            egui::RichText::new(format!("Main Map: {} ({})", main.label, format_size(main.size)))
                                .size(10.5)
                                .color(theme::TEXT_SECONDARY)
                                .strong(),
                        );
                    });
                    ui.add_space(2.0);
                }

                section(ui, "Custom Overlays", &project.custom_maps, theme::ACCENT);
                section(ui, "Quest Maps", &project.quest_maps, Color32::from_rgb(180, 140, 255));
                section(ui, "World Changes", &project.world_change_maps, Color32::from_rgb(255, 180, 100));
                section(ui, "Events", &project.event_maps, Color32::from_rgb(100, 200, 255));
                section(ui, "Other Maps", &project.other_maps, theme::TEXT_MUTED);

                if project.config_map_name.is_some() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "config.lua mapName: \"{}\"",
                            project.config_map_name.as_deref().unwrap_or("?")
                        ))
                        .size(9.5)
                        .color(theme::TEXT_MUTED)
                        .italics(),
                    );
                }
            }
        });

    action
}
