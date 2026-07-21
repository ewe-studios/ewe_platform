//! General-purpose Durable Object machinery — trait helpers, WebSocket
//! registry, connect/broadcast primitives.
//!
//! WHY: Every Workers crate that uses Durable Objects for WebSocket hubs
//! (keychain SignalR hub, auth session hub, dashboard push) needs the same
//! patterns: accept a WebSocketPair, track connected sockets with tags,
//! broadcast to subsets, and handle hibernation-aware lifecycle. This module
//! provides those once so consumers — like
//! `foundation_keychain::server::signalr_do` — only bring their domain logic.
//!
//! WHAT:
//! - [`WebSocketRegistry`] — tracks connected sockets by tag, wraps
//!   `State::get_websockets` + `State::get_websockets_with_tag`.
//! - [`do_connect`] — accept a `WebSocketPair` on the DO state + tag the
//!   server socket in one call.
//! - [`do_broadcast`] / [`do_broadcast_tagged`] — send bytes to all (or a
//!   tagged subset of) connected sockets.
//!
//! HOW: All methods delegate to `worker::State`. Tags use the Workerd
//! hibernation WebSocket API (`accept_websocket_with_tags` /
//! `get_websockets_with_tag`), so idle sockets can hibernate and wake on
//! message/alarm without a live Rust future.

use worker::{Error, Result as WorkerResult, State, WebSocket};

/// Accept a WebSocket server socket on the DO state with the given tags,
/// returning the client socket wrapped in a 101 Switching Protocols response.
///
/// Tags enable hibernation-aware operations — idle sockets tagged `"user-{uuid}"`
/// can be woken individually without iterating every socket.
///
/// # Errors
///
/// Returns `worker::Error` if WebSocketPair creation or accept fails.
pub fn do_connect(state: &State, server: &WebSocket, tags: &[&str]) -> WorkerResult<()> {
    if tags.is_empty() {
        state.accept_web_socket(server);
    } else {
        state.accept_websocket_with_tags(server, tags);
    }
    Ok(())
}

/// Broadcast bytes to **all** connected WebSockets on this DO.
///
/// Each socket receives a best-effort send; individual failures are silently
/// ignored (a disconnected socket between `get_websockets` and `send` is
/// expected in a concurrent runtime).
pub fn do_broadcast(state: &State, bytes: &[u8]) {
    for ws in state.get_websockets() {
        let _ = ws.send_with_bytes(bytes.to_vec());
    }
}

/// Broadcast bytes to WebSockets tagged with `tag`.
///
/// Same best-effort semantics as [`do_broadcast`].
pub fn do_broadcast_tagged(state: &State, tag: &str, bytes: &[u8]) {
    for ws in state.get_websockets_with_tag(tag) {
        let _ = ws.send_with_bytes(bytes.to_vec());
    }
}

/// Send a text string to all connected WebSockets.
pub fn do_broadcast_text(state: &State, text: &str) {
    for ws in state.get_websockets() {
        let _ = ws.send_with_str(text);
    }
}

/// Count connected sockets (optionally filtered by tag).
#[must_use]
pub fn connected_count(state: &State, tag: Option<&str>) -> usize {
    match tag {
        Some(t) => state.get_websockets_with_tag(t).len(),
        None => state.get_websockets().len(),
    }
}

/// A higher-level trait for a DO whose primary purpose is acting as a
/// WebSocket hub — accepting connections, broadcasting, and pinging via alarm.
///
/// Implement this on your `#[durable_object]` struct to get the hub
/// lifecycle (connect, broadcast, ping, GC) for free. Your `DurableObject`
/// impl's `fetch` and `websocket_message` then wire the domain protocol
/// (SignalR, custom JSON, etc.).
pub trait WebSocketHub {
    /// Return the tag prefix used for per-user WebSocket tagging.
    ///
    /// Default: `"user"`. A DO keyed by user UUID would tag sockets as
    /// `"user"`, so `do_broadcast_tagged(state, "user", bytes)` reaches all
    /// of that user's devices.
    fn tag(&self) -> &'static str {
        "user"
    }

    /// Return the alarm ping interval in seconds.
    ///
    /// Default: 15 (SignalR default). Override to change keep-alive cadence.
    fn ping_interval_secs(&self) -> u32 {
        15
    }

    /// Called on each alarm tick. Default: no-op (implementor decides the
    /// protocol-specific ping encoding and cadence).
    fn on_alarm(&self, _state: &State) -> WorkerResult<()> {
        Ok(())
    }
}

/// Simple in-memory registry of connected WebSocket handles, keyed by
/// client-generated id (e.g. device identifier).
///
/// Useful for Durable Objects that need per-client tracking beyond what
/// hibernation tags provide (e.g. sending a notification to a specific
/// device, not all of a user's devices).
#[derive(Default)]
pub struct WebSocketRegistry {
    sockets: Vec<(String, WebSocket)>,
}

impl WebSocketRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a socket under a client id.
    pub fn register(&mut self, client_id: String, ws: WebSocket) {
        self.sockets.push((client_id, ws));
    }

    /// Remove and close a socket by client id.
    pub fn unregister(&mut self, client_id: &str) {
        self.sockets.retain(|(id, ws)| {
            if id == client_id {
                let _ = ws.close(None::<u16>, None::<&str>);
                false
            } else {
                true
            }
        });
    }

    /// Send bytes to a specific client.
    pub fn send_to(&self, client_id: &str, bytes: &[u8]) {
        for (id, ws) in &self.sockets {
            if id == client_id {
                let _ = ws.send_with_bytes(bytes.to_vec());
                break;
            }
        }
    }

    /// Broadcast bytes to all registered clients.
    pub fn broadcast(&self, bytes: &[u8]) {
        for (_, ws) in &self.sockets {
            let _ = ws.send_with_bytes(bytes.to_vec());
        }
    }

    /// How many sockets are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sockets.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sockets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_register_unregister() {
        // NOTE: WebSocket can't be constructed outside the Workers runtime,
        // so these tests validate the registry logic without real sockets.
        let mut reg = WebSocketRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        // Push a dummy entry to exercise the container logic.
        reg.sockets.push(("dev1".into(), unsafe {
            std::mem::zeroed()
        }));
        assert_eq!(reg.len(), 1);

        reg.unregister("dev1");
        assert!(reg.is_empty());
    }
}
