//! Town Editor dialog — create, edit, delete, and navigate to towns.

use crate::state::EditorState;
use crate::theme;

/// Action the town editor wants the app to perform.
#[derive(Debug, Clone)]
pub enum TownAction {
    None,
    /// Navigate camera to a town's temple position.
    GoTo {
        x: u16,
        y: u16,
        z: u8,
    },
}

pub fn show(ctx: &egui::Context, state: &mut EditorState) -> TownAction {
    if !state.show_town_editor {
        return TownAction::None;
    }

    let mut action = TownAction::None;
    let mut open = true;

    egui::Window::new("Town Editor")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([380.0, 420.0])
        .show(ctx, |ui| {
            let Some(ref mut map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            // ── Add new town ──
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("New town:")
                        .size(11.0)
                        .color(theme::TEXT_SECONDARY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.town_new_name)
                        .desired_width(140.0)
                        .hint_text("Town name"),
                );
                let can_add = !state.town_new_name.trim().is_empty();
                if ui
                    .add_enabled(can_add, egui::Button::new("+ Add"))
                    .clicked()
                {
                    let next_id = map.towns.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                    let name = state.town_new_name.trim().to_string();
                    map.towns.push(pte_otbm::Town {
                        id: next_id,
                        name,
                        position: pte_otbm::Position {
                            x: state.camera.center_x as u16,
                            y: state.camera.center_y as u16,
                            z: state.camera.z_level,
                        },
                    });
                    state.town_new_name.clear();
                }
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // ── Town list ──
            if map.towns.is_empty() {
                ui.label(
                    egui::RichText::new("No towns defined. Add one above.")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                return;
            }

            let mut delete_idx: Option<usize> = None;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (idx, town) in map.towns.iter_mut().enumerate() {
                        ui.push_id(format!("town_{}", town.id), |ui| {
                            egui::Frame::NONE
                                .fill(theme::BG_SURFACE)
                                .corner_radius(egui::CornerRadius::same(4))
                                .inner_margin(egui::Margin::same(6))
                                .outer_margin(egui::Margin::symmetric(0, 2))
                                .show(ui, |ui| {
                                    // Header: ID + name
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("#{}", town.id))
                                                .size(10.0)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut town.name)
                                                .desired_width(160.0)
                                                .font(egui::TextStyle::Body),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("🗑")
                                                    .on_hover_text("Delete town")
                                                    .clicked()
                                                {
                                                    delete_idx = Some(idx);
                                                }
                                                if ui
                                                    .small_button("📍")
                                                    .on_hover_text("Go to town")
                                                    .clicked()
                                                {
                                                    action = TownAction::GoTo {
                                                        x: town.position.x,
                                                        y: town.position.y,
                                                        z: town.position.z,
                                                    };
                                                }
                                                if ui
                                                    .small_button("⊕")
                                                    .on_hover_text("Set temple to camera pos")
                                                    .clicked()
                                                {
                                                    town.position.x = state.camera.center_x as u16;
                                                    town.position.y = state.camera.center_y as u16;
                                                    town.position.z = state.camera.z_level;
                                                }
                                            },
                                        );
                                    });

                                    // Position row
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Temple:")
                                                .size(10.0)
                                                .color(theme::TEXT_MUTED),
                                        );

                                        let mut x = town.position.x as i32;
                                        let mut y = town.position.y as i32;
                                        let mut z = town.position.z as i32;

                                        ui.label(
                                            egui::RichText::new("X")
                                                .size(9.5)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut x)
                                                    .range(0..=65535)
                                                    .speed(1),
                                            )
                                            .changed()
                                        {
                                            town.position.x = x.clamp(0, 65535) as u16;
                                        }

                                        ui.label(
                                            egui::RichText::new("Y")
                                                .size(9.5)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut y)
                                                    .range(0..=65535)
                                                    .speed(1),
                                            )
                                            .changed()
                                        {
                                            town.position.y = y.clamp(0, 65535) as u16;
                                        }

                                        ui.label(
                                            egui::RichText::new("Z")
                                                .size(9.5)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut z)
                                                    .range(0..=41)
                                                    .speed(0.1),
                                            )
                                            .changed()
                                        {
                                            town.position.z = z.clamp(0, 41) as u8;
                                        }
                                    });
                                });
                        });
                    }
                });

            // Process deletions after the loop
            if let Some(idx) = delete_idx {
                map.towns.remove(idx);
            }
        });

    if !open {
        state.show_town_editor = false;
    }

    action
}
