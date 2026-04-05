//! Map import — merge tiles from another .otbm into the current map with an offset.

use crate::state::{EditorState, UndoAction};
use crate::theme;

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_import_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Import / Merge Map")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_size([320.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Merge tiles from another .otbm file into the current map.")
                    .size(11.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("Offset X:");
                ui.add(egui::DragValue::new(&mut state.import_offset_x).range(-65535..=65535));
            });
            ui.horizontal(|ui| {
                ui.label("Offset Y:");
                ui.add(egui::DragValue::new(&mut state.import_offset_y).range(-65535..=65535));
            });

            ui.add_space(8.0);

            let has_map = state.map_data.is_some();
            if ui
                .add_enabled(has_map, egui::Button::new("Choose .otbm to import…"))
                .clicked()
            {
                do_import(state);
            }
        });

    if !open {
        state.show_import_dialog = false;
    }
}

fn do_import(state: &mut EditorState) {
    let dialog = rfd::FileDialog::new()
        .add_filter("OTBM Map", &["otbm"])
        .set_title("Select map to import");

    let Some(path) = dialog.pick_file() else {
        return;
    };
    let import_map = match pte_otbm::parse_otbm(&path) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to load import map: {e:#}");
            return;
        }
    };

    let Some(ref mut map) = state.map_data else {
        return;
    };

    let ox = state.import_offset_x;
    let oy = state.import_offset_y;

    let mut undo_before = Vec::new();
    let mut undo_after = Vec::new();
    let mut count = 0usize;

    // Iterate all tiles in the imported map and merge them
    for chunk in import_map.chunks.values() {
        for tile in chunk.values() {
            let nx = (tile.x as i32 + ox).max(0) as u16;
            let ny = (tile.y as i32 + oy).max(0) as u16;
            let nz = tile.z;

            undo_before.push((nx, ny, nz, map.get_tile(nx, ny, nz).cloned()));

            let mut new_tile = tile.clone();
            new_tile.x = nx;
            new_tile.y = ny;
            map.set_tile(new_tile);

            undo_after.push((nx, ny, nz, map.get_tile(nx, ny, nz).cloned()));
            count += 1;
        }
    }

    // Merge towns (skip duplicates by town ID)
    let existing_town_ids: std::collections::HashSet<u32> =
        map.towns.iter().map(|t| t.id).collect();
    let mut towns_added = 0usize;
    for town in &import_map.towns {
        if !existing_town_ids.contains(&town.id) {
            let mut t = town.clone();
            t.position.x = (t.position.x as i32 + ox).max(0) as u16;
            t.position.y = (t.position.y as i32 + oy).max(0) as u16;
            map.towns.push(t);
            towns_added += 1;
        }
    }

    // Merge waypoints (skip duplicates by name)
    let existing_wp_names: std::collections::HashSet<String> =
        map.waypoints.iter().map(|w| w.name.clone()).collect();
    let mut wps_added = 0usize;
    for wp in &import_map.waypoints {
        if !existing_wp_names.contains(&wp.name) {
            let mut w = wp.clone();
            w.position.x = (w.position.x as i32 + ox).max(0) as u16;
            w.position.y = (w.position.y as i32 + oy).max(0) as u16;
            map.waypoints.push(w);
            wps_added += 1;
        }
    }

    // Release the mutable borrow on map_data before accessing other state fields
    let _ = map;

    if !undo_before.is_empty() {
        state.push_undo(UndoAction {
            tiles_before: undo_before,
            tiles_after: undo_after,
        });
    }

    // Merge spawns from the imported map's spawn.xml (if it exists alongside)
    let spawn_path = path.with_file_name(if import_map.spawn_file.is_empty() {
        "spawn.xml".to_string()
    } else {
        import_map.spawn_file.clone()
    });
    let mut spawns_added = 0usize;
    if spawn_path.exists() {
        if let Ok(import_spawns) = crate::spawn_xml::read_spawns(&spawn_path) {
            for mut spawn in import_spawns {
                spawn.center_x = (spawn.center_x as i32 + ox).max(0) as u16;
                spawn.center_y = (spawn.center_y as i32 + oy).max(0) as u16;
                state.spawns.push(spawn);
                spawns_added += 1;
            }
        }
    }

    tracing::info!(
        "Imported {} tiles, {} towns, {} waypoints, {} spawns from {}",
        count,
        towns_added,
        wps_added,
        spawns_added,
        path.display()
    );
    state.show_import_dialog = false;
}
