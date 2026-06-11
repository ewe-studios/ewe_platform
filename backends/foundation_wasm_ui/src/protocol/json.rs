//! WHY: JSON is the readable/debuggable transport and the format for
//! `text/event-stream-json` SSE (decision 022/028).
//!
//! WHAT: [`JsonV1`] — protocol byte `2`, composing `JsonEncoder` (Layer 1) with the
//! `ProtocolHandler` transport (Layer 2).
//!
//! HOW: Identical composition to [`super::ArrowV1`]; only the encoder differs.

use alloc::vec::Vec;

use foundation_ui_traits::{DomOp, JsonEncoder, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{decode_payload, encode_and_write_framed, ship, HandleResult, ProtocolMethods, SendResult};

/// JSON v1 protocol (protocol byte `2`).
#[derive(Clone, Copy, Debug, Default)]
pub struct JsonV1 {
    encoder: JsonEncoder,
}

impl JsonV1 {
    /// Construct the JSON v1 protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            encoder: JsonEncoder,
        }
    }
}

impl ProtocolHandler for JsonV1 {
    fn protocol_byte(&self) -> u8 {
        self.encoder.protocol_byte()
    }

    fn version(&self) -> u8 {
        self.encoder.version()
    }

    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize) {
        ship(memory_id, ptr, len);
    }

    fn handle_from_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize) {
        let _ = self.handle_received(memory_id, ptr, len);
    }
}

impl ProtocolMethods<Vec<DomOp>> for JsonV1 {
    fn encode_and_write(
        &self,
        ops: Vec<DomOp>,
        memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        encode_and_write_framed(self, &self.encoder, ops, memory)
    }

    fn handle_received(&self, _memory_id: MemoryId, ptr: *const u8, len: usize) -> HandleResult {
        decode_payload(&self.encoder, ptr, len)
    }

}
