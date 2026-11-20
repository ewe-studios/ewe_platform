//! Deliver one complete user interface frame or send a general instruction batch.
mod batch_instructions;
mod frame;
mod mock;
pub use batch_instructions::BatchInstructionsV1;
pub use frame::{
    decode_frame, FrameOutput, FrameV1, HandleResult, ProtocolError, ProtocolMethods, WrittenBatch,
    PROTOCOL_FRAME,
};
pub use mock::MockProtocol;

use foundation_wasm::abi::web::host_apply;
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler, WasmEnvelope};
/// Frames collected by a server application before transport.
pub type CollectedFrames =
    alloc::rc::Rc<core::cell::RefCell<alloc::collections::VecDeque<alloc::vec::Vec<u8>>>>;
/// Ship an already-written arena slot to JS via the uniform `host_apply` FFI.
///
/// `host_apply` is an unsafe `extern` import on wasm targets but a safe stub on
/// native ones, so the `unsafe` block is unused off-wasm — hence the allow.
#[allow(unused_unsafe)]
pub(crate) fn ship(memory_id: MemoryId, ptr: *const u8, len: usize) {
    unsafe { host_apply(memory_id.as_u64(), ptr as u64, len as u64) }
}

// ─── Send / handle results ─────────────────────────────────────────────────────

/// The number of operations and bytes that a flush writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendResult {
    /// The arena slot holding the shipped message.
    pub memory_id: MemoryId,
    /// Number of user interface operations in the message.
    pub op_count: usize,
    /// Payload byte size EXCLUDING the envelope header.
    pub encoded_bytes: usize,
}

/// Frame an already-encoded `payload` in a [`WasmEnvelope`] inside one arena slot
/// and return the slot + its live address — WITHOUT shipping. The caller sends via
/// `send_to_js` after it releases the arena lock.
///
/// # Panics
/// Panics if the arena cannot allocate or address the slot — an allocation failure
/// at this point means a fundamental arena leak (decision 028, Error Cases).
pub(crate) fn write_framed<H>(
    handler: &H,
    payload: &[u8],
    memory: &mut MemoryAllocations,
    op_count: usize,
) -> (SendResult, *const u8, usize)
where
    H: ProtocolHandler,
{
    let total = WasmEnvelope::HEADER_LEN + payload.len();

    // Allocate first so the envelope can carry the real memory_id, then fill the slot.
    let mem_id = memory
        .allocate(total as u64)
        .expect("arena allocate failed");
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
    (
        SendResult {
            memory_id: mem_id,
            op_count,
            encoded_bytes: payload.len(),
        },
        ptr,
        len,
    )
}
