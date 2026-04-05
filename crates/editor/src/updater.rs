//! Auto-updater — checks GitHub releases for new versions and self-updates.
//!
//! Flow:
//! 1. On startup, spawn a background thread to check for updates
//! 2. If a newer version is found, show a prompt
//! 3. If user accepts (or auto-update is enabled), download with progress
//! 4. Use self-replace to swap the running binary
//! 5. Prompt restart

use std::path::PathBuf;
use std::sync::mpsc;

/// Current version from Cargo.toml (set at compile time).
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository in "owner/repo" format.
const GITHUB_REPO: &str = "Pix-Elated/pte";

/// User-Agent for GitHub API requests.
const USER_AGENT: &str = concat!("pte-updater/", env!("CARGO_PKG_VERSION"));

// ── Types ────────────────────────────────────────────────────────────────────

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: String,
    pub published_at: String,
}

/// Progress of an ongoing download.
#[derive(Debug, Clone)]
pub enum UpdateProgress {
    /// Checking for updates...
    Checking,
    /// A new version is available.
    Available(UpdateInfo),
    /// No update needed.
    UpToDate,
    /// Downloading: (bytes_downloaded, total_bytes).
    Downloading { downloaded: u64, total: u64 },
    /// Download complete, applying update.
    Applying,
    /// Update applied, restart required.
    ReadyToRestart(PathBuf),
    /// Error occurred.
    Error(String),
}

/// The updater's persistent state.
pub struct UpdaterState {
    /// Channel receiving progress updates from background thread.
    pub progress_rx: Option<mpsc::Receiver<UpdateProgress>>,
    /// Latest progress message.
    pub last_progress: Option<UpdateProgress>,
    /// Whether to show the update UI.
    pub show_ui: bool,
    /// Whether the user dismissed the update prompt.
    pub dismissed: bool,
    /// Auto-update preference (skip prompt, just do it).
    pub auto_update: bool,
    /// Channel to send "go ahead and download" command.
    pub download_tx: Option<mpsc::Sender<bool>>,
    /// Whether we've started the check this session.
    pub check_started: bool,
}

impl Default for UpdaterState {
    fn default() -> Self {
        Self {
            progress_rx: None,
            last_progress: None,
            show_ui: false,
            dismissed: false,
            auto_update: false,
            download_tx: None,
            check_started: false,
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Start the update check in a background thread.
/// Returns immediately; progress is sent via the channel in UpdaterState.
pub fn start_update_check(state: &mut UpdaterState) {
    if state.check_started {
        return;
    }
    state.check_started = true;

    let (progress_tx, progress_rx) = mpsc::channel();
    let (download_tx, download_rx) = mpsc::channel();

    state.progress_rx = Some(progress_rx);
    state.download_tx = Some(download_tx);

    std::thread::spawn(move || {
        update_worker(progress_tx, download_rx);
    });
}

/// Poll for updates from the background thread. Call once per frame.
pub fn poll(state: &mut UpdaterState) {
    let Some(ref rx) = state.progress_rx else {
        return;
    };

    while let Ok(progress) = rx.try_recv() {
        match &progress {
            UpdateProgress::Available(_) => {
                if !state.dismissed {
                    state.show_ui = true;
                }
                // If auto-update, immediately trigger download
                if state.auto_update {
                    if let Some(ref tx) = state.download_tx {
                        let _ = tx.send(true);
                    }
                }
            }
            UpdateProgress::Downloading { .. } | UpdateProgress::Applying => {
                state.show_ui = true;
            }
            UpdateProgress::ReadyToRestart(_) => {
                state.show_ui = true;
            }
            UpdateProgress::Error(e) => {
                tracing::warn!("Update check failed: {}", e);
                // Don't bother the user with network errors
            }
            _ => {}
        }
        state.last_progress = Some(progress);
    }
}

/// Render the update UI overlay. Call in the app's update() after poll().
pub fn show_ui(ctx: &egui::Context, state: &mut UpdaterState) {
    if !state.show_ui {
        return;
    }

    let progress = match &state.last_progress {
        Some(p) => p.clone(),
        None => return,
    };

    match progress {
        UpdateProgress::Available(info) => {
            show_update_available(ctx, state, &info);
        }
        UpdateProgress::Downloading { downloaded, total } => {
            show_download_progress(ctx, downloaded, total);
        }
        UpdateProgress::Applying => {
            show_applying(ctx);
        }
        UpdateProgress::ReadyToRestart(path) => {
            show_restart_prompt(ctx, state, &path);
        }
        UpdateProgress::Error(msg) => {
            show_error(ctx, state, &msg);
        }
        _ => {
            state.show_ui = false;
        }
    }
}

// ── Splash screen (pre-egui) ────────────────────────────────────────────────

/// Run a blocking update check + download with a simple splash window.
/// Returns true if the app should restart.
#[allow(dead_code)]
pub fn blocking_check_and_update() -> bool {
    let progress_tx_dummy = |_: UpdateProgress| {};
    let _ = progress_tx_dummy;

    // Quick version check — don't block startup for more than 3 seconds
    let client = match reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let resp = match client.get(&url).send() {
        Ok(r) if r.status().is_success() => r,
        _ => return false,
    };

    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(_) => return false,
    };

    let tag = json["tag_name"].as_str().unwrap_or("v0.0.0");
    let remote_ver = tag.trim_start_matches('v');

    let current = match semver::Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let remote = match semver::Version::parse(remote_ver) {
        Ok(v) => v,
        Err(_) => return false,
    };

    if remote <= current {
        tracing::info!("Up to date (v{} >= v{})", current, remote);
        return false;
    }

    tracing::info!("Update available: v{} -> v{}", current, remote);
    // The actual download happens via the async UI flow
    false
}

// ── Background worker ────────────────────────────────────────────────────────

fn update_worker(tx: mpsc::Sender<UpdateProgress>, download_rx: mpsc::Receiver<bool>) {
    let _ = tx.send(UpdateProgress::Checking);

    let client = match reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(UpdateProgress::Error(format!("HTTP client: {}", e)));
            return;
        }
    };

    // Fetch latest release
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let resp = match client.get(&url).send() {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let _ = tx.send(UpdateProgress::Error(format!(
                "GitHub API: HTTP {}",
                r.status()
            )));
            return;
        }
        Err(e) => {
            let _ = tx.send(UpdateProgress::Error(format!("Network: {}", e)));
            return;
        }
    };

    let json: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(e) => {
            let _ = tx.send(UpdateProgress::Error(format!("JSON parse: {}", e)));
            return;
        }
    };

    let tag = json["tag_name"].as_str().unwrap_or("v0.0.0").to_string();
    let remote_ver_str = tag.trim_start_matches('v');

    let current = match semver::Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(_) => {
            let _ = tx.send(UpdateProgress::Error("Invalid current version".into()));
            return;
        }
    };
    let remote = match semver::Version::parse(remote_ver_str) {
        Ok(v) => v,
        Err(_) => {
            let _ = tx.send(UpdateProgress::Error(format!(
                "Invalid remote version: {}",
                remote_ver_str
            )));
            return;
        }
    };

    if remote <= current {
        let _ = tx.send(UpdateProgress::UpToDate);
        return;
    }

    // Find the Windows x64 asset
    let assets = json["assets"].as_array();
    let download_url = assets.and_then(|a| {
        a.iter().find_map(|asset| {
            let name = asset["name"].as_str()?;
            if name.contains("windows") && name.contains("x64") {
                asset["browser_download_url"].as_str().map(String::from)
            } else {
                None
            }
        })
    });

    let download_url = match download_url {
        Some(url) => url,
        None => {
            let _ = tx.send(UpdateProgress::Error(
                "No Windows x64 asset in release".into(),
            ));
            return;
        }
    };

    let release_notes = json["body"].as_str().unwrap_or("").to_string();
    let published_at = json["published_at"].as_str().unwrap_or("").to_string();

    let info = UpdateInfo {
        version: remote_ver_str.to_string(),
        release_notes,
        published_at,
    };

    let _ = tx.send(UpdateProgress::Available(info));

    // Wait for user to approve the download
    match download_rx.recv() {
        Ok(true) => {}
        _ => return,
    }

    // Download with progress tracking
    let resp = match client.get(&download_url).send() {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(UpdateProgress::Error(format!("Download failed: {}", e)));
            return;
        }
    };

    let total = resp.content_length().unwrap_or(0);
    let _ = tx.send(UpdateProgress::Downloading {
        downloaded: 0,
        total,
    });

    // Write to a temp file
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("pte-update-{}.exe", remote_ver_str));

    let mut file = match std::fs::File::create(&temp_path) {
        Ok(f) => f,
        Err(e) => {
            let _ = tx.send(UpdateProgress::Error(format!("Create temp file: {}", e)));
            return;
        }
    };

    let mut downloaded: u64 = 0;
    let mut reader = std::io::BufReader::new(resp);
    let mut buf = [0u8; 65536];
    loop {
        use std::io::Read;
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = tx.send(UpdateProgress::Error(format!("Download read: {}", e)));
                return;
            }
        };
        use std::io::Write;
        if let Err(e) = file.write_all(&buf[..n]) {
            let _ = tx.send(UpdateProgress::Error(format!("Write temp: {}", e)));
            return;
        }
        downloaded += n as u64;
        // Send progress every 256KB
        if downloaded % (256 * 1024) < 65536 {
            let _ = tx.send(UpdateProgress::Downloading { downloaded, total });
        }
    }
    drop(file);

    let _ = tx.send(UpdateProgress::Applying);

    // Use self-replace to atomically swap the running binary
    match self_replace::self_replace(&temp_path) {
        Ok(()) => {
            let _ = std::fs::remove_file(&temp_path);
            let exe_path = std::env::current_exe().unwrap_or_default();
            let _ = tx.send(UpdateProgress::ReadyToRestart(exe_path));
        }
        Err(e) => {
            let _ = tx.send(UpdateProgress::Error(format!("Self-replace failed: {}", e)));
        }
    }
}

// ── UI panels ────────────────────────────────────────────────────────────────

fn show_update_available(ctx: &egui::Context, state: &mut UpdaterState, info: &UpdateInfo) {
    egui::Window::new("Update Available")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading(format!("v{} → v{}", CURRENT_VERSION, info.version));
            ui.add_space(4.0);

            if !info.published_at.is_empty() {
                ui.label(
                    egui::RichText::new(format!(
                        "Released: {}",
                        &info.published_at[..10.min(info.published_at.len())]
                    ))
                    .size(11.0)
                    .color(egui::Color32::from_gray(150)),
                );
            }

            if !info.release_notes.is_empty() {
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.label(&info.release_notes);
                    });
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Update Now").clicked() {
                    if let Some(ref tx) = state.download_tx {
                        let _ = tx.send(true);
                    }
                }
                if ui.button("Skip").clicked() {
                    state.dismissed = true;
                    state.show_ui = false;
                }
            });
        });
}

fn show_download_progress(ctx: &egui::Context, downloaded: u64, total: u64) {
    egui::Window::new("Updating...")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let fraction = if total > 0 {
                downloaded as f32 / total as f32
            } else {
                0.0
            };

            ui.label(format!(
                "Downloading: {:.1} / {:.1} MB",
                downloaded as f64 / 1_048_576.0,
                total as f64 / 1_048_576.0,
            ));

            let bar = egui::ProgressBar::new(fraction).text(format!("{:.0}%", fraction * 100.0));
            ui.add(bar);

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Do not close the application.")
                    .size(10.0)
                    .color(crate::theme::WARNING),
            );

            ctx.request_repaint();
        });
}

fn show_applying(ctx: &egui::Context) {
    egui::Window::new("Applying Update")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spinner();
            ui.label("Replacing binary...");
            ctx.request_repaint();
        });
}

fn show_restart_prompt(ctx: &egui::Context, state: &mut UpdaterState, exe_path: &std::path::Path) {
    egui::Window::new("Update Complete")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Update installed successfully!")
                    .color(crate::theme::SUCCESS)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label("Restart the application to use the new version.");

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Restart Now").clicked() {
                    // Launch new process and exit
                    let _ = std::process::Command::new(exe_path).spawn();
                    std::process::exit(0);
                }
                if ui.button("Later").clicked() {
                    state.show_ui = false;
                }
            });
        });
}

fn show_error(ctx: &egui::Context, state: &mut UpdaterState, msg: &str) {
    egui::Window::new("Update Error")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(msg).color(crate::theme::ERROR));
            ui.add_space(8.0);
            if ui.button("OK").clicked() {
                state.show_ui = false;
                state.dismissed = true;
            }
        });
}
