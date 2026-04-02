//! Generate a new blank map with user-specified dimensions.

use crate::state::EditorState;
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_new_map_dialog {
        return;
    }

    let mut open = true;

    egui::Window::new("New Map")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size([300.0, 0.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Grid::new("new_map_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Width:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::DragValue::new(&mut state.new_map_w)
                            .range(64..=65535)
                            .speed(10),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("Height:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::DragValue::new(&mut state.new_map_h)
                            .range(64..=65535)
                            .speed(10),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new("Description:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_map_desc)
                            .desired_width(160.0)
                            .hint_text("My new map"),
                    );
                    ui.end_row();
                });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    let mut map = pte_otbm::MapData::default();
                    map.width = state.new_map_w as u16;
                    map.height = state.new_map_h as u16;
                    map.description = state.new_map_desc.clone();

                    state.map_data = Some(map);
                    state.map_path = None;
                    state.camera.center_x = state.new_map_w as f64 / 2.0;
                    state.camera.center_y = state.new_map_h as f64 / 2.0;
                    state.camera.z_level = crate::state::MAP_SURFACE_Z;
                    state.show_new_map_dialog = false;
                    state.mode = crate::state::EditorMode::MapEditor;
                }
                if ui.button("Cancel").clicked() {
                    state.show_new_map_dialog = false;
                }
            });
        });

    if !open {
        state.show_new_map_dialog = false;
    }
}
