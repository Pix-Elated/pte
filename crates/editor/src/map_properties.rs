//! Map Properties dialog — edit description, dimensions, spawn/house file paths.

use crate::state::EditorState;
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_map_props {
        return;
    }

    let mut open = true;

    egui::Window::new("Map Properties")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([380.0, 280.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let Some(ref mut map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            egui::Grid::new("map_props_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Description:")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut map.description)
                            .desired_width(240.0)
                            .desired_rows(3),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Width:")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    let mut w = map.width as i32;
                    if ui
                        .add(egui::DragValue::new(&mut w).range(1..=65535).speed(1))
                        .changed()
                    {
                        map.width = w.clamp(1, 65535) as u16;
                    }
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Height:")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    let mut h = map.height as i32;
                    if ui
                        .add(egui::DragValue::new(&mut h).range(1..=65535).speed(1))
                        .changed()
                    {
                        map.height = h.clamp(1, 65535) as u16;
                    }
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Spawn File:")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut map.spawn_file)
                            .desired_width(200.0)
                            .hint_text("spawns.xml"),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("House File:")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut map.house_file)
                            .desired_width(200.0)
                            .hint_text("houses.xml"),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("OTBM Version:")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}", map.version))
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Items (major):")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}", map.item_major_version))
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.end_row();

                    ui.label(
                        egui::RichText::new("Items (minor):")
                            .size(11.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}", map.item_minor_version))
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.end_row();
                });
        });

    if !open {
        state.show_map_props = false;
    }
}
