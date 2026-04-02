use crate::state::TileSelection;

/// Start or update a selection rectangle.
pub fn update_selection(start_x: u16, start_y: u16, end_x: u16, end_y: u16) -> TileSelection {
    TileSelection {
        x1: start_x.min(end_x),
        y1: start_y.min(end_y),
        x2: start_x.max(end_x),
        y2: start_y.max(end_y),
    }
}
