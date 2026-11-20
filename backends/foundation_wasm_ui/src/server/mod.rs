//! Send one frame to each connected browser.
//!
//! A broadcaster owns its connections and keeps their replay backlog.
//! The default broadcast sink sends complete EWF1 frames.
//! An application can select a legacy encoder with `BroadcastSink::with_encoder`.

use std::prelude::rust_2021::*;
use std::sync::{Arc, Mutex};

use core::cell::Cell;

use foundation_ui_traits::{encode_with_envelope, ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use crate::protocol::{LegacyHandleResult, LegacyProtocolMethods, SendResult};

/// A live client connection a [`Broadcaster`] fans frames to. The transport
/// layer implements it — for example, the event stream in `foundation_http`. Each method returns
/// `false` when the connection is gone, so the broadcaster can drop it.
pub trait FrameTransport {
    /// Deliver a BINARY (envelope-encoded) frame — the `DomOp` wires (columnar,
    /// Arrow IPC, JSON). SSE transports base64 it; WebSocket sends it raw.
    fn send_binary(&mut self, frame: &[u8]) -> bool;
    /// Deliver a TEXT frame (raw markup) — the HTML protocol.
    fn send_text(&mut self, text: &str) -> bool;
}

/// One channel frame: an envelope-encoded `DomOp` batch, or raw markup (HTML).
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
/// server keeps one handle and shares another into its request `Ctx`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: Arc<Mutex<Inner>>,
}

impl Broadcaster {
    /// An empty broadcaster.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                conns: Vec::new(),
                backlog: Vec::new(),
            })),
        }
    }

    /// A `Send` handle for pushing frames (held by an App's [`BroadcastSink`]).
    #[must_use]
    pub fn sender(&self) -> BroadcastTx {
        BroadcastTx {
            inner: self.inner.clone(),
        }
    }

    /// Register a freshly-connected client, replaying the backlog into it so a
    /// late joiner catches up (kills the connect/stabilize race).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    pub fn register(&self, mut conn: impl FrameTransport + Send + 'static) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        if inner.backlog.iter().all(|frame| frame.write(&mut conn)) {
            inner.conns.push(Box::new(conn));
        }
    }

    /// Drop all connections (teardown closes the sockets).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    pub fn close(&self) {
        self.inner
            .lock()
            .expect("broadcaster poisoned")
            .conns
            .clear();
    }

    /// Number of frames pushed so far (diagnostics).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.inner
            .lock()
            .expect("broadcaster poisoned")
            .backlog
            .len()
    }

    /// Number of currently-connected clients (diagnostics).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
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
    /// Deliver one binary frame. Its bytes identify its format.
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
pub struct LegacyBroadcastSink<E = ColumnarEncoder> {
    encoder: E,
    tx: BroadcastTx,
    next_id: Cell<u32>,
}

impl<E> LegacyBroadcastSink<E> {
    /// A sink over a specific encoder, shipping frames to `tx`.
    pub fn with_encoder(encoder: E, tx: BroadcastTx) -> Self {
        Self {
            encoder,
            tx,
            next_id: Cell::new(0),
        }
    }
}

/// Select the default frame broadcast sink or an explicit legacy codec.
type BroadcastOutput = crate::protocol::FrameOutput<
    Box<dyn Fn(Vec<u8>) -> Result<(), crate::protocol::ProtocolError>>,
>;
pub struct BroadcastSink(BroadcastOutput);

impl BroadcastSink {
    /// Send one bare frame per successful receiver flush.
    #[must_use]
    pub fn new(tx: BroadcastTx) -> Self {
        Self(crate::protocol::FrameOutput::new(
            crate::protocol::FrameV1::new(0),
            Box::new(move |bytes| {
                tx.push(Frame::Binary(bytes));
                Ok(())
            }),
        ))
    }

    /// Use a legacy codec for an independently deployed client.
    pub fn with_encoder<E>(encoder: E, tx: BroadcastTx) -> LegacyBroadcastSink<E> {
        LegacyBroadcastSink::with_encoder(encoder, tx)
    }
}

impl ProtocolHandler for BroadcastSink {
    fn protocol_byte(&self) -> u8 {
        self.0.protocol_byte()
    }
    fn version(&self) -> u8 {
        self.0.version()
    }
    fn send_to_js(&self, _: MemoryId, _: *const u8, _: usize) {}
    fn handle_from_js(&self, _: MemoryId, _: *const u8, _: usize) {}
    fn ack(&self, _: MemoryId, _: &mut MemoryAllocations) {}
}

impl crate::protocol::ProtocolMethods<Vec<foundation_ui_wire::OperationT>> for BroadcastSink {
    fn encode_and_write(
        &self,
        ops: Vec<foundation_ui_wire::OperationT>,
        memory: &mut MemoryAllocations,
    ) -> crate::protocol::WrittenBatch {
        self.0.encode_and_write(ops, memory)
    }
    fn handle_received(&self, bytes: &[u8]) -> crate::protocol::HandleResult {
        self.0.handle_received(bytes)
    }
}

impl<E: ProtocolEncoder<Vec<DomOp>>> ProtocolHandler for LegacyBroadcastSink<E> {
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

impl<E: ProtocolEncoder<Vec<DomOp>>> LegacyProtocolMethods<Vec<DomOp>> for LegacyBroadcastSink<E> {
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
            SendResult {
                memory_id: MemoryId::new(id, 0),
                op_count,
                encoded_bytes,
            },
            core::ptr::null(),
            0,
        )
    }

    // Send-only sink (server → browser); nothing is received back through it.
    fn handle_received(
        &self,
        _memory_id: MemoryId,
        _ptr: *const u8,
        _len: usize,
    ) -> LegacyHandleResult {
        Ok(Vec::new())
    }
}

mod live;
pub use live::{BacklogConfig, LiveChannel, LiveError, LiveTx};
mod live_sink;
pub use live_sink::FrameBroadcastSink;
