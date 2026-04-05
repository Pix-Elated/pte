//! Minimap PNG export.

use crate::state::EditorState;
use crate::theme;
use pte_appearances::{self as appearances, Category};

pub fn show_export_dialog(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_minimap_export {
        return;
    }

    let mut open = true;

    egui::Window::new("Export Minimap Image")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size([300.0, 0.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let Some(ref map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Z-level:")
                        .size(11.0)
                        .color(theme::TEXT_SECONDARY),
                );
                let mut z = state.minimap_export_z as i32;
                if ui
                    .add(egui::DragValue::new(&mut z).range(0..=41).speed(0.1))
                    .changed()
                {
                    state.minimap_export_z = z.clamp(0, 41) as u8;
                }
            });

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Scale:")
                        .size(11.0)
                        .color(theme::TEXT_SECONDARY),
                );
                egui::ComboBox::from_id_salt("minimap_scale")
                    .selected_text(format!("{}x", state.minimap_export_scale))
                    .show_ui(ui, |ui| {
                        for s in [1, 2, 4, 8] {
                            ui.selectable_value(
                                &mut state.minimap_export_scale,
                                s,
                                format!("{}x", s),
                            );
                        }
                    });
            });

            ui.add_space(8.0);

            if let Some(extents) = map.xy_extents(state.minimap_export_z) {
                let w = extents.2 as u32 - extents.0 as u32 + 1;
                let h = extents.3 as u32 - extents.1 as u32 + 1;
                let img_w = w * state.minimap_export_scale as u32;
                let img_h = h * state.minimap_export_scale as u32;
                ui.label(
                    egui::RichText::new(format!(
                        "Tile area: {}×{} → Image: {}×{} px",
                        w, h, img_w, img_h
                    ))
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
                );
            } else {
                ui.label(
                    egui::RichText::new("No tiles on this z-level")
                        .size(10.0)
                        .color(theme::WARNING),
                );
            }

            ui.add_space(8.0);

            if ui.button("Export PNG…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Save Minimap PNG")
                    .add_filter("PNG Image", &["png"])
                    .set_file_name("minimap.png")
                    .save_file()
                {
                    match export_minimap_png(
                        map,
                        &state.appearances,
                        state.minimap_export_z,
                        state.minimap_export_scale,
                        &path,
                    ) {
                        Ok(_) => {
                            state.minimap_export_result =
                                Some(format!("Saved to {}", path.display()))
                        }
                        Err(e) => state.minimap_export_result = Some(format!("Error: {}", e)),
                    }
                }
            }

            if let Some(ref result) = state.minimap_export_result {
                ui.add_space(4.0);
                let color = if result.starts_with("Error") {
                    theme::ERROR
                } else {
                    theme::SUCCESS
                };
                ui.label(egui::RichText::new(result).size(10.0).color(color));
            }
        });

    if !open {
        state.show_minimap_export = false;
    }
}

fn export_minimap_png(
    map: &pte_otbm::MapData,
    appearances: &Option<appearances::LoadedAppearances>,
    z: u8,
    scale: u8,
    path: &std::path::Path,
) -> Result<(), String> {
    let extents = map.xy_extents(z).ok_or("No tiles on this z-level")?;
    let (min_x, min_y, max_x, max_y) = extents;
    let w = (max_x - min_x + 1) as u32;
    let h = (max_y - min_y + 1) as u32;
    let scale = scale as u32;
    let img_w = w * scale;
    let img_h = h * scale;

    let mut pixels = vec![0u8; (img_w * img_h * 3) as usize];

    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let color = if let Some(tile) = map.get_tile(tx, ty, z) {
                minimap_color(tile, appearances)
            } else {
                [0u8, 0, 0]
            };

            let bx = (tx - min_x) as u32 * scale;
            let by = (ty - min_y) as u32 * scale;
            for sy in 0..scale {
                for sx in 0..scale {
                    let px = bx + sx;
                    let py = by + sy;
                    let idx = ((py * img_w + px) * 3) as usize;
                    if idx + 2 < pixels.len() {
                        pixels[idx] = color[0];
                        pixels[idx + 1] = color[1];
                        pixels[idx + 2] = color[2];
                    }
                }
            }
        }
    }

    // Write PNG using the image crate — we'll use a simple manual encoder
    write_png(path, img_w, img_h, &pixels)
}

fn minimap_color(
    tile: &pte_otbm::Tile,
    appearances: &Option<appearances::LoadedAppearances>,
) -> [u8; 3] {
    if let Some(ground_id) = tile.ground {
        if let Some(ref apps) = appearances {
            if let Some(appearance) = apps.get(Category::Object, ground_id as u32) {
                if let Some(ref flags) = appearance.flags {
                    if let Some(ref automap) = flags.automap {
                        if let Some(color_idx) = automap.color {
                            let idx = color_idx as u8;
                            let r = (idx / 36).min(5) * 51;
                            let g = ((idx / 6) % 6).min(5) * 51;
                            let b = (idx % 6).min(5) * 51;
                            return [r, g, b];
                        }
                    }
                }
            }
        }
        // Hash fallback
        let hue = (ground_id as f32 * 137.508) % 360.0;
        let (r, g, b) = hsl_to_rgb(hue, 0.35, 0.3);
        return [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8];
    }

    [20, 20, 35]
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
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
    (r1 + m, g1 + m, b1 + m)
}

/// Write raw RGB pixels to a PNG file.
fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> Result<(), String> {
    let img = image::RgbImage::from_raw(w, h, rgb.to_vec())
        .ok_or_else(|| "Failed to create image buffer".to_string())?;
    img.save(path).map_err(|e| e.to_string())
}
