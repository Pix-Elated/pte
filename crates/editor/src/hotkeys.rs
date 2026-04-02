//! Hotkey system — 10 configurable position/brush hotkeys.

use crate::state::EditorState;
use crate::theme;

/// A single hotkey slot.
#[derive(Debug, Clone)]
pub struct HotkeySlot {
    /// Saved camera position (None = not set).
    pub position: Option<(f64, f64, u8)>,
    /// Saved brush/item to activate (None = position-only).
    pub item_id: Option<u32>,
    /// Label for display
    pub label: String,
}

impl Default for HotkeySlot {
    fn default() -> Self {
        Self {
            position: None,
            item_id: None,
            label: String::new(),
        }
    }
}

/// Save the current camera position + active brush to a hotkey slot.
pub fn save_hotkey(state: &mut EditorState, slot: usize) {
    if slot >= state.hotkeys.len() { return; }
    state.hotkeys[slot].position = Some((
        state.camera.center_x,
        state.camera.center_y,
        state.camera.z_level,
    ));
    state.hotkeys[slot].item_id = state.selected_item_id;
    state.hotkeys[slot].label = format!(
        "{:.0},{:.0},{}",
        state.camera.center_x, state.camera.center_y, state.camera.z_level
    );
}

/// Jump to a hotkey position and optionally activate its brush.
pub fn recall_hotkey(state: &mut EditorState, slot: usize) {
    if slot >= state.hotkeys.len() { return; }
    // Copy data out to avoid borrow conflict
    let position = state.hotkeys[slot].position;
    let item_id = state.hotkeys[slot].item_id;
    if let Some((x, y, z)) = position {
        crate::nav_history::record(state);
        state.camera.center_x = x;
        state.camera.center_y = y;
        state.camera.z_level = z;
    }
    if let Some(item_id) = item_id {
        state.selected_item_id = Some(item_id);
    }
}

/// Show the hotkey editor dialog.
pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_hotkey_editor {
        return;
    }

    let mut open = true;

    egui::Window::new("Hotkeys")
        .open(&mut open)
        .collapsible(true)
        .resizable(false)
        .default_size([360.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Ctrl+0-9 to save, 0-9 (with numpad) to recall")
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(4.0);

            egui::Grid::new("hotkey_grid")
                .num_columns(4)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    for i in 0..10 {
                        let key_label = format!("F{}", i + 1);
                        ui.label(
                            egui::RichText::new(&key_label)
                                .size(11.0)
                                .color(theme::ACCENT)
                                .strong(),
                        );

                        let hk = &state.hotkeys[i];
                        if let Some((x, y, z)) = hk.position {
                            ui.label(
                                egui::RichText::new(format!("{:.0}, {:.0}, {}", x, y, z))
                                    .size(10.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                            if let Some(item) = hk.item_id {
                                ui.label(
                                    egui::RichText::new(format!("Item #{}", item))
                                        .size(10.0)
                                        .color(theme::TEXT_MUTED),
                                );
                            } else {
                                ui.label(egui::RichText::new("—").size(10.0).color(theme::TEXT_MUTED));
                            }

                            if ui.small_button("Goto").clicked() {
                                recall_hotkey(state, i);
                            }
                        } else {
                            ui.label(
                                egui::RichText::new("(empty)")
                                    .size(10.0)
                                    .color(theme::TEXT_MUTED),
                            );
                            ui.label("");
                            ui.label("");
                        }

                        ui.end_row();
                    }
                });

            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Tip: F1-F10 to recall, Shift+F1-F10 to save")
                    .size(9.5)
                    .color(theme::TEXT_MUTED),
            );
        });

    if !open {
        state.show_hotkey_editor = false;
    }
}
