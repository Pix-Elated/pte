//! API command handler — processes API commands in the UI thread.
//!
//! Called from the `update()` loop to drain pending commands from the
//! HTTP server and return results.

use serde_json::{json, Value};

use crate::api_server::{ApiCommand, ApiRequest, ApiResponse};
use crate::state::EditorState;

/// Process all pending API commands (non-blocking).
pub fn process_commands(state: &mut EditorState, cmd_rx: &std::sync::mpsc::Receiver<ApiCommand>) {
    // Drain all pending commands (non-blocking)
    while let Ok(cmd) = cmd_rx.try_recv() {
        let response = handle_request(state, &cmd.request);
        cmd.respond.send(response);
    }
}

fn handle_request(state: &mut EditorState, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "status" | "editor_status" => handle_status(state),
        "get_tile" => handle_get_tile(state, &req.params),
        "set_tile" => handle_set_tile(state, &req.params),
        "remove_tile" => handle_remove_tile(state, &req.params),
        "add_item" => handle_add_item(state, &req.params),
        "fill_area" => handle_fill_area(state, &req.params),
        "get_tiles_in_area" => handle_get_tiles_in_area(state, &req.params),
        "replace_item" => handle_replace_item(state, &req.params),
        "select_item" => handle_select_item(state, &req.params),
        "move_camera" => handle_move_camera(state, &req.params),
        "get_metadata" => handle_get_metadata(state),
        "undo" => handle_undo(state),
        "redo" => handle_redo(state),
        "search_appearances" => handle_search_appearances(state, &req.params),
        "map_stats" => handle_map_stats(state),
        "open_map" => handle_open_map(state, &req.params),
        "load_assets" => handle_load_assets(state, &req.params),
        "load_chunk_dir" => handle_load_chunk_dir(state, &req.params),
        "save_chunk_dir" => handle_save_chunk_dir(state, &req.params),
        _ => ApiResponse::error(format!("Unknown method: {}", req.method)),
    }
}

// ── Handlers ───────────────────────────────────────────────────────

fn handle_status(state: &EditorState) -> ApiResponse {
    let map_loaded = state.map_data.is_some();
    let tile_count = state.cached_tile_count;
    let assets_ready = state.assets_ready();

    ApiResponse::success(json!({
        "mode": format!("{:?}", state.mode),
        "assetsReady": assets_ready,
        "mapLoaded": map_loaded,
        "tileCount": tile_count,
        "cameraX": state.camera.center_x,
        "cameraY": state.camera.center_y,
        "cameraZ": state.camera.z_level,
        "zoom": state.camera.zoom,
        "activeTool": format!("{:?}", state.active_tool),
        "selectedItemId": state.selected_item_id,
        "canUndo": state.can_undo(),
        "canRedo": state.can_redo(),
    }))
}

fn handle_get_tile(state: &EditorState, params: &Value) -> ApiResponse {
    let x = params["x"].as_u64().map(|v| v as u16);
    let y = params["y"].as_u64().map(|v| v as u16);
    let z = params["z"].as_u64().map(|v| v as u8);

    let (Some(x), Some(y), Some(z)) = (x, y, z) else {
        return ApiResponse::error("Missing x, y, or z");
    };

    let Some(map) = &state.map_data else {
        return ApiResponse::error("No map loaded");
    };

    match map.get_tile(x, y, z) {
        Some(tile) => ApiResponse::success(tile_to_json(tile)),
        None => ApiResponse::success(json!(null)),
    }
}

fn handle_set_tile(state: &mut EditorState, params: &Value) -> ApiResponse {
    let x = params["x"].as_u64().map(|v| v as u16);
    let y = params["y"].as_u64().map(|v| v as u16);
    let z = params["z"].as_u64().map(|v| v as u8);
    let ground_id = params["ground_id"].as_u64().map(|v| v as u16);

    let (Some(x), Some(y), Some(z)) = (x, y, z) else {
        return ApiResponse::error("Missing x, y, or z");
    };

    if state.map_data.is_none() {
        return ApiResponse::error("No map loaded");
    }

    let undo = {
        let map = state.map_data.as_mut().unwrap();
        let before = map.get_tile(x, y, z).cloned();

        let tile = map.get_tile_mut(x, y, z);
        tile.ground = ground_id;

        // Set items if provided
        if let Some(items) = params["items"].as_array() {
            tile.items.clear();
            for item_val in items {
                if let Some(id) = item_val["id"].as_u64() {
                    let mut item = pte_otbm::MapItem::new(id as u16);
                    if let Some(aid) = item_val["action_id"].as_u64() {
                        item.action_id = Some(aid as u16);
                    }
                    if let Some(uid) = item_val["unique_id"].as_u64() {
                        item.unique_id = Some(uid as u16);
                    }
                    tile.items.push(item);
                }
            }
        }

        // Set flags if provided
        if let Some(flags) = params["flags"].as_u64() {
            tile.flags = pte_otbm::TileFlags::from_u32(flags as u32);
        }

        let after = map.get_tile(x, y, z).cloned();
        crate::state::UndoAction {
            tiles_before: vec![(x, y, z, before)],
            tiles_after: vec![(x, y, z, after)],
        }
    };

    state.push_undo(undo);
    ApiResponse::success(json!({"set": true}))
}

fn handle_remove_tile(state: &mut EditorState, params: &Value) -> ApiResponse {
    let x = params["x"].as_u64().map(|v| v as u16);
    let y = params["y"].as_u64().map(|v| v as u16);
    let z = params["z"].as_u64().map(|v| v as u8);

    let (Some(x), Some(y), Some(z)) = (x, y, z) else {
        return ApiResponse::error("Missing x, y, or z");
    };

    if state.map_data.is_none() {
        return ApiResponse::error("No map loaded");
    }

    let before = {
        let map = state.map_data.as_mut().unwrap();
        let before = map.get_tile(x, y, z).cloned();
        map.remove_tile(x, y, z);
        before
    };

    if before.is_some() {
        state.push_undo(crate::state::UndoAction {
            tiles_before: vec![(x, y, z, before)],
            tiles_after: vec![(x, y, z, None)],
        });
    }

    ApiResponse::success(json!({"removed": true}))
}

fn handle_add_item(state: &mut EditorState, params: &Value) -> ApiResponse {
    let x = params["x"].as_u64().map(|v| v as u16);
    let y = params["y"].as_u64().map(|v| v as u16);
    let z = params["z"].as_u64().map(|v| v as u8);
    let item_id = params["item_id"].as_u64().map(|v| v as u16);

    let (Some(x), Some(y), Some(z), Some(item_id)) = (x, y, z, item_id) else {
        return ApiResponse::error("Missing x, y, z, or item_id");
    };

    if state.map_data.is_none() {
        return ApiResponse::error("No map loaded");
    }

    let undo = {
        let map = state.map_data.as_mut().unwrap();
        let before = map.get_tile(x, y, z).cloned();

        let tile = map.get_tile_mut(x, y, z);
        let mut item = pte_otbm::MapItem::new(item_id);
        if let Some(aid) = params["action_id"].as_u64() {
            item.action_id = Some(aid as u16);
        }
        if let Some(uid) = params["unique_id"].as_u64() {
            item.unique_id = Some(uid as u16);
        }
        tile.items.push(item);

        let after = map.get_tile(x, y, z).cloned();
        crate::state::UndoAction {
            tiles_before: vec![(x, y, z, before)],
            tiles_after: vec![(x, y, z, after)],
        }
    };

    state.push_undo(undo);
    ApiResponse::success(json!({"added": true}))
}

fn handle_fill_area(state: &mut EditorState, params: &Value) -> ApiResponse {
    let x1 = params["x1"].as_u64().map(|v| v as u16);
    let y1 = params["y1"].as_u64().map(|v| v as u16);
    let x2 = params["x2"].as_u64().map(|v| v as u16);
    let y2 = params["y2"].as_u64().map(|v| v as u16);
    let z = params["z"].as_u64().map(|v| v as u8);
    let item_id = params["item_id"].as_u64().map(|v| v as u16);

    let (Some(x1), Some(y1), Some(x2), Some(y2), Some(z), Some(item_id)) =
        (x1, y1, x2, y2, z, item_id)
    else {
        return ApiResponse::error("Missing x1, y1, x2, y2, z, or item_id");
    };

    // Limit fill area to 10000 tiles to prevent DoS
    let width = (x2.max(x1) - x1.min(x2)) as u32 + 1;
    let height = (y2.max(y1) - y1.min(y2)) as u32 + 1;
    if width * height > 10_000 {
        return ApiResponse::error("Area too large (max 10000 tiles)");
    }

    if state.map_data.is_none() {
        return ApiResponse::error("No map loaded");
    }

    let min_x = x1.min(x2);
    let max_x = x1.max(x2);
    let min_y = y1.min(y2);
    let max_y = y1.max(y2);

    let (count, undo) = {
        let map = state.map_data.as_mut().unwrap();
        let mut count = 0u32;
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                tiles_before.push((x, y, z, map.get_tile(x, y, z).cloned()));
                let tile = map.get_tile_mut(x, y, z);
                tile.ground = Some(item_id);
                tiles_after.push((x, y, z, Some(tile.clone())));
                count += 1;
            }
        }
        (
            count,
            crate::state::UndoAction {
                tiles_before,
                tiles_after,
            },
        )
    };

    if !undo.tiles_before.is_empty() {
        state.push_undo(undo);
    }

    ApiResponse::success(json!({"count": count}))
}

fn handle_get_tiles_in_area(state: &EditorState, params: &Value) -> ApiResponse {
    let x1 = params["x1"].as_u64().map(|v| v as u16);
    let y1 = params["y1"].as_u64().map(|v| v as u16);
    let x2 = params["x2"].as_u64().map(|v| v as u16);
    let y2 = params["y2"].as_u64().map(|v| v as u16);
    let z = params["z"].as_u64().map(|v| v as u8);

    let (Some(x1), Some(y1), Some(x2), Some(y2), Some(z)) = (x1, y1, x2, y2, z) else {
        return ApiResponse::error("Missing x1, y1, x2, y2, or z");
    };

    let Some(map) = &state.map_data else {
        return ApiResponse::error("No map loaded");
    };

    let tiles = map.get_tiles_in_area(x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2), z);
    let json_tiles: Vec<Value> = tiles.iter().map(|t| tile_to_json(t)).collect();

    ApiResponse::success(json!({
        "tiles": json_tiles,
        "count": json_tiles.len(),
    }))
}

fn handle_replace_item(state: &mut EditorState, params: &Value) -> ApiResponse {
    let find_id = params["find_id"].as_u64().map(|v| v as u16);
    let replace_id = params["replace_id"].as_u64().map(|v| v as u16);
    let z_filter = params["z"].as_u64().map(|v| v as u8);

    let (Some(find_id), Some(replace_id)) = (find_id, replace_id) else {
        return ApiResponse::error("Missing find_id or replace_id");
    };

    if state.map_data.is_none() {
        return ApiResponse::error("No map loaded");
    };

    let (count, undo) = {
        let map = state.map_data.as_mut().unwrap();
        let mut count = 0u32;
        let mut tiles_before = Vec::new();
        let mut tiles_after = Vec::new();
        for chunk in map.chunks.values_mut() {
            for tile in chunk.values_mut() {
                if let Some(z) = z_filter {
                    if tile.z != z {
                        continue;
                    }
                }
                let mut changed = false;
                let before = tile.clone();
                if tile.ground == Some(find_id) {
                    tile.ground = Some(replace_id);
                    count += 1;
                    changed = true;
                }
                for item in &mut tile.items {
                    if item.id == find_id {
                        item.id = replace_id;
                        count += 1;
                        changed = true;
                    }
                }
                if changed {
                    tiles_before.push((tile.x, tile.y, tile.z, Some(before)));
                    tiles_after.push((tile.x, tile.y, tile.z, Some(tile.clone())));
                }
            }
        }
        (
            count,
            crate::state::UndoAction {
                tiles_before,
                tiles_after,
            },
        )
    };

    if !undo.tiles_before.is_empty() {
        state.push_undo(undo);
    }

    ApiResponse::success(json!({"count": count}))
}

fn handle_select_item(state: &mut EditorState, params: &Value) -> ApiResponse {
    let id = params["id"].as_u64().map(|v| v as u32);
    let Some(id) = id else {
        return ApiResponse::error("Missing id");
    };
    state.selected_item_id = Some(id);
    ApiResponse::success(json!({"selected": id}))
}

fn handle_move_camera(state: &mut EditorState, params: &Value) -> ApiResponse {
    if let Some(x) = params["x"].as_f64() {
        state.camera.center_x = x;
    }
    if let Some(y) = params["y"].as_f64() {
        state.camera.center_y = y;
    }
    if let Some(z) = params["z"].as_u64() {
        state.camera.z_level = z as u8;
    }
    ApiResponse::success(json!({"moved": true}))
}

fn handle_get_metadata(state: &EditorState) -> ApiResponse {
    let Some(map) = &state.map_data else {
        return ApiResponse::error("No map loaded");
    };

    ApiResponse::success(json!({
        "width": map.width,
        "height": map.height,
        "description": map.description,
        "spawnFile": map.spawn_file,
        "houseFile": map.house_file,
        "tileCount": map.tile_count(),
        "townCount": map.towns.len(),
        "waypointCount": map.waypoints.len(),
    }))
}

fn handle_undo(state: &mut EditorState) -> ApiResponse {
    if state.can_undo() {
        state.undo();
        ApiResponse::success(json!({"undone": true}))
    } else {
        ApiResponse::error("Nothing to undo")
    }
}

fn handle_redo(state: &mut EditorState) -> ApiResponse {
    if state.can_redo() {
        state.redo();
        ApiResponse::success(json!({"redone": true}))
    } else {
        ApiResponse::error("Nothing to redo")
    }
}

fn handle_search_appearances(state: &EditorState, params: &Value) -> ApiResponse {
    let query = params["query"].as_str().unwrap_or("");
    let limit = params["limit"].as_u64().unwrap_or(50).min(200) as usize;

    let Some(appearances) = &state.appearances else {
        return ApiResponse::error("Appearances not loaded");
    };

    let query_lower = query.to_lowercase();
    let query_num: Option<u32> = query.parse().ok();

    let mut results = Vec::new();
    for obj in appearances.objects.values() {
        let obj_id = obj.id.unwrap_or(0);
        if let Some(num) = query_num {
            if obj_id == num {
                results.push(json!({
                    "id": obj_id,
                    "name": obj.name.as_deref(),
                }));
                if results.len() >= limit {
                    break;
                }
                continue;
            }
        }
        if let Some(ref name) = obj.name {
            if name.to_lowercase().contains(&query_lower) {
                results.push(json!({
                    "id": obj_id,
                    "name": name,
                }));
                if results.len() >= limit {
                    break;
                }
            }
        }
    }

    ApiResponse::success(json!({
        "results": results,
        "total": results.len(),
    }))
}

fn handle_map_stats(state: &EditorState) -> ApiResponse {
    let Some(map) = &state.map_data else {
        return ApiResponse::error("No map loaded");
    };

    let z_levels = map.occupied_z_levels();
    let z_range = map.z_range();

    ApiResponse::success(json!({
        "tileCount": map.tile_count(),
        "chunkCount": map.chunks.len(),
        "zLevels": z_levels,
        "zRange": z_range.map(|(min, max)| json!({"min": min, "max": max})),
        "townCount": map.towns.len(),
        "waypointCount": map.waypoints.len(),
    }))
}

fn handle_open_map(state: &mut EditorState, params: &Value) -> ApiResponse {
    let Some(path_str) = params["path"].as_str() else {
        return ApiResponse::error("Missing path parameter");
    };
    let path = std::path::Path::new(path_str);
    if !path.exists() {
        return ApiResponse::error(format!("File not found: {}", path_str));
    }

    match pte_otbm::parse_otbm(path) {
        Ok(map) => {
            let tile_count = map.tile_count();
            let town_count = map.towns.len();
            state.map_data = Some(map);
            state.cache_dirty = true;
            ApiResponse::success(json!({
                "loaded": true,
                "tileCount": tile_count,
                "townCount": town_count,
            }))
        }
        Err(e) => ApiResponse::error(format!("Failed to parse OTBM: {}", e)),
    }
}

fn handle_load_assets(state: &mut EditorState, params: &Value) -> ApiResponse {
    let Some(path_str) = params["path"].as_str() else {
        return ApiResponse::error("Missing path parameter");
    };
    let dir = std::path::Path::new(path_str);
    if !dir.exists() || !dir.is_dir() {
        return ApiResponse::error(format!("Directory not found: {}", path_str));
    }
    // Queue asset loading — the update loop will pick this up
    state.pending_api_asset_load = Some(dir.to_path_buf());
    ApiResponse::success(json!({"queued": true}))
}

fn handle_load_chunk_dir(state: &mut EditorState, params: &Value) -> ApiResponse {
    let Some(path_str) = params["path"].as_str() else {
        return ApiResponse::error("Missing path parameter");
    };
    let dir = std::path::Path::new(path_str);
    if !dir.exists() || !dir.is_dir() {
        return ApiResponse::error(format!("Directory not found: {}", path_str));
    }

    // Read manifest for metadata without loading all tiles
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return ApiResponse::error("manifest.json not found in chunk directory");
    }

    let manifest_data = match std::fs::read_to_string(&manifest_path) {
        Ok(d) => d,
        Err(e) => return ApiResponse::error(format!("Failed to read manifest: {}", e)),
    };
    let manifest: pte_otbm::chunk_io::ChunkManifest = match serde_json::from_str(&manifest_data) {
        Ok(m) => m,
        Err(e) => return ApiResponse::error(format!("Failed to parse manifest: {}", e)),
    };

    // Create an empty map with metadata from manifest
    let mut map = pte_otbm::MapData::new();
    map.width = manifest.world_width;
    map.height = manifest.world_height;
    map.description = manifest.description.clone();
    map.spawn_file = manifest.spawn_file.clone();
    map.house_file = manifest.house_file.clone();
    map.version = 3;
    map.item_major_version = 3;
    map.item_minor_version = 56;

    for t in &manifest.towns {
        map.towns.push(pte_otbm::Town {
            id: t.id,
            name: t.name.clone(),
            position: pte_otbm::Position { x: t.x, y: t.y, z: t.z },
        });
    }
    for wp in &manifest.waypoints {
        map.waypoints.push(pte_otbm::Waypoint {
            name: wp.name.clone(),
            position: pte_otbm::Position { x: wp.x, y: wp.y, z: wp.z },
        });
    }

    // Load only chunks near the camera (viewport-based loading)
    // Default: load chunks within 1024 tiles of camera center
    let cam_x = params["x"].as_f64().unwrap_or(state.camera.center_x) as i32;
    let cam_y = params["y"].as_f64().unwrap_or(state.camera.center_y) as i32;
    let radius = params["radius"].as_i64().unwrap_or(1024) as i32;
    let z_filter = params["z"].as_u64().map(|v| v as u8);

    let mut loaded_count = 0;
    for entry in &manifest.chunks {
        if let Some(z) = z_filter {
            if entry.z != z { continue; }
        }
        // Check if chunk is within radius of camera
        let chunk_cx = entry.base_x as i32 + 128; // center of 256-tile chunk
        let chunk_cy = entry.base_y as i32 + 128;
        let dx = (chunk_cx - cam_x).abs();
        let dy = (chunk_cy - cam_y).abs();
        if dx > radius || dy > radius { continue; }

        let chunk_path = dir.join(&entry.path);
        if !chunk_path.exists() { continue; }

        if let Err(e) = pte_otbm::chunk_io::load_chunk_into(&mut map, &chunk_path) {
            tracing::warn!("Failed to load chunk {}: {}", entry.path, e);
        }
        loaded_count += 1;
    }

    let tile_count = map.tile_count();
    let chunk_count = map.chunks.len();
    let town_count = map.towns.len();

    // Store the chunk dir path for later use (loading more chunks on camera move)
    state.chunk_dir = Some(path_str.to_string());
    state.chunk_manifest = Some(manifest);
    state.map_data = Some(map);

    // Move camera to first town or center of loaded area
    if let Some(town) = state.map_data.as_ref().and_then(|m| m.towns.first()) {
        state.camera.center_x = town.position.x as f64;
        state.camera.center_y = town.position.y as f64;
        state.camera.z_level = town.position.z;
    }

    ApiResponse::success(json!({
        "loaded": true,
        "tileCount": tile_count,
        "chunkCount": chunk_count,
        "chunksLoaded": loaded_count,
        "chunksTotal": state.chunk_manifest.as_ref().map_or(0, |m| m.chunks.len()),
        "townCount": town_count,
        "cameraX": cam_x,
        "cameraY": cam_y,
    }))
}

fn handle_save_chunk_dir(state: &EditorState, params: &Value) -> ApiResponse {
    let Some(path_str) = params["path"].as_str() else {
        return ApiResponse::error("Missing path parameter");
    };
    let Some(map) = &state.map_data else {
        return ApiResponse::error("No map loaded");
    };
    let dir = std::path::Path::new(path_str);

    match pte_otbm::chunk_io::save_chunk_dir(map, dir) {
        Ok(count) => ApiResponse::success(json!({
            "saved": true,
            "chunkCount": count,
        })),
        Err(e) => ApiResponse::error(format!("Failed to save chunk dir: {}", e)),
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn tile_to_json(tile: &pte_otbm::Tile) -> Value {
    let items: Vec<Value> = tile
        .items
        .iter()
        .map(|item| {
            let mut v = json!({"id": item.id});
            if let Some(aid) = item.action_id {
                v["action_id"] = json!(aid);
            }
            if let Some(uid) = item.unique_id {
                v["unique_id"] = json!(uid);
            }
            if let Some(ref text) = item.text {
                v["text"] = json!(text);
            }
            if let Some(ref desc) = item.description {
                v["description"] = json!(desc);
            }
            if let Some(count) = item.count {
                v["count"] = json!(count);
            }
            if let Some(dest) = &item.tele_dest {
                v["tele_dest"] = json!({"x": dest.x, "y": dest.y, "z": dest.z});
            }
            v
        })
        .collect();

    json!({
        "x": tile.x,
        "y": tile.y,
        "z": tile.z,
        "ground": tile.ground,
        "items": items,
        "flags": tile.flags.to_u32(),
        "house_id": tile.house_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_state_with_map() -> EditorState {
        let mut state = EditorState::new();
        let mut map = pte_otbm::MapData::new();
        map.width = 1024;
        map.height = 1024;
        map.version = 2;
        map.description = "Test".to_string();

        let mut tile = pte_otbm::Tile::new(100, 200, 7);
        tile.ground = Some(4526);
        let mut item = pte_otbm::MapItem::new(2160);
        item.action_id = Some(42);
        tile.items.push(item);
        map.set_tile(tile);

        map.towns.push(pte_otbm::Town {
            id: 1,
            name: "TestTown".to_string(),
            position: pte_otbm::Position {
                x: 100,
                y: 200,
                z: 7,
            },
        });

        state.map_data = Some(map);
        state
    }

    fn req(method: &str, params: Value) -> ApiRequest {
        ApiRequest {
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn status_reports_map_loaded() {
        let mut state = make_state_with_map();
        let resp = handle_request(&mut state, &req("status", json!({})));
        assert!(resp.ok);
        let data = resp.data.unwrap();
        assert_eq!(data["mapLoaded"], true);
        assert_eq!(data["tileCount"], 1);
    }

    #[test]
    fn status_reports_no_map_when_empty() {
        let mut state = EditorState::new();
        let resp = handle_request(&mut state, &req("status", json!({})));
        assert!(resp.ok);
        assert_eq!(resp.data.unwrap()["mapLoaded"], false);
    }

    #[test]
    fn get_tile_returns_tile_data() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req("get_tile", json!({"x": 100, "y": 200, "z": 7})),
        );
        assert!(resp.ok);
        let data = resp.data.unwrap();
        assert_eq!(data["ground"], 4526);
        assert_eq!(data["items"][0]["id"], 2160);
        assert_eq!(data["items"][0]["action_id"], 42);
    }

    #[test]
    fn get_tile_returns_null_for_empty() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req("get_tile", json!({"x": 999, "y": 999, "z": 7})),
        );
        assert!(resp.ok);
        assert!(resp.data.unwrap().is_null());
    }

    #[test]
    fn get_tile_missing_params_errors() {
        let mut state = make_state_with_map();
        let resp = handle_request(&mut state, &req("get_tile", json!({"x": 100})));
        assert!(!resp.ok);
    }

    #[test]
    fn set_tile_creates_tile() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req(
                "set_tile",
                json!({"x": 50, "y": 50, "z": 7, "ground_id": 100}),
            ),
        );
        assert!(resp.ok);

        let map = state.map_data.as_ref().unwrap();
        let tile = map.get_tile(50, 50, 7).unwrap();
        assert_eq!(tile.ground, Some(100));
    }

    #[test]
    fn set_tile_with_items() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req(
                "set_tile",
                json!({
                    "x": 60, "y": 60, "z": 7,
                    "ground_id": 100,
                    "items": [{"id": 2160, "action_id": 99}]
                }),
            ),
        );
        assert!(resp.ok);

        let tile = state
            .map_data
            .as_ref()
            .unwrap()
            .get_tile(60, 60, 7)
            .unwrap();
        assert_eq!(tile.items.len(), 1);
        assert_eq!(tile.items[0].action_id, Some(99));
    }

    #[test]
    fn remove_tile_works() {
        let mut state = make_state_with_map();
        assert!(state
            .map_data
            .as_ref()
            .unwrap()
            .get_tile(100, 200, 7)
            .is_some());

        let resp = handle_request(
            &mut state,
            &req("remove_tile", json!({"x": 100, "y": 200, "z": 7})),
        );
        assert!(resp.ok);
        assert!(state
            .map_data
            .as_ref()
            .unwrap()
            .get_tile(100, 200, 7)
            .is_none());
    }

    #[test]
    fn add_item_appends_to_tile() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req(
                "add_item",
                json!({"x": 100, "y": 200, "z": 7, "item_id": 3000}),
            ),
        );
        assert!(resp.ok);

        let tile = state
            .map_data
            .as_ref()
            .unwrap()
            .get_tile(100, 200, 7)
            .unwrap();
        assert_eq!(tile.items.len(), 2);
        assert_eq!(tile.items[1].id, 3000);
    }

    #[test]
    fn fill_area_fills_ground() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req(
                "fill_area",
                json!({"x1": 0, "y1": 0, "x2": 2, "y2": 2, "z": 7, "item_id": 100}),
            ),
        );
        assert!(resp.ok);
        assert_eq!(resp.data.unwrap()["count"], 9);

        let map = state.map_data.as_ref().unwrap();
        for x in 0..=2u16 {
            for y in 0..=2u16 {
                assert_eq!(map.get_tile(x, y, 7).unwrap().ground, Some(100));
            }
        }
    }

    #[test]
    fn fill_area_rejects_too_large() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req(
                "fill_area",
                json!({"x1": 0, "y1": 0, "x2": 200, "y2": 200, "z": 7, "item_id": 100}),
            ),
        );
        assert!(!resp.ok);
        assert!(resp.error.unwrap().contains("too large"));
    }

    #[test]
    fn replace_item_replaces_ground_and_items() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req("replace_item", json!({"find_id": 4526, "replace_id": 9999})),
        );
        assert!(resp.ok);
        assert_eq!(resp.data.unwrap()["count"], 1);

        let tile = state
            .map_data
            .as_ref()
            .unwrap()
            .get_tile(100, 200, 7)
            .unwrap();
        assert_eq!(tile.ground, Some(9999));
    }

    #[test]
    fn select_item_sets_selection() {
        let mut state = make_state_with_map();
        let resp = handle_request(&mut state, &req("select_item", json!({"id": 42})));
        assert!(resp.ok);
        assert_eq!(state.selected_item_id, Some(42));
    }

    #[test]
    fn move_camera_updates_state() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req("move_camera", json!({"x": 500.0, "y": 600.0, "z": 3})),
        );
        assert!(resp.ok);
        assert_eq!(state.camera.center_x, 500.0);
        assert_eq!(state.camera.center_y, 600.0);
        assert_eq!(state.camera.z_level, 3);
    }

    #[test]
    fn get_metadata_returns_map_info() {
        let mut state = make_state_with_map();
        let resp = handle_request(&mut state, &req("get_metadata", json!({})));
        assert!(resp.ok);
        let data = resp.data.unwrap();
        assert_eq!(data["width"], 1024);
        assert_eq!(data["description"], "Test");
        assert_eq!(data["townCount"], 1);
    }

    #[test]
    fn get_metadata_errors_without_map() {
        let mut state = EditorState::new();
        let resp = handle_request(&mut state, &req("get_metadata", json!({})));
        assert!(!resp.ok);
    }

    #[test]
    fn unknown_method_returns_error() {
        let mut state = EditorState::new();
        let resp = handle_request(&mut state, &req("nonexistent", json!({})));
        assert!(!resp.ok);
        assert!(resp.error.unwrap().contains("Unknown method"));
    }

    #[test]
    fn get_tiles_in_area_returns_matching() {
        let mut state = make_state_with_map();
        let resp = handle_request(
            &mut state,
            &req(
                "get_tiles_in_area",
                json!({"x1": 99, "y1": 199, "x2": 101, "y2": 201, "z": 7}),
            ),
        );
        assert!(resp.ok);
        let data = resp.data.unwrap();
        assert_eq!(data["count"], 1);
        assert_eq!(data["tiles"][0]["x"], 100);
    }

    #[test]
    fn map_stats_returns_counts() {
        let mut state = make_state_with_map();
        let resp = handle_request(&mut state, &req("map_stats", json!({})));
        assert!(resp.ok);
        let data = resp.data.unwrap();
        assert_eq!(data["tileCount"], 1);
        assert_eq!(data["townCount"], 1);
    }
}
