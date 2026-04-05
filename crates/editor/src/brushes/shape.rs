//! Brush shapes — circular and rectangular masks matching RME sizes 0–7.
//!
//! RME uses pre-baked bitmaps for circular brushes. We replicate the same
//! approach: each size has a fixed pattern of offsets from the center tile.

/// Shape variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushShape {
    Square,
    Circle,
}

/// Generate the list of (dx, dy) offsets for a brush of the given shape and size.
/// Size 0 = single tile, size 1 = 3×3, up to size 7.
pub fn brush_offsets(shape: BrushShape, size: u32) -> Vec<(i32, i32)> {
    match shape {
        BrushShape::Square => square_offsets(size),
        BrushShape::Circle => circle_offsets(size),
    }
}

fn square_offsets(size: u32) -> Vec<(i32, i32)> {
    let radius = (size / 2) as i32;
    let mut out = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            out.push((dx, dy));
        }
    }
    out
}

/// Circular brush masks matching RME's pre-baked patterns.
/// RME circular sizes:
///   0: single tile (1×1)
///   1: 3×3 diamond (5 tiles)
///   2: 3×3 full (9 tiles)
///   3: 5×5 circle (21 tiles)
///   4: 7×7 circle (37 tiles)
///   5: 9×9 circle (69 tiles)
///   6: 11×11 circle (97 tiles)
///   7: 13×13 circle (129 tiles)
fn circle_offsets(size: u32) -> Vec<(i32, i32)> {
    match size {
        0 => vec![(0, 0)],
        1 => {
            // 3×3 diamond
            vec![(0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)]
        }
        2 => {
            // 3×3 full
            let mut v = Vec::new();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    v.push((dx, dy));
                }
            }
            v
        }
        _ => {
            // For sizes 3–7, use Bresenham-style circle fill
            let radius = match size {
                3 => 2.0_f64,
                4 => 3.0,
                5 => 4.0,
                6 => 5.0,
                7 => 6.0,
                _ => (size as f64) - 1.0,
            };
            let r2 = (radius + 0.5) * (radius + 0.5);
            let ir = radius.ceil() as i32;
            let mut v = Vec::new();
            for dy in -ir..=ir {
                for dx in -ir..=ir {
                    let dist2 = (dx as f64) * (dx as f64) + (dy as f64) * (dy as f64);
                    if dist2 <= r2 {
                        v.push((dx, dy));
                    }
                }
            }
            v
        }
    }
}

/// Expand brush offsets around a center, clamped to u16 range.
pub fn expand_positions(
    center_x: u16,
    center_y: u16,
    z: u8,
    offsets: &[(i32, i32)],
) -> Vec<(u16, u16, u8)> {
    offsets
        .iter()
        .filter_map(|&(dx, dy)| {
            let x = center_x as i32 + dx;
            let y = center_y as i32 + dy;
            if x >= 0 && x <= u16::MAX as i32 && y >= 0 && y <= u16::MAX as i32 {
                Some((x as u16, y as u16, z))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_tile() {
        assert_eq!(brush_offsets(BrushShape::Circle, 0), vec![(0, 0)]);
        assert_eq!(brush_offsets(BrushShape::Square, 1), vec![(0, 0)]);
    }

    #[test]
    fn diamond_shape() {
        let offsets = brush_offsets(BrushShape::Circle, 1);
        assert_eq!(offsets.len(), 5);
        assert!(offsets.contains(&(0, 0)));
        assert!(offsets.contains(&(0, -1)));
    }

    #[test]
    fn square_3x3() {
        let offsets = brush_offsets(BrushShape::Square, 3);
        assert_eq!(offsets.len(), 9);
    }

    #[test]
    fn circle_size_3() {
        let offsets = brush_offsets(BrushShape::Circle, 3);
        // 5×5 minus 4 corners = 21 tiles
        assert!(offsets.len() >= 13);
        // Center should be included
        assert!(offsets.contains(&(0, 0)));
        // Corners at (2,2) should NOT be included for radius 2
        assert!(!offsets.contains(&(2, 2)));
    }
}
