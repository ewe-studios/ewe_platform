//! # BroadcastSink — the App's protocol sink that streams to the browser (Mode 1)
//!
//! WHY: A native `foundation_wasm_ui` App should drive a REAL browser: mount a
//! component + flip a signal in Rust, and the encoded DOM frames stream to the
//! page, where `<mount-stream>` applies them. The App's protocol sink IS the
//! bridge — on each `flush`/`stabilize` it encodes the batch and hands the frame
//! to the [`Broadcaster`](crate::test::stream::Broadcaster) (a `Send` channel to
//! the server), so the App never leaves the test thread and we never touch
//! `CollectedFrames`.
//!
//! WHAT: [`BroadcastSink`] — a [`ProtocolMethods`] impl (mirrors `FrameSink`)
//! over any Layer-1 encoder, shipping framed `Vec<u8>` to a [`BroadcastTx`].
//!
//! HOW: `encode_and_write` does `encode_with_envelope(&encoder, ops)` (exactly as
//! `FrameSink`) then `tx.send(frame)` instead of collecting. Protocol-agnostic:
//! columnar (default, what `<mount-stream protocol="arrow">` decodes), JSON, or
//! Arrow all work behind the same sink.

use core::cell::Cell;

use foundation_ui_traits::{encode_with_envelope, ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};
use foundation_wasm_ui::protocol::{HandleResult, ProtocolMethods, SendResult};

use crate::test::stream::BroadcastTx;

/// A `foundation_wasm_ui` protocol sink that streams framed batches to the
/// browser through a [`BroadcastTx`]. Generic over the Layer-1 encoder `E`.
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
        self.tx.send(&frame); // → broadcaster → SSE → browser

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
