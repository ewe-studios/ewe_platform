//! WHY: On a native server there is no JS host — `host_apply` is a no-op
//! stub, so a normal protocol's flushes silently vanish. Server-driven UIs
//! (spec-42 feature 03 §server) need each flushed batch as ENCODED WIRE
//! FRAMES the server can ship itself: WebSocket binary frames, SSE events,
//! batched HTTP responses (`mount-stream` applies them on the client).
//!
//! WHAT: [`FrameSink<E>`] — generic over the Layer-1 encoder, so a server
//! streams WHICHEVER wire its clients negotiate (feature 04): the compact
//! columnar v1 default ([`FrameSinkV1`]), Apache Arrow IPC v2, or
//! JSON. Every flush lands as a complete envelope-framed `Vec<u8>` in a
//! shared queue instead of going to FFI.
//!
//! HOW: Like `MockProtocol`, it skips the arena entirely:
//! `encode_with_envelope` produces the framed bytes directly (the pure
//! columnar encoder always pads — the universal alignment contract), and
//! `ack` never touches memory.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use foundation_ui_traits::{encode_with_envelope, ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{decode_payload, HandleResult, ProtocolMethods, SendResult};

/// Shared handle to the framed batches, oldest first — drain it into your
/// transport (`pop_front` per WS frame / SSE event).
pub type CollectedFrames = Rc<RefCell<VecDeque<Vec<u8>>>>;

/// Server-side frame collector over any Layer-1 encoder.
#[derive(Default)]
pub struct FrameSink<E> {
    encoder: E,
    frames: CollectedFrames,
    next_id: Cell<u32>,
}

/// The DEFAULT server sink: compact columnar, wire VERSION 1 — the same
/// bytes `ColumnarV1` ships through the arena.
pub type FrameSinkV1 = FrameSink<ColumnarEncoder>;

impl<E: Default> FrameSink<E> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl<E> FrameSink<E> {
    /// A sink over a specific encoder (Arrow IPC v2, JSON, custom).
    pub fn with_encoder(encoder: E) -> Self {
        Self {
            encoder,
            frames: CollectedFrames::default(),
            next_id: Cell::new(0),
        }
    }

    /// Clone the frames handle BEFORE boxing the sink into a runtime.
    #[must_use]
    pub fn frames(&self) -> CollectedFrames {
        Rc::clone(&self.frames)
    }
}

impl<E: ProtocolEncoder<Vec<DomOp>>> ProtocolHandler for FrameSink<E> {
    fn protocol_byte(&self) -> u8 {
        self.encoder.protocol_byte()
    }

    fn version(&self) -> u8 {
        self.encoder.version()
    }

    // Frames are already captured at encode time; there is no JS to ship to.
    fn send_to_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}

    fn handle_from_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}

    // No arena slot was allocated — nothing to dispose.
    fn ack(&self, _memory_id: MemoryId, _memory: &mut MemoryAllocations) {}
}

impl<E: ProtocolEncoder<Vec<DomOp>>> ProtocolMethods<Vec<DomOp>> for FrameSink<E> {
    fn encode_and_write(
        &self,
        ops: Vec<DomOp>,
        _memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        let op_count = ops.len();
        let frame = encode_with_envelope(&self.encoder, ops);
        let encoded_bytes = frame.len();
        self.frames.borrow_mut().push_back(frame);

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

    fn handle_received(&self, _memory_id: MemoryId, ptr: *const u8, len: usize) -> HandleResult {
        decode_payload(&self.encoder, ptr, len)
    }
}
