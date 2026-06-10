//! WHY: The Custom Binary protocol packs ops (and, in the full encoder, the text
//! pool) into a single arena slot — a compact alternative to Arrow/JSON for callers
//! that don't need columnar or readable formats (decision 028).
//!
//! WHAT: [`CustomBinaryV1`] — protocol byte `0`, composing `CustomBinaryEncoder`
//! (Layer 1) with the `ProtocolHandler` transport (Layer 2).
//!
//! HOW: Identical composition to [`super::ArrowV1`]; only the encoder differs.

use alloc::vec::Vec;

use foundation_ui_traits::{CustomBinaryEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{decode_payload, encode_and_ship, ship, HandleResult, ProtocolMethods, SendResult};

/// Custom Binary v1 protocol (protocol byte `0`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CustomBinaryV1 {
    encoder: CustomBinaryEncoder,
}

impl CustomBinaryV1 {
    /// Construct the Custom Binary v1 protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            encoder: CustomBinaryEncoder,
        }
    }
}

impl ProtocolHandler for CustomBinaryV1 {
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

impl ProtocolMethods<Vec<DomOp>> for CustomBinaryV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        encode_and_ship(self, &self.encoder, ops, memory)
    }

    fn handle_received(&self, _memory_id: MemoryId, ptr: *const u8, len: usize) -> HandleResult {
        decode_payload(&self.encoder, ptr, len)
    }

    fn ack(&self, memory_id: MemoryId, memory: &mut MemoryAllocations) {
        let _ = memory.deallocate(memory_id);
    }
}
