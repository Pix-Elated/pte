//! Map switcher panel — browse and load maps from the active project.

use crate::asset_scanner::MapEntry;
use crate::state::EditorState;
use crate::theme;

/// Action returned by the map switcher panel.
pub enum MapSwitcherAction {
    None,
    /// Load a different map from the project.
    LoadMap(std::path::PathBuf),
    /// Create a new blank map in the project's world directory.
    CreateNew,
}

/// Show the map switcher as a window.
pub fn show(ctx: &egui::Context, state: &mut EditorState) -> MapSwitcherAction {
    if !state.show_map_switcher {
        return MapSwitcherAction::None;
    }

    let project = match &state.active_project {
        Some(p) => p.clone(),
        None => {
            state.show_map_switcher = false;
            return MapSwitcherAction::None;
        }
    };

    let mut action = MapSwitcherAction::None;
    let mut open = state.show_map_switcher;

    egui::Window::new("Map Switcher")
        .default_size([320.0, 400.0])
        .resizable(true)
        .collapsible(true)
        .open(&mut open)
        .show(ctx, |ui| {
            // Current map indicator
            if let Some(ref path) = state.map_path {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Current:")
                            .size(10.5)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(name)
                            .size(11.0)
                            .color(theme::SUCCESS)
                            .strong(),
                    );
                    if state.is_dirty() {
                        ui.label(
                            egui::RichText::new("(unsaved)")
                                .size(9.5)
                                .color(theme::WARNING),
                        );
                    }
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
            }

            // New map button
            if ui.button("+ New Blank Map").clicked() {
                action = MapSwitcherAction::CreateNew;
            }
            ui.add_space(8.0);

            // Map list by category
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    map_section(
                        ui,
                        "Main Map",
                        project.main_map.as_slice(),
                        theme::SUCCESS,
                        state,
                        &mut action,
                    );
                    map_section(
                        ui,
                        "Custom Overlays",
                        &project.custom_maps,
                        theme::ACCENT,
                        state,
                        &mut action,
                    );
                    map_section(
                        ui,
                        "Quest Maps",
                        &project.quest_maps,
                        egui::Color32::from_rgb(180, 140, 255),
                        state,
                        &mut action,
                    );
                    map_section(
                        ui,
                        "World Changes",
                        &project.world_change_maps,
                        egui::Color32::from_rgb(255, 180, 100),
                        state,
                        &mut action,
                    );
                    map_section(
                        ui,
                        "Events",
                        &project.event_maps,
                        egui::Color32::from_rgb(100, 200, 255),
                        state,
                        &mut action,
                    );
                    map_section(
                        ui,
                        "Other",
                        &project.other_maps,
                        theme::TEXT_MUTED,
                        state,
                        &mut action,
                    );
                });
        });

    state.show_map_switcher = open;
    action
}

fn map_section(
    ui: &mut egui::Ui,
    label: &str,
    maps: &[MapEntry],
    color: egui::Color32,
    state: &EditorState,
    action: &mut MapSwitcherAction,
) {
    if maps.is_empty() {
        return;
    }

    ui.horizontal(|ui| {
        ui.colored_label(color, "●");
        ui.label(
            egui::RichText::new(format!("{} ({})", label, maps.len()))
                .size(11.0)
                .color(theme::TEXT_SECONDARY)
                .strong(),
        );
    });
    ui.add_space(2.0);

    for entry in maps {
        let is_current = state.map_path.as_ref() == Some(&entry.path);

        ui.horizontal(|ui| {
            ui.add_space(16.0); // indent

            let text_color = if is_current {
                theme::SUCCESS
            } else {
                theme::TEXT_PRIMARY
            };

            let name = entry
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&entry.label);
            let size = format_size(entry.size);
            let display = format!("{} ({})", name, size);

            if is_current {
                // Current map — just show label, not clickable
                ui.label(
                    egui::RichText::new(format!("▸ {}", display))
                        .size(10.5)
                        .color(text_color),
                );
            } else {
                // Clickable to switch
                let resp = ui.add(
                    egui::Label::new(egui::RichText::new(&display).size(10.5).color(text_color))
                        .sense(egui::Sense::click()),
                );
                if resp.clicked() {
                    *action = MapSwitcherAction::LoadMap(entry.path.clone());
                }
                if resp.hovered() {
                    ui.painter().rect_stroke(
                        resp.rect.expand(1.0),
                        2.0,
                        egui::Stroke::new(0.5, theme::ACCENT),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        });
    }
    ui.add_space(6.0);
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
