//! WHY: Protocol byte 1's wire VERSION 2 is real Apache Arrow IPC
//! `RecordBatch`es (spec-39 F05) — consumers that want standard Arrow tooling
//! (the embedded `apache-arrow.js`, server pipelines) pick it over the
//! compact columnar (VERSION 1, our owned format — deliberately NOT called
//! "arrow" anywhere).
//!
//! WHAT: [`ArrowIpcV2`] — protocol byte `1`, VERSION `2`, composing
//! `foundation_arrow::ArrowIpcEncoder` (Layer 1) with the `ProtocolHandler`
//! transport (Layer 2). The `arrow` cargo feature (ON by default —
//! arrow-native support is a project goal; arrow-rs rides
//! default-features=false and builds for wasm32) lets size-sensitive
//! artifacts opt out.
//!
//! HOW: Identical composition to [`super::JsonV1`]; only the encoder
//! differs.

use alloc::vec::Vec;

use foundation_arrow::ArrowIpcEncoder;
use foundation_ui_traits::{DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{
    decode_payload, encode_and_write_framed, ship, HandleResult, ProtocolMethods, SendResult,
};

/// Arrow IPC protocol (protocol byte `1`, wire VERSION `2`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrowIpcV2 {
    encoder: ArrowIpcEncoder,
}

impl ArrowIpcV2 {
    /// Construct the Arrow IPC v2 protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            encoder: ArrowIpcEncoder,
        }
    }
}

impl ProtocolHandler for ArrowIpcV2 {
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

impl ProtocolMethods<Vec<DomOp>> for ArrowIpcV2 {
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
