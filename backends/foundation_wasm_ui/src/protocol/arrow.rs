//! WHY: Arrow is the default DOM-op transport — columnar bytes JS reads via
//! zero-copy `TypedArray` views (decision 010/028).
//!
//! WHAT: [`ArrowV1`] — protocol byte `1`, composing `ArrowEncoder` (Layer 1) with
//! the `ProtocolHandler` transport (Layer 2).
//!
//! HOW: Holds an `ArrowEncoder`; `encode_and_send` frames one arena slot and ships
//! it via the shared helpers in the parent module.

use alloc::vec::Vec;

use foundation_ui_traits::{ArrowEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{decode_payload, encode_and_ship, ship, HandleResult, ProtocolMethods, SendResult};

/// Arrow v1 protocol (protocol byte `1`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrowV1 {
    encoder: ArrowEncoder,
}

impl ArrowV1 {
    /// Construct the Arrow v1 protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            encoder: ArrowEncoder,
        }
    }
}

impl ProtocolHandler for ArrowV1 {
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

impl ProtocolMethods<Vec<DomOp>> for ArrowV1 {
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
