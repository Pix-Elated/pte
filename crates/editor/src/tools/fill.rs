use std::collections::VecDeque;
use pte_otbm::MapData;
use crate::state::UndoAction;

/// Flood fill (4-connected) at the given position.
/// Replaces ground items matching the target with the new item.
const MAX_FILL_TILES: usize = 10_000;

pub fn apply_fill(
    map: &mut MapData,
    start_x: u16,
    start_y: u16,
    z: u8,
    new_item_id: u16,
) -> UndoAction {
    let target_ground = map
        .get_tile(start_x, start_y, z)
        .and_then(|t| t.ground);

    // Don't fill if already the same
    if target_ground == Some(new_item_id) {
        return UndoAction {
            tiles_before: vec![],
            tiles_after: vec![],
        };
    }

    let mut queue = VecDeque::new();
    let mut visited = std::collections::HashSet::new();
    let mut tiles_before = Vec::new();
    let mut tiles_after = Vec::new();

    queue.push_back((start_x, start_y));
    visited.insert((start_x, start_y));

    while let Some((x, y)) = queue.pop_front() {
        if tiles_before.len() >= MAX_FILL_TILES {
            break;
        }

        let current_ground = map.get_tile(x, y, z).and_then(|t| t.ground);
        if current_ground != target_ground {
            continue;
        }

        let before = map.get_tile(x, y, z).cloned();
        tiles_before.push((x, y, z, before));

        let tile = map.get_tile_mut(x, y, z);
        tile.ground = Some(new_item_id);
        tiles_after.push((x, y, z, Some(tile.clone())));

        // 4-connected neighbors
        for (dx, dy) in [(0i32, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx > u16::MAX as i32 || ny > u16::MAX as i32 {
                continue;
            }
            let np = (nx as u16, ny as u16);
            if !visited.contains(&np) {
                visited.insert(np);
                queue.push_back(np);
            }
        }
    }

    UndoAction {
        tiles_before,
        tiles_after,
    }
}
