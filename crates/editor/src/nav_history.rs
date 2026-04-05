//! Navigation history — back/forward position stack.

use crate::state::EditorState;

const MAX_NAV_HISTORY: usize = 50;
/// Minimum distance (in tiles) to record a new position.
const MIN_DISTANCE: f64 = 10.0;

/// A recorded camera position.
#[derive(Debug, Clone, Copy)]
pub struct NavEntry {
    pub x: f64,
    pub y: f64,
    pub z: u8,
}

/// Navigation history state.
#[derive(Debug, Clone, Default)]
pub struct NavHistory {
    entries: Vec<NavEntry>,
    cursor: usize,
}

impl NavHistory {
    /// Record the current camera position (call on significant navigation actions).
    pub fn push(&mut self, x: f64, y: f64, z: u8) {
        // Skip if too close to the current entry
        if let Some(current) = self.entries.get(self.cursor.saturating_sub(1)) {
            let dx = (current.x - x).abs();
            let dy = (current.y - y).abs();
            if dx < MIN_DISTANCE && dy < MIN_DISTANCE && current.z == z {
                return;
            }
        }

        // Truncate forward history
        self.entries.truncate(self.cursor);
        self.entries.push(NavEntry { x, y, z });
        if self.entries.len() > MAX_NAV_HISTORY {
            self.entries.remove(0);
        }
        self.cursor = self.entries.len();
    }

    /// Go back in history. Returns the position to navigate to.
    pub fn go_back(&mut self) -> Option<NavEntry> {
        if self.cursor > 1 {
            self.cursor -= 1;
            Some(self.entries[self.cursor - 1])
        } else {
            None
        }
    }

    /// Go forward in history. Returns the position to navigate to.
    pub fn go_forward(&mut self) -> Option<NavEntry> {
        if self.cursor < self.entries.len() {
            let entry = self.entries[self.cursor];
            self.cursor += 1;
            Some(entry)
        } else {
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 1
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor < self.entries.len()
    }
}

/// Navigate back. Call from hotkey or toolbar.
pub fn go_back(state: &mut EditorState) {
    // Record current position before going back
    let cur = NavEntry {
        x: state.camera.center_x,
        y: state.camera.center_y,
        z: state.camera.z_level,
    };
    // Push current if we haven't already
    if state.nav_history.entries.is_empty()
        || state.nav_history.cursor == state.nav_history.entries.len()
    {
        state.nav_history.push(cur.x, cur.y, cur.z);
    }

    if let Some(entry) = state.nav_history.go_back() {
        state.camera.center_x = entry.x;
        state.camera.center_y = entry.y;
        state.camera.z_level = entry.z;
    }
}

/// Navigate forward.
pub fn go_forward(state: &mut EditorState) {
    if let Some(entry) = state.nav_history.go_forward() {
        state.camera.center_x = entry.x;
        state.camera.center_y = entry.y;
        state.camera.z_level = entry.z;
    }
}

/// Record a navigation point (call after go-to, find-jump, town-goto, etc.)
pub fn record(state: &mut EditorState) {
    state.nav_history.push(
        state.camera.center_x,
        state.camera.center_y,
        state.camera.z_level,
    );
}
