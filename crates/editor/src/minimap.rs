//! Minimap panel — bird's-eye view of the current z-level with click-to-navigate.

use crate::state::EditorState;
use crate::theme;
use egui::{Color32, Pos2, Rect, Sense};

/// Default minimap render size in pixels.
const MINIMAP_SIZE: f32 = 200.0;

/// Show the minimap as a floating window.
pub fn show(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_minimap {
        return;
    }

    let mut open = true;

    egui::Window::new("Minimap")
        .open(&mut open)
        .default_pos([ctx.screen_rect().right() - MINIMAP_SIZE - 30.0, 80.0])
        .resizable(true)
        .collapsible(true)
        .default_size([MINIMAP_SIZE, MINIMAP_SIZE + 30.0])
        .show(ctx, |ui| {
            let Some(ref map) = state.map_data else {
                ui.label(egui::RichText::new("No map loaded").color(theme::TEXT_MUTED));
                return;
            };

            let z = state.camera.z_level;
            let Some((x1, y1, x2, y2)) = map.xy_extents(z) else {
                ui.label(egui::RichText::new("No tiles on this level").color(theme::TEXT_MUTED));
                return;
            };

            let map_w = (x2 - x1 + 1) as f32;
            let map_h = (y2 - y1 + 1) as f32;

            // Available space
            let avail = ui.available_size();
            let panel_size = avail.x.min(avail.y).clamp(100.0, 400.0);

            // Scale to fit
            let scale = panel_size / map_w.max(map_h);
            let render_w = map_w * scale;
            let render_h = map_h * scale;

            let (resp, painter) =
                ui.allocate_painter(egui::vec2(render_w, render_h), Sense::click_and_drag());

            let origin = resp.rect.min;

            // Dark background
            painter.rect_filled(resp.rect, 0.0, Color32::from_rgb(16, 16, 20));

            // Render tiles as dots
            let tiles = map.get_tiles_in_area(x1, y1, x2, y2, z);
            for tile in &tiles {
                let px = (tile.x - x1) as f32 * scale;
                let py = (tile.y - y1) as f32 * scale;
                let tl = Pos2::new(origin.x + px, origin.y + py);
                let s = scale.max(1.0);
                let br = Pos2::new(tl.x + s, tl.y + s);

                let color = minimap_color(tile, &state.appearances);
                painter.rect_filled(Rect::from_min_max(tl, br), 0.0, color);
            }

            // Draw viewport rectangle
            let tile_px = state.camera.tile_size();
            // Estimate visible area in tiles (assume ~1200×800 viewport)
            let vis_w = 1200.0 / tile_px as f64;
            let vis_h = 800.0 / tile_px as f64;

            let cam_cx = state.camera.center_x;
            let cam_cy = state.camera.center_y;
            let vx1 = ((cam_cx - vis_w / 2.0 - x1 as f64) * scale as f64) as f32;
            let vy1 = ((cam_cy - vis_h / 2.0 - y1 as f64) * scale as f64) as f32;
            let vx2 = ((cam_cx + vis_w / 2.0 - x1 as f64) * scale as f64) as f32;
            let vy2 = ((cam_cy + vis_h / 2.0 - y1 as f64) * scale as f64) as f32;

            let view_rect = Rect::from_min_max(
                Pos2::new(origin.x + vx1, origin.y + vy1),
                Pos2::new(origin.x + vx2, origin.y + vy2),
            );

            painter.rect_stroke(
                view_rect,
                0.0,
                (1.5, Color32::from_rgb(255, 255, 80)),
                egui::StrokeKind::Outside,
            );

            // Click-to-navigate
            if let Some(click_pos) = resp.interact_pointer_pos() {
                if resp.clicked() || resp.dragged() {
                    let local_x = click_pos.x - origin.x;
                    let local_y = click_pos.y - origin.y;
                    let world_x = local_x / scale + x1 as f32;
                    let world_y = local_y / scale + y1 as f32;
                    state.camera.center_x = world_x as f64;
                    state.camera.center_y = world_y as f64;
                }
            }

            // Stats line
            ui.label(
                egui::RichText::new(format!(
                    "Z:{} | {}×{} tiles | {:.0},{:.0}",
                    z, map_w as u16, map_h as u16, state.camera.center_x, state.camera.center_y
                ))
                .size(9.5)
                .color(theme::TEXT_MUTED),
            );
        });

    if !open {
        state.show_minimap = false;
    }
}

fn minimap_color(
    tile: &pte_otbm::Tile,
    appearances: &Option<pte_appearances::LoadedAppearances>,
) -> Color32 {
    // Try automap color from ground appearance
    if let Some(ground_id) = tile.ground {
        if let Some(ref apps) = appearances {
            if let Some(app) = apps.get(pte_appearances::Category::Object, ground_id as u32) {
                if let Some(ref flags) = app.flags {
                    if let Some(ref automap) = flags.automap {
                        if let Some(c) = automap.color {
                            // Tibia uses a 6×6×6 RGB cube: c = R*36 + G*6 + B, each 0–5
                            let r = ((c / 36) % 6) as u8 * 51;
                            let g = ((c / 6) % 6) as u8 * 51;
                            let b = (c % 6) as u8 * 51;
                            return Color32::from_rgb(r, g, b);
                        }
                    }
                }
            }
        }
    }

    // Fallback: hash the ground ID for a stable color
    if let Some(gid) = tile.ground {
        let h = (gid as u32).wrapping_mul(2654435761) >> 16;
        Color32::from_rgb(
            (60 + (h & 0x3F)) as u8,
            (80 + ((h >> 6) & 0x3F)) as u8,
            (60 + ((h >> 12) & 0x3F)) as u8,
        )
    } else if !tile.items.is_empty() {
        Color32::from_rgb(80, 80, 90)
    } else {
        Color32::from_rgb(30, 30, 35)
    }
}
