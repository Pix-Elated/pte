//! CLI tool to convert legacy OT assets to protobuf format.
//!
//! Usage: pte-convert <dat_path> <spr_path> <output_dir> [chunk_dir]

mod legacy_adapter;
mod legacy_converter;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: pte-convert <dat_path> <spr_path> <output_dir> [chunk_dir]");
        eprintln!();
        eprintln!("Converts legacy Tibia .dat/.spr files to CIP protobuf format.");
        eprintln!("Optionally converts RCHK chunk maps to OTBM.");
        std::process::exit(1);
    }

    let dat_path = std::path::Path::new(&args[1]);
    let spr_path = std::path::Path::new(&args[2]);
    let out_dir = std::path::Path::new(&args[3]);
    let chunk_dir = args.get(4).map(|s| std::path::Path::new(s.as_str()));

    if !dat_path.exists() {
        eprintln!("DAT file not found: {}", dat_path.display());
        std::process::exit(1);
    }
    if !spr_path.exists() {
        eprintln!("SPR file not found: {}", spr_path.display());
        std::process::exit(1);
    }

    let progress = std::sync::Arc::new(std::sync::Mutex::new(
        legacy_converter::ConvertProgress {
            progress: 0.0,
            message: "Starting…".to_string(),
        },
    ));

    // Print progress in a background thread
    let progress_clone = progress.clone();
    std::thread::spawn(move || {
        let mut last_msg = String::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let (frac, msg) = {
                let p = progress_clone.lock().unwrap();
                (p.progress, p.message.clone())
            };
            if msg != last_msg {
                eprintln!("[{:5.1}%] {}", frac * 100.0, msg);
                last_msg = msg;
            }
            if frac >= 1.0 {
                break;
            }
        }
    });

    match legacy_converter::convert_project(
        dat_path,
        spr_path,
        chunk_dir,
        out_dir,
        &progress,
    ) {
        Ok(()) => {
            eprintln!("Conversion complete! Output: {}", out_dir.display());
        }
        Err(e) => {
            eprintln!("Conversion failed: {e:#}");
            std::process::exit(1);
        }
    }
}
