//! WebTransport browser bridge (spec-55, F07).
//!
//! WHY: Browsers can open native QUIC WebTransport sessions — lower latency
//! than WebSocket relay, no head-of-line blocking. This module provides a
//! sans-I/O Rust session driven by JS callbacks.
//!
//! WHAT: [`WasmWtBridge`] — a browser WebTransport session state machine.
//! The JS side (`web_sys::WebTransport`) handles the QUIC connection; this
//! Rust struct manages the data plane: datagram queuing and the session
//! lifecycle (connecting → open → draining → closed).
//!
//! HOW: The JS bridge calls:
//!   - `on_connected()` when the `ready` promise resolves
//!   - `inject_datagram(data)` when `incomingDatagrams` readable event fires
//!   - `tick()` each animation frame to drain outbound datagrams
//!   - `close(code, reason)` when the session is terminated

use std::collections::VecDeque;

/// WASM browser WebTransport bridge — sans-I/O session driven by JS callbacks.
///
/// The JS side (`web_sys::WebTransport`) handles the QUIC connection and
/// WebTransport handshake. This Rust struct mirrors the session lifecycle
/// and manages the data plane queues.
pub struct WasmWtBridge {
    /// Whether the JS WebTransport `ready` promise has resolved.
    ready: bool,
    /// Whether the session is still open.
    open: bool,
    /// Whether datagrams are enabled (negotiated during CONNECT).
    datagrams_enabled: bool,
    /// Queue of outbound datagrams (Rust → JS: drained by `tick()`).
    outbound: VecDeque<Vec<u8>>,
    /// Queue of inbound datagrams (JS → Rust: pushed by `inject_datagram()`).
    inbound: VecDeque<Vec<u8>>,
    /// Session close info from the peer.
    close_info: Option<(u32, String)>,
}

impl WasmWtBridge {
    /// Create a new bridge. The JS side will call `on_connected()` when the
    /// WebTransport `ready` promise resolves.
    ///
    /// `datagrams_enabled` should be `true` if QUIC datagram support (RFC 9221)
    /// was negotiated during the Extended CONNECT handshake.
    #[must_use]
    pub fn new(datagrams_enabled: bool) -> Self {
        Self {
            ready: false,
            open: false,
            datagrams_enabled,
            outbound: VecDeque::new(),
            inbound: VecDeque::new(),
            close_info: None,
        }
    }

    /// Call when the JS WebTransport `ready` promise resolves.
    ///
    /// Transitions the session from Connecting → Open.
    pub fn on_connected(&mut self) {
        self.ready = true;
        self.open = true;
    }

    /// Whether the session is ready for data transfer.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Whether the session is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the session has been closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        !self.open
    }

    /// Whether datagrams are supported.
    #[must_use]
    pub fn datagrams_enabled(&self) -> bool {
        self.datagrams_enabled
    }

    /// The close code and reason from the peer, if the session was closed.
    #[must_use]
    pub fn close_info(&self) -> Option<(u32, &str)> {
        self.close_info.as_ref().map(|(c, r)| (*c, r.as_str()))
    }

    /// Fuel one tick: drain outbound datagrams for the JS side to send.
    /// Call from `requestAnimationFrame` or a valtron task tick.
    ///
    /// Returns outbound datagrams — the JS bridge sends each via
    /// `web_sys::WebTransport::datagrams().writable.getWriter().write(data)`.
    #[must_use]
    pub fn tick(&mut self) -> Vec<Vec<u8>> {
        if !self.ready {
            return Vec::new();
        }
        self.outbound.drain(..).collect()
    }

    /// Inject an inbound datagram from the JS WebTransport.
    /// Call from the `incomingDatagrams` readable event handler.
    ///
    /// The JS bridge reads from `reader.read()` and passes each chunk
    /// to this method.
    pub fn inject_datagram(&mut self, data: Vec<u8>) {
        if !self.ready {
            return;
        }
        self.inbound.push_back(data);
    }

    /// Drain inbound datagrams for the application to process.
    ///
    /// After processing, the caller forwards decrypted payloads to
    /// the relay or tunnel driver.
    #[must_use]
    pub fn drain_inbound(&mut self) -> Vec<Vec<u8>> {
        self.inbound.drain(..).collect()
    }

    /// Queue an outbound datagram to be sent to the peer.
    /// It will be drained on the next `tick()` call.
    ///
    /// Returns `Err` if datagrams are disabled or the session is closed.
    pub fn queue_datagram(&mut self, data: Vec<u8>) -> Result<(), String> {
        if !self.datagrams_enabled {
            return Err("datagrams not enabled".into());
        }
        if !self.open {
            return Err("session closed".into());
        }
        self.outbound.push_back(data);
        Ok(())
    }

    /// Close the session with an application error code.
    /// The JS side should call `web_sys::WebTransport::close()`
    /// after receiving this signal.
    pub fn close(&mut self, code: u32, reason: String) {
        self.open = false;
        self.close_info = Some((code, reason));
    }

    /// Handle a close event from the peer (received via JS `close` event).
    pub fn on_peer_close(&mut self, code: u32, reason: String) {
        self.open = false;
        self.close_info = Some((code, reason));
    }
}
