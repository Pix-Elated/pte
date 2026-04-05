//! SPR file reader and writer.
//!
//! SPR format:
//! - u32 signature
//! - u16 or u32 sprite_count (u32 for extended/v960+)
//! - u32[sprite_count] offsets (0 = blank sprite)
//! - For each sprite at its offset:
//!   - 3 bytes: RGB key color (usually FF00FF magenta, ignored)
//!   - u16: data_size (bytes of RLE pixel data)
//!   - RLE data: alternating [u16 transparent_pixels, u16 colored_pixels, BGRA*colored_pixels]

use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::types::{SprFile, Sprite};

/// Parse SPR from a file path.
pub fn parse_spr(path: &Path) -> Result<SprFile> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_spr_bytes(&data)
}

/// Parse SPR from bytes.
pub fn parse_spr_bytes(data: &[u8]) -> Result<SprFile> {
    let mut r = Cursor::new(data);

    let signature = r.read_u32::<LittleEndian>()?;

    // Detect extended format: try u32 count first; if it produces a nonsensical
    // value (count > remaining bytes / 4), fall back to u16.
    let pos_after_sig = r.position();
    let count_u32 = r.read_u32::<LittleEndian>()?;
    let remaining_after_u32 = data.len() as u64 - r.position();

    let (sprite_count, extended) =
        if count_u32 > 0 && (count_u32 as u64) * 4 <= remaining_after_u32 + 4 && count_u32 > 0xFFFF
        {
            // Extended: u32 count
            (count_u32, true)
        } else {
            // Standard: u16 count — rewind
            r.set_position(pos_after_sig);
            let count = r.read_u16::<LittleEndian>()? as u32;
            (count, false)
        };

    tracing::info!(
        sprite_count,
        extended,
        signature = format!("{:#010X}", signature),
        "Parsing SPR"
    );

    // Read offset table
    let mut offsets = Vec::with_capacity(sprite_count as usize);
    for _ in 0..sprite_count {
        offsets.push(r.read_u32::<LittleEndian>()?);
    }

    // Decode each sprite
    let mut sprites = Vec::with_capacity(sprite_count as usize);
    for (i, &offset) in offsets.iter().enumerate() {
        if offset == 0 {
            sprites.push(Sprite::new_transparent());
            continue;
        }

        if offset as usize >= data.len() {
            tracing::warn!(
                "Sprite {} offset {} out of bounds, using blank",
                i + 1,
                offset
            );
            sprites.push(Sprite::new_transparent());
            continue;
        }

        match decode_sprite(&data[offset as usize..]) {
            Ok(sprite) => sprites.push(sprite),
            Err(e) => {
                tracing::warn!("Sprite {} decode error: {e:#}, using blank", i + 1);
                sprites.push(Sprite::new_transparent());
            }
        }
    }

    Ok(SprFile {
        signature,
        sprites,
        extended,
    })
}

/// Decode a single sprite from raw bytes at its offset.
fn decode_sprite(data: &[u8]) -> Result<Sprite> {
    if data.len() < 5 {
        bail!("Sprite data too small");
    }

    let mut r = Cursor::new(data);

    // 3 bytes: key color (skip)
    let mut _key = [0u8; 3];
    r.read_exact(&mut _key)?;

    let data_size = r.read_u16::<LittleEndian>()? as usize;

    // RGBA output: 32×32 pixels, initially transparent
    let mut pixels = vec![0u8; Sprite::BYTE_COUNT];
    let mut pixel_idx = 0usize; // linear pixel index (0..1024)

    let data_start = r.position() as usize;
    let data_end = (data_start + data_size).min(data.len());
    let rle_data = &data[data_start..data_end];
    let mut rle = Cursor::new(rle_data);

    while (rle.position() as usize) < rle_data.len() && pixel_idx < Sprite::PIXEL_COUNT {
        // Transparent run
        let transparent_count = match rle.read_u16::<LittleEndian>() {
            Ok(v) => v as usize,
            Err(_) => break,
        };
        pixel_idx += transparent_count;

        if pixel_idx >= Sprite::PIXEL_COUNT {
            break;
        }

        // Colored run
        let colored_count = match rle.read_u16::<LittleEndian>() {
            Ok(v) => v as usize,
            Err(_) => break,
        };

        for _ in 0..colored_count {
            if pixel_idx >= Sprite::PIXEL_COUNT {
                break;
            }

            // Read BGRA (or RGB depending on version, but typically BGRA in SPR)
            let mut bgra = [0u8; 4];
            // SPR stores RGB (3 bytes per pixel), not BGRA
            if rle.read_exact(&mut bgra[0..3]).is_err() {
                break;
            }

            let byte_idx = pixel_idx * 4;
            // BGR → RGBA
            pixels[byte_idx] = bgra[2]; // R
            pixels[byte_idx + 1] = bgra[1]; // G
            pixels[byte_idx + 2] = bgra[0]; // B
            pixels[byte_idx + 3] = 0xFF; // A = opaque

            pixel_idx += 1;
        }
    }

    Ok(Sprite { pixels })
}

/// Write SPR to a file.
pub fn write_spr(spr: &SprFile, path: &Path) -> Result<()> {
    let data = serialize_spr_bytes(spr)?;
    std::fs::write(path, &data)?;
    Ok(())
}

/// Serialize SPR to bytes.
pub fn serialize_spr_bytes(spr: &SprFile) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(4 * 1024 * 1024);

    out.write_u32::<LittleEndian>(spr.signature)?;

    let count = spr.sprites.len() as u32;
    if spr.extended {
        out.write_u32::<LittleEndian>(count)?;
    } else {
        if count > 0xFFFF {
            bail!(
                "Sprite count {} exceeds u16 max for non-extended SPR",
                count
            );
        }
        out.write_u16::<LittleEndian>(count as u16)?;
    }

    // Reserve space for offset table
    let offsets_pos = out.len();
    for _ in 0..count {
        out.write_u32::<LittleEndian>(0)?; // placeholder
    }

    // Encode each sprite and fill in offsets
    let mut offsets = Vec::with_capacity(count as usize);
    for sprite in &spr.sprites {
        if sprite.is_blank() {
            offsets.push(0u32);
            continue;
        }

        let offset = out.len() as u32;
        offsets.push(offset);
        encode_sprite(&mut out, sprite)?;
    }

    // Patch offset table
    let mut cursor = Cursor::new(&mut out[offsets_pos..]);
    for &offset in &offsets {
        cursor.write_u32::<LittleEndian>(offset)?;
    }

    Ok(out)
}

/// Encode a single sprite to RLE format.
fn encode_sprite(out: &mut Vec<u8>, sprite: &Sprite) -> Result<()> {
    // Key color (magenta)
    out.write_all(&[0xFF, 0x00, 0xFF])?;

    // Placeholder for data_size
    let size_pos = out.len();
    out.write_u16::<LittleEndian>(0)?;

    let data_start = out.len();

    let mut pixel_idx = 0usize;
    while pixel_idx < Sprite::PIXEL_COUNT {
        // Count transparent pixels
        let mut transparent_count = 0u16;
        while pixel_idx < Sprite::PIXEL_COUNT {
            let a = sprite.pixels[pixel_idx * 4 + 3];
            if a > 0 {
                break;
            }
            transparent_count += 1;
            pixel_idx += 1;
        }

        if pixel_idx >= Sprite::PIXEL_COUNT {
            // All remaining pixels are transparent
            if transparent_count > 0 {
                out.write_u16::<LittleEndian>(transparent_count)?;
                out.write_u16::<LittleEndian>(0)?;
            }
            break;
        }

        // Count colored pixels
        let colored_start = pixel_idx;
        let mut colored_count = 0u16;
        while pixel_idx < Sprite::PIXEL_COUNT {
            let a = sprite.pixels[pixel_idx * 4 + 3];
            if a == 0 {
                break;
            }
            colored_count += 1;
            pixel_idx += 1;
        }

        out.write_u16::<LittleEndian>(transparent_count)?;
        out.write_u16::<LittleEndian>(colored_count)?;

        // Write RGB pixels (RGBA → BGR for SPR)
        for i in colored_start..(colored_start + colored_count as usize) {
            let bi = i * 4;
            let r = sprite.pixels[bi];
            let g = sprite.pixels[bi + 1];
            let b = sprite.pixels[bi + 2];
            out.write_all(&[b, g, r])?; // BGR
        }
    }

    // Patch data_size
    let data_size = (out.len() - data_start) as u16;
    let mut cursor = Cursor::new(&mut out[size_pos..]);
    cursor.write_u16::<LittleEndian>(data_size)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blank_sprite_roundtrip() {
        let spr = SprFile {
            signature: 0x12345678,
            sprites: vec![Sprite::new_transparent(), Sprite::new_transparent()],
            extended: false,
        };

        let bytes = serialize_spr_bytes(&spr).unwrap();
        let parsed = parse_spr_bytes(&bytes).unwrap();

        assert_eq!(parsed.signature, spr.signature);
        assert_eq!(parsed.sprite_count(), 2);
        assert!(parsed.sprites[0].is_blank());
        assert!(parsed.sprites[1].is_blank());
    }

    #[test]
    fn test_colored_sprite_roundtrip() {
        let mut sprite = Sprite::new_transparent();
        // Set a few pixels
        sprite.pixels[0] = 255; // R
        sprite.pixels[1] = 128; // G
        sprite.pixels[2] = 64; // B
        sprite.pixels[3] = 255; // A

        sprite.pixels[4] = 0;
        sprite.pixels[5] = 255;
        sprite.pixels[6] = 0;
        sprite.pixels[7] = 255;

        let spr = SprFile {
            signature: 0xAABBCCDD,
            sprites: vec![sprite],
            extended: false,
        };

        let bytes = serialize_spr_bytes(&spr).unwrap();
        let parsed = parse_spr_bytes(&bytes).unwrap();

        assert_eq!(parsed.sprite_count(), 1);
        let s = &parsed.sprites[0];
        assert_eq!(s.pixels[0], 255); // R preserved
        assert_eq!(s.pixels[1], 128); // G preserved
        assert_eq!(s.pixels[2], 64); // B preserved
        assert_eq!(s.pixels[3], 255); // A preserved
    }

    #[test]
    fn test_extended_roundtrip() {
        let spr = SprFile {
            signature: 0x11223344,
            sprites: vec![Sprite::new_transparent(); 3],
            extended: true,
        };

        let bytes = serialize_spr_bytes(&spr).unwrap();
        let parsed = parse_spr_bytes(&bytes).unwrap();
        assert_eq!(parsed.sprite_count(), 3);
    }

    #[test]
    fn test_add_delete_sprite() {
        let mut spr = SprFile {
            signature: 0,
            sprites: Vec::new(),
            extended: false,
        };

        let id = spr.add_sprite(Sprite::new_transparent());
        assert_eq!(id, 1);
        assert_eq!(spr.sprite_count(), 1);

        assert!(spr.delete_sprite(1));
        assert!(spr.get_sprite(1).unwrap().is_blank());
    }
}
