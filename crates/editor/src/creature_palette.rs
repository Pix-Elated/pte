//! Creature palette — browse and select creatures from loaded spawn data.

use crate::state::EditorState;
use crate::theme;

/// An entry in the creature palette (derived from spawn data).
#[derive(Debug, Clone)]
pub struct CreatureEntry {
    pub name: String,
    pub is_npc: bool,
    pub spawn_time: u32,
}

/// Rebuild the creature list from loaded spawn data.
pub fn rebuild_creature_list(state: &mut EditorState) {
    let mut entries: Vec<CreatureEntry> = state
        .spawns
        .iter()
        .flat_map(|s| {
            s.creatures.iter().map(|c| CreatureEntry {
                name: c.name.clone(),
                is_npc: c.is_npc,
                spawn_time: c.spawn_time,
            })
        })
        .collect();

    // Deduplicate by name (keep first occurrence)
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries.dedup_by(|a, b| a.name == b.name);
    state.creature_list = entries;
}

pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_creature_palette {
        return;
    }

    let mut open = true;
    egui::Window::new("Creature Palette")
        .open(&mut open)
        .default_size([280.0, 400.0])
        .resizable(true)
        .show(ctx, |ui| {
            // Filter
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Filter:")
                        .color(theme::TEXT_SECONDARY)
                        .size(10.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut state.creature_filter)
                        .desired_width(140.0)
                        .hint_text("Search creatures…"),
                );
                if ui.small_button("✕").clicked() {
                    state.creature_filter.clear();
                }
            });

            let total = state.creature_list.len();
            let filter_lower = state.creature_filter.to_lowercase();

            // Stats
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let monsters = state.creature_list.iter().filter(|c| !c.is_npc).count();
                let npcs = total - monsters;
                ui.label(
                    egui::RichText::new(format!(
                        "{} creatures ({} monsters, {} NPCs)",
                        total, monsters, npcs
                    ))
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
                );
            });

            ui.separator();

            // Spawn management buttons
            ui.horizontal(|ui| {
                if ui.button("Add Spawn Here").clicked() {
                    if let Some(hover) = state.hover_tile {
                        state.spawns.push(crate::spawn_xml::Spawn {
                            center_x: hover.0,
                            center_y: hover.1,
                            center_z: state.camera.z_level,
                            radius: state.spawn_radius,
                            creatures: Vec::new(),
                        });
                    }
                }
                let spawn_count = state.spawns.len();
                ui.label(
                    egui::RichText::new(format!("{} spawn areas", spawn_count))
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                );
            });

            ui.separator();

            // Creature list
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in &state.creature_list {
                    if !filter_lower.is_empty()
                        && !entry.name.to_lowercase().contains(&filter_lower)
                    {
                        continue;
                    }

                    let icon = if entry.is_npc { "👤" } else { "🐀" };
                    let label = format!("{} {}", icon, entry.name);

                    let response = ui.selectable_label(false, &label);
                    if response.clicked() {
                        // Set as active creature for placement
                        state.active_tool = crate::state::ToolType::Creature;
                        state.waypoint_name = entry.name.clone(); // reuse field for creature name
                    }
                    response.on_hover_text(format!(
                        "Type: {}\nSpawn time: {}s",
                        if entry.is_npc { "NPC" } else { "Monster" },
                        entry.spawn_time,
                    ));
                }
            });
        });

    if !open {
        state.show_creature_palette = false;
    }
}
