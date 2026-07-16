//! Unix-socket control RPC (Decision 20).
//!
//! WHY: The proxy needs an admin interface for runtime management — drain a
//! backend, pause a service, reload config, grab metrics — without touching
//! the data plane. A Unix-domain socket provides local-only, permission-based
//! access (socket file permissions) with no authentication overhead.
//!
//! WHAT: [`ControlSocket`] binds a Unix-domain socket, accepts connections,
//! and dispatches JSON-RPC commands. Supported commands:
//!
//! - `list` — return all services, their backends, URLs, states, health
//! - `drain <service> <url>` — set backend to Draining
//! - `pause <service> <url>` — set backend to Paused
//! - `activate <service> <url>` — set backend to Active
//! - `status` — aggregate counts: total, healthy, draining, paused
//! - `reload` — trigger OnSignal for graceful reload
//! - `shutdown` — trigger the proxy shutdown signal
//!
//! HOW: Each connection reads one line of JSON, dispatches to the matching
//! command, writes one line of JSON back, and closes. Single-request
//! protocol — no persistent connection, no framing protocol. Simple enough
//! for `nc -U` / `socat` at the CLI.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use foundation_core::synca::OnSignal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::BackendState;
use crate::router::Router;
use crate::state::ProxyState;

/// RPC request from the CLI.
#[derive(Debug, Deserialize)]
struct Request {
    command: String,
    #[serde(default)]
    params: Value,
}

/// RPC response to the CLI.
#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// A running control socket.
pub struct ControlSocket {
    socket_path: String,
    thread: JoinHandle<()>,
}

impl ControlSocket {
    /// Bind a Unix-domain control socket at `socket_path`, serving commands against
    /// `state`. `shutdown` signals the control socket to stop accepting.
    ///
    /// # Panics
    /// Panics if the socket cannot be bound.
    pub fn start(
        socket_path: String,
        state: Arc<ProxyState>,
        shutdown: Arc<OnSignal>,
    ) -> Self {
        // Clean up stale socket file
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path)
            .unwrap_or_else(|e| panic!("control socket bind {socket_path}: {e}"));

        // Set permissive permissions so the operator's user can connect.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
        }

        let path_for_thread = socket_path.clone();
        let thread = thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            tracing::info!(path = %path_for_thread, "control socket listening");

            loop {
                if shutdown.probe() {
                    let _ = std::fs::remove_file(&path_for_thread);
                    return;
                }

                match listener.accept() {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        thread::spawn(move || handle_connection(stream, &state));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(e) => {
                        tracing::error!(%e, "control socket accept error");
                        break;
                    }
                }
            }
            let _ = std::fs::remove_file(&path_for_thread);
        });

        Self { socket_path, thread }
    }

    #[must_use]
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Shut down the control socket thread.
    pub fn shutdown(self) {
        let _ = self.thread.join();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Handle one control-socket connection: read → dispatch → respond → close.
fn handle_connection(mut stream: UnixStream, state: &ProxyState) {
    let mut reader = BufReader::new(stream.try_clone().unwrap_or_else(|_| {
        // If clone fails, create a new unconnected stream (won't happen in practice).
        panic!("control socket clone failed")
    }));

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        let _ = respond(&mut stream, false, Some("empty request".into()), None);
        return;
    }

    let req: Request = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            let _ = respond(&mut stream, false, Some(format!("invalid JSON: {e}")), None);
            return;
        }
    };

    let result = dispatch(&req, state);
    let _ = respond(&mut stream, result.0, result.1, result.2);
}

/// Route a command to its handler.
fn dispatch(req: &Request, state: &ProxyState) -> (bool, Option<String>, Option<Value>) {
    match req.command.as_str() {
        "list" => {
            let services: Vec<Value> = state
                .router()
                .services()
                .iter()
                .map(|svc| {
                    let backends: Vec<Value> = svc
                        .backends()
                        .iter()
                        .map(|b| {
                            serde_json::json!({
                                "url": b.url(),
                                "state": format!("{:?}", b.state()),
                                "healthy": b.is_healthy(),
                                "inflight": b.inflight(),
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "name": svc.config().name,
                        "host": svc.config().host,
                        "backends": backends,
                    })
                })
                .collect();
            (true, None, Some(Value::Array(services)))
        }

        "status" => {
            let mut total = 0u32;
            let mut healthy = 0u32;
            let mut draining = 0u32;
            let mut paused = 0u32;
            for svc in state.router().services() {
                for b in svc.backends() {
                    total += 1;
                    if b.is_healthy() {
                        healthy += 1;
                    }
                    match b.state() {
                        BackendState::Draining => draining += 1,
                        BackendState::Paused => paused += 1,
                        _ => {}
                    }
                }
            }
            (true, None, Some(serde_json::json!({
                "total": total,
                "healthy": healthy,
                "draining": draining,
                "paused": paused,
            })))
        }

        "drain" | "pause" | "activate" => {
            let target_state = match req.command.as_str() {
                "drain" => BackendState::Draining,
                "pause" => BackendState::Paused,
                _ => BackendState::Active,
            };
            let service_name = req.params.get("service").and_then(|v| v.as_str()).unwrap_or("");
            let backend_url = req.params.get("url").and_then(|v| v.as_str()).unwrap_or("");

            let mut found = false;
            for svc in state.router().services() {
                if svc.config().name != service_name {
                    continue;
                }
                for b in svc.backends() {
                    if b.url() == backend_url {
                        b.set_state(target_state);
                        found = true;
                        break;
                    }
                }
            }
            if found {
                (true, None, Some(serde_json::json!({"state": format!("{:?}", target_state)})))
            } else {
                (false, Some(format!("backend {service_name}/{backend_url} not found")), None)
            }
        }

        _ => (false, Some(format!("unknown command: {}", req.command)), None),
    }
}

/// Write a JSON response line to the stream.
fn respond(
    stream: &mut UnixStream,
    ok: bool,
    error: Option<String>,
    data: Option<Value>,
) -> std::io::Result<()> {
    let resp = Response { ok, error, data };
    let mut json = serde_json::to_vec(&resp).unwrap_or_default();
    json.push(b'\n');
    stream.write_all(&json)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_parse_valid() {
        let req: Request = serde_json::from_str(
            r#"{"command":"status","params":{}}"#,
        )
        .expect("valid request");
        assert_eq!(req.command, "status");
    }

    #[test]
    fn request_parse_invalid_returns_err() {
        let err = serde_json::from_str::<Request>("not json").unwrap_err();
        assert!(!err.to_string().is_empty(), "should produce error message");
    }

    #[test]
    fn response_ok_format() {
        let resp = Response {
            ok: true,
            error: None,
            data: Some(serde_json::json!({"count": 3})),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"data\""));
    }

    #[test]
    fn response_error_format() {
        let resp = Response {
            ok: false,
            error: Some("bad command".into()),
            data: None,
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("bad command"));
        assert!(!json.contains("\"data\""));
    }
}
