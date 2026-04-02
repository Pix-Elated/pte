//! Sprite sheet grid layout and individual sprite extraction.

use crate::catalog::SpriteType;

/// Standard sprite size in pixels (most sprites).
pub const SPRITE_SIZE: u32 = 32;

/// Maximum sheet texture size (CIP standard).
const SHEET_SIZE: u32 = 384;

/// A fully decompressed sprite sheet.
#[derive(Debug)]
pub struct SpriteSheet {
    pub first_sprite_id: u32,
    pub last_sprite_id: u32,
    pub sprite_type: SpriteType,
    pub width: u32,
    pub height: u32,
    /// RGBA pixel data, row-major, top-down.
    pub pixels: Vec<u8>,
}

impl SpriteSheet {
    /// Number of sprites in this sheet.
    pub fn sprite_count(&self) -> u32 {
        self.last_sprite_id - self.first_sprite_id + 1
    }

    /// Individual sprite dimensions for this sheet type.
    pub fn sprite_dimensions(&self) -> (u32, u32) {
        self.sprite_type.sprite_dimensions()
    }

    /// Number of columns in the sprite grid.
    pub fn columns(&self) -> u32 {
        let (sw, _) = self.sprite_dimensions();
        self.width / sw
    }

    /// Extract a single sprite's RGBA pixels.
    /// Returns None if the sprite ID is out of range.
    pub fn get_sprite(&self, sprite_id: u32) -> Option<Vec<u8>> {
        if sprite_id < self.first_sprite_id || sprite_id > self.last_sprite_id {
            return None;
        }

        let local_id = sprite_id - self.first_sprite_id;
        let (sw, sh) = self.sprite_dimensions();
        let cols = self.columns();
        let col = local_id % cols;
        let row = local_id / cols;

        let x0 = (col * sw) as usize;
        let y0 = (row * sh) as usize;
        let row_stride = (self.width * 4) as usize;

        let mut sprite_pixels = vec![0u8; (sw * sh * 4) as usize];
        for y in 0..sh as usize {
            let src_start = (y0 + y) * row_stride + x0 * 4;
            let src_end = src_start + (sw as usize) * 4;
            let dst_start = y * (sw as usize) * 4;
            let dst_end = dst_start + (sw as usize) * 4;
            sprite_pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
        }

        Some(sprite_pixels)
    }

    /// Get the pixel rect (x, y, w, h) for a sprite ID within the sheet texture.
    pub fn sprite_rect(&self, sprite_id: u32) -> Option<(u32, u32, u32, u32)> {
        if sprite_id < self.first_sprite_id || sprite_id > self.last_sprite_id {
            return None;
        }

        let local_id = sprite_id - self.first_sprite_id;
        let (sw, sh) = self.sprite_dimensions();
        let cols = self.columns();
        let col = local_id % cols;
        let row = local_id / cols;

        Some((col * sw, row * sh, sw, sh))
    }

    /// Replace a single sprite's RGBA pixels within the sheet.
    /// Returns false if sprite_id is out of range.
    pub fn set_sprite(&mut self, sprite_id: u32, sprite_pixels: &[u8]) -> bool {
        let (x0, y0, sw, sh) = match self.sprite_rect(sprite_id) {
            Some(r) => r,
            None => return false,
        };

        let expected = (sw * sh * 4) as usize;
        if sprite_pixels.len() != expected {
            return false;
        }

        let row_stride = (self.width * 4) as usize;
        let x0 = x0 as usize;
        let y0 = y0 as usize;

        for y in 0..sh as usize {
            let src_start = y * (sw as usize) * 4;
            let src_end = src_start + (sw as usize) * 4;
            let dst_start = (y0 + y) * row_stride + x0 * 4;
            let dst_end = dst_start + (sw as usize) * 4;
            self.pixels[dst_start..dst_end].copy_from_slice(&sprite_pixels[src_start..src_end]);
        }

        true
    }

    /// Clear a single sprite (make it transparent). Returns false if out of range.
    pub fn delete_sprite(&mut self, sprite_id: u32) -> bool {
        let (sw, sh) = self.sprite_dimensions();
        let blank = vec![0u8; (sw * sh * 4) as usize];
        self.set_sprite(sprite_id, &blank)
    }
}

/// Calculate sheet dimensions for a given sprite count and type.
pub fn sheet_dimensions(sprite_count: u32, sprite_type: SpriteType) -> (u32, u32) {
    let (sw, sh) = sprite_type.sprite_dimensions();
    let cols = SHEET_SIZE / sw;
    let rows = sprite_count.div_ceil(cols);
    (cols * sw, rows * sh)
}
