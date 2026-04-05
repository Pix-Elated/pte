//! Sprite detail side panel — shows full info about a selected appearance.
//!
//! Opens when a sprite is clicked in the sprite picker. Shows:
//! - Large preview (animated if applicable)
//! - All frame groups / directions / animation frames
//! - Appearance flags and properties
//! - Quick actions: Edit, Duplicate, Delete, Export

use egui::{Color32, Vec2};
use pte_appearances::Category;

use crate::state::EditorState;
use crate::theme;
use crate::viewport::resolve_appearance_sprite;

const PREVIEW_SIZE: f32 = 128.0;
const FRAME_THUMB_SIZE: f32 = 40.0;

/// Show the sprite detail panel. Returns an action to perform (if any).
pub fn show(ui: &mut egui::Ui, state: &mut EditorState) -> DetailAction {
    let mut action = DetailAction::None;

    let item_id = match state.effective_selected_id() {
        Some(id) => id,
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new("Select a sprite")
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Click any item in the grid")
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            });
            return action;
        }
    };

    let apps = match &state.appearances {
        Some(a) => a,
        None => return action,
    };

    let category = match state.sprite_category {
        crate::state::CategoryFilter::Object => Category::Object,
        crate::state::CategoryFilter::Outfit => Category::Outfit,
        crate::state::CategoryFilter::Effect => Category::Effect,
        crate::state::CategoryFilter::Missile => Category::Missile,
    };

    let appearance = match apps.get(category, item_id) {
        Some(a) => a.clone(),
        None => {
            ui.label(
                egui::RichText::new(format!("#{} not found", item_id))
                    .size(11.0)
                    .color(theme::TEXT_MUTED),
            );
            return action;
        }
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Header ──
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("#{}", item_id))
                        .size(14.0)
                        .color(theme::ACCENT)
                        .strong(),
                );
                if let Some(ref name) = appearance.name {
                    ui.label(
                        egui::RichText::new(name)
                            .size(13.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                }
            });

            if let Some(ref desc) = appearance.description {
                ui.label(
                    egui::RichText::new(desc)
                        .size(10.5)
                        .color(theme::TEXT_SECONDARY),
                );
            }

            ui.add_space(8.0);

            // ── Large preview ──
            let direction = match state.sprite_category {
                crate::state::CategoryFilter::Outfit => state.sprite_preview_direction,
                _ => 0,
            };
            let anim_time_ms = (ui.ctx().input(|i| i.time) * 1000.0) as u64;
            let sprite_id = resolve_appearance_sprite(
                &appearance,
                direction,
                state.animate_sprites,
                anim_time_ms,
            );

            let (preview_rect, _) =
                ui.allocate_exact_size(Vec2::splat(PREVIEW_SIZE), egui::Sense::hover());

            // Checkerboard background for transparency
            draw_checkerboard(ui.painter(), preview_rect, 8.0);

            if let Some(sid) = sprite_id {
                if let Some(tex) = crate::viewport::get_or_upload(
                    &mut state.sprite_textures,
                    &state.sprite_sheets,
                    ui.ctx(),
                    sid,
                ) {
                    ui.painter().image(
                        tex.id(),
                        preview_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            }

            ui.add_space(8.0);

            // ── All frame group sprites ──
            if !appearance.frame_group.is_empty() {
                ui.label(
                    egui::RichText::new("FRAME GROUPS")
                        .size(10.0)
                        .color(theme::TEXT_MUTED)
                        .strong(),
                );
                ui.add_space(4.0);

                for (fg_idx, fg) in appearance.frame_group.iter().enumerate() {
                    if appearance.frame_group.len() > 1 {
                        ui.label(
                            egui::RichText::new(format!("Group {}", fg_idx))
                                .size(10.0)
                                .color(theme::TEXT_SECONDARY),
                        );
                    }

                    if let Some(ref sf) = fg.sprite_info {
                        let sprites_per_row = ((ui.available_width() + 2.0)
                            / (FRAME_THUMB_SIZE + 2.0))
                            .floor()
                            .max(1.0) as usize;

                        let sprite_ids: Vec<u32> = sf.sprite_id.to_vec();

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                            for (i, &sid) in sprite_ids.iter().enumerate() {
                                if i > 0 && i % sprites_per_row == 0 {
                                    ui.end_row();
                                }

                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::splat(FRAME_THUMB_SIZE),
                                    egui::Sense::click(),
                                );

                                let is_current = sprite_id == Some(sid);
                                let bg = if is_current {
                                    theme::ACCENT_MUTED
                                } else if resp.hovered() {
                                    theme::BG_RAISED
                                } else {
                                    theme::BG_SURFACE
                                };
                                let border = if is_current {
                                    theme::ACCENT
                                } else {
                                    theme::BORDER
                                };

                                ui.painter().rect_filled(rect, 3.0, bg);
                                ui.painter().rect_stroke(
                                    rect,
                                    3.0,
                                    (0.5, border),
                                    egui::StrokeKind::Outside,
                                );

                                if let Some(tex) = crate::viewport::get_or_upload(
                                    &mut state.sprite_textures,
                                    &state.sprite_sheets,
                                    ui.ctx(),
                                    sid,
                                ) {
                                    let inner = rect.shrink(2.0);
                                    ui.painter().image(
                                        tex.id(),
                                        inner,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                }

                                // Tooltip with sprite ID
                                resp.on_hover_text(format!("Sprite #{}", sid));
                            }
                        });

                        // Sprite info stats
                        ui.add_space(4.0);
                        let info_text = format!(
                            "{}×{} px  •  {} layers  •  {} dirs  •  {} frames  •  {} sprites",
                            sf.pattern_width.unwrap_or(1) * 32,
                            sf.pattern_height.unwrap_or(1) * 32,
                            sf.layers.unwrap_or(1),
                            sf.pattern_depth.unwrap_or(1),
                            sf.animation.as_ref().map_or(1, |a| a.sprite_phase.len()),
                            sprite_ids.len(),
                        );
                        ui.label(
                            egui::RichText::new(info_text)
                                .size(9.5)
                                .color(theme::TEXT_MUTED),
                        );

                        // Animation info
                        if let Some(ref anim) = sf.animation {
                            let loop_label = if anim.default_start_phase.is_some() {
                                "loop"
                            } else {
                                "once"
                            };
                            let phase_count = anim.sprite_phase.len();
                            ui.label(
                                egui::RichText::new(format!(
                                    "Animation: {} phases, {}",
                                    phase_count, loop_label
                                ))
                                .size(9.5)
                                .color(theme::TEXT_MUTED),
                            );
                        }
                    }
                }
            }

            ui.add_space(8.0);

            // ── Flags ──
            if let Some(ref flags) = appearance.flags {
                ui.label(
                    egui::RichText::new("FLAGS")
                        .size(10.0)
                        .color(theme::TEXT_MUTED)
                        .strong(),
                );
                ui.add_space(4.0);

                let mut flag_list: Vec<&str> = Vec::new();

                // Check all the boolean-ish flags via the protobuf fields
                macro_rules! check_flag {
                    ($field:ident, $name:expr) => {
                        if flags.$field.is_some() {
                            flag_list.push($name);
                        }
                    };
                }

                check_flag!(bank, "Ground/Bank");
                check_flag!(clip, "Clip");
                check_flag!(bottom, "Bottom");
                check_flag!(top, "Top");
                check_flag!(container, "Container");
                check_flag!(cumulative, "Cumulative");
                check_flag!(usable, "Usable");
                check_flag!(forceuse, "Force Use");
                check_flag!(multiuse, "Multi Use");
                check_flag!(write, "Writeable");
                check_flag!(write_once, "Write Once");
                check_flag!(liquidpool, "Liquid Pool");
                check_flag!(unpass, "Impassable");
                check_flag!(unmove, "Unmovable");
                check_flag!(unsight, "Block Sight");
                check_flag!(avoid, "Avoid");
                check_flag!(take, "Pickupable");
                check_flag!(hang, "Hangable");
                check_flag!(rotate, "Rotatable");
                check_flag!(light, "Light Source");
                check_flag!(dont_hide, "Always Visible");
                check_flag!(translucent, "Translucent");
                check_flag!(shift, "Shifted");
                check_flag!(height, "Has Height");
                check_flag!(lying_object, "Lying");
                check_flag!(animate_always, "Always Animated");
                check_flag!(automap, "Automap Color");
                check_flag!(lenshelp, "Lens Help");
                check_flag!(fullbank, "Full Ground");
                check_flag!(ignore_look, "Ignore Look");
                check_flag!(clothes, "Wearable");
                check_flag!(market, "Marketable");
                check_flag!(wrap, "Wrappable");
                check_flag!(unwrap, "Unwrappable");
                check_flag!(topeffect, "Top Effect");
                check_flag!(corpse, "Corpse");
                check_flag!(player_corpse, "Player Corpse");

                if !flags.npcsaledata.is_empty() {
                    flag_list.push("NPC Sale Data");
                }

                if flag_list.is_empty() {
                    ui.label(
                        egui::RichText::new("No flags set")
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    );
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                        for flag_name in &flag_list {
                            let badge = egui::Button::new(
                                egui::RichText::new(*flag_name)
                                    .size(9.5)
                                    .color(theme::TEXT_PRIMARY),
                            )
                            .fill(theme::BG_RAISED)
                            .corner_radius(egui::CornerRadius::same(2))
                            .min_size(egui::vec2(0.0, 16.0));
                            ui.add_enabled(false, badge);
                        }
                    });
                }

                // Specific numeric flags
                if let Some(ref bank) = flags.bank {
                    if let Some(waypoints) = bank.waypoints {
                        prop_row(ui, "Ground Speed", &waypoints.to_string());
                    }
                }
                if let Some(ref light) = flags.light {
                    if let Some(brightness) = light.brightness {
                        prop_row(
                            ui,
                            "Light",
                            &format!(
                                "brightness={} color={}",
                                brightness,
                                light.color.unwrap_or(0)
                            ),
                        );
                    }
                }
                if let Some(ref automap) = flags.automap {
                    if let Some(color) = automap.color {
                        prop_row(ui, "Automap Color", &color.to_string());
                    }
                }
            }

            ui.add_space(12.0);

            // ── Action buttons ──
            ui.label(
                egui::RichText::new("ACTIONS")
                    .size(10.0)
                    .color(theme::TEXT_MUTED)
                    .strong(),
            );
            ui.add_space(4.0);

            let btn_width = ui.available_width();

            let edit_btn = egui::Button::new(
                egui::RichText::new("Edit Sprite")
                    .size(12.0)
                    .color(Color32::WHITE),
            )
            .fill(theme::ACCENT)
            .stroke(egui::Stroke::new(1.0, theme::ACCENT_HOVER))
            .corner_radius(egui::CornerRadius::same(4))
            .min_size(egui::vec2(btn_width, 28.0));

            if ui.add(edit_btn).clicked() {
                action = DetailAction::EditSprite;
            }

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let half = (btn_width - 4.0) / 2.0;
                if action_btn(ui, "Duplicate", half).clicked() {
                    action = DetailAction::Duplicate;
                }
                if action_btn(ui, "Export PNG", half).clicked() {
                    action = DetailAction::ExportPng;
                }
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let half = (btn_width - 4.0) / 2.0;
                if action_btn(ui, "New Blank", half).clicked() {
                    action = DetailAction::NewBlank;
                }

                let del_btn =
                    egui::Button::new(egui::RichText::new("Delete").size(11.0).color(theme::ERROR))
                        .fill(theme::BG_SURFACE)
                        .stroke(egui::Stroke::new(0.5, theme::ERROR))
                        .corner_radius(egui::CornerRadius::same(3))
                        .min_size(egui::vec2(half, 24.0));
                if ui.add(del_btn).clicked() {
                    action = DetailAction::Delete;
                }
            });
        });

    action
}

/// Action returned from the detail panel for the parent to handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailAction {
    None,
    EditSprite,
    Duplicate,
    Delete,
    ExportPng,
    NewBlank,
}

fn action_btn(ui: &mut egui::Ui, label: &str, width: f32) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.0)
                .color(theme::TEXT_PRIMARY),
        )
        .fill(theme::BG_SURFACE)
        .stroke(egui::Stroke::new(0.5, theme::BORDER))
        .corner_radius(egui::CornerRadius::same(3))
        .min_size(egui::vec2(width, 24.0)),
    )
}

fn prop_row(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).size(10.0).color(theme::TEXT_MUTED));
        ui.label(
            egui::RichText::new(value)
                .size(10.0)
                .color(theme::TEXT_PRIMARY),
        );
    });
}

fn draw_checkerboard(painter: &egui::Painter, rect: egui::Rect, cell_size: f32) {
    let c1 = Color32::from_gray(35);
    let c2 = Color32::from_gray(50);

    let cols = ((rect.width() / cell_size).ceil() as usize).max(1);
    let rows = ((rect.height() / cell_size).ceil() as usize).max(1);

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 { c1 } else { c2 };
            let x = rect.min.x + col as f32 * cell_size;
            let y = rect.min.y + row as f32 * cell_size;
            let cell_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cell_size, cell_size))
                    .intersect(rect);
            painter.rect_filled(cell_rect, 0.0, color);
        }
    }
}
