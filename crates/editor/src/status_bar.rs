//! Status bar at the bottom of the editor.

use crate::state::EditorState;
use crate::theme;

pub fn show(ui: &mut egui::Ui, state: &EditorState) {
    let text_style = |s: &str| egui::RichText::new(s).size(10.5).color(theme::TEXT_MUTED);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;

        if let Some(ref apps) = state.appearances {
            ui.label(text_style(&format!("{} appearances", apps.total_count())));
        }

        if let Some(ref map) = state.map_data {
            ui.label(text_style(&format!("{} tiles", map.tile_count())));
        }

        ui.label(text_style(state.active_tool.label()));

        // Z-level
        let z = state.camera.z_level;
        let s = state.z_surface;
        let z_label = if z == s {
            format!("Z:{} Ground", z)
        } else if z < s {
            format!("Z:{} +{}", z, s - z)
        } else {
            format!("Z:{} -{}", z, z - s)
        };
        ui.label(text_style(&z_label));

        ui.label(text_style(&format!("{:.0}%", state.camera.zoom * 100.0)));

        // Selection size
        if let Some(ref sel) = state.selection {
            let w = sel.x2 - sel.x1 + 1;
            let h = sel.y2 - sel.y1 + 1;
            ui.label(text_style(&format!("sel:{}×{}", w, h)));
        }

        // Undo/Redo
        let undos = state.undo_cursor;
        let redos = state.undo_stack.len() - state.undo_cursor;
        if undos > 0 || redos > 0 {
            ui.label(
                egui::RichText::new(format!("undo:{} redo:{}", undos, redos))
                    .size(10.5)
                    .color(if undos > 0 { theme::WARNING } else { theme::TEXT_MUTED }),
            );
        }

        // Hover coordinates (right-aligned)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((hx, hy)) = state.hover_tile {
                ui.label(text_style(&format!("{}, {}, {}", hx, hy, state.camera.z_level)));
            }
        });
    });
}
