//! Map Statistics dialog — overview of the loaded map.

use crate::state::EditorState;
use crate::theme;
use std::collections::HashMap;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_stats_dialog {
        return;
    }

    let mut open = true;

    egui::Window::new("Map Statistics")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([360.0, 400.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let Some(ref map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            // -- General info --
            ui.heading("General");
            ui.horizontal(|ui| {
                stat_row(ui, "Map size", &format!("{}×{}", map.width, map.height));
            });
            if !map.description.is_empty() {
                ui.horizontal(|ui| {
                    stat_row(ui, "Description", &map.description);
                });
            }
            stat_row_ui(ui, "Spawn file", &map.spawn_file);
            stat_row_ui(ui, "House file", &map.house_file);

            ui.add_space(8.0);
            ui.separator();

            // -- Tile counts --
            ui.heading("Tiles");
            let total_tiles = map.tile_count();
            stat_row_ui(ui, "Total tiles", &format!("{}", total_tiles));
            stat_row_ui(ui, "Total chunks", &format!("{}", map.chunks.len()));

            // Per-z-level breakdown
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

            stat_row_ui(ui, "Ground tiles", &format!("{}", ground_count));
            stat_row_ui(ui, "Total items placed", &format!("{}", item_count));
            stat_row_ui(ui, "Unique item IDs", &format!("{}", unique_items.len()));
            stat_row_ui(ui, "Flagged tiles", &format!("{}", flagged_tiles));
            stat_row_ui(ui, "House tiles", &format!("{}", house_tiles));
            stat_row_ui(ui, "Towns", &format!("{}", map.towns.len()));
            stat_row_ui(ui, "Waypoints", &format!("{}", map.waypoints.len()));

            ui.add_space(8.0);
            ui.separator();

            // -- Z-Level breakdown --
            ui.heading("Tiles per Z-Level");
            let mut z_list: Vec<_> = z_counts.into_iter().collect();
            z_list.sort_by_key(|(z, _)| *z);

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

                            for (z, count) in &z_list {
                                let pct = if total_tiles > 0 {
                                    (*count as f64 / total_tiles as f64) * 100.0
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
            let mut item_list: Vec<_> = unique_items.into_iter().collect();
            item_list.sort_by(|a, b| b.1.cmp(&a.1));

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

                            for (id, count) in item_list.iter().take(20) {
                                ui.label(format!("{}", id));
                                ui.label(format!("{}", count));
                                ui.end_row();
                            }
                        });
                });
        });

    if !open {
        state.show_stats_dialog = false;
    }
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).size(10.0).color(theme::TEXT_MUTED));
    ui.label(egui::RichText::new(value).size(10.0).color(theme::TEXT_PRIMARY));
}

fn stat_row_ui(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        stat_row(ui, label, value);
    });
}
