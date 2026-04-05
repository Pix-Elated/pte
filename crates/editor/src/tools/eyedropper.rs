use crate::brushes::registry::BrushRegistry;
use crate::brushes::BrushId;
use pte_otbm::MapData;

/// Pick result from the eyedropper.
pub struct PickResult {
    /// The raw item/ground ID that was picked.
    pub item_id: u32,
    /// The brush that owns this item, if any.
    pub brush_id: Option<BrushId>,
}

/// Pick the item under the cursor and resolve its brush.
/// Returns the ground ID or top item ID, plus the owning brush if found.
pub fn pick_item(
    map: &MapData,
    x: u16,
    y: u16,
    z: u8,
    registry: &BrushRegistry,
) -> Option<PickResult> {
    let tile = map.get_tile(x, y, z)?;

    let item_id = if let Some(last_item) = tile.items.last() {
        last_item.id as u32
    } else {
        tile.ground? as u32
    };

    // Try all brush lookup maps in priority order
    let brush_id = registry
        .ground_brush_for_item(item_id as u16)
        .or_else(|| registry.wall_brush_for_item(item_id as u16))
        .map(|b| b.id());

    Some(PickResult { item_id, brush_id })
}

/// Simple pick — returns only the item ID (used by context menu, legacy).
pub fn pick_item_id(map: &MapData, x: u16, y: u16, z: u8) -> Option<u32> {
    let tile = map.get_tile(x, y, z)?;
    if let Some(last_item) = tile.items.last() {
        Some(last_item.id as u32)
    } else {
        tile.ground.map(|g| g as u32)
    }
}
