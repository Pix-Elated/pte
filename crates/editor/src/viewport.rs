//! Map viewport — renders tiles on a pannable/zoomable canvas.

use egui::{Color32, Pos2, Rect};
use pte_appearances::{self as appearances, Category};

use crate::brushes::shape::BrushShape;
use crate::state::{EditorState, ToolType};

/// Draw the map viewport.
pub fn show(ui: &mut egui::Ui, state: &mut EditorState) {
    let map = match &state.map_data {
        Some(m) => m,
        None => {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading("No Map Loaded");
                    ui.add_space(12.0);
                    ui.label("Open a .otbm file from File \u{2192} Open Map or Open Chunk Directory");
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "Tip: Switch to the Sprite Viewer tab above to browse sprites without a map.",
                        )
                        .size(11.0)
                        .color(egui::Color32::from_rgb(140, 140, 160)),
                    );
                    ui.add_space(20.0);
                    if ui.button("Open Map...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("OTBM Map", &["otbm"])
                            .pick_file()
                        {
                            state.pending_map_load = Some(path);
                        }
                    }
                });
            });
            return;
        }
    };

    // Copy camera values to avoid borrow conflicts
    // First: animate zoom interpolation
    let dt = ui.input(|i| i.stable_dt);
    if state.camera.animate_zoom(dt) {
        ui.ctx().request_repaint(); // Keep animating
    }

    let cam_center_x = state.camera.center_x;
    let cam_center_y = state.camera.center_y;
    let cam_z = state.camera.z_level;
    let tile_px = state.camera.tile_size();
    let avail = ui.available_size();

    // Calculate visible tile range
    let half_w = avail.x / (2.0 * tile_px);
    let half_h = avail.y / (2.0 * tile_px);
    let vis_x1 = (cam_center_x - half_w as f64 - 1.0).max(0.0) as u16;
    let vis_y1 = (cam_center_y - half_h as f64 - 1.0).max(0.0) as u16;
    let vis_x2 = (cam_center_x + half_w as f64 + 1.0).min(65535.0) as u16;
    let vis_y2 = (cam_center_y + half_h as f64 + 1.0).min(65535.0) as u16;

    // LOD levels based on tile pixel size:
    //   LOD 0: tile_px < 0.5  → chunk overview (one colored block per 64×64 chunk)
    //   LOD 1: tile_px < 6    → minimap mode (colored rects per tile, no sprites)
    //   LOD 2: tile_px < 14   → ground-only (ground sprite, skip items/grid)
    //   LOD 3: tile_px >= 14  → full detail with items + grid
    let lod = if tile_px < 0.5 {
        0
    } else if tile_px < 6.0 {
        1
    } else if tile_px < 14.0 {
        2
    } else {
        3
    };

    // Allocate canvas
    let (response, painter) = ui.allocate_painter(avail, egui::Sense::click_and_drag());
    let canvas_rect = response.rect;

    // Dark background
    painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(17, 17, 34));

    // World-to-screen transform
    let world_to_screen = |wx: f64, wy: f64| -> Pos2 {
        let sx = ((wx - cam_center_x) * tile_px as f64) + (avail.x as f64 / 2.0);
        let sy = ((wy - cam_center_y) * tile_px as f64) + (avail.y as f64 / 2.0);
        Pos2::new(canvas_rect.min.x + sx as f32, canvas_rect.min.y + sy as f32)
    };

    let screen_to_world = |sx: f32, sy: f32| -> (f64, f64) {
        let wx = ((sx - canvas_rect.min.x) as f64 - avail.x as f64 / 2.0) / tile_px as f64
            + cam_center_x;
        let wy = ((sy - canvas_rect.min.y) as f64 - avail.y as f64 / 2.0) / tile_px as f64
            + cam_center_y;
        (wx, wy)
    };

    // Advance animation timer
    let dt = ui.input(|i| i.stable_dt) as f64;
    state.anim_time += dt;
    let anim_time_ms = (state.anim_time * 1000.0) as u64;
    let ctx = ui.ctx().clone();

    // Request repaint for animations
    if state.animate_sprites {
        ctx.request_repaint();
    }

    // ── Multi-z ghost floors (render floors ABOVE current z at reduced opacity) ──
    // Like RME: when above ground (z <= 7), render from ground up to current floor;
    // each floor gets a diagonal shift and fading opacity.
    // We render only 1-2 ghost floors for performance.
    if state.show_ghost_floors && lod >= 2 {
        let ghost_floors: Vec<(u8, u8)> = if cam_z > 0 && cam_z <= 7 {
            // Surface: show 1-2 floors below current (higher z = lower)
            let mut floors = Vec::new();
            if cam_z < 7 {
                floors.push((cam_z + 1, 50)); // 1 floor below, faint
            }
            if cam_z + 2 <= 7 {
                floors.push((cam_z + 2, 25)); // 2 floors below, very faint
            }
            floors
        } else if cam_z > 7 {
            // Underground: show 1 floor above (lower z number)
            let mut floors = Vec::new();
            if cam_z > 8 {
                floors.push((cam_z - 1, 50));
            }
            floors
        } else {
            Vec::new()
        };

        for (ghost_z, alpha) in &ghost_floors {
            let offset_tiles = (cam_z as i32 - *ghost_z as i32) as f64;
            let ghost_tiles = map.get_tiles_in_area(vis_x1, vis_y1, vis_x2, vis_y2, *ghost_z);
            for tile in &ghost_tiles {
                // Diagonal offset: each z-level shift moves 1 tile diag
                let wx = tile.x as f64 - offset_tiles;
                let wy = tile.y as f64 - offset_tiles;
                let tl = world_to_screen(wx, wy);
                let br = world_to_screen(wx + 1.0, wy + 1.0);
                let tile_rect = Rect::from_min_max(tl, br);

                if let Some(ground_id) = tile.ground {
                    draw_item_sprite_alpha(
                        &painter,
                        tile_rect,
                        ground_id as u32,
                        &state.appearances,
                        &mut state.sprite_textures,
                        &state.sprite_sheets,
                        &ctx,
                        state.animate_sprites,
                        anim_time_ms,
                        *alpha,
                        &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                    );
                }
                if lod >= 3 {
                    for item in &tile.items {
                        draw_item_sprite_alpha(
                            &painter,
                            tile_rect,
                            item.id as u32,
                            &state.appearances,
                            &mut state.sprite_textures,
                            &state.sprite_sheets,
                            &ctx,
                            state.animate_sprites,
                            anim_time_ms,
                            *alpha,
                            &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                        );
                    }
                }
            }
        }
    }

    // ── Main tile rendering ──
    if lod == 0 {
        // Chunk overview mode — render one colored block per occupied chunk
        let chunk_size = pte_otbm::CHUNK_SIZE as f64;
        let cx1 = (vis_x1 as f64 / chunk_size).floor() as i32;
        let cy1 = (vis_y1 as f64 / chunk_size).floor() as i32;
        let cx2 = (vis_x2 as f64 / chunk_size).ceil() as i32;
        let cy2 = (vis_y2 as f64 / chunk_size).ceil() as i32;

        for cx in cx1..=cx2 {
            for cy in cy1..=cy2 {
                let key = pte_otbm::ChunkKey { cx, cy, z: cam_z };
                if let Some(chunk) = map.chunks.get(&key) {
                    if chunk.is_empty() {
                        continue;
                    }
                    let wx = cx as f64 * chunk_size;
                    let wy = cy as f64 * chunk_size;
                    let tl = world_to_screen(wx, wy);
                    let br = world_to_screen(wx + chunk_size, wy + chunk_size);
                    let chunk_rect = Rect::from_min_max(tl, br);

                    // Color based on dominant tile type in chunk
                    // Sample up to 16 tiles and pick the most common minimap color
                    let mut color_counts: [u32; 256] = [0; 256];
                    let mut sampled = 0u32;
                    for tile in chunk.values() {
                        if sampled >= 16 { break; }
                        if let Some(ground_id) = tile.ground {
                            if let Some(ref apps) = state.appearances {
                                if let Some(app) = apps.get(appearances::Category::Object, ground_id as u32) {
                                    if let Some(ref flags) = app.flags {
                                        if let Some(ref automap) = flags.automap {
                                            if let Some(ci) = automap.color {
                                                color_counts[ci as usize & 0xFF] += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        sampled += 1;
                    }
                    // Find dominant color
                    let mut best_idx = 0u8;
                    let mut best_count = 0u32;
                    for (i, &c) in color_counts.iter().enumerate() {
                        if c > best_count { best_count = c; best_idx = i as u8; }
                    }
                    let color = if best_count > 0 {
                        tibia_minimap_color(best_idx)
                    } else {
                        // Fallback: density-based green
                        let density = chunk.len() as f32 / (chunk_size * chunk_size) as f32;
                        let intensity = (40.0 + density * 160.0) as u8;
                        Color32::from_rgb(intensity / 2, intensity, intensity / 3)
                    };
                    painter.rect_filled(chunk_rect, 0.0, color);
                }
            }
        }
    } else {
        // LOD 1-3: per-tile rendering with budget cap
        let vis_tiles_wide = (vis_x2 as u32).saturating_sub(vis_x1 as u32) + 1;
        let vis_tiles_high = (vis_y2 as u32).saturating_sub(vis_y1 as u32) + 1;
        let vis_area = vis_tiles_wide as u64 * vis_tiles_high as u64;

        // Budget: cap at ~500k visible tile area to prevent freezing
        // If exceeded, fall back to sampling (render every Nth tile)
        let stride = if vis_area > 500_000 {
            ((vis_area as f64 / 500_000.0).sqrt().ceil() as usize).max(2)
        } else {
            1
        };

        let tiles = map.get_tiles_in_area(vis_x1, vis_y1, vis_x2, vis_y2, cam_z);

        // Local caches for this frame (Fix 2, 3, 4)
        let mut minimap_cache: std::collections::HashMap<u16, Color32> = std::collections::HashMap::new();
        let mut local_render_cache: std::collections::HashMap<u32, u8> = std::collections::HashMap::new();
        let mut sort_buf: Vec<(u8, usize)> = Vec::with_capacity(32);

        for tile in &tiles {
            // Apply stride sampling at extreme zoom-out to stay responsive
            if stride > 1
                && (!(tile.x as usize).is_multiple_of(stride)
                    || !(tile.y as usize).is_multiple_of(stride))
            {
                continue;
            }

            let tl = world_to_screen(tile.x as f64, tile.y as f64);
            let br = world_to_screen(tile.x as f64 + 1.0, tile.y as f64 + 1.0);
            // At low zoom, expand tile rect to eliminate seam artifacts (black lines between tiles)
            let tile_rect = if tile_px < 16.0 {
                let pad = if tile_px < 8.0 { 1.0 } else { 0.5 };
                Rect::from_min_max(tl, Pos2::new(br.x + pad, br.y + pad))
            } else {
                Rect::from_min_max(tl, br)
            };

            if lod == 1 {
                // Minimap mode — single colored pixel per tile (cached by ground_id)
                let ground_id = tile.ground.unwrap_or(0);
                let color = *minimap_cache.entry(ground_id).or_insert_with(|| {
                    minimap_tile_color(tile, &state.appearances)
                });
                painter.rect_filled(tile_rect, 0.0, color);
            } else {
                // LOD 2+ — render ground sprite
                if state.show_ground {
                    if let Some(ground_id) = tile.ground {
                        draw_item_sprite(
                            &painter,
                            tile_rect,
                            ground_id as u32,
                            &state.appearances,
                            &mut state.sprite_textures,
                            &state.sprite_sheets,
                            &ctx,
                            state.animate_sprites,
                            anim_time_ms,
                            &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                        );
                    }
                }

                if lod >= 3 && state.show_items {
                    // Full detail — render all items sorted by z-order flags
                    // Order: clip → bottom → normal → top → topeffect
                    sort_buf.clear();
                    sort_buf.extend(tile.items.iter().enumerate().map(|(idx, item)| {
                        let order = *local_render_cache.entry(item.id as u32).or_insert_with(|| {
                            item_render_order(item.id as u32, &state.appearances)
                        });
                        (order, idx)
                    }));
                    sort_buf.sort_by_key(|&(order, idx)| (order, idx));

                    for &(_order, idx) in &sort_buf {
                        let item = &tile.items[idx];
                        draw_item_sprite(
                            &painter,
                            tile_rect,
                            item.id as u32,
                            &state.appearances,
                            &mut state.sprite_textures,
                            &state.sprite_sheets,
                            &ctx,
                            state.animate_sprites,
                            anim_time_ms,
                            &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                        );
                    }

                    // Stackable count overlay — show count on the first stackable item
                    if tile_px > 12.0 {
                        for item in &tile.items {
                            if let Some(count) = item.count {
                                if count > 1 {
                                    let text = format!("{}", count);
                                    let pos =
                                        Pos2::new(tile_rect.max.x - 2.0, tile_rect.max.y - 1.0);
                                    // Shadow
                                    painter.text(
                                        Pos2::new(pos.x + 1.0, pos.y + 1.0),
                                        egui::Align2::RIGHT_BOTTOM,
                                        &text,
                                        egui::FontId::proportional(9.0),
                                        Color32::BLACK,
                                    );
                                    // Foreground
                                    painter.text(
                                        pos,
                                        egui::Align2::RIGHT_BOTTOM,
                                        &text,
                                        egui::FontId::proportional(9.0),
                                        Color32::from_rgb(255, 255, 200),
                                    );
                                    break; // Only show count for the first stackable
                                }
                            }
                        }
                    }
                }

                // Tile flag overlays
                if state.show_zone_overlays && tile.flags.any() {
                    let overlay_color = if tile.flags.protection_zone {
                        Color32::from_rgba_unmultiplied(0, 200, 0, 40)
                    } else if tile.flags.no_pvp {
                        Color32::from_rgba_unmultiplied(0, 100, 200, 40)
                    } else if tile.flags.pvp_zone {
                        Color32::from_rgba_unmultiplied(200, 0, 0, 40)
                    } else {
                        Color32::from_rgba_unmultiplied(200, 200, 0, 30)
                    };
                    painter.rect_filled(tile_rect, 0.0, overlay_color);
                }

                // House overlay
                if state.show_house_overlay {
                    if let Some(hid) = tile.house_id {
                        let fill = crate::house_brush::house_color(hid);
                        painter.rect_filled(tile_rect, 0.0, fill);
                        if tile_px > 12.0 {
                            let border = crate::house_brush::house_border_color(hid);
                            painter.rect_stroke(
                                tile_rect,
                                0.0,
                                (0.5, border),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                }

                // Item type highlights (pickupable/moveable/blocking/hooks)
                if lod >= 3
                    && (state.highlight_pickupable
                        || state.highlight_moveable
                        || state.highlight_blocking
                        || state.highlight_hooks)
                {
                    for item in &tile.items {
                        if let Some(color) = crate::view_overlays::highlight_color_for_item(
                            item.id as u32,
                            &state.appearances,
                            state,
                        ) {
                            painter.rect_filled(tile_rect, 0.0, color);
                        }
                    }
                }

                // Light source visualization
                if lod >= 2 {
                    crate::view_overlays::draw_light_overlays(
                        &painter,
                        tile,
                        tile_rect,
                        tile_px,
                        &state.appearances,
                        state,
                    );
                }
            }
        }
    }

    // Grid lines (only at LOD 3 — full detail, capped, and when enabled)
    if state.show_grid && lod >= 3 && tile_px > 8.0 {
        let grid_w = (vis_x2 as u32).saturating_sub(vis_x1 as u32) + 1;
        let grid_h = (vis_y2 as u32).saturating_sub(vis_y1 as u32) + 1;
        // Only draw grid if visible area is reasonable (< 2000 lines)
        if grid_w < 1000 && grid_h < 1000 {
            let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 12);
            for x in vis_x1..=vis_x2 {
                let p = world_to_screen(x as f64, vis_y1 as f64);
                let p2 = world_to_screen(x as f64, vis_y2 as f64 + 1.0);
                painter.line_segment([p, p2], (0.5, grid_color));
            }
            for y in vis_y1..=vis_y2 {
                let p = world_to_screen(vis_x1 as f64, y as f64);
                let p2 = world_to_screen(vis_x2 as f64 + 1.0, y as f64);
                painter.line_segment([p, p2], (0.5, grid_color));
            }
        }
    }

    // Hover highlight
    // ── Brush preview cursor ──
    if let Some(hover_pos) = response.hover_pos() {
        let (wx, wy) = screen_to_world(hover_pos.x, hover_pos.y);
        let hx = wx.floor() as u16;
        let hy = wy.floor() as u16;
        state.hover_tile = Some((hx, hy));

        let is_painting_tool = matches!(
            state.active_tool,
            ToolType::Brush
                | ToolType::Eraser
                | ToolType::Door
                | ToolType::Creature
                | ToolType::Spawn
                | ToolType::Waypoint
        );

        if is_painting_tool && lod >= 2 {
            // Show brush footprint preview
            let (brush_size, brush_shape) =
                if matches!(state.active_tool, ToolType::Brush | ToolType::Eraser) {
                    (state.brush_size, state.brush_shape)
                } else {
                    (1, BrushShape::Square) // placement tools = single tile
                };

            let offsets = crate::brushes::shape::brush_offsets(brush_shape, brush_size);
            let is_eraser = state.active_tool == ToolType::Eraser;

            // For each tile in the footprint
            for &(dx, dy) in &offsets {
                let tx = hx as i32 + dx;
                let ty = hy as i32 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tx = tx as u16;
                let ty = ty as u16;

                let tl = world_to_screen(tx as f64, ty as f64);
                let br = world_to_screen(tx as f64 + 1.0, ty as f64 + 1.0);
                let tile_rect = Rect::from_min_max(tl, br);

                if is_eraser {
                    // Eraser: red-tinted overlay + cross pattern
                    painter.rect_filled(
                        tile_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(220, 50, 50, 45),
                    );
                    painter.rect_stroke(
                        tile_rect,
                        0.0,
                        (1.0, Color32::from_rgba_unmultiplied(220, 70, 70, 140)),
                        egui::StrokeKind::Outside,
                    );
                    // Draw X across tile for erase indicator
                    if tile_px > 6.0 {
                        let x_color = Color32::from_rgba_unmultiplied(255, 80, 80, 100);
                        painter.line_segment([tl, br], (0.8, x_color));
                        painter.line_segment(
                            [Pos2::new(tl.x, br.y), Pos2::new(br.x, tl.y)],
                            (0.8, x_color),
                        );
                    }
                } else {
                    // Brush/placement: ghost sprite preview + highlight
                    // Draw a ghost of what will be placed
                    let preview_drawn = if tile_px >= 8.0 {
                        draw_brush_ghost(
                            &painter,
                            tile_rect,
                            &state.appearances,
                            &mut state.sprite_textures,
                            &state.sprite_sheets,
                            &ctx,
                            state.active_brush,
                            state.selected_item_id,
                            &state.brush_registry,
                            state.animate_sprites,
                            anim_time_ms,
                            &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                        )
                    } else {
                        false
                    };

                    if !preview_drawn {
                        // Fallback: green-tinted highlight
                        painter.rect_filled(
                            tile_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(80, 200, 120, 35),
                        );
                    }

                    painter.rect_stroke(
                        tile_rect,
                        0.0,
                        (1.0, Color32::from_rgba_unmultiplied(80, 220, 130, 120)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        } else {
            // Non-painting tools (select, eyedropper, fill): simple single-tile hover
            let tl = world_to_screen(hx as f64, hy as f64);
            let br = world_to_screen(hx as f64 + 1.0, hy as f64 + 1.0);
            painter.rect_stroke(
                Rect::from_min_max(tl, br),
                0.0,
                (1.0, Color32::from_white_alpha(100)),
                egui::StrokeKind::Outside,
            );
        }
    } else {
        state.hover_tile = None;
    }

    // Spawn radius visualization
    if state.active_tool == ToolType::Spawn {
        if let Some((hx, hy)) = state.hover_tile {
            let r = state.spawn_radius as f64;
            let center = world_to_screen(hx as f64 + 0.5, hy as f64 + 0.5);
            let radius_px = r * tile_px as f64;
            painter.circle_stroke(
                center,
                radius_px as f32,
                (1.5, Color32::from_rgba_unmultiplied(80, 180, 255, 120)),
            );
            painter.circle_filled(
                center,
                radius_px as f32,
                Color32::from_rgba_unmultiplied(80, 180, 255, 15),
            );
        }
    }

    // Selection rectangle
    if let Some(sel) = &state.selection {
        let tl = world_to_screen(sel.x1 as f64, sel.y1 as f64);
        let br = world_to_screen(sel.x2 as f64 + 1.0, sel.y2 as f64 + 1.0);
        let sel_rect = Rect::from_min_max(tl, br);
        painter.rect_filled(
            sel_rect,
            0.0,
            Color32::from_rgba_unmultiplied(233, 69, 96, 20),
        );
        painter.rect_stroke(
            sel_rect,
            0.0,
            (1.5, Color32::from_rgb(233, 69, 96)),
            egui::StrokeKind::Outside,
        );
    }

    // Shade non-selected areas
    crate::view_overlays::draw_shade(&painter, state, canvas_rect, &world_to_screen);

    // Client viewport box
    crate::view_overlays::draw_client_box(&painter, state, &world_to_screen);

    // Spawn area circles
    crate::view_overlays::draw_spawn_overlays(&painter, state, &world_to_screen, tile_px);

    // Paste preview ghost
    if state.paste_preview {
        if let Some((hx, hy)) = state.hover_tile {
            if let Some(ref clip) = state.clipboard {
                for (dx, dy, src_tile) in &clip.tiles {
                    let tx = hx as i32 + *dx as i32;
                    let ty = hy as i32 + *dy as i32;
                    if tx < 0 || ty < 0 {
                        continue;
                    }

                    let tl = world_to_screen(tx as f64, ty as f64);
                    let br = world_to_screen(tx as f64 + 1.0, ty as f64 + 1.0);
                    let tile_rect = Rect::from_min_max(tl, br);

                    // Ghost render of each tile's ground and items
                    if tile_px >= 8.0 {
                        if let Some(gid) = src_tile.ground {
                            draw_item_sprite_alpha(
                                &painter,
                                tile_rect,
                                gid as u32,
                                &state.appearances,
                                &mut state.sprite_textures,
                                &state.sprite_sheets,
                                &ctx,
                                state.animate_sprites,
                                anim_time_ms,
                                120,
                                &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                            );
                        }
                        for item in &src_tile.items {
                            draw_item_sprite_alpha(
                                &painter,
                                tile_rect,
                                item.id as u32,
                                &state.appearances,
                                &mut state.sprite_textures,
                                &state.sprite_sheets,
                                &ctx,
                                state.animate_sprites,
                                anim_time_ms,
                                120,
                                &mut state.texture_lru_gen,
                            &mut state.texture_lru_counter,
                            );
                        }
                    }

                    painter.rect_filled(
                        tile_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(100, 180, 255, 25),
                    );
                    painter.rect_stroke(
                        tile_rect,
                        0.0,
                        (1.0, Color32::from_rgba_unmultiplied(100, 180, 255, 100)),
                        egui::StrokeKind::Outside,
                    );
                }

                // Outer bounding rect
                let tl = world_to_screen(hx as f64, hy as f64);
                let br = world_to_screen(
                    hx as f64 + clip.width as f64,
                    hy as f64 + clip.height as f64,
                );
                painter.rect_stroke(
                    Rect::from_min_max(tl, br),
                    0.0,
                    (2.0, Color32::from_rgba_unmultiplied(100, 180, 255, 180)),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }

    // Update performance stats
    state.perf.update(dt);
    state.perf.visible_tiles = {
        let w = (vis_x2 as u64).saturating_sub(vis_x1 as u64) + 1;
        let h = (vis_y2 as u64).saturating_sub(vis_y1 as u64) + 1;
        w * h
    };
    state.perf.total_tiles = state.cached_tile_count as u64;

    // Handle input
    handle_viewport_input(&response, state, &screen_to_world);
}

fn handle_viewport_input(
    response: &egui::Response,
    state: &mut EditorState,
    screen_to_world: &dyn Fn(f32, f32) -> (f64, f64),
) {
    // Pan with middle mouse or Ctrl+drag
    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary)
            && response.ctx.input(|i| i.modifiers.ctrl))
    {
        let delta = response.drag_delta();
        let tile_px = state.camera.tile_size();
        state.camera.center_x -= delta.x as f64 / tile_px as f64;
        state.camera.center_y -= delta.y as f64 / tile_px as f64;
    }

    // Zoom with scroll wheel — smooth logarithmic zoom towards cursor
    let scroll = response.ctx.input(|i| i.smooth_scroll_delta.y);
    let ctrl_held = response.ctx.input(|i| i.modifiers.ctrl);
    if scroll != 0.0 && response.hovered() {
        // Store world position under cursor before zoom change
        if let Some(hover_pos) = response.hover_pos() {
            let (wx_before, wy_before) = screen_to_world(hover_pos.x, hover_pos.y);
            state.camera.zoom_by_scroll_fine(scroll, ctrl_held);
            // Use zoom_target to compute the final camera offset
            // (animate_zoom will interpolate there smoothly)
            let target_tile_px = 32.0 * state.camera.zoom_target;
            let rect = response.rect;
            let dx_screen = hover_pos.x - (rect.min.x + rect.width() / 2.0);
            let dy_screen = hover_pos.y - (rect.min.y + rect.height() / 2.0);
            let new_wx = state.camera.center_x + dx_screen as f64 / target_tile_px as f64;
            let new_wy = state.camera.center_y + dy_screen as f64 / target_tile_px as f64;
            state.camera.center_x += wx_before - new_wx;
            state.camera.center_y += wy_before - new_wy;
        } else {
            state.camera.zoom_by_scroll_fine(scroll, ctrl_held);
        }
    }

    // Tool actions: support BOTH single-click AND drag
    let is_ctrl = response.ctx.input(|i| i.modifiers.ctrl);
    let is_left_click = response.clicked_by(egui::PointerButton::Primary) && !is_ctrl;
    let is_left_drag = response.dragged_by(egui::PointerButton::Primary) && !is_ctrl;

    // Paste preview mode: left click → paste at cursor
    if state.paste_preview && is_left_click {
        if let Some(pos) = response.interact_pointer_pos() {
            let (wx, wy) = screen_to_world(pos.x, pos.y);
            let tx = wx.floor().max(0.0) as u16;
            let ty = wy.floor().max(0.0) as u16;
            crate::clipboard::paste_at(state, tx, ty);
        }
        return;
    }

    if is_left_click || is_left_drag {
        if let Some(pos) = response.interact_pointer_pos() {
            let (wx, wy) = screen_to_world(pos.x, pos.y);
            let tx = wx.floor().max(0.0) as u16;
            let ty = wy.floor().max(0.0) as u16;
            let z = state.camera.z_level;
            let shift = response.ctx.input(|i| i.modifiers.shift);

            match state.active_tool {
                ToolType::Brush => {
                    // Zone flag brush takes priority
                    if let Some(zone_flag) = state.active_zone_flag {
                        if !state.stroke_touched(tx, ty, z) {
                            let offsets = crate::brushes::shape::brush_offsets(
                                state.brush_shape,
                                state.brush_size,
                            );
                            for &(dx, dy) in &offsets {
                                let fx = tx as i32 + dx;
                                let fy = ty as i32 + dy;
                                if fx >= 0 && fy >= 0 {
                                    crate::zone_brush::apply_zone(
                                        state, fx as u16, fy as u16, z, zone_flag, shift,
                                    );
                                }
                            }
                        }
                    }
                    // House brush: paint house_id onto tiles
                    else if let Some(house_id) = state.active_house_id {
                        if !state.stroke_touched(tx, ty, z) {
                            let offsets = crate::brushes::shape::brush_offsets(
                                state.brush_shape,
                                state.brush_size,
                            );
                            for &(dx, dy) in &offsets {
                                let fx = tx as i32 + dx;
                                let fy = ty as i32 + dy;
                                if fx >= 0 && fy >= 0 {
                                    crate::house_brush::apply_house_brush(
                                        state, fx as u16, fy as u16, z, house_id, shift,
                                    );
                                }
                            }
                        }
                    }
                    // Regular brush
                    else if (state.active_brush.is_some() || state.selected_item_id.is_some())
                        && !state.stroke_touched(tx, ty, z)
                    {
                        if let Some(ref mut map) = state.map_data {
                            let result = crate::tools::brush::apply_brush(
                                map,
                                tx,
                                ty,
                                z,
                                state.active_brush,
                                state.selected_item_id,
                                &state.brush_registry,
                                state.brush_size,
                                state.brush_shape,
                                &state.appearances,
                            );
                            if !result.dirty_positions.is_empty() {
                                crate::brushes::process_borders(
                                    map,
                                    &state.brush_registry,
                                    &result.dirty_positions,
                                );
                            }
                            state.stroke_add(result.undo);
                        }
                    }
                }
                ToolType::Eraser => {
                    match state.eraser_mode {
                        crate::state::EraserMode::TopItem => {
                            if !state.stroke_touched(tx, ty, z) {
                                if let Some(ref mut map) = state.map_data {
                                    let action = crate::tools::eraser::apply_eraser(
                                        map,
                                        tx,
                                        ty,
                                        z,
                                        state.brush_size,
                                        state.brush_shape,
                                        shift,
                                        state.eraser_flags_only,
                                    );
                                    state.stroke_add(action);
                                }
                            }
                        }
                        crate::state::EraserMode::Selective => {
                            // Open the selective eraser picker for this tile
                            if let Some(ref map) = state.map_data {
                                if let Some(tile) = map.get_tile(tx, ty, z) {
                                    let mut items = Vec::new();
                                    if let Some(ground) = tile.ground {
                                        state.selective_eraser.has_ground = true;
                                        state.selective_eraser.ground_id = ground as u32;
                                    } else {
                                        state.selective_eraser.has_ground = false;
                                        state.selective_eraser.ground_id = 0;
                                    }
                                    for item in &tile.items {
                                        items.push((item.id as u32, format!("Item #{}", item.id)));
                                    }
                                    state.selective_eraser.tile_x = tx;
                                    state.selective_eraser.tile_y = ty;
                                    state.selective_eraser.tile_z = z;
                                    state.selective_eraser.items = items;
                                    state.selective_eraser.open = true;
                                }
                            }
                        }
                        crate::state::EraserMode::FullTile => {
                            if !state.stroke_touched(tx, ty, z) {
                                if let Some(ref mut map) = state.map_data {
                                    let action = crate::tools::eraser::apply_eraser(
                                        map,
                                        tx,
                                        ty,
                                        z,
                                        state.brush_size,
                                        state.brush_shape,
                                        true, // clear_all = true
                                        false,
                                    );
                                    state.stroke_add(action);
                                }
                            }
                        }
                    }
                }
                ToolType::Fill => {
                    if let Some(item_id) = state.selected_item_id {
                        if let Some(ref mut map) = state.map_data {
                            let action =
                                crate::tools::fill::apply_fill(map, tx, ty, z, item_id as u16);
                            state.push_undo(action);
                        }
                    }
                }
                ToolType::Eyedropper => {
                    if let Some(ref map) = state.map_data {
                        if let Some(result) = crate::tools::eyedropper::pick_item(
                            map,
                            tx,
                            ty,
                            z,
                            &state.brush_registry,
                        ) {
                            state.selected_item_id = Some(result.item_id);
                            if let Some(bid) = result.brush_id {
                                state.active_brush = Some(bid);
                            }
                            state.active_tool = ToolType::Brush;
                        }
                    }
                }
                ToolType::Select => {
                    // If clicking inside an existing selection, drag-move it
                    if let Some(ref sel) = state.selection {
                        if tx >= sel.x1 && tx <= sel.x2 && ty >= sel.y1 && ty <= sel.y2 {
                            // Initiate drag
                            if state.selection_drag_origin.is_none() {
                                state.selection_drag_origin = Some((tx, ty));
                            }
                            // Track offset from drag origin
                            if let Some((ox, oy)) = state.selection_drag_origin {
                                state.selection_drag_offset =
                                    Some((tx as i32 - ox as i32, ty as i32 - oy as i32));
                            }
                        } else {
                            // Clicked outside selection — start a new one
                            state.selection = None;
                            state.selection_drag_origin = None;
                            state.selection_drag_offset = None;
                            state.select_start = Some((tx, ty));
                        }
                    } else if state.select_start.is_none() {
                        state.select_start = Some((tx, ty));
                    }
                    if let Some((sx, sy)) = state.select_start {
                        state.selection =
                            Some(crate::tools::select::update_selection(sx, sy, tx, ty));
                    }
                }
                ToolType::Door | ToolType::Creature | ToolType::Spawn | ToolType::Waypoint => {
                    // These placement tools use the active_brush via the Brush trait
                    if let Some(brush_id) = state.active_brush {
                        if !state.stroke_touched(tx, ty, z) {
                            // Check brush type for post-draw spawn handling
                            let brush_type =
                                state.brush_registry.get(brush_id).map(|b| b.brush_type());
                            let creature_name = state
                                .brush_registry
                                .get(brush_id)
                                .filter(|b| b.brush_type() == crate::brushes::BrushType::Creature)
                                .map(|b| b.name().to_string());

                            if let Some(ref mut map) = state.map_data {
                                let result = crate::tools::brush::apply_brush(
                                    map,
                                    tx,
                                    ty,
                                    z,
                                    Some(brush_id),
                                    None,
                                    &state.brush_registry,
                                    1, // single-tile placement
                                    BrushShape::Square,
                                    &state.appearances,
                                );
                                if !result.dirty_positions.is_empty() {
                                    crate::brushes::process_borders(
                                        map,
                                        &state.brush_registry,
                                        &result.dirty_positions,
                                    );
                                }
                                state.stroke_add(result.undo);
                            }

                            // Update spawn data for creature placement
                            if let Some(name) = creature_name {
                                use crate::spawn_xml::{Spawn, SpawnCreature};
                                // Find or create a spawn within radius of this position
                                let existing = state.spawns.iter_mut().find(|s| {
                                    let dx = (s.center_x as i32 - tx as i32).abs();
                                    let dy = (s.center_y as i32 - ty as i32).abs();
                                    dx <= s.radius as i32 && dy <= s.radius as i32
                                });
                                if let Some(spawn) = existing {
                                    // Add creature to existing spawn (offset from center)
                                    spawn.creatures.push(SpawnCreature {
                                        name,
                                        is_npc: false,
                                        offset_x: tx as i32 - spawn.center_x as i32,
                                        offset_y: ty as i32 - spawn.center_y as i32,
                                        z,
                                        spawn_time: 60,
                                    });
                                } else {
                                    // Create new spawn centered on this tile
                                    state.spawns.push(Spawn {
                                        center_x: tx,
                                        center_y: ty,
                                        center_z: z,
                                        radius: state.spawn_radius,
                                        creatures: vec![SpawnCreature {
                                            name,
                                            is_npc: false,
                                            offset_x: 0,
                                            offset_y: 0,
                                            z,
                                            spawn_time: 60,
                                        }],
                                    });
                                }
                            }

                            // Update spawn data for spawn brush — create empty spawn area
                            if brush_type == Some(crate::brushes::BrushType::Spawn) {
                                use crate::spawn_xml::Spawn;
                                let already = state.spawns.iter().any(|s| {
                                    s.center_x == tx && s.center_y == ty && s.center_z == z
                                });
                                if !already {
                                    state.spawns.push(Spawn {
                                        center_x: tx,
                                        center_y: ty,
                                        center_z: z,
                                        radius: state.spawn_radius,
                                        creatures: vec![],
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Commit stroke when drag ends OR on single click (single click = start + commit)
    if response.drag_stopped() || is_left_click {
        // If we were dragging a selection, commit the move
        if let (Some(sel), Some((dx, dy))) = (&state.selection, state.selection_drag_offset) {
            if dx != 0 || dy != 0 {
                if let Some(ref mut map) = state.map_data {
                    let z = state.camera.z_level;
                    let mut undo_before = Vec::new();
                    let mut undo_after = Vec::new();
                    let mut tiles_to_move = Vec::new();

                    // Collect tiles in selection
                    for ty in sel.y1..=sel.y2 {
                        for tx in sel.x1..=sel.x2 {
                            if let Some(tile) = map.get_tile(tx, ty, z) {
                                tiles_to_move.push(tile.clone());
                            }
                        }
                    }

                    // Save originals and clear source tiles
                    for ty in sel.y1..=sel.y2 {
                        for tx in sel.x1..=sel.x2 {
                            undo_before.push((tx, ty, z, map.get_tile(tx, ty, z).cloned()));
                            map.remove_tile(tx, ty, z);
                            undo_after.push((tx, ty, z, None));
                        }
                    }

                    // Place tiles at new positions
                    for tile in tiles_to_move {
                        let nx = (tile.x as i32 + dx).max(0) as u16;
                        let ny = (tile.y as i32 + dy).max(0) as u16;
                        undo_before.push((nx, ny, z, map.get_tile(nx, ny, z).cloned()));
                        let mut new_tile = tile;
                        new_tile.x = nx;
                        new_tile.y = ny;
                        map.set_tile(new_tile);
                        undo_after.push((nx, ny, z, map.get_tile(nx, ny, z).cloned()));
                    }

                    state.push_undo(crate::state::UndoAction {
                        tiles_before: undo_before,
                        tiles_after: undo_after,
                    });

                    // Move the selection rectangle too
                    if let Some(ref mut sel) = state.selection {
                        sel.x1 = (sel.x1 as i32 + dx).max(0) as u16;
                        sel.y1 = (sel.y1 as i32 + dy).max(0) as u16;
                        sel.x2 = (sel.x2 as i32 + dx).max(0) as u16;
                        sel.y2 = (sel.y2 as i32 + dy).max(0) as u16;
                    }
                }
            }
            state.selection_drag_origin = None;
            state.selection_drag_offset = None;
        }

        state.stroke_commit();
        state.select_start = None;
    }
}

/// Draw a ghost preview of the sprite that will be placed.
/// Returns true if a ghost sprite was drawn, false if nothing to preview.
#[allow(clippy::too_many_arguments)]
fn draw_brush_ghost(
    painter: &egui::Painter,
    rect: Rect,
    appearances: &Option<appearances::LoadedAppearances>,
    textures: &mut std::collections::HashMap<u32, egui::TextureHandle>,
    sheets: &std::collections::HashMap<String, pte_assets::SpriteSheet>,
    ctx: &egui::Context,
    active_brush: Option<crate::brushes::BrushId>,
    selected_item_id: Option<u32>,
    brush_registry: &crate::brushes::registry::BrushRegistry,
    animate_sprites: bool,
    anim_time_ms: u64,
    texture_lru_gen: &mut std::collections::HashMap<u32, u64>,
    texture_lru_counter: &mut u64,
) -> bool {
    let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
    let tint = Color32::from_rgba_unmultiplied(255, 255, 255, 100);

    let item_id: Option<u32> = if let Some(brush_id) = active_brush {
        brush_registry
            .get(brush_id)
            .and_then(|b| b.preview_item_id())
            .map(|id| id as u32)
    } else {
        selected_item_id
    };

    let Some(item_id) = item_id else { return false };

    if let Some(ref apps) = appearances {
        if let Some(appearance) = apps.get(appearances::Category::Object, item_id) {
            if let Some(sid) =
                resolve_appearance_sprite(appearance, 0, animate_sprites, anim_time_ms)
            {
                if let Some(tex) = get_or_upload(textures, sheets, ctx, sid, texture_lru_gen, texture_lru_counter) {
                    // Handle oversized sprites (64×64 etc.) — extend UP and LEFT
                    let [tex_w, tex_h] = tex.size();
                    let tile_w = rect.width();
                    let tile_h = rect.height();
                    let tiles_w = (tex_w as f32 / 32.0).max(1.0);
                    let tiles_h = (tex_h as f32 / 32.0).max(1.0);

                    let draw_rect = if tiles_w > 1.0 || tiles_h > 1.0 {
                        Rect::from_min_max(
                            Pos2::new(rect.max.x - tile_w * tiles_w, rect.max.y - tile_h * tiles_h),
                            rect.max,
                        )
                    } else {
                        rect
                    };

                    painter.image(tex.id(), draw_rect, uv, tint);
                    return true;
                }
            }
        }
    }

    if let Some(tex) = get_or_upload(textures, sheets, ctx, item_id, texture_lru_gen, texture_lru_counter) {
        let [tex_w, tex_h] = tex.size();
        let tile_w = rect.width();
        let tile_h = rect.height();
        let tiles_w = (tex_w as f32 / 32.0).max(1.0);
        let tiles_h = (tex_h as f32 / 32.0).max(1.0);

        let draw_rect = if tiles_w > 1.0 || tiles_h > 1.0 {
            Rect::from_min_max(
                Pos2::new(rect.max.x - tile_w * tiles_w, rect.max.y - tile_h * tiles_h),
                rect.max,
            )
        } else {
            rect
        };

        painter.image(tex.id(), draw_rect, uv, tint);
        return true;
    }

    false
}

fn hsl_to_color32(h: f32, s: f32, l: f32) -> Color32 {
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
    Color32::from_rgb(
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

/// Get a minimap-style color for a tile (used at very low zoom LOD).
/// Return the render order bucket for an item based on appearance flags.
/// 0 = clip (always below), 1 = bottom (borders), 2 = normal, 3 = top, 4 = topeffect.
fn item_render_order(item_id: u32, appearances: &Option<appearances::LoadedAppearances>) -> u8 {
    let Some(ref apps) = appearances else {
        return 2;
    };
    let Some(appearance) = apps.get(Category::Object, item_id) else {
        return 2;
    };
    let Some(ref flags) = appearance.flags else {
        return 2;
    };
    if flags.clip.is_some() {
        return 0;
    }
    if flags.bottom.is_some() {
        return 1;
    }
    if flags.top.is_some() {
        return 3;
    }
    if flags.topeffect.is_some() {
        return 4;
    }
    2
}

///
/// Priority: automap flag color > HSL hash of ground ID > dark fallback.
/// Uses the Tibia minimap palette: automap color index → RGB via standard table.
fn minimap_tile_color(
    tile: &pte_otbm::Tile,
    appearances: &Option<appearances::LoadedAppearances>,
) -> Color32 {
    // Try to get automap color from the ground item's appearance flags
    if let Some(ground_id) = tile.ground {
        if let Some(ref apps) = appearances {
            if let Some(appearance) = apps.get(Category::Object, ground_id as u32) {
                if let Some(ref flags) = appearance.flags {
                    if let Some(ref automap) = flags.automap {
                        if let Some(color_idx) = automap.color {
                            return tibia_minimap_color(color_idx as u8);
                        }
                    }
                }
            }
        }
        // Fallback: deterministic color from ground ID
        let hue = (ground_id as f32 * 137.508) % 360.0;
        return hsl_to_color32(hue, 0.35, 0.3);
    }

    // Empty tile
    Color32::from_rgb(20, 20, 35)
}

/// Convert a Tibia automap color index (0-215) to RGB.
/// Tibia uses a 6×6×6 RGB color cube: R = idx/(6*6), G = (idx/6)%6, B = idx%6.
/// Each component maps: 0→0, 1→51, 2→102, 3→153, 4→204, 5→255.
fn tibia_minimap_color(idx: u8) -> Color32 {
    let r_level = idx / 36;
    let g_level = (idx / 6) % 6;
    let b_level = idx % 6;
    Color32::from_rgb(
        r_level.min(5) * 51,
        g_level.min(5) * 51,
        b_level.min(5) * 51,
    )
}

/// Compute the sprite_id index for a given animation frame and direction.
///
/// Tibia sprite_id layout: `(((frame * pd + pz) * ph + py) * pw + px) * layers + layer`
/// For outfits: pw = 4 directions (0=N, 1=E, 2=S, 3=W)
pub(crate) fn resolve_sprite_index(
    si: &appearances::SpriteInfo,
    frame: usize,
    direction: usize,
) -> usize {
    let layers = si.layers.unwrap_or(1) as usize;
    let pw = si.pattern_width.unwrap_or(1) as usize;
    let ph = si.pattern_height.unwrap_or(1) as usize;
    let pd = si.pattern_depth.unwrap_or(1) as usize;

    let px = direction.min(pw.saturating_sub(1));
    // py=0 (no addon), pz=0 (no mount), layer=0 (base)
    (((frame * pd) * ph) * pw + px) * layers
}

/// Resolve a sprite from an Appearance with direction + animation support.
pub(crate) fn resolve_appearance_sprite(
    appearance: &appearances::Appearance,
    direction: usize,
    animate: bool,
    anim_time_ms: u64,
) -> Option<u32> {
    let fg = appearance.frame_group.first()?;
    let si = fg.sprite_info.as_ref()?;
    if si.sprite_id.is_empty() {
        return None;
    }

    let num_phases = si
        .animation
        .as_ref()
        .map(|a| a.sprite_phase.len().max(1))
        .unwrap_or(1);

    let frame = if animate && num_phases > 1 {
        si.animation
            .as_ref()
            .map(|a| compute_anim_frame(a, anim_time_ms))
            .unwrap_or(0)
    } else {
        0
    };

    let idx = resolve_sprite_index(si, frame, direction);
    si.sprite_id.get(idx).copied()
}

/// Resolve an item ID to its current sprite texture and draw it.
///
/// Handles: animation frames, oversized sprites (64x64 → 2x2 tiles).
#[allow(clippy::too_many_arguments)]
fn draw_item_sprite(
    painter: &egui::Painter,
    rect: Rect,
    item_id: u32,
    appearances: &Option<appearances::LoadedAppearances>,
    textures: &mut std::collections::HashMap<u32, egui::TextureHandle>,
    sheets: &std::collections::HashMap<String, pte_assets::SpriteSheet>,
    ctx: &egui::Context,
    animate: bool,
    anim_time_ms: u64,
    texture_lru_gen: &mut std::collections::HashMap<u32, u64>,
    texture_lru_counter: &mut u64,
) {
    let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));

    if let Some(ref apps) = appearances {
        if let Some(appearance) = apps.get(Category::Object, item_id) {
            if let Some(sid) = resolve_appearance_sprite(appearance, 0, animate, anim_time_ms) {
                let tex = get_or_upload(textures, sheets, ctx, sid, texture_lru_gen, texture_lru_counter);
                if let Some(tex) = tex {
                    // Handle oversized sprites — large sprites extend UP and LEFT
                    let [tex_w, tex_h] = tex.size();
                    let tile_w = rect.width();
                    let tile_h = rect.height();
                    let tiles_w = (tex_w as f32 / 32.0).max(1.0);
                    let tiles_h = (tex_h as f32 / 32.0).max(1.0);

                    let draw_rect = if tiles_w > 1.0 || tiles_h > 1.0 {
                        Rect::from_min_max(
                            Pos2::new(rect.max.x - tile_w * tiles_w, rect.max.y - tile_h * tiles_h),
                            rect.max,
                        )
                    } else {
                        rect
                    };

                    painter.image(tex.id(), draw_rect, uv, Color32::WHITE);
                    return;
                }
            }
        }
    }

    // Fallback: try direct sprite_id lookup
    if let Some(tex) = get_or_upload(textures, sheets, ctx, item_id, texture_lru_gen, texture_lru_counter) {
        painter.image(tex.id(), rect, uv, Color32::WHITE);
        return;
    }

    // Last resort: HSL color block
    let hue = (item_id as f32 * 137.508) % 360.0;
    let c = hsl_to_color32(hue, 0.4, 0.3);
    painter.rect_filled(rect, 0.0, c);
}

/// Like draw_item_sprite but with a custom alpha for ghost/preview rendering.
#[allow(clippy::too_many_arguments)]
fn draw_item_sprite_alpha(
    painter: &egui::Painter,
    rect: Rect,
    item_id: u32,
    appearances: &Option<appearances::LoadedAppearances>,
    textures: &mut std::collections::HashMap<u32, egui::TextureHandle>,
    sheets: &std::collections::HashMap<String, pte_assets::SpriteSheet>,
    ctx: &egui::Context,
    animate: bool,
    anim_time_ms: u64,
    alpha: u8,
    texture_lru_gen: &mut std::collections::HashMap<u32, u64>,
    texture_lru_counter: &mut u64,
) {
    let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
    let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

    if let Some(ref apps) = appearances {
        if let Some(appearance) = apps.get(Category::Object, item_id) {
            if let Some(sid) = resolve_appearance_sprite(appearance, 0, animate, anim_time_ms) {
                if let Some(tex) = get_or_upload(textures, sheets, ctx, sid, texture_lru_gen, texture_lru_counter) {
                    let [tex_w, tex_h] = tex.size();
                    let tile_w = rect.width();
                    let tile_h = rect.height();
                    let tiles_w = (tex_w as f32 / 32.0).max(1.0);
                    let tiles_h = (tex_h as f32 / 32.0).max(1.0);

                    let draw_rect = if tiles_w > 1.0 || tiles_h > 1.0 {
                        Rect::from_min_max(
                            Pos2::new(rect.max.x - tile_w * tiles_w, rect.max.y - tile_h * tiles_h),
                            rect.max,
                        )
                    } else {
                        rect
                    };

                    painter.image(tex.id(), draw_rect, uv, tint);
                    return;
                }
            }
        }
    }

    if let Some(tex) = get_or_upload(textures, sheets, ctx, item_id, texture_lru_gen, texture_lru_counter) {
        painter.image(tex.id(), rect, uv, tint);
    }
}

/// Lazy texture upload: get from cache or upload from sprite sheets on first use.
/// Updates LRU generation counter on both cache hits and misses for O(1) access tracking.
pub(crate) fn get_or_upload(
    textures: &mut std::collections::HashMap<u32, egui::TextureHandle>,
    sheets: &std::collections::HashMap<String, pte_assets::SpriteSheet>,
    ctx: &egui::Context,
    sprite_id: u32,
    texture_lru_gen: &mut std::collections::HashMap<u32, u64>,
    texture_lru_counter: &mut u64,
) -> Option<egui::TextureHandle> {
    get_or_upload_lazy(textures, sheets, ctx, sprite_id, texture_lru_gen, texture_lru_counter, None)
}

/// get_or_upload with optional lazy sheet loader fallback.
pub(crate) fn get_or_upload_lazy(
    textures: &mut std::collections::HashMap<u32, egui::TextureHandle>,
    sheets: &std::collections::HashMap<String, pte_assets::SpriteSheet>,
    ctx: &egui::Context,
    sprite_id: u32,
    texture_lru_gen: &mut std::collections::HashMap<u32, u64>,
    texture_lru_counter: &mut u64,
    lazy_loader: Option<&mut pte_assets::LazySheetLoader>,
) -> Option<egui::TextureHandle> {
    if let Some(tex) = textures.get(&sprite_id) {
        // Cache hit — update generation for LRU tracking (O(1))
        texture_lru_gen.insert(sprite_id, *texture_lru_counter);
        *texture_lru_counter += 1;
        return Some(tex.clone());
    }

    // Try eagerly-loaded sheets first
    for sheet in sheets.values() {
        if sprite_id >= sheet.first_sprite_id && sprite_id <= sheet.last_sprite_id {
            if let Some(pixels) = sheet.get_sprite(sprite_id) {
                let (w, h) = sheet.sprite_dimensions();
                let tex =
                    crate::sprite_picker::upload_sprite_texture(ctx, sprite_id, &pixels, w, h);
                textures.insert(sprite_id, tex.clone());
                texture_lru_gen.insert(sprite_id, *texture_lru_counter);
                *texture_lru_counter += 1;
                return Some(tex);
            }
        }
    }

    // Fallback: lazy loader (loads sheet on-demand from disk)
    if let Some(loader) = lazy_loader {
        if let Some(sheet) = loader.get_sheet(sprite_id) {
            if let Some(pixels) = sheet.get_sprite(sprite_id) {
                let (w, h) = sheet.sprite_dimensions();
                let tex =
                    crate::sprite_picker::upload_sprite_texture(ctx, sprite_id, &pixels, w, h);
                textures.insert(sprite_id, tex.clone());
                texture_lru_gen.insert(sprite_id, *texture_lru_counter);
                *texture_lru_counter += 1;
                return Some(tex);
            }
        }
    }

    None
}

/// Compute the current animation frame index based on elapsed time and sprite phase durations.
pub(crate) fn compute_anim_frame(anim: &appearances::SpriteAnimation, time_ms: u64) -> usize {
    let phases = &anim.sprite_phase;
    if phases.is_empty() {
        return 0;
    }

    // Total cycle duration
    let mut cycle_ms: u64 = 0;
    let mut durations: Vec<u64> = Vec::with_capacity(phases.len());
    for phase in phases {
        // Average of min/max duration, default 100ms
        let d = match (phase.duration_min, phase.duration_max) {
            (Some(min), Some(max)) => ((min as u64 + max as u64) / 2).max(1),
            (Some(d), None) | (None, Some(d)) => (d as u64).max(1),
            (None, None) => 100,
        };
        durations.push(d);
        cycle_ms += d;
    }

    if cycle_ms == 0 {
        return 0;
    }

    // Position in the current cycle
    let pos = time_ms % cycle_ms;
    let mut acc = 0u64;
    for (i, d) in durations.iter().enumerate() {
        acc += d;
        if pos < acc {
            return i;
        }
    }

    0
}
