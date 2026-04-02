//! Waypoint Palette — list, navigate to, and manage waypoints.

use crate::state::EditorState;
use crate::theme;

/// Action the waypoint palette wants the app to perform.
#[derive(Debug, Clone)]
pub enum WaypointAction {
    None,
    /// Navigate camera to a waypoint position.
    GoTo { x: u16, y: u16, z: u8 },
}

pub fn show(ctx: &egui::Context, state: &mut EditorState) -> WaypointAction {
    if !state.show_waypoint_palette {
        return WaypointAction::None;
    }

    let mut action = WaypointAction::None;
    let mut open = true;

    egui::Window::new("Waypoints")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([320.0, 380.0])
        .show(ctx, |ui| {
            let Some(ref mut map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            // ── Search filter ──
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔍").size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut state.waypoint_filter)
                        .desired_width(180.0)
                        .hint_text("Filter waypoints..."),
                );
                if ui.button("Clear").clicked() {
                    state.waypoint_filter.clear();
                }
            });

            ui.add_space(4.0);

            let filter_lower = state.waypoint_filter.to_lowercase();

            // Count matches for the header
            let match_count = if filter_lower.is_empty() {
                map.waypoints.len()
            } else {
                map.waypoints
                    .iter()
                    .filter(|w| w.name.to_lowercase().contains(&filter_lower))
                    .count()
            };

            ui.label(
                egui::RichText::new(format!(
                    "{} waypoint{}",
                    match_count,
                    if match_count == 1 { "" } else { "s" }
                ))
                .size(10.0)
                .color(theme::TEXT_MUTED),
            );

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);

            if map.waypoints.is_empty() {
                ui.label(
                    egui::RichText::new("No waypoints. Use the Waypoint tool to place them.")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                return;
            }

            let mut delete_idx: Option<usize> = None;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (idx, wp) in map.waypoints.iter_mut().enumerate() {
                        // Apply filter
                        if !filter_lower.is_empty()
                            && !wp.name.to_lowercase().contains(&filter_lower)
                        {
                            continue;
                        }

                        ui.push_id(format!("wp_{idx}"), |ui| {
                            egui::Frame::NONE
                                .fill(theme::BG_SURFACE)
                                .corner_radius(egui::CornerRadius::same(3))
                                .inner_margin(egui::Margin::same(4))
                                .outer_margin(egui::Margin::symmetric(0, 1))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Editable name
                                        ui.add(
                                            egui::TextEdit::singleline(&mut wp.name)
                                                .desired_width(120.0)
                                                .font(egui::TextStyle::Small),
                                        );

                                        // Position display
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "({}, {}, {})",
                                                wp.position.x, wp.position.y, wp.position.z
                                            ))
                                            .size(9.5)
                                            .color(theme::TEXT_MUTED),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("🗑")
                                                    .on_hover_text("Delete waypoint")
                                                    .clicked()
                                                {
                                                    delete_idx = Some(idx);
                                                }
                                                if ui
                                                    .small_button("📍")
                                                    .on_hover_text("Go to waypoint")
                                                    .clicked()
                                                {
                                                    action = WaypointAction::GoTo {
                                                        x: wp.position.x,
                                                        y: wp.position.y,
                                                        z: wp.position.z,
                                                    };
                                                }
                                            },
                                        );
                                    });
                                });
                        });
                    }
                });

            if let Some(idx) = delete_idx {
                map.waypoints.remove(idx);
            }
        });

    if !open {
        state.show_waypoint_palette = false;
    }

    action
}
