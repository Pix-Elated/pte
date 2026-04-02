//! Reader/writer for legacy Tibia SPR and DAT files.
//!
//! SPR format: sprite images (32×32, RLE-compressed BGRA with magenta transparency)
//! DAT format: item/outfit/effect/missile metadata (byte-tagged attributes)

mod dat;
mod spr;
mod types;

pub use dat::{parse_dat, parse_dat_bytes, serialize_dat_bytes, write_dat};
pub use spr::{parse_spr, parse_spr_bytes, serialize_spr_bytes, write_spr};
pub use types::*;
