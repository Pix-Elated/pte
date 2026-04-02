//! Z-level (layer) panel.

use crate::state::EditorState;
use crate::theme;

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) {
    let z_min = state.z_min;
    let z_max = state.z_max;
    let z_surface = state.z_surface;

    // Quick-jump row
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if compact_btn(ui, &format!("Top {}", z_min)) {
            state.camera.z_level = z_min;
        }
        if compact_btn(ui, &format!("Gnd {}", z_surface)) {
            state.camera.z_level = z_surface;
        }
        if compact_btn(ui, &format!("Bot {}", z_max)) {
            state.camera.z_level = z_max;
        }
    });

    ui.add_space(4.0);

    // Occupied z-levels
    let occupied: std::collections::HashSet<u8> = state
        .map_data
        .as_ref()
        .map(|m| m.occupied_z_levels().into_iter().collect())
        .unwrap_or_default();

    egui::ScrollArea::vertical()
        .max_height(ui.available_height())
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for z in z_min..=z_max {
                let selected = state.camera.z_level == z;
                let has_tiles = occupied.contains(&z);

                let label = z_level_label(z, z_surface);
                let text_color = if selected {
                    egui::Color32::WHITE
                } else if has_tiles {
                    theme::TEXT_PRIMARY
                } else {
                    theme::TEXT_MUTED
                };

                let response = ui.add(
                    egui::SelectableLabel::new(
                        selected,
                        egui::RichText::new(&label)
                            .size(11.0)
                            .color(text_color),
                    ),
                );
                if response.clicked() {
                    state.camera.z_level = z;
                }
            }
        });
}

fn compact_btn(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(10.0))
            .min_size(egui::vec2(0.0, 20.0))
    ).clicked()
}

/// Human-readable label for a z-level.
pub fn z_level_label(z: u8, surface: u8) -> String {
    if z == surface {
        format!(" {:>2}  Ground / Sea Level", z)
    } else if z < surface {
        format!(" {:>2}  Sky +{}", z, surface - z)
    } else {
        format!(" {:>2}  Underground -{}", z, z - surface)
    }
}
