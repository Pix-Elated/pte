//! Pixelated's Tibia Editor — native Rust application.

// Hide the console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod api_handler;
mod api_server;
mod asset_scanner;
mod brush_palette;
mod brushes;
mod clipboard;
mod context_menu;
mod find_replace;
mod goto_dialog;
mod house_brush;
mod house_palette;
mod item_properties;
mod layers;
mod map_cleanup;
mod map_stats;
mod minimap;
mod nav_history;
mod properties;
mod sprite_detail;
mod sprite_editor;
mod sprite_picker;
mod state;
mod status_bar;
mod theme;
mod toolbar;
mod tools;
mod town_editor;
mod viewport;
mod waypoint_palette;
mod zone_brush;
mod hotkeys;
mod map_properties;
mod minimap_export;
mod new_map;
mod new_project;
mod perf_monitor;
mod preferences;
mod selection_ops;
mod shortcuts_panel;
mod tile_stack;
mod view_overlays;
mod spawn_xml;
mod creature_palette;
mod map_import;
mod map_switcher;
mod selective_eraser;
mod editor_config;
#[allow(dead_code)]
mod icons;
mod updater;

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Pixelated's Tibia Editor");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Pixelated's Tibia Editor v{}", updater::CURRENT_VERSION))
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Pixelated's Tibia Editor",
        options,
        Box::new(|cc| {
            crate::theme::apply(&cc.egui_ctx);
            crate::icons::register_font(&cc.egui_ctx);
            Ok(Box::new(app::MapEditorApp::new(cc)))
        }),
    )
}
