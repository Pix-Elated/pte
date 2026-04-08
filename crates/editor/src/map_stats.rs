//! Map Statistics dialog — overview of the loaded map.

use crate::state::EditorState;
use crate::theme;
use std::collections::HashMap;

/// Cached statistics for the map stats dialog.
/// Computed once when the dialog opens, displayed every frame.
pub struct MapStatsCache {
    pub width: u16,
    pub height: u16,
    pub description: String,
    pub spawn_file: String,
    pub house_file: String,
    pub total_tiles: usize,
    pub total_chunks: usize,
    pub ground_count: usize,
    pub item_count: usize,
    pub unique_items_count: usize,
    pub flagged_tiles: usize,
    pub house_tiles: usize,
    pub town_count: usize,
    pub waypoint_count: usize,
    pub z_list: Vec<(u8, usize)>,
    pub top_items: Vec<(u16, usize)>,
}

impl MapStatsCache {
    pub fn compute(map: &pte_otbm::MapData) -> Self {
        let total_tiles = map.tile_count();
        let total_chunks = map.chunks.len();

        let mut z_counts: HashMap<u8, usize> = HashMap::new();
        let mut ground_count: usize = 0;
        let mut item_count: usize = 0;
        let mut unique_items: HashMap<u16, usize> = HashMap::new();
        let mut flagged_tiles: usize = 0;
        let mut house_tiles: usize = 0;

        for chunk in map.chunks.values() {
            for tile in chunk.values() {
                *z_counts.entry(tile.z).or_default() += 1;
                if tile.ground.is_some() {
                    ground_count += 1;
                }
                if let Some(gid) = tile.ground {
                    *unique_items.entry(gid).or_default() += 1;
                }
                for item in &tile.items {
                    item_count += 1;
                    *unique_items.entry(item.id).or_default() += 1;
                }
                if tile.flags.any() {
                    flagged_tiles += 1;
                }
                if tile.house_id.is_some() {
                    house_tiles += 1;
                }
            }
        }

        let unique_items_count = unique_items.len();

        let mut z_list: Vec<_> = z_counts.into_iter().collect();
        z_list.sort_by_key(|(z, _)| *z);

        let mut top_items: Vec<_> = unique_items.into_iter().collect();
        top_items.sort_by(|a, b| b.1.cmp(&a.1));
        top_items.truncate(20);

        Self {
            width: map.width,
            height: map.height,
            description: map.description.clone(),
            spawn_file: map.spawn_file.clone(),
            house_file: map.house_file.clone(),
            total_tiles,
            total_chunks,
            ground_count,
            item_count,
            unique_items_count,
            flagged_tiles,
            house_tiles,
            town_count: map.towns.len(),
            waypoint_count: map.waypoints.len(),
            z_list,
            top_items,
        }
    }
}

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_stats_dialog {
        // Clear cache when dialog is closed
        state.stats_cache = None;
        return;
    }

    // Compute stats on first frame the dialog is open
    if state.stats_cache.is_none() {
        if let Some(ref map) = state.map_data {
            state.stats_cache = Some(MapStatsCache::compute(map));
        }
    }

    let mut open = true;
    let mut needs_refresh = false;

    egui::Window::new("Map Statistics")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([360.0, 400.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let Some(ref stats) = state.stats_cache else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            // Refresh button — deferred to avoid borrow conflict
            if ui.button("Refresh").clicked() {
                needs_refresh = true;
            }
            ui.add_space(4.0);

            // -- General info --
            ui.heading("General");
            ui.horizontal(|ui| {
                stat_row(ui, "Map size", &format!("{}x{}", stats.width, stats.height));
            });
            if !stats.description.is_empty() {
                ui.horizontal(|ui| {
                    stat_row(ui, "Description", &stats.description);
                });
            }
            stat_row_ui(ui, "Spawn file", &stats.spawn_file);
            stat_row_ui(ui, "House file", &stats.house_file);

            ui.add_space(8.0);
            ui.separator();

            // -- Tile counts --
            ui.heading("Tiles");
            stat_row_ui(ui, "Total tiles", &format!("{}", stats.total_tiles));
            stat_row_ui(ui, "Total chunks", &format!("{}", stats.total_chunks));
            stat_row_ui(ui, "Ground tiles", &format!("{}", stats.ground_count));
            stat_row_ui(ui, "Total items placed", &format!("{}", stats.item_count));
            stat_row_ui(ui, "Unique item IDs", &format!("{}", stats.unique_items_count));
            stat_row_ui(ui, "Flagged tiles", &format!("{}", stats.flagged_tiles));
            stat_row_ui(ui, "House tiles", &format!("{}", stats.house_tiles));
            stat_row_ui(ui, "Towns", &format!("{}", stats.town_count));
            stat_row_ui(ui, "Waypoints", &format!("{}", stats.waypoint_count));

            ui.add_space(8.0);
            ui.separator();

            // -- Z-Level breakdown --
            ui.heading("Tiles per Z-Level");

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("z_stats_grid")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Z").strong().size(10.0));
                            ui.label(egui::RichText::new("Tiles").strong().size(10.0));
                            ui.label(egui::RichText::new("%").strong().size(10.0));
                            ui.end_row();

                            for (z, count) in &stats.z_list {
                                let pct = if stats.total_tiles > 0 {
                                    (*count as f64 / stats.total_tiles as f64) * 100.0
                                } else {
                                    0.0
                                };
                                ui.label(format!("{}", z));
                                ui.label(format!("{}", count));
                                ui.label(format!("{:.1}%", pct));
                                ui.end_row();
                            }
                        });
                });

            ui.add_space(8.0);
            ui.separator();

            // -- Top 20 most used items --
            ui.heading("Top 20 Most Used Items");

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("item_stats_grid")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Item ID").strong().size(10.0));
                            ui.label(egui::RichText::new("Count").strong().size(10.0));
                            ui.end_row();

                            for (id, count) in &stats.top_items {
                                ui.label(format!("{}", id));
                                ui.label(format!("{}", count));
                                ui.end_row();
                            }
                        });
                });
        });

    // Handle deferred refresh (after the borrow on stats_cache is released)
    if needs_refresh {
        if let Some(ref map) = state.map_data {
            state.stats_cache = Some(MapStatsCache::compute(map));
        }
    }

    if !open {
        state.show_stats_dialog = false;
        state.stats_cache = None;
    }
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(10.0)
            .color(theme::TEXT_MUTED),
    );
    ui.label(
        egui::RichText::new(value)
            .size(10.0)
            .color(theme::TEXT_PRIMARY),
    );
}

fn stat_row_ui(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        stat_row(ui, label, value);
    });
}
