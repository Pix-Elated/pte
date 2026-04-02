//! Preferences dialog — editor settings.

use crate::state::EditorState;
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_preferences {
        return;
    }

    let mut open = true;

    egui::Window::new("Preferences")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([420.0, 350.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("General");
            ui.add_space(4.0);

            egui::Grid::new("prefs_general_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Undo Limit:").size(11.0).color(theme::TEXT_SECONDARY));
                    let mut limit = state.undo_limit as i32;
                    if ui.add(egui::DragValue::new(&mut limit).range(10..=1000).speed(1)).changed() {
                        state.undo_limit = limit.clamp(10, 1000) as usize;
                    }
                    ui.end_row();

                    ui.label(egui::RichText::new("Autosave:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.autosave_enabled, "Enabled");
                    ui.end_row();

                    ui.label(egui::RichText::new("Interval (sec):").size(11.0).color(theme::TEXT_SECONDARY));
                    let mut interval = state.autosave_interval_secs as i32;
                    if ui.add(egui::DragValue::new(&mut interval).range(30..=3600).speed(10)).changed() {
                        state.autosave_interval_secs = interval.clamp(30, 3600) as u32;
                    }
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.heading("View");
            ui.add_space(4.0);

            egui::Grid::new("prefs_view_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Animations:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.animate_sprites, "Play animations");
                    ui.end_row();

                    ui.label(egui::RichText::new("Ghost floors:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.show_ghost_floors, "Show adjacent z-levels");
                    ui.end_row();

                    ui.label(egui::RichText::new("Show grid:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.show_grid, "Tile grid lines");
                    ui.end_row();

                    ui.label(egui::RichText::new("Show tooltips:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.show_tooltips, "Item info on hover");
                    ui.end_row();

                    ui.label(egui::RichText::new("Client box:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.show_client_box, "Show ingame viewport");
                    ui.end_row();

                    ui.label(egui::RichText::new("Light overlay:").size(11.0).color(theme::TEXT_SECONDARY));
                    ui.checkbox(&mut state.show_light_overlay, "Show light sources");
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.heading("Highlighting");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.checkbox(&mut state.highlight_pickupable, "Pickupable");
                ui.checkbox(&mut state.highlight_moveable, "Moveable");
                ui.checkbox(&mut state.highlight_blocking, "Blocking");
                ui.checkbox(&mut state.highlight_hooks, "Wall Hooks");
            });
        });

    if !open {
        state.show_preferences = false;
    }
}
