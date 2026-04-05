//! Z-level (layer) panel — shows occupied floors with clean labels.

use crate::state::{EditorState, MAP_MAX_Z};
use crate::theme;

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) {
    let z_surface = state.z_surface;
    let current_z = state.camera.z_level;

    // Collect occupied z-levels from the map
    let occupied: std::collections::BTreeSet<u8> = state
        .map_data
        .as_ref()
        .map(|m| m.occupied_z_levels().into_iter().collect())
        .unwrap_or_default();

    // Build the display list: occupied floors + always show surface + current
    let mut visible: Vec<u8> = occupied.iter().copied().collect();
    if !visible.contains(&z_surface) {
        visible.push(z_surface);
    }
    if !visible.contains(&current_z) {
        visible.push(current_z);
    }
    visible.sort_unstable();
    visible.dedup();

    // Quick-jump row
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if let Some(&first) = visible.first() {
            if compact_btn(
                ui,
                &format!("Top ({})", z_level_label_short(first, z_surface)),
            ) {
                state.camera.z_level = first;
            }
        }
        if compact_btn(ui, "Ground") {
            state.camera.z_level = z_surface;
        }
        if let Some(&last) = visible.last() {
            if last != z_surface {
                if compact_btn(
                    ui,
                    &format!("Bot ({})", z_level_label_short(last, z_surface)),
                ) {
                    state.camera.z_level = last;
                }
            }
        }
    });

    ui.add_space(4.0);

    // Add floor button
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if compact_btn(ui, "+ Above") {
            let new_z = current_z.saturating_sub(1);
            state.camera.z_level = new_z;
            // Ensure z_min/z_max encompass the new floor
            if new_z < state.z_min {
                state.z_min = new_z;
            }
        }
        if compact_btn(ui, "+ Below") {
            let new_z = (current_z + 1).min(MAP_MAX_Z);
            state.camera.z_level = new_z;
            if new_z > state.z_max {
                state.z_max = new_z;
            }
        }
    });

    ui.add_space(4.0);

    // Floor list
    egui::ScrollArea::vertical()
        .max_height(ui.available_height())
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let mut prev_z: Option<u8> = None;

            for &z in &visible {
                // Show gap indicator if floors were skipped
                if let Some(prev) = prev_z {
                    if z > prev + 1 {
                        let gap = z - prev - 1;
                        ui.label(
                            egui::RichText::new(format!("  ··· {} empty ···", gap))
                                .size(9.0)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                }
                prev_z = Some(z);

                let selected = current_z == z;
                let has_tiles = occupied.contains(&z);
                let is_surface = z == z_surface;

                let label = z_level_label(z, z_surface);
                let text_color = if selected {
                    egui::Color32::WHITE
                } else if is_surface {
                    theme::ACCENT
                } else if has_tiles {
                    theme::TEXT_PRIMARY
                } else {
                    theme::TEXT_MUTED
                };

                let tile_count = state
                    .map_data
                    .as_ref()
                    .map(|m| {
                        m.chunks
                            .iter()
                            .filter(|(k, _)| k.z == z)
                            .map(|(_, c)| c.len())
                            .sum::<usize>()
                    })
                    .unwrap_or(0);

                let display = if tile_count > 0 {
                    format!("{}  ({})", label, tile_count)
                } else {
                    format!("{}  (empty)", label)
                };

                ui.horizontal(|ui| {
                    let response = ui.add(egui::SelectableLabel::new(
                        selected,
                        egui::RichText::new(&display).size(11.0).color(text_color),
                    ));
                    if response.clicked() {
                        state.camera.z_level = z;
                    }
                });
            }
        });
}

fn compact_btn(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(text).size(10.0)).min_size(egui::vec2(0.0, 20.0)))
        .clicked()
}

/// Short label for quick-jump buttons.
fn z_level_label_short(z: u8, surface: u8) -> String {
    if z == surface {
        "Ground".to_string()
    } else if z < surface {
        format!("+{}", surface - z)
    } else {
        format!("-{}", z - surface)
    }
}

/// Human-readable label for a z-level.
pub fn z_level_label(z: u8, surface: u8) -> String {
    if z == surface {
        "Ground".to_string()
    } else if z < surface {
        let above = surface - z;
        format!("+{}", above)
    } else {
        let below = z - surface;
        format!("-{}", below)
    }
}
