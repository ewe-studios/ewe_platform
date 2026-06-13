//! # Server-driven UI: broadcast an App's frames to live clients
//!
//! WHY: A native `foundation_wasm_ui` App can drive REAL browsers — mount a
//! component + flip a signal in Rust, and the encoded DOM frames stream to every
//! connected page where `<mount-stream>` applies them. That fan-out (with a
//! replayed backlog so late joiners catch up) is a real SERVER capability, not a
//! test detail, so it lives here. The App never leaves its thread; only the
//! `Send` [`BroadcastTx`] crosses to the server/worker threads.
//!
//! WHAT: [`FrameTransport`] (a live client connection the transport layer
//! implements — SSE, WebSocket, …), [`Broadcaster`] (owned connections + a
//! replayed backlog), [`BroadcastTx`] (the `Send` push handle), and
//! [`BroadcastSink`] (a [`ProtocolMethods`] sink that ships an App's encoded
//! batches to a `BroadcastTx` on every `stabilize`).
//!
//! HOW: Transport-agnostic by design — the broadcaster holds
//! `Box<dyn FrameTransport + Send>`, so this module depends on NO socket/HTTP
//! crate; foundation_http (or any server) provides the `FrameTransport` impl.
//! `BroadcastSink::encode_and_write` does `encode_with_envelope(&encoder, ops)`
//! (exactly as [`FrameSink`](crate::protocol::FrameSink)) then `tx.send(frame)`.
//! The wire is the envelope's business: columnar (default), JSON, or Arrow IPC
//! all ride [`BroadcastTx::send`]; raw markup (the HTML protocol) rides
//! [`BroadcastTx::send_text`]. Native-only (`std`), target-gated like the CLI.

use std::prelude::rust_2021::*;
use std::sync::{Arc, Mutex};

use core::cell::Cell;

use foundation_ui_traits::{encode_with_envelope, ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use crate::protocol::{HandleResult, ProtocolMethods, SendResult};

/// A live client connection a [`Broadcaster`] fans frames to. The transport
/// layer implements it — e.g. foundation_http's SSE stream. Each method returns
/// `false` when the connection is gone, so the broadcaster can drop it.
pub trait FrameTransport {
    /// Deliver a BINARY (envelope-encoded) frame — the DomOp wires (columnar,
    /// Arrow IPC, JSON). SSE transports base64 it; WebSocket sends it raw.
    fn send_binary(&mut self, frame: &[u8]) -> bool;
    /// Deliver a TEXT frame (raw markup) — the HTML protocol.
    fn send_text(&mut self, text: &str) -> bool;
}

/// One channel frame: an envelope-encoded DomOp batch, or raw markup (HTML).
enum Frame {
    Binary(Vec<u8>),
    Text(String),
}

impl Frame {
    /// Write this frame to a connection the way its protocol expects.
    fn write(&self, conn: &mut (dyn FrameTransport + Send)) -> bool {
        match self {
            Frame::Binary(bytes) => conn.send_binary(bytes),
            Frame::Text(text) => conn.send_text(text),
        }
    }
}

/// Owned connections + the frame backlog replayed to late joiners.
struct Inner {
    conns: Vec<Box<dyn FrameTransport + Send>>,
    backlog: Vec<Frame>,
}

/// Shared fan-out of server-driven UI connections. Cheap to clone (`Arc`) — the
/// server keeps one handle and shares another into its request `ContextBag`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: Arc<Mutex<Inner>>,
}

impl Broadcaster {
    /// An empty broadcaster.
    #[must_use]
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(Inner { conns: Vec::new(), backlog: Vec::new() })) }
    }

    /// A `Send` handle for pushing frames (held by an App's [`BroadcastSink`]).
    #[must_use]
    pub fn sender(&self) -> BroadcastTx {
        BroadcastTx { inner: self.inner.clone() }
    }

    /// Register a freshly-connected client, replaying the backlog into it so a
    /// late joiner catches up (kills the connect/stabilize race).
    pub fn register(&self, mut conn: impl FrameTransport + Send + 'static) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        if inner.backlog.iter().all(|frame| frame.write(&mut conn)) {
            inner.conns.push(Box::new(conn));
        }
    }

    /// Drop all connections (teardown closes the sockets).
    pub fn close(&self) {
        self.inner.lock().expect("broadcaster poisoned").conns.clear();
    }

    /// Number of frames pushed so far (diagnostics).
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.inner.lock().expect("broadcaster poisoned").backlog.len()
    }

    /// Number of currently-connected clients (diagnostics).
    #[must_use]
    pub fn conn_count(&self) -> usize {
        self.inner.lock().expect("broadcaster poisoned").conns.len()
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// A `Send` write handle: push a frame to every connected client (and the
/// backlog). Cheap to clone (`Arc`).
#[derive(Clone)]
pub struct BroadcastTx {
    inner: Arc<Mutex<Inner>>,
}

impl BroadcastTx {
    /// Deliver one BINARY (envelope-encoded) frame — the DomOp wires: columnar,
    /// Arrow IPC, JSON. The envelope self-describes its protocol/version.
    pub fn send(&self, frame: &[u8]) {
        self.push(Frame::Binary(frame.to_vec()));
    }

    /// Deliver one TEXT frame (raw markup) — the HTML protocol, routed through
    /// the runtime's `routeHtml` (materialize / island morph).
    pub fn send_text(&self, markup: &str) {
        self.push(Frame::Text(markup.to_string()));
    }

    fn push(&self, frame: Frame) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        inner.conns.retain_mut(|conn| frame.write(conn.as_mut()));
        inner.backlog.push(frame);
    }
}

/// An App protocol sink that streams framed batches to clients through a
/// [`BroadcastTx`]. Generic over the Layer-1 encoder `E` (columnar default,
/// JSON, Arrow IPC) — the wire is just an envelope byte to the broadcaster.
pub struct BroadcastSink<E = ColumnarEncoder> {
    encoder: E,
    tx: BroadcastTx,
    next_id: Cell<u32>,
}

impl<E> BroadcastSink<E> {
    /// A sink over a specific encoder, shipping frames to `tx`.
    pub fn with_encoder(encoder: E, tx: BroadcastTx) -> Self {
        Self { encoder, tx, next_id: Cell::new(0) }
    }
}

impl BroadcastSink<ColumnarEncoder> {
    /// The default columnar sink (matches `<mount-stream protocol="arrow">`).
    #[must_use]
    pub fn new(tx: BroadcastTx) -> Self {
        Self::with_encoder(ColumnarEncoder, tx)
    }
}

impl<E: ProtocolEncoder<Vec<DomOp>>> ProtocolHandler for BroadcastSink<E> {
    fn protocol_byte(&self) -> u8 {
        self.encoder.protocol_byte()
    }
    fn version(&self) -> u8 {
        self.encoder.version()
    }
    // Frames are shipped at encode time; there is no wasm/JS host to call into.
    fn send_to_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}
    fn handle_from_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}
    fn ack(&self, _memory_id: MemoryId, _memory: &mut MemoryAllocations) {}
}

impl<E: ProtocolEncoder<Vec<DomOp>>> ProtocolMethods<Vec<DomOp>> for BroadcastSink<E> {
    fn encode_and_write(
        &self,
        ops: Vec<DomOp>,
        _memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        let op_count = ops.len();
        let frame = encode_with_envelope(&self.encoder, ops);
        let encoded_bytes = frame.len();
        self.tx.send(&frame); // → broadcaster → transport → browser

        let id = self.next_id.get();
        self.next_id.set(id + 1);
        (
            SendResult { memory_id: MemoryId::new(id, 0), op_count, encoded_bytes },
            core::ptr::null(),
            0,
        )
    }

    // Send-only sink (server → browser); nothing is received back through it.
    fn handle_received(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) -> HandleResult {
        Ok(Vec::new())
    }
}
