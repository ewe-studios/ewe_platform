//! Send general WebAssembly instruction batches through the host interface.
use super::{ship, write_framed, SendResult};
use alloc::vec::Vec;
use foundation_wasm::{BatchMessage, MemoryAllocations, MemoryId, ProtocolHandler};
#[derive(Clone, Copy, Debug, Default)]
pub struct BatchInstructionsV1;

impl BatchInstructionsV1 {
    /// Construct the byte-0 protocol handler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Build an [`Instructions`](foundation_wasm::Instructions) batch from `message` and pack it into ONE envelope
    /// slot (`[texts_off:u32][texts_len:u32][ops][texts]`) WITHOUT shipping —
    /// the write stage for any
    /// [`BatchMessage`] type.
    ///
    /// # Panics
    /// Panics if the arena cannot allocate or address a slot — an allocation
    /// failure here means a fundamental arena leak (decision 028, Error Cases).
    pub fn write_message<T: BatchMessage>(
        &self,
        message: &T,
        memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        let batch = memory
            .batch_for(64, 64, true)
            .expect("arena batch_for failed");
        message.encode_batch(&batch).expect("batch encode failed");
        let completed = batch.complete().expect("batch complete failed");

        let ops_bytes = memory
            .get(completed.ops_id)
            .expect("arena get ops failed")
            .clone_memory()
            .expect("read ops bytes");
        let text_bytes = memory
            .get(completed.text_id)
            .expect("arena get texts failed")
            .clone_memory()
            .expect("read text bytes");
        memory.deallocate(completed.ops_id).expect("release completed instruction slot");
        memory.deallocate(completed.text_id).expect("release completed text slot");

        let texts_off = 8 + ops_bytes.len();
        let mut payload = Vec::with_capacity(texts_off + text_bytes.len());
        payload.extend_from_slice(
            &u32::try_from(texts_off)
                .expect("ops too large")
                .to_le_bytes(),
        );
        payload.extend_from_slice(
            &u32::try_from(text_bytes.len())
                .expect("texts too large")
                .to_le_bytes(),
        );
        payload.extend_from_slice(&ops_bytes);
        payload.extend_from_slice(&text_bytes);

        // General messages do not carry a user interface operation count.
        write_framed(self, &payload, memory, 0)
    }

    /// One-call build + ship for OWNED arenas. Do NOT call while holding the
    /// global arena lock — `host_apply` re-enters WASM (JS ACK path); use
    /// [`write_message`](Self::write_message) under the lock and `send_to_js`
    /// outside it instead.
    pub fn send_message<T: BatchMessage>(
        &self,
        message: &T,
        memory: &mut MemoryAllocations,
    ) -> SendResult {
        let (result, ptr, len) = self.write_message(message, memory);
        self.send_to_js(result.memory_id, ptr, len);
        result
    }
}

impl ProtocolHandler for BatchInstructionsV1 {
    fn protocol_byte(&self) -> u8 {
        0
    }

    fn version(&self) -> u8 {
        1
    }

    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize) {
        ship(memory_id, ptr, len);
    }

    fn handle_from_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}
}
