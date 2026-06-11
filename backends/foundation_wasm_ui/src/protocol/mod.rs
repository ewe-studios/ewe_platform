//! WHY: Layers 1 and 2 are deliberately separate — `foundation_ui_traits` encodes
//! `DomOp`s to bytes with no WASM knowledge, and `foundation_wasm` ships bytes across
//! the boundary with no encoding knowledge. Something has to compose them into a
//! single "encode these ops and ship them" call. That is Layer 3.
//!
//! WHAT: The [`ProtocolMethods`] trait plus the three concrete protocol
//! implementations — [`ArrowV1`], [`BatchInstructionsV1`] (Custom Binary, byte 0 —
//! the foundation_wasm Instructions format per decision 022), [`JsonV1`] — and the
//! uniform `host_apply` FFI they all ship through (decision 028/030).
//!
//! HOW: Each impl pairs a Layer-1 encoder with the Layer-2 [`ProtocolHandler`]
//! transport. `encode_and_send` encodes the ops, frames them in a 14-byte
//! [`WasmEnvelope`] inside one arena slot, and ships `(memory_id, ptr, len)` via the
//! single uniform `host_apply` import — so JS reads the protocol byte from the buffer
//! and `dispose_allocation`s the slot by `memory_id`.

mod arrow;
mod batch_instructions;
mod json;

pub use arrow::ArrowV1;
pub use batch_instructions::{BatchInstructionsV1, DomOpsBatch, BATCH_OP_APPLY_DOM};
pub use json::JsonV1;

use alloc::vec::Vec;

use foundation_ui_traits::{DecodeError, DomOp, ProtocolEncoder};
// `host_apply` is the generic Layer-2 transport import — it lives in the ABI crate
// (`foundation_wasm::abi::web`), not here, because it ships any protocol's bytes
// regardless of DOM. Layer 3 only composes encoders with it.
use foundation_wasm::abi::web::host_apply;
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler, WasmEnvelope};

// ─── Uniform host transport (decision 028 — one `host_apply` for all protocols) ─

/// Ship an already-written arena slot to JS via the uniform `host_apply` FFI.
///
/// `host_apply` is an unsafe `extern` import on wasm targets but a safe stub on
/// native ones, so the `unsafe` block is unused off-wasm — hence the allow.
#[allow(unused_unsafe)]
pub(crate) fn ship(memory_id: MemoryId, ptr: *const u8, len: usize) {
    unsafe { host_apply(memory_id.as_u64(), ptr as u64, len as u64) }
}

// ─── Send / handle results ─────────────────────────────────────────────────────

/// Outcome of [`ProtocolMethods::encode_and_send`] — the arena slot that JS must
/// `dispose_allocation` after applying the batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendResult {
    /// The arena slot holding the shipped message.
    pub memory_id: MemoryId,
}

/// Outcome of [`ProtocolMethods::handle_received`] — the decoded `DomOp` batch or a
/// decode error.
pub type HandleResult = Result<Vec<DomOp>, DecodeError>;

// ─── ProtocolMethods trait (Layer 3) ───────────────────────────────────────────

/// WHY: `InstructionReceiver` (and any caller) needs one object that both encodes a
/// `DomOp` batch and ships it, without caring which wire format is configured.
///
/// WHAT: Extends the Layer-2 [`ProtocolHandler`] transport with encode-and-ship and
/// receive-and-decode operations over a payload type `T`.
///
/// HOW: `encode_and_send` composes a Layer-1 encoder with the Layer-2 transport
/// through one arena slot; `handle_received` decodes an incoming payload. Slot
/// release (`ack`) comes from the [`ProtocolHandler`] supertrait — it is part of
/// the transport contract, shared by every handler.
pub trait ProtocolMethods<T>: ProtocolHandler {
    /// Encode `data`, frame it in one arena slot, and ship it to JS.
    fn encode_and_send(&self, data: T, memory: &mut MemoryAllocations) -> SendResult;

    /// Decode an incoming payload (envelope already stripped) at `(ptr, len)`.
    ///
    /// `ptr`/`len` must describe a valid readable region for this call.
    ///
    /// # Errors
    /// Returns [`DecodeError`] if the bytes are malformed for this protocol.
    fn handle_received(&self, memory_id: MemoryId, ptr: *const u8, len: usize) -> HandleResult;
}

// ─── Shared composition helpers ────────────────────────────────────────────────

/// Encode `ops`, allocate one arena slot, frame the payload in a [`WasmEnvelope`]
/// (so JS can read the protocol byte + `memory_id`), and ship it.
///
/// Shared by every protocol impl — the only thing that varies is the `encoder`.
///
/// # Panics
/// Panics if the arena cannot allocate or address the slot — an allocation failure
/// at this point means a fundamental arena leak (decision 028, Error Cases).
pub(crate) fn encode_and_ship<H, E>(
    handler: &H,
    encoder: &E,
    ops: Vec<DomOp>,
    memory: &mut MemoryAllocations,
) -> SendResult
where
    H: ProtocolHandler,
    E: ProtocolEncoder<Vec<DomOp>>,
{
    let payload = encoder.encode(ops);
    ship_payload(handler, &payload, memory)
}

/// Frame an already-encoded `payload` in a [`WasmEnvelope`] inside one arena slot
/// and ship it — the tail every protocol shares, whether the payload came from a
/// Layer-1 encoder (Arrow/JSON) or an Instructions batch (Custom Binary).
///
/// # Panics
/// Panics if the arena cannot allocate or address the slot — an allocation failure
/// at this point means a fundamental arena leak (decision 028, Error Cases).
pub(crate) fn ship_payload<H>(
    handler: &H,
    payload: &[u8],
    memory: &mut MemoryAllocations,
) -> SendResult
where
    H: ProtocolHandler,
{
    let total = WasmEnvelope::HEADER_LEN + payload.len();

    // Allocate first so the envelope can carry the real memory_id, then fill the slot.
    let mem_id = memory.allocate(total as u64).expect("arena allocate failed");
    let framed = WasmEnvelope::write(
        handler.protocol_byte(),
        handler.version(),
        mem_id.as_u64(),
        payload,
    );
    let slot = memory.get(mem_id).expect("arena get failed");
    slot.apply(|mem| {
        mem.clear();
        mem.extend_from_slice(&framed);
    });

    let (ptr, len) = slot.as_address().expect("arena address failed");
    // The slot length came from a `usize` (`total`), so this never truncates; use a
    // checked conversion so a 32-bit target can't silently lose the high bits.
    let len = usize::try_from(len).expect("arena slot length exceeds usize::MAX");
    handler.send_to_js(mem_id, ptr, len);
    SendResult { memory_id: mem_id }
}

/// Decode a payload slice at `(ptr, len)` with `encoder`.
pub(crate) fn decode_payload<E>(encoder: &E, ptr: *const u8, len: usize) -> HandleResult
where
    E: ProtocolEncoder<Vec<DomOp>>,
{
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    encoder.decode(slice)
}
