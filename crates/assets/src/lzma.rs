//! CIP (TibiaFiles) LZMA decompression.
//!
//! CIP sprite sheets use a custom 32-byte header before the raw LZMA1 stream.
//! The format matches what OTClient's `spriteappearances.cpp` parses:
//!
//! ```text
//! [0x00 padding...] [70 0A FA 80 24] [7-bit encoded compressed size]
//! [lclppb] [dict_size LE u32] [compressed_size LE u64] [raw LZMA1 data...]
//! ```
//!
//! The header is always 32 bytes total.

use anyhow::{bail, Context, Result};

/// CIP header is always exactly 32 bytes.
const CIP_HEADER_SIZE: usize = 32;

/// Decompress a CIP LZMA sprite sheet.
///
/// Follows OTClient's approach:
/// 1. Skip the 32-byte CIP header
/// 2. Parse LZMA1 properties byte (lc, lp, pb)
/// 3. Parse 4-byte little-endian dictionary size
/// 4. Skip 8-byte CIP compressed size field
/// 5. Feed remaining bytes to lzma-rs raw LZMA1 decoder
pub fn decompress_cip(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < CIP_HEADER_SIZE + 14 {
        bail!(
            "CIP file too small ({} bytes, need at least {})",
            data.len(),
            CIP_HEADER_SIZE + 14
        );
    }

    // Skip the 32-byte CIP header entirely.
    // OTClient skips NULLs, the magic [70 0A FA 80 24], and the 7-bit encoded size.
    // Since it's always 32 bytes, we just jump past it.
    let after_header = &data[CIP_HEADER_SIZE..];

    // Byte 0: LZMA properties byte (encodes lc, lp, pb)
    // Bytes 1-4: dictionary size (LE u32)
    // Bytes 5-12: size field (may be compressed size, NOT reliable as uncompressed size)
    // Bytes 13+: raw LZMA1 compressed data
    //
    // Build a standard LZMA "alone" header that lzma-rs can parse,
    // but replace the size field with -1 (unknown) to decompress until EOS marker.
    let mut lzma_alone = Vec::with_capacity(13 + after_header.len() - 13);
    lzma_alone.extend_from_slice(&after_header[0..5]); // props + dict_size
    lzma_alone.extend_from_slice(&[0xFF; 8]); // uncompressed size = unknown
    lzma_alone.extend_from_slice(&after_header[13..]); // compressed stream

    let mut output = Vec::new();
    let mut reader = std::io::Cursor::new(&lzma_alone);

    lzma_rs::lzma_decompress(&mut reader, &mut output)
        .context("LZMA1 decompression failed")?;

    Ok(output)
}

/// Compress data to CIP LZMA format (32-byte header + LZMA1 stream).
pub fn compress_cip(data: &[u8]) -> Result<Vec<u8>> {
    // First compress with lzma-rs
    let mut compressed = Vec::new();
    {
        let mut writer = std::io::Cursor::new(&mut compressed);
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(data), &mut writer)
            .context("LZMA1 compression failed")?;
    }

    // compressed now has LZMA "alone" format:
    // [1 byte props] [4 bytes dict] [8 bytes uncompressed_size] [stream]
    // We need to build the CIP format:
    // [32 bytes header] [1 byte props] [4 bytes dict] [8 bytes size] [stream]

    // Build 32-byte CIP header:
    // - Padding zeros (variable)
    // - Magic: [70 0A FA 80 24]
    // - 7-bit encoded compressed stream size
    let stream_len = compressed.len() as u64;

    // Encode stream length as 7-bit varint
    let mut varint = Vec::new();
    {
        let mut val = stream_len;
        loop {
            let byte = (val & 0x7F) as u8;
            val >>= 7;
            if val == 0 {
                varint.push(byte);
                break;
            } else {
                varint.push(byte | 0x80);
            }
        }
    }

    let magic = [0x70u8, 0x0A, 0xFA, 0x80, 0x24];
    let used = magic.len() + varint.len();
    let padding = CIP_HEADER_SIZE - used;

    let mut result = Vec::with_capacity(CIP_HEADER_SIZE + compressed.len());
    result.extend(std::iter::repeat_n(0u8, padding));
    result.extend_from_slice(&magic);
    result.extend_from_slice(&varint);
    debug_assert_eq!(result.len(), CIP_HEADER_SIZE);
    result.extend_from_slice(&compressed);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_small() {
        let data = vec![0u8; 10];
        assert!(decompress_cip(&data).is_err());
    }

    #[test]
    fn test_header_exactly_min_size() {
        // 32 header + 13 bytes LZMA minimum = too small for valid LZMA, but past size check
        let data = vec![0u8; CIP_HEADER_SIZE + 14];
        // Should fail during LZMA decompression, not size check
        let result = decompress_cip(&data);
        assert!(result.is_err());
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(err_msg.contains("LZMA"), "Expected LZMA error, got: {err_msg}");
    }

    /// Test against a real CIP `.bmp.lzma` file from the client data.
    /// Skipped if the test asset directory doesn't exist.
    #[test]
    fn test_real_cip_decompress() {
        let asset_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../client/data/things/1500");
        if !asset_dir.exists() {
            eprintln!("Skipping real CIP test — asset dir not found");
            return;
        }

        // Read catalog to find first sprite sheet
        let catalog_path = asset_dir.join("catalog-content.json");
        let catalog_json = std::fs::read_to_string(&catalog_path).unwrap();
        let catalog = crate::catalog::parse_catalog(&catalog_json).unwrap();
        let first_sheet = &catalog.sprite_sheets[0];

        let sheet_path = asset_dir.join(&first_sheet.file);
        let data = std::fs::read(&sheet_path).unwrap();

        // Verify CIP header structure
        assert!(data.len() > CIP_HEADER_SIZE + 14, "File too small");
        assert_eq!(data[32], 0x5D, "LZMA properties byte should be at offset 32");

        // Decompress
        let decompressed = decompress_cip(&data).unwrap();

        // Should be a BMP (starts with 'BM')
        assert!(decompressed.len() > 2, "Decompressed data too small");
        assert_eq!(&decompressed[0..2], b"BM", "Expected BMP magic bytes");

        eprintln!(
            "Decompressed {} → {} bytes ({})",
            data.len(),
            decompressed.len(),
            first_sheet.file
        );
    }
}
