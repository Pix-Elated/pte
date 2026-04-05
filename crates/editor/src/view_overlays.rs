//! View overlays — client box, item type highlights, light sources, shade, tooltips.
//!
//! All overlays are drawn in `render_overlays()` and called from the viewport.

use crate::state::EditorState;
use egui::{Color32, Pos2, Rect};
use pte_appearances as appearances;

/// Tibia client viewport size in tiles (visible area in the game client).
const CLIENT_VIEW_W: f64 = 15.0;
const CLIENT_VIEW_H: f64 = 11.0;

/// Draw the in-game client viewport rectangle centered on the camera.
pub fn draw_client_box(
    painter: &egui::Painter,
    state: &EditorState,
    world_to_screen: &dyn Fn(f64, f64) -> Pos2,
) {
    if !state.show_client_box {
        return;
    }

    let cx = state.camera.center_x;
    let cy = state.camera.center_y;
    let half_w = CLIENT_VIEW_W / 2.0;
    let half_h = CLIENT_VIEW_H / 2.0;

    let tl = world_to_screen(cx - half_w, cy - half_h);
    let br = world_to_screen(cx + half_w, cy + half_h);
    let rect = Rect::from_min_max(tl, br);

    painter.rect_stroke(
        rect,
        0.0,
        (2.0, Color32::from_rgba_unmultiplied(255, 200, 50, 180)),
        egui::StrokeKind::Inside,
    );

    // Label
    painter.text(
        Pos2::new(rect.min.x + 3.0, rect.min.y - 14.0),
        egui::Align2::LEFT_BOTTOM,
        "Client View",
        egui::FontId::proportional(10.0),
        Color32::from_rgba_unmultiplied(255, 200, 50, 200),
    );
}

/// Item type highlight colors.
pub fn highlight_color_for_item(
    item_id: u32,
    appearances: &Option<appearances::LoadedAppearances>,
    state: &EditorState,
) -> Option<Color32> {
    let apps = appearances.as_ref()?;
    let appearance = apps.get(appearances::Category::Object, item_id)?;
    let flags = appearance.flags.as_ref()?;

    if state.highlight_pickupable && flags.take.is_some() {
        return Some(Color32::from_rgba_unmultiplied(50, 200, 50, 40));
    }
    if state.highlight_moveable && flags.unmove.is_none() && flags.bank.is_none() {
        // Item is moveable if not explicitly unpassable and not ground
        return Some(Color32::from_rgba_unmultiplied(50, 50, 200, 40));
    }
    if state.highlight_blocking && flags.unpass.is_some() {
        return Some(Color32::from_rgba_unmultiplied(200, 50, 50, 40));
    }
    if state.highlight_hooks && flags.hook.is_some() {
        return Some(Color32::from_rgba_unmultiplied(200, 200, 50, 50));
    }

    None
}

/// Draw light source glow overlays.
pub fn draw_light_overlays(
    painter: &egui::Painter,
    tile: &pte_otbm::Tile,
    tile_rect: Rect,
    tile_px: f32,
    appearances: &Option<appearances::LoadedAppearances>,
    state: &EditorState,
) {
    if !state.show_light_overlay {
        return;
    }

    let apps = match appearances.as_ref() {
        Some(a) => a,
        None => return,
    };

    // Check ground
    if let Some(gid) = tile.ground {
        draw_light_for_item(painter, gid as u32, tile_rect, tile_px, apps);
    }
    // Check items
    for item in &tile.items {
        draw_light_for_item(painter, item.id as u32, tile_rect, tile_px, apps);
    }
}

fn draw_light_for_item(
    painter: &egui::Painter,
    item_id: u32,
    tile_rect: Rect,
    tile_px: f32,
    apps: &appearances::LoadedAppearances,
) {
    let appearance = match apps.get(appearances::Category::Object, item_id) {
        Some(a) => a,
        None => return,
    };

    let flags = match appearance.flags.as_ref() {
        Some(f) => f,
        None => return,
    };

    let light = match flags.light.as_ref() {
        Some(l) => l,
        None => return,
    };

    let brightness = light.brightness.unwrap_or(0) as f32;
    let color = light.color.unwrap_or(0) as u16;

    if brightness <= 0.0 {
        return;
    }

    // Tibia light color is an index into a light color table
    // Approximate: warm yellow for most, white for color 215
    let (r, g, b) = if color >= 200 {
        (255u8, 255, 220)
    } else if color >= 100 {
        (255, 200, 100)
    } else {
        (255, 180, 80)
    };

    let center = tile_rect.center();
    let radius = brightness * tile_px * 0.45;
    let alpha = (brightness * 8.0).min(60.0) as u8;

    painter.circle_filled(
        center,
        radius,
        Color32::from_rgba_unmultiplied(r, g, b, alpha),
    );
    painter.circle_stroke(
        center,
        radius,
        (0.5, Color32::from_rgba_unmultiplied(r, g, b, alpha / 2)),
    );

    // Inner bright spot
    painter.circle_filled(
        center,
        radius * 0.3,
        Color32::from_rgba_unmultiplied(r, g, b, alpha * 2),
    );
}

/// Shade non-selected areas (darken everything outside the selection).
pub fn draw_shade(
    painter: &egui::Painter,
    state: &EditorState,
    viewport_rect: Rect,
    world_to_screen: &dyn Fn(f64, f64) -> Pos2,
) {
    if !state.show_shade {
        return;
    }

    let Some(ref sel) = state.selection else {
        return;
    };

    let sel_tl = world_to_screen(sel.x1 as f64, sel.y1 as f64);
    let sel_br = world_to_screen(sel.x2 as f64 + 1.0, sel.y2 as f64 + 1.0);
    let sel_rect = Rect::from_min_max(sel_tl, sel_br);

    let shade_color = Color32::from_rgba_unmultiplied(0, 0, 0, 120);

    // Top strip
    if sel_rect.min.y > viewport_rect.min.y {
        painter.rect_filled(
            Rect::from_min_max(
                viewport_rect.min,
                Pos2::new(viewport_rect.max.x, sel_rect.min.y),
            ),
            0.0,
            shade_color,
        );
    }
    // Bottom strip
    if sel_rect.max.y < viewport_rect.max.y {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(viewport_rect.min.x, sel_rect.max.y),
                viewport_rect.max,
            ),
            0.0,
            shade_color,
        );
    }
    // Left strip
    let strip_top = sel_rect.min.y.max(viewport_rect.min.y);
    let strip_bot = sel_rect.max.y.min(viewport_rect.max.y);
    if sel_rect.min.x > viewport_rect.min.x {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(viewport_rect.min.x, strip_top),
                Pos2::new(sel_rect.min.x, strip_bot),
            ),
            0.0,
            shade_color,
        );
    }
    // Right strip
    if sel_rect.max.x < viewport_rect.max.x {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(sel_rect.max.x, strip_top),
                Pos2::new(viewport_rect.max.x, strip_bot),
            ),
            0.0,
            shade_color,
        );
    }
}

/// Show tooltip for the hovered tile.
pub fn draw_tooltip(ctx: &egui::Context, state: &EditorState) {
    if !state.show_tooltips {
        return;
    }

    let Some((hx, hy)) = state.hover_tile else {
        return;
    };
    let z = state.camera.z_level;

    let Some(ref map) = state.map_data else {
        return;
    };
    let Some(tile) = map.get_tile(hx, hy, z) else {
        return;
    };

    // Build tooltip text
    let mut lines = Vec::new();
    lines.push(format!("Tile: {}, {}, {}", hx, hy, z));

    if let Some(gid) = tile.ground {
        let name = item_name(gid as u32, &state.appearances);
        lines.push(format!("Ground: #{} {}", gid, name));
    }

    for item in &tile.items {
        let name = item_name(item.id as u32, &state.appearances);
        let mut extra = String::new();
        if let Some(aid) = item.action_id {
            extra.push_str(&format!(" aid={}", aid));
        }
        if let Some(uid) = item.unique_id {
            extra.push_str(&format!(" uid={}", uid));
        }
        lines.push(format!("  #{}{}{}", item.id, name, extra));
    }

    if let Some(hid) = tile.house_id {
        lines.push(format!("House: #{}", hid));
    }

    if tile.flags.protection_zone {
        lines.push("  [PZ]".into());
    }
    if tile.flags.no_pvp {
        lines.push("  [NoPvP]".into());
    }
    if tile.flags.pvp_zone {
        lines.push("  [PvP]".into());
    }
    if tile.flags.no_logout {
        lines.push("  [NoLog]".into());
    }

    let text = lines.join("\n");

    egui::show_tooltip_at_pointer(
        ctx,
        egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tile_tooltip_layer")),
        egui::Id::new("tile_tooltip"),
        |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(10.0)
                    .color(Color32::from_rgb(220, 220, 220)),
            );
        },
    );
}

fn item_name(item_id: u32, appearances: &Option<appearances::LoadedAppearances>) -> String {
    let Some(ref apps) = appearances else {
        return String::new();
    };
    let Some(appearance) = apps.get(appearances::Category::Object, item_id) else {
        return String::new();
    };
    if let Some(ref name) = appearance.name {
        format!(" ({})", name)
    } else {
        String::new()
    }
}

/// Draw spawn area circles on the viewport.
pub fn draw_spawn_overlays(
    painter: &egui::Painter,
    state: &EditorState,
    world_to_screen: &dyn Fn(f64, f64) -> Pos2,
    tile_px: f32,
) {
    if !state.show_spawns {
        return;
    }

    let z = state.camera.z_level;

    for spawn in &state.spawns {
        if spawn.center_z != z {
            continue;
        }

        let cx = spawn.center_x as f64 + 0.5;
        let cy = spawn.center_y as f64 + 0.5;
        let center = world_to_screen(cx, cy);
        let radius_px = spawn.radius as f32 * tile_px;

        // Semi-transparent circle fill + stroke
        painter.circle_filled(
            center,
            radius_px,
            Color32::from_rgba_unmultiplied(100, 200, 255, 15),
        );
        painter.circle_stroke(
            center,
            radius_px,
            (1.0, Color32::from_rgba_unmultiplied(100, 200, 255, 80)),
        );

        // Center marker
        painter.circle_filled(
            center,
            3.0,
            Color32::from_rgba_unmultiplied(100, 200, 255, 160),
        );

        // Creature dots
        for creature in &spawn.creatures {
            let cpos = world_to_screen(
                spawn.center_x as f64 + creature.offset_x as f64 + 0.5,
                spawn.center_y as f64 + creature.offset_y as f64 + 0.5,
            );
            painter.circle_filled(
                cpos,
                2.5,
                Color32::from_rgba_unmultiplied(255, 150, 50, 180),
            );
        }
    }
}
