//! Go-To Position dialog — jump the camera to a specific x, y, z coordinate.

use crate::state::EditorState;
use crate::theme;

/// Show the Go-To dialog as an egui::Window.
pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_goto_dialog {
        return;
    }

    let mut do_goto = false;
    let mut do_close = false;

    egui::Window::new("Go To Position")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([240.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("X:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut state.goto_x)
                        .desired_width(60.0)
                        .hint_text("0–65535"),
                );
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Y:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut state.goto_y)
                        .desired_width(60.0)
                        .hint_text("0–65535"),
                );
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Z:").color(theme::TEXT_SECONDARY));
                ui.add(
                    egui::TextEdit::singleline(&mut state.goto_z)
                        .desired_width(40.0)
                        .hint_text("0–41"),
                );
            });

            ui.add_space(6.0);

            ui.horizontal(|ui| {
                if ui.button("Go").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_goto = true;
                }
                if ui.button("Cancel").clicked() {
                    do_close = true;
                }
            });

            // Show current position for reference
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Current: {:.0}, {:.0}, {}",
                    state.camera.center_x, state.camera.center_y, state.camera.z_level
                ))
                .size(9.5)
                .color(theme::TEXT_MUTED),
            );
        });

    if do_goto {
        let x = state.goto_x.trim().parse::<f64>().unwrap_or(state.camera.center_x);
        let y = state.goto_y.trim().parse::<f64>().unwrap_or(state.camera.center_y);
        let z = state.goto_z.trim().parse::<u8>().unwrap_or(state.camera.z_level);

        crate::nav_history::record(state);
        state.camera.center_x = x.clamp(0.0, 65535.0);
        state.camera.center_y = y.clamp(0.0, 65535.0);
        state.camera.z_level = z.clamp(state.z_min, state.z_max);
        state.show_goto_dialog = false;
    }

    if do_close {
        state.show_goto_dialog = false;
    }
}
