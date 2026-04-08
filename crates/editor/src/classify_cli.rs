//! CLI tool to extract and visually classify all sprites from protobuf assets.
//!
//! Usage: pte-classify <asset_dir> <output_json>

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: pte-classify <asset_dir> <output_json>");
        std::process::exit(1);
    }

    let asset_dir = std::path::Path::new(&args[1]);
    let output_path = std::path::Path::new(&args[2]);

    // Load catalog
    let catalog_path = asset_dir.join("catalog-content.json");
    let catalog = pte_assets::load_catalog(&catalog_path).expect("Failed to load catalog");
    tracing::info!("{} sprite sheets in catalog", catalog.sprite_sheets.len());

    // Load appearances
    let app_files: Vec<_> = std::fs::read_dir(asset_dir)
        .expect("Cannot read asset dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("appearances-") && name.ends_with(".dat")
        })
        .collect();

    if app_files.is_empty() {
        eprintln!("No appearances-*.dat found in {}", asset_dir.display());
        std::process::exit(1);
    }

    let mut appearances = None;
    for app_file in &app_files {
        tracing::info!("Trying {}...", app_file.file_name().to_string_lossy());
        match pte_appearances::load_appearances(&app_file.path()) {
            Ok(a) => {
                tracing::info!("  {} objects loaded", a.objects.len());
                appearances = Some(a);
                break;
            }
            Err(e) => {
                tracing::warn!("  Failed: {e:#}");
            }
        }
    }
    let appearances = appearances.expect("No valid appearances file found");

    // Load all sprite sheets
    tracing::info!("Loading sprite sheets...");
    let sheets = pte_assets::load_all_sheets(asset_dir, &catalog, |done, total| {
        if done % 500 == 0 || done == total {
            tracing::info!("  Sheets: {}/{}", done, total);
        }
    });
    tracing::info!("{} sheets loaded", sheets.len());

    // Collect sheets into a vec for lookup
    let sheet_vec: Vec<&pte_assets::SpriteSheet> = sheets.values().collect();

    fn find_sheet<'a>(
        sprite_id: u32,
        sheets: &'a [&'a pte_assets::SpriteSheet],
    ) -> Option<&'a pte_assets::SpriteSheet> {
        sheets
            .iter()
            .find(|s| sprite_id >= s.first_sprite_id && sprite_id <= s.last_sprite_id)
            .copied()
    }

    // Classify every object
    let mut results: Vec<serde_json::Value> = Vec::with_capacity(appearances.objects.len());
    let mut processed = 0u32;
    let mut failed = 0u32;

    for (&item_id, app) in &appearances.objects {
        // Get first sprite ID from first frame group
        let sprite_id = app
            .frame_group
            .first()
            .and_then(|fg| fg.sprite_info.as_ref())
            .and_then(|si| si.sprite_id.first().copied());

        let sprite_id = match sprite_id {
            Some(id) if id > 0 => id,
            _ => {
                failed += 1;
                continue;
            }
        };

        // Get pattern info
        let (pattern_w, pattern_h, total_sprites) = app
            .frame_group
            .first()
            .and_then(|fg| fg.sprite_info.as_ref())
            .map(|si| {
                (
                    si.pattern_width.unwrap_or(1),
                    si.pattern_height.unwrap_or(1),
                    si.sprite_id.len(),
                )
            })
            .unwrap_or((1, 1, 0));

        // Extract sprite pixels
        let sheet = match find_sheet(sprite_id, &sheet_vec) {
            Some(s) => s,
            None => {
                failed += 1;
                continue;
            }
        };

        let pixels = match sheet.get_sprite(sprite_id) {
            Some(p) => p,
            None => {
                failed += 1;
                continue;
            }
        };

        // Pixel analysis
        let (sw, sh) = sheet.sprite_dimensions();
        let total_px = (sw * sh) as usize;
        let mut opaque = 0u32;
        let mut sum_r = 0u64;
        let mut sum_g = 0u64;
        let mut sum_b = 0u64;
        let mut quads = [0u32; 4];

        for i in 0..total_px {
            let a = pixels[i * 4 + 3];
            if a < 10 {
                continue;
            }
            opaque += 1;
            sum_r += pixels[i * 4] as u64;
            sum_g += pixels[i * 4 + 1] as u64;
            sum_b += pixels[i * 4 + 2] as u64;
            let x = (i % sw as usize) as u32;
            let y = (i / sw as usize) as u32;
            let qi = if x < sw / 2 { 0 } else { 1 } + if y < sh / 2 { 0 } else { 2 };
            quads[qi] += 1;
        }

        let coverage = opaque as f32 / total_px as f32 * 100.0;
        let (avg_r, avg_g, avg_b) = if opaque > 0 {
            (
                (sum_r / opaque as u64) as u8,
                (sum_g / opaque as u64) as u8,
                (sum_b / opaque as u64) as u8,
            )
        } else {
            (0, 0, 0)
        };

        // Classify from flags
        let flags = app.flags.as_ref();
        let is_ground = flags.map_or(false, |f| f.bank.is_some());
        let is_clip = flags.map_or(false, |f| f.clip.unwrap_or(false));
        let is_bottom = flags.map_or(false, |f| f.bottom.unwrap_or(false));
        let is_top = flags.map_or(false, |f| f.top.unwrap_or(false));
        let is_container = flags.map_or(false, |f| f.container.unwrap_or(false));
        let is_hang = flags.map_or(false, |f| f.hang.unwrap_or(false));
        let is_unpass = flags.map_or(false, |f| f.unpass.unwrap_or(false));
        let is_take = flags.map_or(false, |f| f.take.unwrap_or(false));
        let is_cumulative = flags.map_or(false, |f| f.cumulative.unwrap_or(false));
        let is_fullbank = flags.map_or(false, |f| f.fullbank.unwrap_or(false));
        let is_unmove = flags.map_or(false, |f| f.unmove.unwrap_or(false));
        let is_rotate = flags.map_or(false, |f| f.rotate.unwrap_or(false));
        let is_animate = flags.map_or(false, |f| f.animate_always.unwrap_or(false));
        let is_liquid = flags.map_or(false, |f| f.liquidpool.unwrap_or(false));
        let has_light = flags.map_or(false, |f| f.light.is_some());
        let has_elevation = flags.map_or(false, |f| f.height.is_some());
        let has_automap = flags.map_or(false, |f| f.automap.is_some());
        let hook_south = flags.map_or(false, |f| f.hook_south.unwrap_or(false));
        let hook_east = flags.map_or(false, |f| f.hook_east.unwrap_or(false));

        let cat = if is_ground {
            "ground"
        } else if is_clip {
            "border"
        } else if is_bottom {
            "bottom"
        } else if is_top {
            "top"
        } else if is_container {
            "container"
        } else if is_hang {
            "hangable"
        } else if is_unpass {
            "wall"
        } else if is_take {
            "pickupable"
        } else if is_cumulative {
            "stackable"
        } else {
            "decoration"
        };

        let quarter = total_px as f32 / 4.0;
        let entry = serde_json::json!({
            "id": item_id,
            "cat": cat,
            "sprite_id": sprite_id,
            "sprite_size": format!("{}x{}", sw, sh),
            "coverage": (coverage * 10.0).round() / 10.0,
            "avg_color": [avg_r, avg_g, avg_b],
            "quads": {
                "TL": (quads[0] as f32 / quarter * 1000.0).round() / 10.0,
                "TR": (quads[1] as f32 / quarter * 1000.0).round() / 10.0,
                "BL": (quads[2] as f32 / quarter * 1000.0).round() / 10.0,
                "BR": (quads[3] as f32 / quarter * 1000.0).round() / 10.0,
            },
            "flags": {
                "ground": is_ground, "full_ground": is_fullbank,
                "clip": is_clip, "bottom": is_bottom, "top": is_top,
                "container": is_container, "blocking": is_unpass,
                "immovable": is_unmove, "pickupable": is_take,
                "hangable": is_hang, "rotatable": is_rotate,
                "light": has_light, "animated": is_animate,
                "liquid": is_liquid, "elevated": has_elevation,
                "minimap": has_automap, "hook_south": hook_south,
                "hook_east": hook_east,
            },
            "pattern": format!("{}x{}", pattern_w, pattern_h),
            "total_sprites": total_sprites,
        });

        results.push(entry);
        processed += 1;

        if processed % 5000 == 0 {
            tracing::info!("  {}/{} classified ({} failed)", processed, appearances.objects.len(), failed);
        }
    }

    tracing::info!("Done: {} classified, {} failed", processed, failed);

    // Summary
    let mut cats = std::collections::HashMap::new();
    for r in &results {
        *cats.entry(r["cat"].as_str().unwrap_or("?").to_string()).or_insert(0u32) += 1;
    }
    let mut cat_vec: Vec<_> = cats.into_iter().collect();
    cat_vec.sort_by(|a, b| b.1.cmp(&a.1));
    for (cat, count) in &cat_vec {
        tracing::info!("  {}: {}", cat, count);
    }

    let json = serde_json::to_string(&results).expect("JSON failed");
    std::fs::write(output_path, &json).expect("Write failed");
    tracing::info!("Written {} entries to {}", results.len(), output_path.display());
}
