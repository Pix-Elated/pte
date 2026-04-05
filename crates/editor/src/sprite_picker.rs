//! Sprite picker panel — searchable, paginated grid of appearances.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use pte_appearances::{Appearance, Category};

use crate::state::{CategoryFilter, EditorState, ObjectSubcategory};
use crate::theme;
use crate::viewport::resolve_appearance_sprite;

const ITEMS_PER_PAGE: usize = 200;
const CELL_SIZE: f32 = 48.0;
const CELL_PADDING: f32 = 2.0;

/// Check whether an appearance matches the given subcategory filter.
fn matches_subcategory(appearance: &Appearance, subcat: ObjectSubcategory) -> bool {
    if subcat == ObjectSubcategory::All {
        return true;
    }
    let flags = match &appearance.flags {
        Some(f) => f,
        None => return subcat == ObjectSubcategory::Other,
    };

    match subcat {
        ObjectSubcategory::All => true,
        ObjectSubcategory::Ground => flags.bank.is_some() || flags.fullbank.is_some(),
        ObjectSubcategory::Borders => flags.bottom.is_some() && flags.bank.is_none(),
        ObjectSubcategory::Walls => {
            flags.unpass.is_some() && flags.bank.is_none() && flags.bottom.is_none()
        }
        ObjectSubcategory::Containers => {
            flags.container.is_some() || flags.liquidcontainer.is_some()
        }
        ObjectSubcategory::Equipment => flags.clothes.is_some(),
        ObjectSubcategory::Decoration => {
            flags.deco_kit.is_some()
                || flags.lying_object.is_some()
                || flags.market.as_ref().is_some_and(|m| {
                    m.category == Some(pte_appearances::proto::ItemCategory::Decoration as i32)
                })
        }
        ObjectSubcategory::Pickupable => flags.take.is_some(),
        ObjectSubcategory::Stackable => flags.cumulative.is_some(),
        ObjectSubcategory::Hangable => {
            flags.hang.is_some() || flags.hook_south.is_some() || flags.hook_east.is_some()
        }
        ObjectSubcategory::Liquid => flags.liquidpool.is_some(),
        ObjectSubcategory::Usable => {
            flags.usable.is_some() || flags.multiuse.is_some() || flags.forceuse.is_some()
        }
        ObjectSubcategory::Weapons => {
            if let Some(ref market) = flags.market {
                if let Some(cat) = market.category {
                    use pte_appearances::proto::ItemCategory;
                    matches!(
                        ItemCategory::try_from(cat).ok(),
                        Some(ItemCategory::Axes)
                            | Some(ItemCategory::Clubs)
                            | Some(ItemCategory::Swords)
                            | Some(ItemCategory::DistanceWeapons)
                            | Some(ItemCategory::WandsRods)
                            | Some(ItemCategory::FistWeapons)
                            | Some(ItemCategory::Ammunition)
                    )
                } else {
                    false
                }
            } else {
                flags.ammo.is_some()
            }
        }
        ObjectSubcategory::Light => flags.light.is_some(),
        ObjectSubcategory::Writable => flags.write.is_some() || flags.write_once.is_some(),
        ObjectSubcategory::Corpse => flags.corpse.is_some() || flags.player_corpse.is_some(),
        ObjectSubcategory::Other => {
            // "Other" = everything not matching any specific subcategory
            !(flags.bank.is_some()
                || flags.fullbank.is_some()
                || (flags.bottom.is_some() && flags.bank.is_none())
                || (flags.unpass.is_some() && flags.bank.is_none() && flags.bottom.is_none())
                || flags.container.is_some()
                || flags.liquidcontainer.is_some()
                || flags.clothes.is_some()
                || flags.take.is_some()
                || flags.cumulative.is_some()
                || flags.hang.is_some()
                || flags.hook_south.is_some()
                || flags.hook_east.is_some()
                || flags.liquidpool.is_some()
                || flags.usable.is_some()
                || flags.multiuse.is_some()
                || flags.forceuse.is_some()
                || flags.light.is_some()
                || flags.write.is_some()
                || flags.write_once.is_some()
                || flags.corpse.is_some()
                || flags.player_corpse.is_some()
                || flags.ammo.is_some()
                || flags.deco_kit.is_some()
                || flags.lying_object.is_some())
        }
    }
}

/// Draw the sprite picker panel.
pub fn show(ui: &mut egui::Ui, state: &mut EditorState) {
    let apps = match &state.appearances {
        Some(a) => a,
        None => {
            ui.label(
                egui::RichText::new("No appearances loaded")
                    .size(11.0)
                    .color(theme::TEXT_MUTED),
            );
            return;
        }
    };

    // Category tabs
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for cat in [
            CategoryFilter::Object,
            CategoryFilter::Outfit,
            CategoryFilter::Effect,
            CategoryFilter::Missile,
        ] {
            let selected = state.sprite_category == cat;
            let btn = egui::Button::new(egui::RichText::new(cat.label()).size(10.5).color(
                if selected {
                    Color32::WHITE
                } else {
                    theme::TEXT_SECONDARY
                },
            ))
            .fill(if selected {
                theme::ACCENT
            } else {
                theme::BG_RAISED
            })
            .corner_radius(egui::CornerRadius::same(3))
            .min_size(egui::vec2(0.0, 20.0));
            if ui.add(btn).clicked() {
                state.sprite_category = cat;
                state.sprite_page = 0;
            }
        }
    });

    ui.add_space(4.0);

    // Search bar
    if ui
        .add(
            egui::TextEdit::singleline(&mut state.sprite_search)
                .desired_width(ui.available_width())
                .hint_text("Search by ID or name…"),
        )
        .changed()
    {
        state.sprite_page = 0;
    }

    ui.add_space(4.0);

    // Direction selector for outfits
    if state.sprite_category == CategoryFilter::Outfit {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Dir")
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
            );
            for (dir, label) in [(2, "S"), (0, "N"), (1, "E"), (3, "W")] {
                let selected = state.sprite_preview_direction == dir;
                let btn =
                    egui::Button::new(egui::RichText::new(label).size(10.0).color(if selected {
                        Color32::WHITE
                    } else {
                        theme::TEXT_SECONDARY
                    }))
                    .fill(if selected {
                        theme::ACCENT
                    } else {
                        Color32::TRANSPARENT
                    })
                    .min_size(egui::vec2(22.0, 18.0));
                if ui.add(btn).clicked() {
                    state.sprite_preview_direction = dir;
                }
            }
        });
        ui.add_space(4.0);
    }

    // Object subcategory filter (only for Objects tab)
    if state.sprite_category == CategoryFilter::Object {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Type")
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
            );
            egui::ComboBox::from_id_salt("obj_subcat")
                .width(ui.available_width() - 4.0)
                .selected_text(egui::RichText::new(state.object_subcategory.label()).size(10.0))
                .show_ui(ui, |ui| {
                    for &subcat in ObjectSubcategory::ALL_SUBCATEGORIES {
                        if ui
                            .selectable_label(
                                state.object_subcategory == subcat,
                                egui::RichText::new(subcat.label()).size(10.0),
                            )
                            .clicked()
                        {
                            state.object_subcategory = subcat;
                            state.sprite_page = 0;
                        }
                    }
                });
        });
        ui.add_space(4.0);
    }

    // Get filtered appearances
    let category = match state.sprite_category {
        CategoryFilter::Object => Category::Object,
        CategoryFilter::Outfit => Category::Outfit,
        CategoryFilter::Effect => Category::Effect,
        CategoryFilter::Missile => Category::Missile,
    };

    let all_items = apps.get_all(category);
    let search_lower = state.sprite_search.to_lowercase();
    let subcat = state.object_subcategory;
    let is_object = state.sprite_category == CategoryFilter::Object;

    let filtered: Vec<_> = all_items
        .into_iter()
        .filter(|a| {
            // Apply subcategory filter for objects
            if is_object && subcat != ObjectSubcategory::All && !matches_subcategory(a, subcat) {
                return false;
            }
            // Apply text search
            if !search_lower.is_empty() {
                let id_match =
                    a.id.is_some_and(|id| id.to_string().contains(&search_lower));
                let name_match = a
                    .name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(&search_lower));
                if !id_match && !name_match {
                    return false;
                }
            }
            true
        })
        .collect();

    let total_items = filtered.len();
    let total_pages = total_items.div_ceil(ITEMS_PER_PAGE);
    let page = state.sprite_page.min(total_pages.saturating_sub(1));
    let start = page * ITEMS_PER_PAGE;
    let page_items = &filtered[start..(start + ITEMS_PER_PAGE).min(total_items)];

    // Count and pagination
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{} items", total_items))
                .size(10.0)
                .color(theme::TEXT_MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let arrow_btn = |ui: &mut egui::Ui, text: &str| -> bool {
                ui.add(
                    egui::Button::new(egui::RichText::new(text).size(10.0))
                        .min_size(egui::vec2(20.0, 18.0)),
                )
                .clicked()
            };
            if arrow_btn(ui, "\u{25b6}") && page + 1 < total_pages {
                state.sprite_page = page + 1;
            }
            ui.label(
                egui::RichText::new(format!("{}/{}", page + 1, total_pages.max(1)))
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
            );
            if arrow_btn(ui, "\u{25c0}") && page > 0 {
                state.sprite_page = page - 1;
            }
        });
    });

    ui.add_space(2.0);

    // Grid of sprites
    let available_width = ui.available_width();
    let cols = ((available_width + CELL_PADDING) / (CELL_SIZE + CELL_PADDING))
        .floor()
        .max(1.0) as usize;

    let current_selected = state.effective_selected_id();
    let current_tab = state.active_tab;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let grid = egui::Grid::new("sprite_grid")
                .spacing([CELL_PADDING, CELL_PADDING])
                .min_col_width(CELL_SIZE);

            grid.show(ui, |ui| {
                for (i, appearance) in page_items.iter().enumerate() {
                    let id = appearance.id.unwrap_or(0);
                    let direction = match state.sprite_category {
                        CategoryFilter::Outfit => state.sprite_preview_direction,
                        _ => 0,
                    };
                    let anim_time_ms = (ui.ctx().input(|i| i.time) * 1000.0) as u64;
                    let sprite_id = resolve_appearance_sprite(
                        appearance,
                        direction,
                        state.animate_sprites,
                        anim_time_ms,
                    );
                    let selected = current_selected == Some(id);

                    // Frame color
                    let frame_color = if selected {
                        theme::ACCENT
                    } else {
                        theme::BORDER
                    };

                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(CELL_SIZE, CELL_SIZE), egui::Sense::click());

                    // Background
                    let bg_color = if selected {
                        theme::ACCENT_MUTED
                    } else if response.hovered() {
                        theme::BG_RAISED
                    } else {
                        theme::BG_SURFACE
                    };

                    ui.painter().rect_filled(rect, 4.0, bg_color);
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        (1.0, frame_color),
                        egui::StrokeKind::Outside,
                    );

                    // Try to draw sprite texture
                    if let Some(sid) = sprite_id {
                        if let Some(tex) = crate::viewport::get_or_upload(
                            &mut state.sprite_textures,
                            &state.sprite_sheets,
                            ui.ctx(),
                            sid,
                        ) {
                            let inner = rect.shrink(3.0);
                            ui.painter().image(
                                tex.id(),
                                inner,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                Color32::WHITE,
                            );
                        } else {
                            // Fallback: color based on ID hash
                            let hue = (sid as f32 * 137.508) % 360.0;
                            let rgb = hsl_to_rgb(hue, 0.5, 0.35);
                            let inner = rect.shrink(6.0);
                            ui.painter().rect_filled(
                                inner,
                                2.0,
                                Color32::from_rgb(rgb.0, rgb.1, rgb.2),
                            );
                        }
                    }

                    // ID label at bottom
                    let label_pos = rect.center_bottom() - egui::vec2(0.0, 10.0);
                    ui.painter().text(
                        label_pos,
                        egui::Align2::CENTER_CENTER,
                        format!("{}", id),
                        egui::FontId::monospace(9.0),
                        Color32::from_gray(120),
                    );

                    // Click to select
                    if response.clicked() {
                        match current_tab {
                            crate::state::WorkspaceTab::SpriteViewer => {
                                state.viewer_selected_id = Some(id);
                            }
                            crate::state::WorkspaceTab::MapEditor => {
                                state.selected_item_id = Some(id);
                            }
                        }
                    }

                    // Double-click to open sprite editor
                    if response.double_clicked() {
                        match current_tab {
                            crate::state::WorkspaceTab::SpriteViewer => {
                                state.viewer_selected_id = Some(id);
                            }
                            crate::state::WorkspaceTab::MapEditor => {
                                state.selected_item_id = Some(id);
                            }
                        }
                        // Signal that we want to open the editor — handled by app
                        state.sprite_editor.open = false; // will be opened by app
                        state.sprite_editor.editing_sprite_id = Some(id);
                    }

                    // Tooltip
                    if response.hovered() {
                        let name = appearance.name.as_deref().unwrap_or("(unnamed)");
                        response.on_hover_text(format!("#{} {}", id, name));
                    }

                    // Row wrap
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });

            // Keep repainting for sprite animations
            if state.animate_sprites {
                ui.ctx().request_repaint();
            }
        });
}

/// Upload a sprite's pixel data to an egui texture.
pub fn upload_sprite_texture(
    ctx: &egui::Context,
    sprite_id: u32,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> TextureHandle {
    let image = ColorImage::from_rgba_unmultiplied([width as _, height as _], pixels);
    ctx.load_texture(
        format!("sprite_{}", sprite_id),
        image,
        TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Nearest,
            ..Default::default()
        },
    )
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h2 = h / 60.0;
    let x = c * (1.0 - ((h2 % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match h2 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}
