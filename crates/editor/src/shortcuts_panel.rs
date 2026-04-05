//! Keyboard shortcuts reference panel.

use crate::state::EditorState;
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_shortcuts {
        return;
    }

    let mut open = true;

    egui::Window::new("Keyboard Shortcuts")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([400.0, 500.0])
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    section(
                        ui,
                        "Tools",
                        &[
                            ("B", "Brush"),
                            ("E", "Eraser"),
                            ("G", "Fill (Bucket)"),
                            ("S", "Select"),
                            ("I", "Eyedropper"),
                            ("D", "Door brush"),
                            ("C", "Creature brush"),
                            ("N", "Spawn brush"),
                            ("W", "Waypoint brush"),
                        ],
                    );

                    section(
                        ui,
                        "File",
                        &[
                            ("Ctrl+O", "Open map"),
                            ("Ctrl+S", "Save map"),
                            ("Ctrl+Shift+S", "Save as…"),
                        ],
                    );

                    section(
                        ui,
                        "Edit",
                        &[
                            ("Ctrl+Z", "Undo"),
                            ("Ctrl+Y", "Redo"),
                            ("Ctrl+C", "Copy selection"),
                            ("Ctrl+X", "Cut selection"),
                            ("Ctrl+V", "Paste (ghost mode)"),
                            ("Delete", "Delete selection"),
                        ],
                    );

                    section(
                        ui,
                        "Navigation",
                        &[
                            ("Ctrl+G", "Go to position"),
                            ("Ctrl+F", "Find/Replace items"),
                            ("Alt+←", "Navigate back"),
                            ("Alt+→", "Navigate forward"),
                            ("PageUp", "Z-level up"),
                            ("PageDown", "Z-level down"),
                            ("M", "Toggle minimap"),
                        ],
                    );

                    section(
                        ui,
                        "View",
                        &[
                            ("Ctrl+H", "Toggle house palette"),
                            ("Ctrl+P", "Toggle item properties"),
                            ("?", "This shortcuts panel"),
                            ("Scroll", "Zoom in/out"),
                            ("Middle-drag", "Pan camera"),
                            ("Ctrl+drag", "Pan camera (alt)"),
                        ],
                    );

                    section(
                        ui,
                        "Hotkeys",
                        &[
                            ("F1-F10", "Recall hotkey slot"),
                            ("Shift+F1-F10", "Save hotkey slot"),
                        ],
                    );

                    section(
                        ui,
                        "Selection",
                        &[
                            ("Ctrl+A", "Select all (z-level)"),
                            ("Escape", "Deselect / cancel paste"),
                            ("Arrow keys", "Nudge selection"),
                        ],
                    );
                });
        });

    if !open {
        state.show_shortcuts = false;
    }
}

fn section(ui: &mut egui::Ui, title: &str, shortcuts: &[(&str, &str)]) {
    ui.label(
        egui::RichText::new(title)
            .size(12.0)
            .color(theme::ACCENT)
            .strong(),
    );
    ui.add_space(2.0);

    egui::Grid::new(format!("shortcuts_{}", title))
        .num_columns(2)
        .spacing([16.0, 3.0])
        .show(ui, |ui| {
            for (key, desc) in shortcuts {
                ui.label(
                    egui::RichText::new(*key)
                        .size(10.5)
                        .color(theme::TEXT_PRIMARY)
                        .strong()
                        .background_color(theme::BG_SURFACE),
                );
                ui.label(
                    egui::RichText::new(*desc)
                        .size(10.5)
                        .color(theme::TEXT_SECONDARY),
                );
                ui.end_row();
            }
        });

    ui.add_space(8.0);
}
