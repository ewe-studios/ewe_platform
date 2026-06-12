//! WHY: The compact columnar form is the default DOM-op transport — columnar
//! bytes JS reads via zero-copy `TypedArray` views (decision 010/028). Protocol
//! byte 1 is the spec's "Arrow" slot; wire VERSION 1 is THIS owned layout, wire
//! version 2 is real Arrow IPC (`foundation_arrow::ArrowIpcEncoder`, servers).
//!
//! WHAT: [`ColumnarV1`] — protocol byte `1`, composing `ColumnarEncoder` (Layer 1) with
//! the `ProtocolHandler` transport (Layer 2).
//!
//! HOW: Holds an `ColumnarEncoder`; `encode_and_send` frames one arena slot and ships
//! it via the shared helpers in the parent module.

use alloc::vec::Vec;

use foundation_ui_traits::{ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{decode_payload, encode_and_write_framed, ship, HandleResult, ProtocolMethods, SendResult};

/// Compact-columnar protocol handler (protocol byte `1`, wire version 1).
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnarV1 {
    encoder: ColumnarEncoder,
}

impl ColumnarV1 {
    /// Construct the columnar protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            encoder: ColumnarEncoder,
        }
    }
}

impl ProtocolHandler for ColumnarV1 {
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

impl ProtocolMethods<Vec<DomOp>> for ColumnarV1 {
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
