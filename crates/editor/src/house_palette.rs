//! House Palette — list, create, edit, delete houses, and paint house tiles.

use crate::state::EditorState;
use crate::theme;

/// Action the house palette wants the app to perform.
#[derive(Debug, Clone)]
pub enum HouseAction {
    None,
    /// Navigate camera to a house exit position.
    GoTo {
        x: u16,
        y: u16,
        z: u8,
    },
}

pub fn show(ctx: &egui::Context, state: &mut EditorState) -> HouseAction {
    if !state.show_house_palette {
        return HouseAction::None;
    }

    let mut action = HouseAction::None;
    let mut open = true;

    egui::Window::new("Houses")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_size([400.0, 480.0])
        .show(ctx, |ui| {
            let Some(ref mut map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            // ── Add new house ──
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("New:")
                        .size(11.0)
                        .color(theme::TEXT_SECONDARY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.house_new_name)
                        .desired_width(140.0)
                        .hint_text("House name"),
                );
                let can_add = !state.house_new_name.trim().is_empty();
                if ui
                    .add_enabled(can_add, egui::Button::new("+ Add"))
                    .clicked()
                {
                    let next_id = map.houses.iter().map(|h| h.id).max().unwrap_or(0) + 1;
                    let name = state.house_new_name.trim().to_string();
                    map.houses.push(pte_otbm::House {
                        id: next_id,
                        name,
                        rent: 0,
                        town_id: map.towns.first().map(|t| t.id).unwrap_or(0),
                        exit: pte_otbm::Position {
                            x: state.camera.center_x as u16,
                            y: state.camera.center_y as u16,
                            z: state.camera.z_level,
                        },
                    });
                    state.house_new_name.clear();
                    state.active_house_id = Some(next_id);
                }
            });

            // ── House brush indicator ──
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if let Some(hid) = state.active_house_id {
                    let name = map
                        .houses
                        .iter()
                        .find(|h| h.id == hid)
                        .map(|h| h.name.as_str())
                        .unwrap_or("???");
                    ui.label(
                        egui::RichText::new(format!("🖌 Painting: #{} {}", hid, name))
                            .size(11.0)
                            .color(theme::ACCENT)
                            .strong(),
                    );
                    if ui.small_button("Clear").clicked() {
                        state.active_house_id = None;
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Select a house below to start painting tiles")
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    );
                }
            });

            ui.add_space(4.0);

            // ── Search/filter ──
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔍").size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut state.house_filter)
                        .desired_width(160.0)
                        .hint_text("Filter houses..."),
                );
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);

            if map.houses.is_empty() {
                // Check for orphaned house IDs on tiles
                let orphans = map.orphan_house_ids();
                if orphans.is_empty() {
                    ui.label(
                        egui::RichText::new("No houses defined. Add one above.")
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!(
                            "⚠ {} orphaned house ID(s) on tiles: {:?}",
                            orphans.len(),
                            &orphans[..orphans.len().min(10)]
                        ))
                        .size(10.0)
                        .color(theme::WARNING),
                    );
                    if ui.button("Create entries for orphans").clicked() {
                        for &hid in &orphans {
                            map.houses.push(pte_otbm::House {
                                id: hid,
                                name: format!("House #{}", hid),
                                rent: 0,
                                town_id: map.towns.first().map(|t| t.id).unwrap_or(0),
                                exit: pte_otbm::Position { x: 0, y: 0, z: 7 },
                            });
                        }
                    }
                }
                return;
            }

            let filter_lower = state.house_filter.to_lowercase();
            let mut delete_idx: Option<usize> = None;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (idx, house) in map.houses.iter_mut().enumerate() {
                        // Apply filter
                        if !filter_lower.is_empty()
                            && !house.name.to_lowercase().contains(&filter_lower)
                            && !house.id.to_string().contains(&filter_lower)
                        {
                            continue;
                        }

                        let is_active = state.active_house_id == Some(house.id);

                        ui.push_id(format!("house_{}", house.id), |ui| {
                            let frame_fill = if is_active {
                                theme::ACCENT_MUTED
                            } else {
                                theme::BG_SURFACE
                            };

                            egui::Frame::NONE
                                .fill(frame_fill)
                                .corner_radius(egui::CornerRadius::same(4))
                                .inner_margin(egui::Margin::same(6))
                                .outer_margin(egui::Margin::symmetric(0, 2))
                                .show(ui, |ui| {
                                    // Header: ID + name + actions
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("#{}", house.id))
                                                .size(10.0)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(&mut house.name)
                                                .desired_width(120.0)
                                                .font(egui::TextStyle::Body),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("🗑")
                                                    .on_hover_text("Delete house")
                                                    .clicked()
                                                {
                                                    delete_idx = Some(idx);
                                                }
                                                if ui
                                                    .small_button("📍")
                                                    .on_hover_text("Go to exit")
                                                    .clicked()
                                                {
                                                    action = HouseAction::GoTo {
                                                        x: house.exit.x,
                                                        y: house.exit.y,
                                                        z: house.exit.z,
                                                    };
                                                }

                                                let paint_label = if is_active {
                                                    "■ Stop"
                                                } else {
                                                    "🖌 Paint"
                                                };
                                                if ui
                                                    .small_button(paint_label)
                                                    .on_hover_text("Select as active house brush")
                                                    .clicked()
                                                {
                                                    if is_active {
                                                        state.active_house_id = None;
                                                    } else {
                                                        state.active_house_id = Some(house.id);
                                                    }
                                                }
                                            },
                                        );
                                    });

                                    // Properties row
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Rent:")
                                                .size(9.5)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        let mut rent = house.rent as i64;
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut rent)
                                                    .range(0..=1_000_000)
                                                    .speed(10),
                                            )
                                            .changed()
                                        {
                                            house.rent = rent.max(0) as u32;
                                        }

                                        ui.add_space(8.0);

                                        ui.label(
                                            egui::RichText::new("Town:")
                                                .size(9.5)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        let mut tid = house.town_id as i64;
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut tid)
                                                    .range(0..=9999)
                                                    .speed(1),
                                            )
                                            .changed()
                                        {
                                            house.town_id = tid.max(0) as u32;
                                        }
                                    });

                                    // Exit position
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Exit:")
                                                .size(9.5)
                                                .color(theme::TEXT_MUTED),
                                        );
                                        let mut x = house.exit.x as i32;
                                        let mut y = house.exit.y as i32;
                                        let mut z = house.exit.z as i32;

                                        ui.label(
                                            egui::RichText::new("X")
                                                .size(9.0)
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
                                            house.exit.x = x.clamp(0, 65535) as u16;
                                        }
                                        ui.label(
                                            egui::RichText::new("Y")
                                                .size(9.0)
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
                                            house.exit.y = y.clamp(0, 65535) as u16;
                                        }
                                        ui.label(
                                            egui::RichText::new("Z")
                                                .size(9.0)
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
                                            house.exit.z = z.clamp(0, 41) as u8;
                                        }

                                        if ui
                                            .small_button("⊕")
                                            .on_hover_text("Set exit to camera pos")
                                            .clicked()
                                        {
                                            house.exit.x = state.camera.center_x as u16;
                                            house.exit.y = state.camera.center_y as u16;
                                            house.exit.z = state.camera.z_level;
                                        }
                                    });

                                    // Tile count (read-only computed)
                                    // We can't call map methods while iterating mut,
                                    // so we compute this info outside. Show placeholder.
                                });
                        });
                    }
                });

            // Process deletions — also clear house_id from tiles
            if let Some(idx) = delete_idx {
                let hid = map.houses[idx].id;
                // Clear house_id from all tiles with this house
                for chunk in map.chunks.values_mut() {
                    for tile in chunk.values_mut() {
                        if tile.house_id == Some(hid) {
                            tile.house_id = None;
                        }
                    }
                }
                map.houses.remove(idx);
                if state.active_house_id == Some(hid) {
                    state.active_house_id = None;
                }
            }
        });

    if !open {
        state.show_house_palette = false;
    }

    action
}
