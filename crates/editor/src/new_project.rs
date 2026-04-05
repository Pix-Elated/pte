//! New Project wizard — create a fresh OT project from scratch.
//!
//! Generates:
//! - catalog-content.json + blank sprite sheet(s) + appearances.dat (protobuf), OR
//! - Tibia.dat + Tibia.spr (legacy format)
//! - A blank OTBM map
//! - config.lua for Canary server
//! - spawn.xml, houses.xml
//! - Optional: fetch OTClient and/or Canary from GitHub releases

use crate::state::EditorState;
use crate::theme;
use egui::Color32;
use std::path::PathBuf;
use std::sync::mpsc;

// ── Wizard state ─────────────────────────────────────────────────────────────

/// Asset format for the new project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetFormat {
    /// Modern CIP protobuf: catalog-content.json + .cip sheets + appearances.dat
    Protobuf,
    /// Legacy: Tibia.dat + Tibia.spr
    Legacy,
}

/// Protocol version presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    /// 13.40+ (Canary default, CIP protobuf assets)
    V1340,
    /// 12.90 (last major CIP protobuf, widely used)
    V1290,
    /// 10.98 (popular OTClient legacy, uses .dat/.spr)
    V1098,
    /// 8.60 (classic OT, uses .dat/.spr)
    V860,
}

impl ProtocolVersion {
    pub fn label(self) -> &'static str {
        match self {
            Self::V1340 => "13.40+ (Canary default)",
            Self::V1290 => "12.90",
            Self::V1098 => "10.98",
            Self::V860 => "8.60 (Classic)",
        }
    }

    pub fn format(self) -> AssetFormat {
        match self {
            Self::V1340 | Self::V1290 => AssetFormat::Protobuf,
            Self::V1098 | Self::V860 => AssetFormat::Legacy,
        }
    }

    pub fn otbm_version(self) -> u32 {
        match self {
            Self::V1340 | Self::V1290 => 3,
            Self::V1098 => 2,
            Self::V860 => 1,
        }
    }

    pub fn dat_signature(self) -> u32 {
        match self {
            Self::V1340 => 0,
            Self::V1290 => 0,
            Self::V1098 => 0x5741_5102, // standard 10.98 sig
            Self::V860 => 0x439D_5A33,
        }
    }

    pub fn spr_signature(self) -> u32 {
        match self {
            Self::V1340 | Self::V1290 => 0,
            Self::V1098 => 0x5741_5102,
            Self::V860 => 0x439D_5A33,
        }
    }

    pub fn is_extended_spr(self) -> bool {
        match self {
            Self::V1340 | Self::V1290 | Self::V1098 => true,
            Self::V860 => false,
        }
    }
}

/// Background task result.
pub enum ProjectCreationResult {
    Success(PathBuf),
    Error(String),
}

/// Wizard step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Setup,
    Creating,
    Done,
}

/// Full wizard state.
pub struct NewProjectWizard {
    pub open: bool,
    step: WizardStep,

    // ── Setup fields ──
    pub project_name: String,
    pub project_dir: Option<PathBuf>,
    pub protocol: ProtocolVersion,
    pub map_width: u32,
    pub map_height: u32,

    // ── Optional installs ──
    pub fetch_otclient: bool,
    pub fetch_canary: bool,

    // ── Background work ──
    creation_rx: Option<mpsc::Receiver<ProjectCreationResult>>,
    creation_status: String,
    result_path: Option<PathBuf>,
    error: Option<String>,
}

impl Default for NewProjectWizard {
    fn default() -> Self {
        Self {
            open: false,
            step: WizardStep::Setup,
            project_name: "my-ot-server".to_string(),
            project_dir: dirs::document_dir().or_else(dirs::home_dir),
            protocol: ProtocolVersion::V1340,
            map_width: 2048,
            map_height: 2048,
            fetch_otclient: false,
            fetch_canary: false,
            creation_rx: None,
            creation_status: String::new(),
            result_path: None,
            error: None,
        }
    }
}

// ── UI ───────────────────────────────────────────────────────────────────────

pub fn show(ctx: &egui::Context, wizard: &mut NewProjectWizard) {
    if !wizard.open {
        return;
    }

    // Poll background task
    if let Some(ref rx) = wizard.creation_rx {
        if let Ok(result) = rx.try_recv() {
            match result {
                ProjectCreationResult::Success(path) => {
                    wizard.result_path = Some(path);
                    wizard.step = WizardStep::Done;
                    wizard.error = None;
                }
                ProjectCreationResult::Error(msg) => {
                    wizard.error = Some(msg);
                    wizard.step = WizardStep::Setup;
                }
            }
            wizard.creation_rx = None;
        }
    }

    let mut is_open = wizard.open;

    egui::Window::new("New Project")
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .default_size([480.0, 0.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| match wizard.step {
            WizardStep::Setup => show_setup(ui, wizard),
            WizardStep::Creating => show_creating(ui, wizard),
            WizardStep::Done => show_done(ui, wizard),
        });

    wizard.open = is_open;
}

fn show_setup(ui: &mut egui::Ui, wizard: &mut NewProjectWizard) {
    ui.heading("Create a New OT Project");
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "Set up a project with blank assets, ready for creating your own content.",
        )
        .size(11.0)
        .color(theme::TEXT_SECONDARY),
    );
    ui.add_space(12.0);

    // ── Project Info ──
    egui::Grid::new("new_proj_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            // Name
            ui.label(
                egui::RichText::new("Project Name:")
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut wizard.project_name)
                    .desired_width(280.0)
                    .hint_text("my-ot-server"),
            );
            ui.end_row();

            // Directory
            ui.label(
                egui::RichText::new("Location:")
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.horizontal(|ui| {
                let dir_str = wizard
                    .project_dir
                    .as_ref()
                    .map_or("(not set)".to_string(), |d| d.display().to_string());
                let truncated = if dir_str.len() > 40 {
                    format!("…{}", &dir_str[dir_str.len() - 38..])
                } else {
                    dir_str
                };
                ui.label(
                    egui::RichText::new(truncated)
                        .size(10.5)
                        .color(theme::TEXT_PRIMARY),
                );
                if ui.small_button("Browse…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Select project location")
                        .pick_folder()
                    {
                        wizard.project_dir = Some(dir);
                    }
                }
            });
            ui.end_row();

            // Protocol version
            ui.label(
                egui::RichText::new("Protocol:")
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            egui::ComboBox::from_id_salt("protocol_combo")
                .selected_text(wizard.protocol.label())
                .width(280.0)
                .show_ui(ui, |ui| {
                    for ver in [
                        ProtocolVersion::V1340,
                        ProtocolVersion::V1290,
                        ProtocolVersion::V1098,
                        ProtocolVersion::V860,
                    ] {
                        ui.selectable_value(&mut wizard.protocol, ver, ver.label());
                    }
                });
            ui.end_row();

            // Format indicator
            ui.label(
                egui::RichText::new("Asset Format:")
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            let fmt_str = match wizard.protocol.format() {
                AssetFormat::Protobuf => {
                    "Protobuf (catalog-content.json + appearances.dat + .cip sheets)"
                }
                AssetFormat::Legacy => "Legacy (Tibia.dat + Tibia.spr)",
            };
            ui.label(
                egui::RichText::new(fmt_str)
                    .size(10.5)
                    .color(theme::TEXT_MUTED),
            );
            ui.end_row();

            // Map dimensions
            ui.label(
                egui::RichText::new("Map Size:")
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut wizard.map_width)
                        .range(256..=65535)
                        .speed(64),
                );
                ui.label("×");
                ui.add(
                    egui::DragValue::new(&mut wizard.map_height)
                        .range(256..=65535)
                        .speed(64),
                );
                ui.label(
                    egui::RichText::new("tiles")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            });
            ui.end_row();
        });

    ui.add_space(16.0);

    // ── Optional: Fetch client / server ──
    ui.label(
        egui::RichText::new("OPTIONAL: AUTO-INSTALL")
            .size(10.0)
            .color(theme::TEXT_MUTED)
            .strong(),
    );
    ui.add_space(4.0);

    ui.checkbox(
        &mut wizard.fetch_canary,
        "Fetch & install Canary server (latest release)",
    );
    ui.indent("canary_info", |ui| {
        ui.label(
            egui::RichText::new(
                "Downloads the latest Canary release from GitHub and sets up a server directory. \
            The default map will be replaced with your blank project map.",
            )
            .size(10.0)
            .color(theme::TEXT_MUTED),
        );
    });

    ui.add_space(4.0);

    ui.checkbox(
        &mut wizard.fetch_otclient,
        "Fetch & install OTClient (latest release)",
    );
    ui.indent("otclient_info", |ui| {
        ui.label(
            egui::RichText::new(
                "Downloads the latest OTClient release from GitHub. \
            Client data files will be replaced with your project's blank assets.",
            )
            .size(10.0)
            .color(theme::TEXT_MUTED),
        );
    });

    ui.add_space(16.0);

    // ── Error display ──
    if let Some(ref err) = wizard.error {
        ui.colored_label(theme::ERROR, format!("Error: {err}"));
        ui.add_space(8.0);
    }

    // ── Action buttons ──
    ui.horizontal(|ui| {
        let can_create = !wizard.project_name.is_empty() && wizard.project_dir.is_some();

        let create_btn = egui::Button::new(
            egui::RichText::new("Create Project")
                .size(13.0)
                .color(Color32::WHITE),
        )
        .fill(theme::ACCENT)
        .min_size(egui::vec2(140.0, 32.0));

        if ui.add_enabled(can_create, create_btn).clicked() {
            start_creation(wizard);
        }

        if ui.button("Cancel").clicked() {
            wizard.open = false;
            *wizard = NewProjectWizard::default();
        }
    });
}

fn show_creating(ui: &mut egui::Ui, wizard: &mut NewProjectWizard) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.spinner();
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(&wizard.creation_status)
                .size(12.0)
                .color(theme::TEXT_SECONDARY),
        );
    });
    ui.add_space(20.0);
}

fn show_done(ui: &mut egui::Ui, wizard: &mut NewProjectWizard) {
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("✓ Project created successfully!")
                .size(14.0)
                .color(theme::SUCCESS)
                .strong(),
        );
        ui.add_space(8.0);
        if let Some(ref path) = wizard.result_path {
            ui.label(
                egui::RichText::new(path.display().to_string())
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
        }
    });
    ui.add_space(16.0);

    ui.horizontal(|ui| {
        let open_btn = egui::Button::new(
            egui::RichText::new("Open Project")
                .size(13.0)
                .color(Color32::WHITE),
        )
        .fill(theme::ACCENT)
        .min_size(egui::vec2(140.0, 32.0));

        if ui.add(open_btn).clicked() {
            // Signal the app to scan and open this project
            wizard.open = false;
        }

        if ui.button("Close").clicked() {
            wizard.open = false;
            *wizard = NewProjectWizard::default();
        }
    });
}

// ── Project creation (runs on background thread) ─────────────────────────────

fn start_creation(wizard: &mut NewProjectWizard) {
    let project_name = wizard.project_name.clone();
    let parent_dir = wizard.project_dir.clone().unwrap();
    let protocol = wizard.protocol;
    let map_w = wizard.map_width;
    let map_h = wizard.map_height;
    let fetch_canary = wizard.fetch_canary;
    let fetch_otclient = wizard.fetch_otclient;

    let (tx, rx) = mpsc::channel();
    wizard.creation_rx = Some(rx);
    wizard.step = WizardStep::Creating;
    wizard.creation_status = "Creating project structure…".to_string();
    wizard.error = None;

    std::thread::spawn(move || {
        let result = create_project(
            &project_name,
            &parent_dir,
            protocol,
            map_w,
            map_h,
            fetch_canary,
            fetch_otclient,
        );
        let _ = tx.send(result);
    });
}

fn create_project(
    name: &str,
    parent: &std::path::Path,
    protocol: ProtocolVersion,
    map_w: u32,
    map_h: u32,
    fetch_canary: bool,
    fetch_otclient: bool,
) -> ProjectCreationResult {
    let root = parent.join(name);
    if root.exists() {
        return ProjectCreationResult::Error(format!(
            "Directory already exists: {}",
            root.display()
        ));
    }

    // Create directory structure
    let data_dir = root.join("data");
    let world_dir = data_dir.join("world");
    let custom_dir = world_dir.join("custom");

    for dir in [&root, &data_dir, &world_dir, &custom_dir] {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return ProjectCreationResult::Error(format!(
                "Failed to create {}: {e}",
                dir.display()
            ));
        }
    }

    // Generate assets based on format
    match protocol.format() {
        AssetFormat::Protobuf => {
            if let Err(e) = create_protobuf_assets(&data_dir, protocol) {
                return ProjectCreationResult::Error(format!("Asset generation failed: {e:#}"));
            }
        }
        AssetFormat::Legacy => {
            if let Err(e) = create_legacy_assets(&data_dir, protocol) {
                return ProjectCreationResult::Error(format!("Asset generation failed: {e:#}"));
            }
        }
    }

    // Create blank map
    if let Err(e) = create_blank_map(&world_dir, name, map_w, map_h, protocol) {
        return ProjectCreationResult::Error(format!("Map generation failed: {e:#}"));
    }

    // Create empty spawn.xml and houses.xml
    if let Err(e) = create_support_files(&world_dir) {
        return ProjectCreationResult::Error(format!("Support file creation failed: {e:#}"));
    }

    // Create config.lua
    if let Err(e) = create_config_lua(&root, name, protocol) {
        return ProjectCreationResult::Error(format!("Config creation failed: {e:#}"));
    }

    // Optional: fetch Canary
    if fetch_canary {
        match fetch_and_setup_canary(&root, &world_dir, name) {
            Ok(()) => tracing::info!("Canary server installed"),
            Err(e) => tracing::warn!("Canary fetch failed (non-fatal): {e:#}"),
        }
    }

    // Optional: fetch OTClient
    if fetch_otclient {
        let client_dir = root.join("client");
        match fetch_and_setup_otclient(&client_dir, &data_dir, protocol) {
            Ok(()) => tracing::info!("OTClient installed"),
            Err(e) => tracing::warn!("OTClient fetch failed (non-fatal): {e:#}"),
        }
    }

    tracing::info!("Project '{}' created at {}", name, root.display());
    ProjectCreationResult::Success(root)
}

// ── Protobuf asset generation ────────────────────────────────────────────────

fn create_protobuf_assets(
    data_dir: &std::path::Path,
    _protocol: ProtocolVersion,
) -> anyhow::Result<()> {
    // Create a minimal appearances.dat with a few essential objects:
    //   ID 100 = void/black ground tile
    //   ID 101 = basic ground tile (visible)
    let mut apps = pte_appearances::LoadedAppearances::default();

    // Sprite IDs we'll create: 1 = transparent, 2 = solid dark tile, 3 = basic ground
    let sprite_ids_for_void = vec![2u32];
    let sprite_ids_for_ground = vec![3u32];

    // Object 100: void ground (dark, walkable)
    let void_app = pte_appearances::Appearance {
        id: Some(100),
        frame_group: vec![pte_appearances::FrameGroup {
            fixed_frame_group: None,
            id: Some(0),
            sprite_info: Some(pte_appearances::SpriteInfo {
                pattern_width: Some(1),
                pattern_height: Some(1),
                pattern_depth: Some(1),
                layers: Some(1),
                sprite_id: sprite_ids_for_void,
                bounding_square: None,
                animation: None,
                is_opaque: Some(true),
                bounding_box_per_direction: vec![],
            }),
        }],
        flags: Some(pte_appearances::AppearanceFlags {
            bank: Some(pte_appearances::proto::AppearanceFlagBank {
                waypoints: Some(150),
            }),
            ..Default::default()
        }),
        name: Some("Void".to_string()),
        description: None,
    };

    // Object 101: basic ground tile
    let ground_app = pte_appearances::Appearance {
        id: Some(101),
        frame_group: vec![pte_appearances::FrameGroup {
            fixed_frame_group: None,
            id: Some(0),
            sprite_info: Some(pte_appearances::SpriteInfo {
                pattern_width: Some(1),
                pattern_height: Some(1),
                pattern_depth: Some(1),
                layers: Some(1),
                sprite_id: sprite_ids_for_ground,
                bounding_square: None,
                animation: None,
                is_opaque: Some(true),
                bounding_box_per_direction: vec![],
            }),
        }],
        flags: Some(pte_appearances::AppearanceFlags {
            bank: Some(pte_appearances::proto::AppearanceFlagBank {
                waypoints: Some(150),
            }),
            ..Default::default()
        }),
        name: Some("Ground".to_string()),
        description: None,
    };

    pte_appearances::upsert_appearance(&mut apps, pte_appearances::Category::Object, void_app);
    pte_appearances::upsert_appearance(&mut apps, pte_appearances::Category::Object, ground_app);

    pte_appearances::save_appearances(&apps, &data_dir.join("appearances.dat"))?;

    // Create sprite sheet with 3 sprites:
    //   1 = transparent (blank)
    //   2 = dark tile (20,20,20 gray)
    //   3 = ground tile (139,119,80 brown, with border)
    let sheet_w = 384u32;
    let sheet_h = 32u32; // one row: sprites 1-12 fit in 384/32 = 12 columns
    let mut pixels = vec![0u8; (sheet_w * sheet_h * 4) as usize];

    // Sprite 1 (col 0): fully transparent — already zeroed

    // Sprite 2 (col 1): dark void tile
    fill_sprite_rect(&mut pixels, sheet_w, 32, 0, 32, 32, [20, 20, 20, 255]);

    // Sprite 3 (col 2): brown ground tile with subtle border
    fill_sprite_rect(&mut pixels, sheet_w, 64, 0, 32, 32, [139, 119, 80, 255]);
    // Top and left border (slightly darker)
    for x in 64..96 {
        set_pixel(&mut pixels, sheet_w, x, 0, [100, 85, 55, 255]);
    }
    for y in 0..32 {
        set_pixel(&mut pixels, sheet_w, 64, y, [100, 85, 55, 255]);
    }

    let sheet = pte_assets::SpriteSheet {
        first_sprite_id: 1,
        last_sprite_id: 3,
        sprite_type: pte_assets::SpriteType::Size32x32,
        width: sheet_w,
        height: sheet_h,
        pixels,
    };

    pte_assets::save_sprite_sheet(&sheet, &data_dir.join("0.cip"))?;

    // Write catalog-content.json
    let catalog = serde_json::json!([
        {
            "type": "sprite",
            "file": "0.cip",
            "spritetype": 0,
            "firstspriteid": 1,
            "lastspriteid": 3
        },
        {
            "type": "appearances",
            "file": "appearances.dat"
        }
    ]);
    std::fs::write(
        data_dir.join("catalog-content.json"),
        serde_json::to_string_pretty(&catalog)?,
    )?;

    tracing::info!("Created protobuf assets (2 objects, 3 sprites)");
    Ok(())
}

// ── Legacy asset generation ──────────────────────────────────────────────────

fn create_legacy_assets(
    data_dir: &std::path::Path,
    protocol: ProtocolVersion,
) -> anyhow::Result<()> {
    use tibia_spr_dat::*;

    // Create SPR with 3 sprites
    let mut spr = SprFile {
        signature: protocol.spr_signature(),
        sprites: Vec::new(),
        extended: protocol.is_extended_spr(),
    };

    // Sprite 1: transparent
    spr.add_sprite(Sprite::new_transparent());

    // Sprite 2: dark void tile
    let mut void_pixels = vec![0u8; Sprite::BYTE_COUNT];
    for chunk in void_pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[20, 20, 20, 255]);
    }
    spr.add_sprite(Sprite {
        pixels: void_pixels,
    });

    // Sprite 3: brown ground tile
    let mut ground_pixels = vec![0u8; Sprite::BYTE_COUNT];
    for chunk in ground_pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[139, 119, 80, 255]);
    }
    // Top border
    for x in 0..32usize {
        let idx = x * 4;
        ground_pixels[idx..idx + 4].copy_from_slice(&[100, 85, 55, 255]);
    }
    // Left border
    for y in 0..32usize {
        let idx = y * 32 * 4;
        ground_pixels[idx..idx + 4].copy_from_slice(&[100, 85, 55, 255]);
    }
    spr.add_sprite(Sprite {
        pixels: ground_pixels,
    });

    write_spr(&spr, &data_dir.join("Tibia.spr"))?;

    // Create DAT with 2 items (100, 101)
    let dat = DatFile {
        signature: protocol.dat_signature(),
        items: vec![
            DatEntry {
                id: 100,
                category: DatCategory::Item,
                flags: DatFlags {
                    is_ground: Some(150),
                    ..Default::default()
                },
                sprite_layout: SpriteLayout {
                    width: 1,
                    height: 1,
                    exact_size: None,
                    layers: 1,
                    pattern_x: 1,
                    pattern_y: 1,
                    pattern_z: 1,
                    frames: 1,
                    sprite_ids: vec![2],
                },
            },
            DatEntry {
                id: 101,
                category: DatCategory::Item,
                flags: DatFlags {
                    is_ground: Some(150),
                    ..Default::default()
                },
                sprite_layout: SpriteLayout {
                    width: 1,
                    height: 1,
                    exact_size: None,
                    layers: 1,
                    pattern_x: 1,
                    pattern_y: 1,
                    pattern_z: 1,
                    frames: 1,
                    sprite_ids: vec![3],
                },
            },
        ],
        outfits: Vec::new(),
        effects: Vec::new(),
        missiles: Vec::new(),
    };

    write_dat(&dat, &data_dir.join("Tibia.dat"))?;

    tracing::info!("Created legacy assets (2 items, 3 sprites)");
    Ok(())
}

// ── Blank map generation ─────────────────────────────────────────────────────

fn create_blank_map(
    world_dir: &std::path::Path,
    name: &str,
    width: u32,
    height: u32,
    protocol: ProtocolVersion,
) -> anyhow::Result<()> {
    let mut map = pte_otbm::MapData::new();
    map.width = width as u16;
    map.height = height as u16;
    map.description = format!("{} - Created with Pixelated's Tibia Editor", name);
    map.spawn_file = "spawn.xml".to_string();
    map.house_file = "houses.xml".to_string();
    map.version = protocol.otbm_version();
    map.item_major_version = 3;
    map.item_minor_version = 57;

    // Place a small spawn area at the center of the map (ground floor = z=7)
    let cx = width as u16 / 2;
    let cy = height as u16 / 2;
    let z = crate::state::MAP_SURFACE_Z as u8;

    // 10×10 ground patch around center as a starting area
    for dy in 0..10u16 {
        for dx in 0..10u16 {
            let x = cx - 5 + dx;
            let y = cy - 5 + dy;
            let mut tile = pte_otbm::Tile::new(x, y, z);
            tile.ground = Some(101); // our basic ground tile
            map.set_tile(tile);
        }
    }

    // Add a default town at center
    map.towns.push(pte_otbm::Town {
        id: 1,
        name: "Temple".to_string(),
        position: pte_otbm::Position { x: cx, y: cy, z },
    });

    // Add a waypoint at spawn
    map.waypoints.push(pte_otbm::Waypoint {
        name: "spawn".to_string(),
        position: pte_otbm::Position { x: cx, y: cy, z },
    });

    let map_filename = format!("{}.otbm", name);
    pte_otbm::serialize_otbm(&map, &world_dir.join(&map_filename))?;

    tracing::info!(
        "Created blank map {}×{} at {}",
        width,
        height,
        world_dir.display()
    );
    Ok(())
}

// ── Support files ────────────────────────────────────────────────────────────

fn create_support_files(world_dir: &std::path::Path) -> anyhow::Result<()> {
    // Empty spawn.xml
    std::fs::write(
        world_dir.join("spawn.xml"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<spawns>\n</spawns>\n",
    )?;

    // Empty houses.xml
    std::fs::write(
        world_dir.join("houses.xml"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<houses>\n</houses>\n",
    )?;

    Ok(())
}

fn create_config_lua(
    root: &std::path::Path,
    name: &str,
    protocol: ProtocolVersion,
) -> anyhow::Result<()> {
    let ip = "127.0.0.1";
    let port = 7172;
    let login_port = 7171;

    let config = format!(
        r#"-- {name} server configuration
-- Generated by Pixelated's Tibia Editor

-- Server
serverName = "{name}"
ip = "{ip}"
loginPort = {login_port}
gamePort = {port}

-- Map
mapName = "{name}"
mapAuthor = "PTE"

-- Protocol
protocolVersion = {proto_ver}

-- Rates (default 1x, customize as needed)
rateExp = 1
rateSkill = 1
rateLoot = 1
rateMagic = 1
rateSpawn = 1

-- Misc
maxPlayers = 100
statusTimeout = 5000
replaceKickOnLogin = true
forceNewLoginOnNewCharacter = true
"#,
        name = name,
        ip = ip,
        login_port = login_port,
        port = port,
        proto_ver = match protocol {
            ProtocolVersion::V1340 => "1340",
            ProtocolVersion::V1290 => "1290",
            ProtocolVersion::V1098 => "1098",
            ProtocolVersion::V860 => "860",
        },
    );

    std::fs::write(root.join("config.lua"), config)?;
    Ok(())
}

// ── GitHub release fetching ──────────────────────────────────────────────────

/// Fetch the latest release asset URL from a GitHub repo.
fn fetch_latest_release_asset(
    owner: &str,
    repo: &str,
    asset_filter: &dyn Fn(&str) -> bool,
) -> anyhow::Result<(String, String)> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::blocking::Client::builder()
        .user_agent("PTE-Editor/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let resp: serde_json::Value = client.get(&url).send()?.json()?;

    let tag = resp["tag_name"].as_str().unwrap_or("unknown").to_string();

    let assets = resp["assets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets in release"))?;

    for asset in assets {
        let name = asset["name"].as_str().unwrap_or("");
        if asset_filter(name) {
            let download_url = asset["browser_download_url"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No download URL for asset"))?
                .to_string();
            return Ok((download_url, tag));
        }
    }

    anyhow::bail!("No matching asset found in {owner}/{repo} latest release")
}

/// Download a URL to a file.
fn download_file(url: &str, dest: &std::path::Path) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("PTE-Editor/1.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let mut resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let mut file = std::fs::File::create(dest)?;
    std::io::copy(&mut resp, &mut file)?;
    Ok(())
}

/// Extract a zip archive to a directory.
fn extract_zip(zip_path: &std::path::Path, dest_dir: &std::path::Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };

        // Strip the top-level directory if present (common in GitHub releases)
        let out_path = if let Ok(stripped) = name.strip_prefix(
            name.components()
                .next()
                .unwrap_or(std::path::Component::CurDir),
        ) {
            if stripped.as_os_str().is_empty() {
                continue; // skip the top-level dir itself
            }
            dest_dir.join(stripped)
        } else {
            dest_dir.join(name)
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }

    Ok(())
}

// ── Canary setup ─────────────────────────────────────────────────────────────

fn fetch_and_setup_canary(
    root: &std::path::Path,
    world_dir: &std::path::Path,
    map_name: &str,
) -> anyhow::Result<()> {
    tracing::info!("Fetching latest Canary release…");

    let (url, tag) = fetch_latest_release_asset("opentibiabr", "canary", &|name: &str| {
        let lower = name.to_lowercase();
        lower.contains("windows") && lower.ends_with(".zip")
    })?;

    tracing::info!("Downloading Canary {} …", tag);
    let zip_path = root.join("canary-download.zip");
    download_file(&url, &zip_path)?;

    let server_dir = root.join("server");
    std::fs::create_dir_all(&server_dir)?;
    extract_zip(&zip_path, &server_dir)?;

    // Clean up zip
    let _ = std::fs::remove_file(&zip_path);

    // Replace Canary's default map with our blank one
    // Canary looks for data/world/<mapName>.otbm
    let canary_world = server_dir.join("data").join("world");
    if canary_world.exists() {
        // Remove default map files (usually "world.otbm" and overlays)
        for entry in std::fs::read_dir(&canary_world)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "otbm") {
                let _ = std::fs::remove_file(&path);
            }
        }
    } else {
        std::fs::create_dir_all(&canary_world)?;
    }

    // Copy our blank map into the Canary world dir
    let our_map = world_dir.join(format!("{}.otbm", map_name));
    if our_map.exists() {
        std::fs::copy(&our_map, canary_world.join(format!("{}.otbm", map_name)))?;
    }

    // Copy spawn.xml and houses.xml
    let our_spawn = world_dir.join("spawn.xml");
    if our_spawn.exists() {
        std::fs::copy(&our_spawn, canary_world.join("spawn.xml"))?;
    }
    let our_houses = world_dir.join("houses.xml");
    if our_houses.exists() {
        std::fs::copy(&our_houses, canary_world.join("houses.xml"))?;
    }

    // Update Canary's config.lua to use our map name
    let canary_config = server_dir.join("config.lua");
    if canary_config.exists() {
        let config = std::fs::read_to_string(&canary_config)?;
        let updated = config
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("mapName") {
                    format!("mapName = \"{}\"", map_name)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&canary_config, updated)?;
    }

    tracing::info!("Canary server set up in {}", server_dir.display());
    Ok(())
}

// ── OTClient setup ───────────────────────────────────────────────────────────

fn fetch_and_setup_otclient(
    client_dir: &std::path::Path,
    data_dir: &std::path::Path,
    protocol: ProtocolVersion,
) -> anyhow::Result<()> {
    tracing::info!("Fetching latest OTClient release…");

    // Try mehah/otclient (most popular fork)
    let (url, tag) = fetch_latest_release_asset("mehah", "otclient", &|name: &str| {
        let lower = name.to_lowercase();
        lower.contains("windows") && (lower.ends_with(".zip") || lower.ends_with(".7z"))
    })?;

    tracing::info!("Downloading OTClient {} …", tag);
    let zip_path = client_dir
        .parent()
        .unwrap_or(client_dir)
        .join("otclient-download.zip");
    std::fs::create_dir_all(client_dir)?;
    download_file(&url, &zip_path)?;

    // Only extract if it's a zip (skip .7z for now — no native support)
    if zip_path.extension().map_or(false, |e| e == "zip") {
        extract_zip(&zip_path, client_dir)?;
    }

    let _ = std::fs::remove_file(&zip_path);

    // Copy our asset files into OTClient's data directory
    // OTClient typically looks in data/things/<version>/
    let things_dir = client_dir.join("data").join("things");
    let version_dir = things_dir.join(match protocol {
        ProtocolVersion::V1340 => "1340",
        ProtocolVersion::V1290 => "1290",
        ProtocolVersion::V1098 => "1098",
        ProtocolVersion::V860 => "860",
    });
    std::fs::create_dir_all(&version_dir)?;

    match protocol.format() {
        AssetFormat::Protobuf => {
            // Copy catalog + appearances + sheet
            for name in ["catalog-content.json", "appearances.dat", "0.cip"] {
                let src = data_dir.join(name);
                if src.exists() {
                    std::fs::copy(&src, version_dir.join(name))?;
                }
            }
        }
        AssetFormat::Legacy => {
            // Copy Tibia.dat and Tibia.spr
            for name in ["Tibia.dat", "Tibia.spr"] {
                let src = data_dir.join(name);
                if src.exists() {
                    std::fs::copy(&src, version_dir.join(name))?;
                }
            }
        }
    }

    tracing::info!("OTClient set up in {}", client_dir.display());
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Fill a rectangle in an RGBA pixel buffer.
fn fill_sprite_rect(
    pixels: &mut [u8],
    sheet_width: u32,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    color: [u8; 4],
) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let idx = ((y * sheet_width + x) * 4) as usize;
            pixels[idx..idx + 4].copy_from_slice(&color);
        }
    }
}

/// Set a single pixel in an RGBA buffer.
fn set_pixel(pixels: &mut [u8], sheet_width: u32, x: u32, y: u32, color: [u8; 4]) {
    let idx = ((y * sheet_width + x) * 4) as usize;
    pixels[idx..idx + 4].copy_from_slice(&color);
}

/// Return the path the user should scan after creation to open the project.
/// Called from app.rs when the "Open Project" button is clicked in the Done step.
impl NewProjectWizard {
    pub fn take_result_path(&mut self) -> Option<PathBuf> {
        self.result_path.take()
    }
}
