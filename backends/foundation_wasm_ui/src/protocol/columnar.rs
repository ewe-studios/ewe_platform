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

use foundation_ui_traits::{ColumnarBatch, ColumnarEncoder, DomOp, ProtocolEncoder};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler, WasmEnvelope};

use super::{decode_payload, ship, HandleResult, ProtocolMethods, SendResult};

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

impl ColumnarV1 {
    /// Frame an accumulated [`ColumnarBatch`] into one arena slot WITHOUT
    /// shipping — the columnar-native flush path (feature 19 §3): no
    /// `Vec<DomOp>` exists anywhere on this route.
    ///
    /// ALIGNMENT (feature 19 §2): the slot address is known here, so the
    /// payload's `pad_len` shim is computed against the ABSOLUTE address —
    /// the 8-byte columnar header lands on an 8-byte boundary in linear
    /// memory and the JS parser takes true zero-copy `TypedArray` views.
    ///
    /// # Panics
    /// Panics if the arena cannot allocate or address the slot (decision 028
    /// Error Cases — an allocation failure here means a fundamental leak).
    pub fn write_batch(
        &self,
        batch: &ColumnarBatch,
        memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        // Worst-case slot size: envelope + payload with the largest shim.
        let max_total = WasmEnvelope::HEADER_LEN + batch.serialized_len(7);
        let mem_id = memory
            .allocate(max_total as u64)
            .expect("arena allocate failed");
        let slot = memory.get(mem_id).expect("arena get failed");

        // `allocate` zero-fills to `max_total`, so the Vec's buffer (and its
        // address) is FIXED — later writes never exceed this capacity, so no
        // reallocation can move it between the address read and the ship.
        let (base_ptr, _) = slot.as_address().expect("arena address failed");
        let payload_at = base_ptr as usize + WasmEnvelope::HEADER_LEN;
        // Header lands at payload_at + 1 (marker) + pad — solve for pad.
        #[allow(clippy::cast_possible_truncation)] // result is 0..=7 by construction
        let pad = ((8 - ((payload_at + 1) % 8)) % 8) as u8;

        let payload = batch.serialize_with_pad(pad);
        let framed = WasmEnvelope::write(
            self.protocol_byte(),
            self.version(),
            mem_id.as_u64(),
            &payload,
        );
        slot.apply(|m| {
            m.clear();
            m.extend_from_slice(&framed);
        });

        let (ptr, len) = slot.as_address().expect("arena address failed");
        debug_assert_eq!(ptr, base_ptr, "slot buffer must not move");
        let len = usize::try_from(len).expect("slot length exceeds usize");
        (
            SendResult {
                memory_id: mem_id,
                op_count: batch.len(),
                encoded_bytes: payload.len(),
            },
            ptr,
            len,
        )
    }
}

impl ProtocolMethods<Vec<DomOp>> for ColumnarV1 {
    fn encode_and_write(
        &self,
        ops: Vec<DomOp>,
        memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        // Route the row-shaped path through the SAME aligned framing.
        let mut batch = ColumnarBatch::new();
        for op in &ops {
            batch.push(op);
        }
        self.write_batch(&batch, memory)
    }

    fn handle_received(&self, _memory_id: MemoryId, ptr: *const u8, len: usize) -> HandleResult {
        decode_payload(&self.encoder, ptr, len)
    }

}
