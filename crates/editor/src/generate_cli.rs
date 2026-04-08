//! pte-generate — Rust map generator for X-Trails
//!
//! Reads a world map PNG (1:1 pixel-to-tile). Three colors:
//!   Blue = ocean (z=7), Green = grass (z=6), White = dirt/roads/towns (z=6)
//! Town circles get cobblestone roads with grass transitions.
//! No buildings, no furniture — user hand-builds in PTE.
//!
//! Usage: pte-generate <world_png> <output_chunk_dir>

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use image::GenericImageView;
use pte_otbm::{MapData, MapItem, Position, Tile, TileFlags, Town, Waypoint};
use rand::Rng;
use rand::SeedableRng;

// ============================================================
// Z-levels
// ============================================================
const WATER_Z: u8 = 7; // ocean = base ground
const LAND_Z: u8 = 6;  // land above water

// ============================================================
// Verified Item IDs
// ============================================================
const GRASS: &[u16] = &[4515, 4516, 4517, 4518, 4519, 4520, 4521, 4522, 4523, 4524, 4525, 4526, 4527, 4528, 4529, 4530];
const SHALLOW_WATER: &[u16] = &[4597, 4598, 4599, 4600, 4601, 4602];
const DEEP_WATER: &[u16] = &[4609, 4610, 4611, 4612, 4613, 4614];
const CLEAN_DIRT: u16 = 101; // smooth dark earth, no rocks/divots
const COBBLE: u16 = 870;

// Shore items
const SHORE_N: u16 = 4633;
const SHORE_E: u16 = 4634;
const SHORE_S: u16 = 4635;
const SHORE_W: u16 = 4636;
const SHORE_NW_OUTER: u16 = 4637;
const SHORE_NE_OUTER: u16 = 4638;
const SHORE_SW_OUTER: u16 = 4639;
const SHORE_SE_OUTER: u16 = 4640;
const SHORE_NW_INNER: u16 = 4641;
const SHORE_NE_INNER: u16 = 4642;
const SHORE_SW_INNER: u16 = 4643;
const SHORE_SE_INNER: u16 = 4644;

// Grass borders
const GB_N: u16 = 4531;
const GB_E: u16 = 4532;
const GB_S: u16 = 4533;
const GB_W: u16 = 4534;
const GB_NW_IN: u16 = 4535;
const GB_NE_IN: u16 = 4536;
const GB_SW_IN: u16 = 4537;
const GB_SE_IN: u16 = 4538;
const GB_NW_OUT: u16 = 4539;
const GB_NE_OUT: u16 = 4540;
const GB_SW_OUT: u16 = 4541;
const GB_SE_OUT: u16 = 4542;

// ============================================================
// Terrain
// ============================================================
#[derive(Clone, Copy, PartialEq, Eq)]
enum Terrain { Water, Grass, Dirt }

struct WorldMap {
    width: u32,
    height: u32,
    terrain: Vec<Terrain>,
}

impl WorldMap {
    fn load(path: &Path) -> Result<Self> {
        tracing::info!("Loading world map from {}...", path.display());
        let img = image::open(path).context("opening world map image")?;
        let (w, h) = img.dimensions();
        tracing::info!("  {}x{} pixels (1:1 tiles)", w, h);

        let mut terrain = vec![Terrain::Water; (w * h) as usize];
        let (mut gc, mut dc, mut wc) = (0u64, 0u64, 0u64);

        for y in 0..h {
            for x in 0..w {
                let px = img.get_pixel(x, y);
                let (r, g, b) = (px[0] as u16, px[1] as u16, px[2] as u16);
                let idx = (y * w + x) as usize;

                // SVG map: rgb(0,128,128)=ocean, rgb(0,128,0)=grass, rgb(249,249,249)=dirt
                if r > 200 && g > 200 && b > 200 {
                    terrain[idx] = Terrain::Dirt; dc += 1;
                } else if g > 100 && b > 100 && r < 50 && (b as i16 - g as i16).abs() < 20 {
                    // Teal/cyan = ocean (g≈128, b≈128)
                    terrain[idx] = Terrain::Water; wc += 1;
                } else if g > 100 && b < 50 && r < 50 {
                    // Pure green = grass (g≈128, b≈0)
                    terrain[idx] = Terrain::Grass; gc += 1;
                } else if b > g && b > r {
                    // Fallback blue-ish = water
                    terrain[idx] = Terrain::Water; wc += 1;
                } else {
                    // Fallback = grass
                    terrain[idx] = Terrain::Grass; gc += 1;
                }
            }
        }
        tracing::info!("  {} grass, {} dirt, {} water", gc, dc, wc);
        Ok(Self { width: w, height: h, terrain })
    }

    fn get(&self, x: i32, y: i32) -> Terrain {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { Terrain::Water }
        else { self.terrain[(y as u32 * self.width + x as u32) as usize] }
    }

    fn is_land(&self, x: i32, y: i32) -> bool {
        match self.get(x, y) {
            Terrain::Water => false,
            _ => true,
        }
    }


}

// ============================================================
// Cleanup: fix misclassified edge pixels from Upscayl blending
// ============================================================
fn cleanup_terrain(world: &mut WorldMap) {
    tracing::info!("[CLEANUP] Fixing misclassified edge pixels...");
    let w = world.width as i32;
    let h = world.height as i32;
    let mut fixes = 0u32;

    // Multiple passes to propagate fixes
    for pass in 0..3 {
        let mut changes = Vec::new();

        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = (y * w + x) as usize;
                let me = world.terrain[idx];

                // Count neighbors by type
                let mut land_neighbors = 0u32;
                let mut water_neighbors = 0u32;
                let mut grass_n = 0u32;
                let mut dirt_n = 0u32;

                for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        if dx == 0 && dy == 0 { continue; }
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= w || ny >= h { continue; }
                        match world.terrain[(ny * w + nx) as usize] {
                            Terrain::Water => water_neighbors += 1,
                            Terrain::Grass => { land_neighbors += 1; grass_n += 1; }
                            Terrain::Dirt => { land_neighbors += 1; dirt_n += 1; }
                        }
                    }
                }

                match me {
                    // Water tile surrounded by mostly land → should be land
                    Terrain::Water if land_neighbors >= 6 => {
                        let replacement = if dirt_n > grass_n { Terrain::Dirt } else { Terrain::Grass };
                        changes.push((idx, replacement));
                    }
                    // Land tile surrounded by mostly water → should be water
                    Terrain::Grass | Terrain::Dirt if water_neighbors >= 7 => {
                        changes.push((idx, Terrain::Water));
                    }
                    _ => {}
                }
            }
        }

        let pass_fixes = changes.len();
        for (idx, terrain) in changes {
            world.terrain[idx] = terrain;
        }
        fixes += pass_fixes as u32;

        if pass_fixes == 0 { break; }
        tracing::info!("  Pass {}: {} fixes", pass + 1, pass_fixes);
    }

    tracing::info!("  {} total edge fixes", fixes);
}

/// Find red dot markers from a reference image (separate from the map image).
fn find_red_markers(path: &Path) -> Result<Vec<(u16, u16)>> {
    tracing::info!("Loading reference image for town markers: {}", path.display());
    let img = image::open(path).context("opening reference image")?;
    let (w, h) = img.dimensions();

    // Find red pixels
    let mut red_mask = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let px = img.get_pixel(x, y);
            if px[0] > 150 && px[1] < 80 && px[2] < 80 {
                red_mask[(y * w + x) as usize] = true;
            }
        }
    }

    // Flood fill to find clusters
    let mut visited = vec![false; (w * h) as usize];
    let mut towns = Vec::new();

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let idx = (y as u32 * w + x as u32) as usize;
            if visited[idx] || !red_mask[idx] { continue; }

            let mut stack = vec![(x, y)];
            let mut points = Vec::new();
            while let Some((cx, cy)) = stack.pop() {
                if cx < 0 || cy < 0 || cx >= w as i32 || cy >= h as i32 { continue; }
                let ci = (cy as u32 * w + cx as u32) as usize;
                if visited[ci] || !red_mask[ci] { continue; }
                visited[ci] = true;
                points.push((cx, cy));
                stack.push((cx+1,cy)); stack.push((cx-1,cy));
                stack.push((cx,cy+1)); stack.push((cx,cy-1));
            }

            if points.len() < 10 { continue; } // skip noise
            let avg_x = (points.iter().map(|p| p.0 as i64).sum::<i64>() / points.len() as i64) as u16;
            let avg_y = (points.iter().map(|p| p.1 as i64).sum::<i64>() / points.len() as i64) as u16;
            towns.push((avg_x, avg_y));
        }
    }

    tracing::info!("  Found {} red markers in {}x{}", towns.len(), w, h);
    Ok(towns)
}

fn add_item(map: &mut MapData, x: u16, y: u16, z: u8, item_id: u16) {
    let tile = map.get_tile_mut(x, y, z);
    tile.items.push(MapItem::new(item_id));
}

// ============================================================
// PASS 1: Terrain
// ============================================================
fn generate_terrain(map: &mut MapData, world: &WorldMap) {
    tracing::info!("[PASS 1] Generating terrain...");
    let t0 = std::time::Instant::now();
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let (mut gc, mut dc, mut wc) = (0u64, 0u64, 0u64);

    for y in 0..world.height as u16 {
        for x in 0..world.width as u16 {
            // EVERY tile gets ocean on z=7 (the base layer)
            let water_ground = if rng.random_bool(0.76) {
                SHALLOW_WATER[rng.random_range(0..SHALLOW_WATER.len())]
            } else {
                DEEP_WATER[rng.random_range(0..DEEP_WATER.len())]
            };
            map.set_tile(Tile { x, y, z: WATER_Z, ground: Some(water_ground), items: vec![], flags: TileFlags::default(), house_id: None });
            wc += 1;

            // Land pixels ALSO get a tile on z=6 (above water)
            match world.get(x as i32, y as i32) {
                Terrain::Grass => {
                    let ground = GRASS[rng.random_range(0..GRASS.len())];
                    map.set_tile(Tile { x, y, z: LAND_Z, ground: Some(ground), items: vec![], flags: TileFlags::default(), house_id: None });
                    gc += 1;
                }
                Terrain::Dirt => {
                    map.set_tile(Tile { x, y, z: LAND_Z, ground: Some(CLEAN_DIRT), items: vec![], flags: TileFlags::default(), house_id: None });
                    dc += 1;
                }
                Terrain::Water => {} // already placed on z=7
            }
        }
        if y % 1000 == 0 { tracing::info!("  Row {}/{}", y, world.height); }
    }
    tracing::info!("  Grass: {}, Dirt: {}, Water: {} ({:.1}s)", gc, dc, wc, t0.elapsed().as_secs_f32());
}

// ============================================================
// PASS 2: Shores
// ============================================================
fn generate_shores(map: &mut MapData, world: &WorldMap) {
    tracing::info!("[PASS 2] Generating shores...");
    let t0 = std::time::Instant::now();
    let mut count = 0u32;

    for y in 0..world.height as i32 {
        for x in 0..world.width as i32 {
            if !world.is_land(x, y) { continue; }

            let n  = !world.is_land(x, y-1);
            let s  = !world.is_land(x, y+1);
            let e  = !world.is_land(x+1, y);
            let w  = !world.is_land(x-1, y);
            let nw = !world.is_land(x-1, y-1);
            let ne = !world.is_land(x+1, y-1);
            let sw = !world.is_land(x-1, y+1);
            let se = !world.is_land(x+1, y+1);

            if !n && !s && !e && !w && !nw && !ne && !sw && !se { continue; }
            let (ux, uy) = (x as u16, y as u16);

            if n && !e && !w { add_item(map, ux, uy, LAND_Z, SHORE_N); count += 1; }
            if s && !e && !w { add_item(map, ux, uy, LAND_Z, SHORE_S); count += 1; }
            if e && !n && !s { add_item(map, ux, uy, LAND_Z, SHORE_E); count += 1; }
            if w && !n && !s { add_item(map, ux, uy, LAND_Z, SHORE_W); count += 1; }
            if n && w { add_item(map, ux, uy, LAND_Z, SHORE_NW_OUTER); count += 1; }
            if n && e { add_item(map, ux, uy, LAND_Z, SHORE_NE_OUTER); count += 1; }
            if s && w { add_item(map, ux, uy, LAND_Z, SHORE_SW_OUTER); count += 1; }
            if s && e { add_item(map, ux, uy, LAND_Z, SHORE_SE_OUTER); count += 1; }
            if nw && !n && !w { add_item(map, ux, uy, LAND_Z, SHORE_NW_INNER); count += 1; }
            if ne && !n && !e { add_item(map, ux, uy, LAND_Z, SHORE_NE_INNER); count += 1; }
            if sw && !s && !w { add_item(map, ux, uy, LAND_Z, SHORE_SW_INNER); count += 1; }
            if se && !s && !e { add_item(map, ux, uy, LAND_Z, SHORE_SE_INNER); count += 1; }
        }
    }
    tracing::info!("  {} shore items ({:.1}s)", count, t0.elapsed().as_secs_f32());
}

// ============================================================
// ============================================================
// PASS 3: Town roads — cobblestone at red marker locations
// ============================================================
fn generate_town_roads(map: &mut MapData, towns: &[(u16, u16)]) {
    tracing::info!("[PASS 3] Town roads at {} markers...", towns.len());
    let mut rng = rand::rngs::StdRng::seed_from_u64(7777);
    let mut total = 0u32;

    for (town_idx, &(cx, cy)) in towns.iter().enumerate() {
        let mut count = 0u32;
        let ci = cx as i32;
        let cj = cy as i32;

        // Use town index as extra seed for variety
        let seed_offset = (town_idx as u64 + 1) * 31337;
        let mut trng = rand::rngs::StdRng::seed_from_u64(7777 + seed_offset);

        let lay_cobble = |map: &mut MapData, x: i32, y: i32, count: &mut u32| {
            if x < 0 || y < 0 { return; }
            let (ux, uy) = (x as u16, y as u16);
            if let Some(tile) = map.get_tile_mut_if_exists(ux, uy, LAND_Z) {
                if tile.ground != Some(COBBLE) {
                    tile.ground = Some(COBBLE);
                    tile.flags.protection_zone = true;
                    *count += 1;
                }
            }
        };

        let lay_road = |map: &mut MapData, x1: i32, y1: i32, x2: i32, y2: i32, width: i32, count: &mut u32| {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let dist = ((dx*dx + dy*dy) as f64).sqrt().ceil() as i32;
            let hw = width / 2;
            for s in 0..=dist {
                let t = if dist > 0 { s as f64 / dist as f64 } else { 0.0 };
                let px = (x1 as f64 + dx as f64 * t).round() as i32;
                let py = (y1 as f64 + dy as f64 * t).round() as i32;
                for ox in -hw..=hw { for oy in -hw..=hw {
                    lay_cobble(map, px + ox, py + oy, count);
                }}
            }
        };

        // Small plaza at center (5x5 to 9x9 depending on town)
        let plaza_r = trng.random_range(2..5i32);
        for dx in -plaza_r..=plaza_r { for dy in -plaza_r..=plaza_r {
            lay_cobble(map, ci + dx, cj + dy, &mut count);
        }}

        // Main cross streets — 4 cardinal roads with slight offset for organic feel
        let main_len = trng.random_range(30..50i32);

        // North, South, East, West main roads
        let j1 = trng.random_range(-3..4i32);
        lay_road(map, ci, cj - plaza_r, ci + j1, cj - main_len, 3, &mut count);
        let j2 = trng.random_range(-3..4i32);
        lay_road(map, ci, cj + plaza_r, ci + j2, cj + main_len, 3, &mut count);
        let j3 = trng.random_range(-3..4i32);
        lay_road(map, ci + plaza_r, cj, ci + main_len, cj + j3, 3, &mut count);
        let j4 = trng.random_range(-3..4i32);
        lay_road(map, ci - plaza_r, cj, ci - main_len, cj + j4, 3, &mut count);

        // Cross streets perpendicular to main roads — creates city blocks
        let num_cross = trng.random_range(3..6i32);
        for i in 1..=num_cross {
            let offset = main_len * i / (num_cross + 1);
            let cross_len = trng.random_range(15..30i32);

            let j = trng.random_range(-3..4i32);
            lay_road(map, ci - cross_len, cj - offset + j, ci + cross_len, cj - offset + j, 2, &mut count);
            let j = trng.random_range(-3..4i32);
            lay_road(map, ci - cross_len, cj + offset + j, ci + cross_len, cj + offset + j, 2, &mut count);
            let j = trng.random_range(-3..4i32);
            lay_road(map, ci - offset + j, cj - cross_len, ci - offset + j, cj + cross_len, 2, &mut count);
            let j = trng.random_range(-3..4i32);
            lay_road(map, ci + offset + j, cj - cross_len, ci + offset + j, cj + cross_len, 2, &mut count);
        }

        // A couple diagonal alleys for character
        let diag_len = trng.random_range(15..25i32);
        lay_road(map, ci + plaza_r, cj + plaza_r, ci + diag_len, cj + diag_len, 2, &mut count);
        lay_road(map, ci - plaza_r, cj - plaza_r, ci - diag_len, cj - diag_len, 2, &mut count);

        tracing::info!("  Town ({},{}) = {} cobble", cx, cy, count);
        total += count;
    }
    tracing::info!("  {} total cobble tiles", total);
}

// ============================================================
// PASS 4: Grass borders on non-grass land tiles
// ============================================================
fn generate_borders(map: &mut MapData, world: &WorldMap) {
    tracing::info!("[PASS 4] Generating grass borders...");
    let t0 = std::time::Instant::now();
    let mut count = 0u32;

    for y in 1..world.height as i32 - 1 {
        for x in 1..world.width as i32 - 1 {
            let (ux, uy) = (x as u16, y as u16);
            let t = match map.get_tile(ux, uy, LAND_Z) { Some(t) => t, None => continue };
            let g = match t.ground { Some(g) => g, None => continue };
            if GRASS.contains(&g) { continue; }

            let is_grass = |nx: i32, ny: i32| -> bool {
                map.get_tile(nx as u16, ny as u16, LAND_Z)
                    .and_then(|t| t.ground)
                    .map(|g| GRASS.contains(&g))
                    .unwrap_or(false)
            };

            let (gn, gs, ge, gw) = (is_grass(x,y-1), is_grass(x,y+1), is_grass(x+1,y), is_grass(x-1,y));
            let (gnw, gne, gsw, gse) = (is_grass(x-1,y-1), is_grass(x+1,y-1), is_grass(x-1,y+1), is_grass(x+1,y+1));

            if !gn && !gs && !ge && !gw && !gnw && !gne && !gsw && !gse { continue; }

            if gn && !ge && !gw { add_item(map, ux, uy, LAND_Z, GB_N); count += 1; }
            if gs && !ge && !gw { add_item(map, ux, uy, LAND_Z, GB_S); count += 1; }
            if ge && !gn && !gs { add_item(map, ux, uy, LAND_Z, GB_E); count += 1; }
            if gw && !gn && !gs { add_item(map, ux, uy, LAND_Z, GB_W); count += 1; }
            if gnw && !gn && !gw { add_item(map, ux, uy, LAND_Z, GB_NW_IN); count += 1; }
            if gne && !gn && !ge { add_item(map, ux, uy, LAND_Z, GB_NE_IN); count += 1; }
            if gsw && !gs && !gw { add_item(map, ux, uy, LAND_Z, GB_SW_IN); count += 1; }
            if gse && !gs && !ge { add_item(map, ux, uy, LAND_Z, GB_SE_IN); count += 1; }
            if gn && gw { add_item(map, ux, uy, LAND_Z, GB_NW_OUT); count += 1; }
            if gn && ge { add_item(map, ux, uy, LAND_Z, GB_NE_OUT); count += 1; }
            if gs && gw { add_item(map, ux, uy, LAND_Z, GB_SW_OUT); count += 1; }
            if gs && ge { add_item(map, ux, uy, LAND_Z, GB_SE_OUT); count += 1; }
        }
    }
    tracing::info!("  {} border items ({:.1}s)", count, t0.elapsed().as_secs_f32());
}

// ============================================================
// MAIN
// ============================================================
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: pte-generate <world_png> <output_chunk_dir>");
        std::process::exit(1);
    }

    let world_png = Path::new(&args[1]);
    let chunk_dir = Path::new(&args[2]);

    tracing::info!("=== pte-generate ===");
    let mut world = WorldMap::load(world_png)?;

    // Detect town centers from red markers in the REFERENCE image (upscayl)
    let ref_path = world_png.parent().unwrap_or(Path::new(".")).join("simplified_color_8x_upscayl.png");
    let town_markers = if ref_path.exists() {
        find_red_markers(&ref_path)?
    } else {
        tracing::warn!("No reference image for town markers: {}", ref_path.display());
        vec![]
    };
    tracing::info!("  {} town markers detected:", town_markers.len());
    for (i, &(cx, cy)) in town_markers.iter().enumerate() {
        tracing::info!("    #{}: ({},{})", i+1, cx, cy);
    }

    let mut map = MapData::new();
    map.width = world.width as u16;
    map.height = world.height as u16;
    map.description = "X-Trails World".to_string();
    map.spawn_file = "xtrails-monster.xml".to_string();
    map.house_file = "xtrails-house.xml".to_string();
    map.version = 3;
    map.item_major_version = 3;
    map.item_minor_version = 56;

    // Sort town markers: Crossmark (nearest to map center) first
    let map_cx = world.width as f64 / 2.0;
    let map_cy = world.height as f64 / 2.0;
    let mut town_markers = town_markers;
    town_markers.sort_by(|a, b| {
        let da = (a.0 as f64 - map_cx).powi(2) + (a.1 as f64 - map_cy).powi(2);
        let db = (b.0 as f64 - map_cx).powi(2) + (b.1 as f64 - map_cy).powi(2);
        da.partial_cmp(&db).unwrap()
    });

    let town_names = [
        "Crossmark Island", "Valdenmoor Haven", "Galdenmoor",
        "Fjordheim", "Sunreach", "Cliffward Landing",
        "Shen Lowlands", "Heavenpeak Hold", "Fracture Port",
        "Dawnspire", "Ashwatch", "Goldtide", "Iron Ridge",
        "Moonhaven", "Thornwall",
    ];
    tracing::info!("  Towns (Crossmark first):");
    for (i, &(cx, cy)) in town_markers.iter().enumerate() {
        if i >= town_names.len() { break; }
        tracing::info!("    {}: ({},{}) = {}", i+1, cx, cy, town_names[i]);
        map.towns.push(Town {
            id: (i+1) as u32,
            name: town_names[i].to_string(),
            position: Position { x: cx, y: cy, z: LAND_Z },
        });
    }

    // Clean up misclassified edge pixels from Upscayl anti-aliasing
    cleanup_terrain(&mut world);
    generate_terrain(&mut map, &world);
    generate_shores(&mut map, &world);
    // No auto-generated town roads — user will hand-build in PTE
    generate_borders(&mut map, &world);

    tracing::info!("Total tiles: {}", map.tile_count());

    // Write chunks for PTE
    if chunk_dir.exists() { std::fs::remove_dir_all(chunk_dir)?; }
    let chunks = pte_otbm::chunk_io::save_chunk_dir(&map, chunk_dir)?;
    tracing::info!("{} chunks written", chunks);

    // Also write monolith OTBM for Canary server
    let otbm_path = chunk_dir.parent().unwrap_or(chunk_dir).join("xtrails.otbm");
    tracing::info!("Writing monolith OTBM for server: {}...", otbm_path.display());
    pte_otbm::serialize_otbm(&map, &otbm_path)?;
    let otbm_size = std::fs::metadata(&otbm_path)?.len();
    tracing::info!("  {:.1} MB", otbm_size as f64 / 1048576.0);

    let world_dir = chunk_dir.parent().unwrap_or(chunk_dir);
    std::fs::write(world_dir.join("xtrails-monster.xml"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<spawns>\r\n</spawns>\r\n")?;
    std::fs::write(world_dir.join("xtrails-house.xml"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<houses>\r\n</houses>\r\n")?;
    std::fs::write(world_dir.join("xtrails-npc.xml"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<npcs>\r\n</npcs>\r\n")?;

    tracing::info!("=== DONE ===");
    Ok(())
}
