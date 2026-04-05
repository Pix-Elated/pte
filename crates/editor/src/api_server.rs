//! Embedded HTTP API server for MCP integration.
//!
//! Runs on a background thread, communicating with the UI via channels.
//! The Python MCP bridge sends JSON-RPC-style requests; this server
//! translates them into `ApiCommand`s, which the UI processes in its
//! `update()` loop, then sends back the response.

use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;

/// Port for the embedded API server.
pub const DEFAULT_PORT: u16 = 9982;

// ── Request / Response types ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApiRequest {
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ApiResponse {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// ── Command channel types ──────────────────────────────────────────

/// A command sent from the HTTP thread to the UI thread.
pub struct ApiCommand {
    pub request: ApiRequest,
    pub respond: oneshot::Sender<ApiResponse>,
}

/// Oneshot channel for sending a single response back.
pub mod oneshot {
    use std::sync::mpsc;

    pub struct Sender<T>(mpsc::SyncSender<T>);
    pub struct Receiver<T>(mpsc::Receiver<T>);

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (Sender(tx), Receiver(rx))
    }

    impl<T> Sender<T> {
        pub fn send(self, value: T) {
            let _ = self.0.send(value);
        }
    }

    impl<T> Receiver<T> {
        pub fn recv_timeout(&self, dur: std::time::Duration) -> Option<T> {
            self.0.recv_timeout(dur).ok()
        }
    }
}

/// Start the API server on a background thread.
/// Returns the command receiver that the UI should poll.
pub fn start(port: u16) -> mpsc::Receiver<ApiCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<ApiCommand>();

    thread::Builder::new()
        .name("api-server".into())
        .spawn(move || {
            run_server(port, cmd_tx);
        })
        .expect("failed to spawn API server thread");

    cmd_rx
}

fn run_server(port: u16, cmd_tx: mpsc::Sender<ApiCommand>) {
    let addr = format!("127.0.0.1:{}", port);
    let server = match tiny_http::Server::http(&addr) {
        Ok(s) => {
            tracing::info!("API server listening on http://localhost:{}", port);
            s
        }
        Err(e) => {
            tracing::error!("Failed to start API server on port {}: {}", port, e);
            return;
        }
    };

    for mut request in server.incoming_requests() {
        // Only accept POST
        if request.method() != &tiny_http::Method::Post {
            let resp = tiny_http::Response::from_string(
                serde_json::to_string(&ApiResponse::error("Use POST")).unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(tiny_http::Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
            .with_status_code(405);
            let _ = request.respond(resp);
            continue;
        }

        // Handle CORS preflight
        if request.method().as_str() == "OPTIONS" {
            let resp = tiny_http::Response::from_string("")
                .with_header(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Methods", "POST, OPTIONS")
                        .unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes("Access-Control-Allow-Headers", "Content-Type")
                        .unwrap(),
                );
            let _ = request.respond(resp);
            continue;
        }

        // Read body (limit to 1MB)
        let content_length = request.body_length().unwrap_or(0);
        if content_length > 1_048_576 {
            let resp = tiny_http::Response::from_string(
                serde_json::to_string(&ApiResponse::error("Request too large")).unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_status_code(413);
            let _ = request.respond(resp);
            continue;
        }

        let mut body = String::new();
        if let Err(e) = request.as_reader().read_to_string(&mut body) {
            let resp = tiny_http::Response::from_string(
                serde_json::to_string(&ApiResponse::error(format!("Read error: {e}"))).unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_status_code(400);
            let _ = request.respond(resp);
            continue;
        }

        // Parse JSON request
        let api_req: ApiRequest = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                let resp = tiny_http::Response::from_string(
                    serde_json::to_string(&ApiResponse::error(format!("Invalid JSON: {e}")))
                        .unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                )
                .with_status_code(400);
                let _ = request.respond(resp);
                continue;
            }
        };

        // Create response channel and send command to UI thread
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = ApiCommand {
            request: api_req,
            respond: resp_tx,
        };

        if cmd_tx.send(cmd).is_err() {
            // UI thread shut down
            break;
        }

        // Wait for UI thread to process (timeout 10s)
        let api_resp = resp_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|| ApiResponse::error("Timeout waiting for editor response"));

        let json = serde_json::to_string(&api_resp)
            .unwrap_or_else(|_| r#"{"ok":false,"error":"Serialization error"}"#.to_string());

        let resp = tiny_http::Response::from_string(json)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(
                tiny_http::Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap(),
            );
        let _ = request.respond(resp);
    }
}
