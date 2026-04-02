//! BMP → RGBA conversion for CIP sprite sheets.
//!
//! CIP's decompressed sprite data is a BMP file (usually bottom-up, BGRA or BGR).
//! We convert to top-down RGBA with magenta (0xFF00FF) → transparent.

use anyhow::{bail, Result};

/// Magenta color used as transparency key in CIP sprites.
const MAGENTA: [u8; 3] = [0xFF, 0x00, 0xFF];

/// Decode raw BMP bytes into RGBA pixel data.
/// Returns a Vec<u8> of length width * height * 4.
pub fn decode_bmp_to_rgba(bmp_data: &[u8], expected_width: u32, expected_height: u32) -> Result<Vec<u8>> {
    // Use the `image` crate for robust BMP parsing.
    match image::load_from_memory_with_format(bmp_data, image::ImageFormat::Bmp) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let actual_w = rgba.width();
            let actual_h = rgba.height();

            // Apply magenta → transparent
            let mut pixels = rgba.into_raw();
            for chunk in pixels.chunks_exact_mut(4) {
                if chunk[0] == MAGENTA[0] && chunk[1] == MAGENTA[1] && chunk[2] == MAGENTA[2] {
                    chunk[3] = 0; // transparent
                }
            }

            // If BMP dimensions match expected, return directly
            if actual_w == expected_width && actual_h == expected_height {
                return Ok(pixels);
            }

            // Dimensions differ — copy into correctly-sized buffer
            // This handles CIP sheets where the BMP actual dimensions
            // don't match the calculated sheet dimensions
            let expected_len = (expected_width * expected_height * 4) as usize;
            let mut output = vec![0u8; expected_len];
            let copy_w = actual_w.min(expected_width) as usize;
            let copy_h = actual_h.min(expected_height) as usize;
            let src_stride = (actual_w * 4) as usize;
            let dst_stride = (expected_width * 4) as usize;

            for y in 0..copy_h {
                let src_start = y * src_stride;
                let dst_start = y * dst_stride;
                let copy_bytes = copy_w * 4;
                output[dst_start..dst_start + copy_bytes]
                    .copy_from_slice(&pixels[src_start..src_start + copy_bytes]);
            }

            Ok(output)
        }
        Err(_) => {
            // Fallback: raw BGRA pixel data (no BMP header).
            // Some CIP sheets are raw pixel buffers.
            decode_raw_bgra(bmp_data, expected_width, expected_height)
        }
    }
}

/// Decode raw BGRA pixel data (no header) into RGBA.
fn decode_raw_bgra(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let expected_len = (width * height * 4) as usize;
    if data.len() < expected_len {
        bail!(
            "Raw pixel data too small: {} bytes for {}x{} (need {})",
            data.len(),
            width,
            height,
            expected_len
        );
    }

    let mut rgba = vec![0u8; expected_len];

    // BMP is bottom-up, so we need to flip rows
    let row_bytes = (width * 4) as usize;
    for y in 0..height as usize {
        let src_row = (height as usize - 1 - y) * row_bytes;
        let dst_row = y * row_bytes;
        for x in 0..width as usize {
            let si = src_row + x * 4;
            let di = dst_row + x * 4;
            let b = data[si];
            let g = data[si + 1];
            let r = data[si + 2];
            let a = data[si + 3];

            // BGRA → RGBA with magenta transparency
            rgba[di] = r;
            rgba[di + 1] = g;
            rgba[di + 2] = b;
            if r == MAGENTA[0] && g == MAGENTA[1] && b == MAGENTA[2] {
                rgba[di + 3] = 0;
            } else {
                rgba[di + 3] = a;
            }
        }
    }

    Ok(rgba)
}

/// Encode RGBA pixel data to BMP bytes.
/// Transparent pixels (alpha=0) are written as magenta (0xFF00FF).
pub fn encode_rgba_to_bmp(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    use image::{ImageBuffer, Rgba, RgbaImage};

    let expected = (width * height * 4) as usize;
    if rgba.len() != expected {
        bail!("RGBA data size mismatch: got {}, expected {}", rgba.len(), expected);
    }

    // Build image, converting transparent → magenta
    let mut img: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    for pixel in img.pixels_mut() {
        if pixel[3] == 0 {
            *pixel = Rgba([MAGENTA[0], MAGENTA[1], MAGENTA[2], 0xFF]);
        }
    }

    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Bmp)
        .map_err(|e| anyhow::anyhow!("BMP encode failed: {e}"))?;

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magenta_transparency() {
        // Minimal 1x1 BMP with magenta pixel (BGRA: FF 00 FF FF)
        // Build a raw BGRA buffer
        let bgra = vec![0xFF, 0x00, 0xFF, 0xFF]; // B=FF, G=00, R=FF, A=FF
        let result = decode_raw_bgra(&bgra, 1, 1).unwrap();
        assert_eq!(result.len(), 4);
        // Should be RGBA: R=FF G=00 B=FF A=0 (transparent)
        assert_eq!(result[0], 0xFF); // R
        assert_eq!(result[1], 0x00); // G
        assert_eq!(result[2], 0xFF); // B
        assert_eq!(result[3], 0x00); // A = transparent
    }

    #[test]
    fn test_non_magenta_preserved() {
        // Green pixel: BGRA = 00 FF 00 FF
        let bgra = vec![0x00, 0xFF, 0x00, 0xFF];
        let result = decode_raw_bgra(&bgra, 1, 1).unwrap();
        assert_eq!(result, vec![0x00, 0xFF, 0x00, 0xFF]); // RGBA preserved
    }

    #[test]
    fn test_raw_bgra_too_small() {
        let data = vec![0u8; 8]; // Only 2 pixels worth
        assert!(decode_raw_bgra(&data, 2, 2).is_err()); // Need 16 bytes
    }

    /// End-to-end: decompress real CIP file and decode BMP
    #[test]
    fn test_real_bmp_decode() {
        let asset_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../client/data/things/1500");
        if !asset_dir.exists() {
            eprintln!("Skipping real BMP test — asset dir not found");
            return;
        }

        let catalog_json =
            std::fs::read_to_string(asset_dir.join("catalog-content.json")).unwrap();
        let catalog = crate::catalog::parse_catalog(&catalog_json).unwrap();
        let info = &catalog.sprite_sheets[0];

        let data = std::fs::read(asset_dir.join(&info.file)).unwrap();
        let decompressed = crate::lzma::decompress_cip(&data).unwrap();

        let sprite_count = info.last_sprite_id - info.first_sprite_id + 1;
        let (w, h) = crate::sprite_sheet::sheet_dimensions(sprite_count, info.sprite_type);
        let rgba = decode_bmp_to_rgba(&decompressed, w, h).unwrap();

        assert_eq!(rgba.len(), (w * h * 4) as usize);
        eprintln!("Decoded BMP: {w}x{h}, {} bytes RGBA", rgba.len());
    }
}
