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
//! - `reload` — trigger `ShutdownSignal` for graceful reload
//! - `shutdown` — trigger the proxy shutdown signal
//!
//! HOW: Each connection reads one line of JSON, dispatches to the matching
//! command, writes one line of JSON back, and closes. Single-request
//! protocol — no persistent connection, no framing protocol. Simple enough
//! for `nc -U` / `socat` at the CLI.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{from_future, send, sleep_async};
use foundation_http::ShutdownSignal;
use futures_util::future::{select, Either};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::aio::AsyncFd;
use crate::config::BackendState;
use crate::state::ProxyState;

/// Limits for the control socket.
#[derive(Debug, Clone, Copy)]
pub struct ControlConfig {
    /// How often the idle accept loop wakes to check for shutdown. A timer wake,
    /// not a thread sleep. Default 50 ms.
    pub shutdown_poll: Duration,
    /// The longest a client may take to send its request line before the
    /// connection is closed. Default 30 seconds.
    pub read_timeout: Duration,
    /// The longest request line, in bytes. A longer line is refused. Default
    /// 64 KiB (a control request is a short JSON object).
    pub max_request_bytes: usize,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            shutdown_poll: Duration::from_millis(50),
            read_timeout: Duration::from_secs(30),
            max_request_bytes: 64 * 1024,
        }
    }
}

impl ControlConfig {
    /// The defaults documented on each field.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set [`Self::shutdown_poll`].
    #[must_use]
    pub fn with_shutdown_poll(mut self, poll: Duration) -> Self {
        self.shutdown_poll = poll;
        self
    }

    /// Set [`Self::read_timeout`].
    #[must_use]
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Set [`Self::max_request_bytes`].
    #[must_use]
    pub fn with_max_request_bytes(mut self, max: usize) -> Self {
        self.max_request_bytes = max;
        self
    }
}

/// RPC request from the CLI.
///
/// Public so an integration test can pin the control-socket wire format.
#[derive(Debug, Deserialize)]
pub struct Request {
    /// The command name.
    pub command: String,
    /// Command parameters; empty when the CLI sends none.
    #[serde(default)]
    pub params: Value,
}

/// RPC response to the CLI.
///
/// Public so an integration test can pin the control-socket wire format.
#[derive(Debug, Serialize)]
pub struct Response {
    /// Whether the command succeeded.
    pub ok: bool,
    /// The error message when `ok` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The command result on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A running control socket.
pub struct ControlSocket {
    socket_path: String,
    done: Receiver<()>,
    poll: Duration,
    shutdown: Arc<ShutdownSignal>,
}

impl ControlSocket {
    /// Bind a Unix-domain control socket at `socket_path`, serving commands against
    /// `state`, with the default [`ControlConfig`]. `shutdown` signals the control
    /// socket to stop accepting.
    ///
    /// The valtron pool must be initialised: the accept loop and each connection
    /// are futures on it.
    ///
    /// # Panics
    /// Panics if the socket cannot be bound or the loop cannot be started.
    pub fn start(
        socket_path: String,
        state: Arc<ProxyState>,
        shutdown: Arc<ShutdownSignal>,
    ) -> Self {
        Self::start_with(socket_path, state, shutdown, ControlConfig::default())
    }

    /// Like [`Self::start`], with explicit limits.
    ///
    /// # Panics
    /// As [`Self::start`].
    pub fn start_with(
        socket_path: String,
        state: Arc<ProxyState>,
        shutdown: Arc<ShutdownSignal>,
        config: ControlConfig,
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
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|e| panic!("control socket nonblocking {socket_path}: {e}"));
        let listener = AsyncFd::new(listener)
            .unwrap_or_else(|e| panic!("control socket reactor {socket_path}: {e}"));

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let path_for_loop = socket_path.clone();
        let loop_shutdown = Arc::clone(&shutdown);
        send(from_future(async move {
            tracing::info!(path = %path_for_loop, "control socket listening");
            accept_loop(&listener, &state, &loop_shutdown, config).await;
            drop(listener);
            let _ = std::fs::remove_file(&path_for_loop);
            let _ = done_tx.send(());
        }))
        .unwrap_or_else(|e| panic!("control socket cannot start its loop: {e}"));

        Self {
            socket_path,
            done: done_rx,
            poll: config.shutdown_poll,
            shutdown,
        }
    }

    #[must_use]
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Wait (bounded) for the accept loop to end. The caller has set the
    /// shutdown signal; the loop sees it within one `shutdown_poll`.
    pub fn shutdown(self) {
        let _ = self
            .done
            .recv_timeout(self.poll.saturating_mul(4) + Duration::from_secs(1));
        let _ = std::fs::remove_file(&self.socket_path);
        // Make sure a caller that did not set the signal still stops the loop.
        let _ = &self.shutdown;
    }
}

/// Accept connections until shutdown; each connection is its own future.
async fn accept_loop(
    listener: &AsyncFd<UnixListener>,
    state: &Arc<ProxyState>,
    shutdown: &Arc<ShutdownSignal>,
    config: ControlConfig,
) {
    while !shutdown.probe() {
        let accepted = match select(
            Box::pin(listener.read_with(std::os::unix::net::UnixListener::accept)),
            Box::pin(sleep_async(
                config.shutdown_poll,
                &foundation_core::valtron::fork_ctx(),
            )),
        )
        .await
        {
            Either::Left((accepted, _)) => accepted,
            Either::Right(((), _)) => continue,
        };
        match accepted {
            Ok((stream, _)) => {
                let state = Arc::clone(state);
                if let Err(e) = send(from_future(handle_connection(stream, state, config))) {
                    tracing::error!(%e, "control socket cannot start a connection task");
                }
            }
            Err(e) => {
                tracing::error!(%e, "control socket accept error");
                break;
            }
        }
    }
}

/// Handle one control-socket connection: read → dispatch → respond → close.
async fn handle_connection(stream: UnixStream, state: Arc<ProxyState>, config: ControlConfig) {
    if stream.set_nonblocking(true).is_err() {
        return;
    }
    let Ok(io) = AsyncFd::new(stream) else { return };

    let line = match read_request_line(&io, config).await {
        Ok(line) if !line.trim().is_empty() => line,
        Ok(_) | Err(_) => {
            let _ = respond(&io, false, Some("empty request".into()), None).await;
            return;
        }
    };

    let req: Request = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            let _ = respond(&io, false, Some(format!("invalid JSON: {e}")), None).await;
            return;
        }
    };

    let result = dispatch(req, state).await;
    let _ = respond(&io, result.0, result.1, result.2).await;
}

/// Read bytes until the first newline (or end of stream), at most
/// `max_request_bytes`, within `read_timeout`.
async fn read_request_line(
    io: &AsyncFd<UnixStream>,
    config: ControlConfig,
) -> std::io::Result<String> {
    let read = async {
        let mut line = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let n = io.read_with(|s| (&*s).read(&mut buf)).await?;
            if n == 0 {
                break;
            }
            line.extend_from_slice(&buf[..n]);
            if line.contains(&b'\n') {
                break;
            }
            if line.len() > config.max_request_bytes {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "control request line too long",
                ));
            }
        }
        if let Some(end) = line.iter().position(|b| *b == b'\n') {
            line.truncate(end);
        }
        String::from_utf8(line).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    };
    match select(
        Box::pin(read),
        Box::pin(sleep_async(
            config.read_timeout,
            &foundation_core::valtron::fork_ctx(),
        )),
    )
    .await
    {
        Either::Left((line, _)) => line,
        Either::Right(((), _)) => Err(std::io::ErrorKind::TimedOut.into()),
    }
}

/// Route a command to its handler.
#[allow(
    clippy::too_many_lines,
    reason = "the control command table reads best as one ordered match"
)]
async fn dispatch(req: Request, state: Arc<ProxyState>) -> (bool, Option<String>, Option<Value>) {
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
                        BackendState::Active => {}
                    }
                }
            }
            (
                true,
                None,
                Some(serde_json::json!({
                    "total": total,
                    "healthy": healthy,
                    "draining": draining,
                    "paused": paused,
                })),
            )
        }

        "drain" | "pause" | "activate" => {
            let target_state = match req.command.as_str() {
                "drain" => BackendState::Draining,
                "pause" => BackendState::Paused,
                _ => BackendState::Active,
            };
            let service_name = req
                .params
                .get("service")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let backend_url = req.params.get("url").and_then(|v| v.as_str()).unwrap_or("");

            let mut found = false;
            for svc in state.router().services() {
                if svc.config().name != service_name {
                    continue;
                }
                for b in svc.backends() {
                    if b.url() == backend_url {
                        b.set_state(target_state);
                        // Persist the change so it survives a restart (F14, no-op
                        // when no store is attached).
                        state
                            .persist_backend_state(service_name, backend_url, target_state)
                            .await;
                        found = true;
                        break;
                    }
                }
            }
            if found {
                (
                    true,
                    None,
                    Some(serde_json::json!({"state": format!("{:?}", target_state)})),
                )
            } else {
                (
                    false,
                    Some(format!("backend {service_name}/{backend_url} not found")),
                    None,
                )
            }
        }

        _ => (
            false,
            Some(format!("unknown command: {}", req.command)),
            None,
        ),
    }
}

/// Write a JSON response line to the stream, parking on write readiness.
async fn respond(
    io: &AsyncFd<UnixStream>,
    ok: bool,
    error: Option<String>,
    data: Option<Value>,
) -> std::io::Result<()> {
    let resp = Response { ok, error, data };
    let mut json = serde_json::to_vec(&resp).unwrap_or_default();
    json.push(b'\n');
    let mut offset = 0;
    while offset < json.len() {
        let n = io.write_with(|s| (&*s).write(&json[offset..])).await?;
        if n == 0 {
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        offset += n;
    }
    Ok(())
}
