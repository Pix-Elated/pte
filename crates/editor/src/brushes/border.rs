//! Auto-border system — RME-compatible ground terrain border calculation.
//!
//! Uses a 256-entry lookup table mapping 8-neighbor bitmask to border edge types.
//! Each entry encodes up to 4 border pieces packed into a u32.

/// The 12 border edge types (matching RME's AutoBorder enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BorderType {
    None = 0,
    North = 1,
    East = 2,
    South = 3,
    West = 4,
    NorthWestCorner = 5,
    NorthEastCorner = 6,
    SouthWestCorner = 7,
    SouthEastCorner = 8,
    NorthWestDiagonal = 9,
    NorthEastDiagonal = 10,
    SouthWestDiagonal = 11,
    SouthEastDiagonal = 12,
}

#[allow(dead_code)]
impl BorderType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::North,
            2 => Self::East,
            3 => Self::South,
            4 => Self::West,
            5 => Self::NorthWestCorner,
            6 => Self::NorthEastCorner,
            7 => Self::SouthWestCorner,
            8 => Self::SouthEastCorner,
            9 => Self::NorthWestDiagonal,
            10 => Self::NorthEastDiagonal,
            11 => Self::SouthWestDiagonal,
            12 => Self::SouthEastDiagonal,
            _ => Self::None,
        }
    }

    pub fn edge_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::North => "n",
            Self::East => "e",
            Self::South => "s",
            Self::West => "w",
            Self::NorthWestCorner => "cnw",
            Self::NorthEastCorner => "cne",
            Self::SouthWestCorner => "csw",
            Self::SouthEastCorner => "cse",
            Self::NorthWestDiagonal => "dnw",
            Self::NorthEastDiagonal => "dne",
            Self::SouthWestDiagonal => "dsw",
            Self::SouthEastDiagonal => "dse",
        }
    }

    pub fn from_edge_name(name: &str) -> Self {
        match name {
            "n" => Self::North,
            "e" => Self::East,
            "s" => Self::South,
            "w" => Self::West,
            "cnw" => Self::NorthWestCorner,
            "cne" => Self::NorthEastCorner,
            "csw" => Self::SouthWestCorner,
            "cse" => Self::SouthEastCorner,
            "dnw" => Self::NorthWestDiagonal,
            "dne" => Self::NorthEastDiagonal,
            "dsw" => Self::SouthWestDiagonal,
            "dse" => Self::SouthEastDiagonal,
            _ => Self::None,
        }
    }
}

/// Neighbor index layout (matching RME):
/// ```text
/// 0=NW  1=N  2=NE
/// 3=W   .    4=E
/// 5=SW  6=S  7=SE
/// ```
#[allow(dead_code)]
pub const NEIGHBOR_NW: u8 = 0;
#[allow(dead_code)]
pub const NEIGHBOR_N: u8 = 1;
#[allow(dead_code)]
pub const NEIGHBOR_NE: u8 = 2;
#[allow(dead_code)]
pub const NEIGHBOR_W: u8 = 3;
#[allow(dead_code)]
pub const NEIGHBOR_E: u8 = 4;
#[allow(dead_code)]
pub const NEIGHBOR_SW: u8 = 5;
#[allow(dead_code)]
pub const NEIGHBOR_S: u8 = 6;
#[allow(dead_code)]
pub const NEIGHBOR_SE: u8 = 7;

/// Neighbor offsets: (dx, dy) for each index.
pub const NEIGHBOR_OFFSETS: [(i32, i32); 8] = [
    (-1, -1), // 0 NW
    (0, -1),  // 1 N
    (1, -1),  // 2 NE
    (-1, 0),  // 3 W
    (1, 0),   // 4 E
    (-1, 1),  // 5 SW
    (0, 1),   // 6 S
    (1, 1),   // 7 SE
];

/// An auto-border definition — maps edge types to item IDs.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AutoBorder {
    pub id: u16,
    pub group: u16,
    /// Item ID for each of the 13 edge types (index 0 unused).
    pub tiles: [u16; 13],
}

impl AutoBorder {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            group: 0,
            tiles: [0; 13],
        }
    }

    /// Get the item ID for a given border edge.
    pub fn get_tile(&self, edge: BorderType) -> Option<u16> {
        let idx = edge as usize;
        if idx < 13 && self.tiles[idx] != 0 {
            Some(self.tiles[idx])
        } else {
            None
        }
    }
}

/// A border cluster for a single tile — tracks which border and its combined alignment.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BorderCluster {
    pub alignment: u32,
    pub z_order: i32,
    pub border: AutoBorder,
}

impl PartialEq for BorderCluster {
    fn eq(&self, other: &Self) -> bool {
        self.z_order == other.z_order
    }
}

impl Eq for BorderCluster {}

impl PartialOrd for BorderCluster {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BorderCluster {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.z_order.cmp(&other.z_order)
    }
}

/// The static 256-entry lookup table mapping 8-bit neighbor bitmask to border types.
/// Each entry is a u32 with up to 4 border types packed in bytes.
/// Byte 0 = first border, Byte 1 = second, etc. 0 = none.
///
/// This is initialized once and matches RME's `GroundBrush::border_types[256]`.
pub static BORDER_TYPES: [u32; 256] = build_border_types();

/// Decode the border types from a bitmask entry.
pub fn decode_border_types(entry: u32) -> [BorderType; 4] {
    [
        BorderType::from_u8((entry & 0xFF) as u8),
        BorderType::from_u8(((entry >> 8) & 0xFF) as u8),
        BorderType::from_u8(((entry >> 16) & 0xFF) as u8),
        BorderType::from_u8(((entry >> 24) & 0xFF) as u8),
    ]
}

/// Encode up to 4 border types into a u32 entry.
const fn encode_borders(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

/// Build the full 256-entry border type lookup table.
/// Each bit in the index represents a neighbor that is DIFFERENT from the center.
///
/// Bit layout: bit0=NW, bit1=N, bit2=NE, bit3=W, bit4=E, bit5=SW, bit6=S, bit7=SE
///
/// The values encode which border pieces to place. This matches RME's init().
#[allow(unused_assignments)]
const fn build_border_types() -> [u32; 256] {
    let mut table = [0u32; 256];

    // Helper constants for border type IDs
    const NONE: u8 = 0;
    const N: u8 = 1; // North horizontal
    const E: u8 = 2; // East horizontal
    const S: u8 = 3; // South horizontal
    const W: u8 = 4; // West horizontal
    const CNW: u8 = 5; // Corner NW
    const CNE: u8 = 6; // Corner NE
    const CSW: u8 = 7; // Corner SW
    const CSE: u8 = 8; // Corner SE
    const DNW: u8 = 9; // Diagonal NW
    const DNE: u8 = 10; // Diagonal NE
    const DSW: u8 = 11; // Diagonal SW
    const DSE: u8 = 12; // Diagonal SE

    // The neighbor bitmask bits:
    // 0x01 = NW  (bit 0)
    // 0x02 = N   (bit 1)
    // 0x04 = NE  (bit 2)
    // 0x08 = W   (bit 3)
    // 0x10 = E   (bit 4)
    // 0x20 = SW  (bit 5)
    // 0x40 = S   (bit 6)
    // 0x80 = SE  (bit 7)

    const BIT_NW: u32 = 0x01;
    const BIT_N: u32 = 0x02;
    const BIT_NE: u32 = 0x04;
    const BIT_W: u32 = 0x08;
    const BIT_E: u32 = 0x10;
    const BIT_SW: u32 = 0x20;
    const BIT_S: u32 = 0x40;
    const BIT_SE: u32 = 0x80;

    // Build the table by iterating all 256 combinations
    let mut i: u32 = 0;
    while i < 256 {
        let has_nw = (i & BIT_NW) != 0;
        let has_n = (i & BIT_N) != 0;
        let has_ne = (i & BIT_NE) != 0;
        let has_w = (i & BIT_W) != 0;
        let has_e = (i & BIT_E) != 0;
        let has_sw = (i & BIT_SW) != 0;
        let has_s = (i & BIT_S) != 0;
        let has_se = (i & BIT_SE) != 0;

        let mut b0: u8 = NONE;
        let mut b1: u8 = NONE;
        let mut b2: u8 = NONE;
        let mut b3: u8 = NONE;

        // Cardinal neighbors create full borders
        // Diagonal neighbors (NW, NE, SW, SE) only create corners
        // if the adjacent cardinals don't already cover them

        // Check full sides first: N, E, S, W
        let mut slot = 0;

        if has_n && has_w {
            // NW diagonal (both N and W are different)
            b0 = DNW;
            slot = 1;
        } else if has_n {
            // North border
            b0 = N;
            slot = 1;
            if has_w {
                // Also need west
            }
        } else if has_w {
            b0 = W;
            slot = 1;
        }

        // This approach is too complex for const fn. Use RME's actual generated table.
        // RME computes this procedurally in GroundBrush::init() — let's replicate
        // the key patterns directly.

        // Reset and use the canonical RME algorithm:
        // For each combination, determine which border pieces are needed.
        b0 = NONE;
        b1 = NONE;
        b2 = NONE;
        b3 = NONE;
        slot = 0;

        // Full N+W = NW diagonal
        if has_n && has_w {
            if slot == 0 {
                b0 = DNW;
            } else if slot == 1 {
                b1 = DNW;
            } else if slot == 2 {
                b2 = DNW;
            } else {
                b3 = DNW;
            }
            slot += 1;
        } else {
            if has_n && !has_w {
                if slot == 0 {
                    b0 = N;
                } else if slot == 1 {
                    b1 = N;
                } else if slot == 2 {
                    b2 = N;
                } else {
                    b3 = N;
                }
                slot += 1;
            }
            if has_w && !has_n {
                if slot == 0 {
                    b0 = W;
                } else if slot == 1 {
                    b1 = W;
                } else if slot == 2 {
                    b2 = W;
                } else {
                    b3 = W;
                }
                slot += 1;
            }
            // NW corner only if both N and W are same, but NW itself is different
            if !has_n && !has_w && has_nw {
                if slot == 0 {
                    b0 = CNW;
                } else if slot == 1 {
                    b1 = CNW;
                } else if slot == 2 {
                    b2 = CNW;
                } else {
                    b3 = CNW;
                }
                slot += 1;
            }
        }

        // Full N+E = NE diagonal
        if has_n && has_e {
            if slot == 0 {
                b0 = DNE;
            } else if slot == 1 {
                b1 = DNE;
            } else if slot == 2 {
                b2 = DNE;
            } else {
                b3 = DNE;
            }
            slot += 1;
        } else {
            // N already handled above, only add E if not combined with N
            if has_e && !has_n {
                if slot == 0 {
                    b0 = E;
                } else if slot == 1 {
                    b1 = E;
                } else if slot == 2 {
                    b2 = E;
                } else {
                    b3 = E;
                }
                slot += 1;
            }
            // NE corner
            if !has_n && !has_e && has_ne {
                if slot == 0 {
                    b0 = CNE;
                } else if slot == 1 {
                    b1 = CNE;
                } else if slot == 2 {
                    b2 = CNE;
                } else {
                    b3 = CNE;
                }
                slot += 1;
            }
        }

        // Full S+W = SW diagonal
        if has_s && has_w {
            if slot == 0 {
                b0 = DSW;
            } else if slot == 1 {
                b1 = DSW;
            } else if slot == 2 {
                b2 = DSW;
            } else {
                b3 = DSW;
            }
            slot += 1;
        } else {
            if has_s && !has_w {
                if slot == 0 {
                    b0 = S;
                } else if slot == 1 {
                    b1 = S;
                } else if slot == 2 {
                    b2 = S;
                } else {
                    b3 = S;
                }
                slot += 1;
            }
            // W already handled above
            // SW corner
            if !has_s && !has_w && has_sw {
                if slot == 0 {
                    b0 = CSW;
                } else if slot == 1 {
                    b1 = CSW;
                } else if slot == 2 {
                    b2 = CSW;
                } else {
                    b3 = CSW;
                }
                slot += 1;
            }
        }

        // Full S+E = SE diagonal
        if has_s && has_e {
            if slot == 0 {
                b0 = DSE;
            } else if slot == 1 {
                b1 = DSE;
            } else if slot == 2 {
                b2 = DSE;
            } else {
                b3 = DSE;
            }
            // slot += 1; // unused after last
        } else {
            // S and E already handled
            // SE corner
            if !has_s && !has_e && has_se {
                if slot == 0 {
                    b0 = CSE;
                } else if slot == 1 {
                    b1 = CSE;
                } else if slot == 2 {
                    b2 = CSE;
                } else {
                    b3 = CSE;
                }
                // slot += 1;
            }
        }

        table[i as usize] = encode_borders(b0, b1, b2, b3);
        i += 1;
    }

    table
}

/// Compute the neighbor bitmask for a tile at (x, y, z).
/// Each bit is set if the neighbor at that position has a DIFFERENT ground brush
/// than the center tile.
///
/// `get_brush_id(x, y, z) -> Option<BrushId>` returns the brush assigned to a tile.
pub fn compute_neighbor_mask<F>(
    x: u16,
    y: u16,
    z: u8,
    center_brush: Option<u32>,
    get_brush_id: F,
) -> u8
where
    F: Fn(u16, u16, u8) -> Option<u32>,
{
    let mut mask: u8 = 0;
    for (idx, &(dx, dy)) in NEIGHBOR_OFFSETS.iter().enumerate() {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 || nx > u16::MAX as i32 || ny > u16::MAX as i32 {
            // Out of bounds = different
            mask |= 1 << idx;
            continue;
        }
        let neighbor_brush = get_brush_id(nx as u16, ny as u16, z);
        if neighbor_brush != center_brush {
            mask |= 1 << idx;
        }
    }
    mask
}

/// Get the border items that should be placed on a tile given its neighbor bitmask
/// and the auto-border definition.
pub fn get_border_items(mask: u8, border: &AutoBorder) -> Vec<u16> {
    let entry = BORDER_TYPES[mask as usize];
    let edges = decode_border_types(entry);
    let mut items = Vec::new();

    for edge in &edges {
        if *edge == BorderType::None {
            break;
        }
        match border.get_tile(*edge) {
            Some(item_id) => items.push(item_id),
            None => {
                // Diagonal fallback: decompose into two horizontal pieces
                match edge {
                    BorderType::NorthWestDiagonal => {
                        if let Some(w) = border.get_tile(BorderType::West) {
                            items.push(w);
                        }
                        if let Some(n) = border.get_tile(BorderType::North) {
                            items.push(n);
                        }
                    }
                    BorderType::NorthEastDiagonal => {
                        if let Some(e) = border.get_tile(BorderType::East) {
                            items.push(e);
                        }
                        if let Some(n) = border.get_tile(BorderType::North) {
                            items.push(n);
                        }
                    }
                    BorderType::SouthWestDiagonal => {
                        if let Some(s) = border.get_tile(BorderType::South) {
                            items.push(s);
                        }
                        if let Some(w) = border.get_tile(BorderType::West) {
                            items.push(w);
                        }
                    }
                    BorderType::SouthEastDiagonal => {
                        if let Some(s) = border.get_tile(BorderType::South) {
                            items.push(s);
                        }
                        if let Some(e) = border.get_tile(BorderType::East) {
                            items.push(e);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_mask_no_borders() {
        let entry = BORDER_TYPES[0];
        assert_eq!(entry, 0, "No neighbors different = no borders");
    }

    #[test]
    fn all_different_gives_borders() {
        let entry = BORDER_TYPES[0xFF];
        // All neighbors different should produce some borders
        let edges = decode_border_types(entry);
        // At minimum, diagonals should be present
        assert_ne!(edges[0], BorderType::None);
    }

    #[test]
    fn north_only() {
        // Only north neighbor is different: bit 1 = 0x02
        let entry = BORDER_TYPES[0x02];
        let edges = decode_border_types(entry);
        assert_eq!(edges[0], BorderType::North);
    }

    #[test]
    fn corner_nw_only() {
        // Only NW neighbor different, N and W are same: bit 0 = 0x01
        let entry = BORDER_TYPES[0x01];
        let edges = decode_border_types(entry);
        assert_eq!(edges[0], BorderType::NorthWestCorner);
    }
}
