//! Performance monitor overlay — FPS, tile count, memory.

use crate::state::EditorState;
use crate::theme;

/// Tracks frame times for FPS calculation.
#[derive(Debug, Clone)]
pub struct PerfStats {
    frame_times: Vec<f64>,
    last_update: std::time::Instant,
    pub fps: f64,
    pub frame_ms: f64,
    pub visible_tiles: u64,
    pub total_tiles: u64,
}

impl Default for PerfStats {
    fn default() -> Self {
        Self {
            frame_times: Vec::with_capacity(120),
            last_update: std::time::Instant::now(),
            fps: 0.0,
            frame_ms: 0.0,
            visible_tiles: 0,
            total_tiles: 0,
        }
    }
}

impl PerfStats {
    /// Call once per frame with the frame delta in seconds.
    pub fn update(&mut self, dt: f64) {
        self.frame_times.push(dt);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }

        let now = std::time::Instant::now();
        if now.duration_since(self.last_update).as_millis() >= 500 {
            // Recalculate stats every 500ms
            if !self.frame_times.is_empty() {
                let avg = self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
                self.fps = if avg > 0.0 { 1.0 / avg } else { 0.0 };
                self.frame_ms = avg * 1000.0;
            }
            self.last_update = now;
        }
    }
}

/// Draw the performance overlay in the top-right of the viewport.
pub fn show_overlay(ctx: &egui::Context, state: &EditorState) {
    if !state.show_perf_monitor {
        return;
    }

    egui::Area::new(egui::Id::new("perf_overlay"))
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 36.0])
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(15, 15, 25, 200))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_width(140.0);

                    // FPS
                    let fps_color = if state.perf.fps >= 55.0 {
                        theme::SUCCESS
                    } else if state.perf.fps >= 30.0 {
                        theme::WARNING
                    } else {
                        theme::ERROR
                    };

                    ui.label(
                        egui::RichText::new(format!("{:.0} FPS", state.perf.fps))
                            .size(13.0)
                            .color(fps_color)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("{:.1} ms/frame", state.perf.frame_ms))
                            .size(9.5)
                            .color(theme::TEXT_MUTED),
                    );

                    ui.add_space(4.0);

                    // Tiles
                    ui.label(
                        egui::RichText::new(format!(
                            "Visible: {} tiles",
                            format_count(state.perf.visible_tiles)
                        ))
                        .size(9.5)
                        .color(theme::TEXT_SECONDARY),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "Total: {} tiles",
                            format_count(state.perf.total_tiles)
                        ))
                        .size(9.5)
                        .color(theme::TEXT_SECONDARY),
                    );

                    // Zoom
                    ui.label(
                        egui::RichText::new(format!(
                            "Zoom: {:.1}%",
                            state.camera.zoom * 100.0
                        ))
                        .size(9.5)
                        .color(theme::TEXT_MUTED),
                    );
                });
        });
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}
