//! WHY: Receiver/runtime tests need to assert WHAT was flushed and WHEN —
//! without encoding bytes, arena slots, or an FFI host (feature 04 section 7).
//!
//! WHAT: [`MockProtocol`] — a [`ProtocolMethods`] impl that records every
//! batch and every ACK, using the reserved test protocol byte `255`.
//!
//! HOW: The recorders are `Rc<RefCell<...>>` handles; tests clone them BEFORE
//! boxing the mock into a receiver, so assertions read the shared records
//! after the receiver has consumed the mock.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use foundation_ui_traits::DomOp;
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};

use super::{HandleResult, ProtocolMethods, SendResult};

/// Recording protocol for tests (protocol byte `255` — reserved).
#[derive(Default)]
pub struct MockProtocol {
    sent_batches: Rc<RefCell<Vec<Vec<DomOp>>>>,
    acked_ids: Rc<RefCell<Vec<MemoryId>>>,
    next_id: Cell<u32>,
}

impl MockProtocol {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Shared handle to every batch sent, in order. Clone this BEFORE boxing
    /// the mock into a receiver.
    #[must_use]
    pub fn sent_batches(&self) -> Rc<RefCell<Vec<Vec<DomOp>>>> {
        Rc::clone(&self.sent_batches)
    }

    /// Shared handle to every `ACK`ed `MemoryId`, in order.
    #[must_use]
    pub fn acked_ids(&self) -> Rc<RefCell<Vec<MemoryId>>> {
        Rc::clone(&self.acked_ids)
    }
}

impl ProtocolHandler for MockProtocol {
    fn protocol_byte(&self) -> u8 {
        255 // reserved for testing
    }

    fn version(&self) -> u8 {
        0
    }

    fn send_to_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}

    fn handle_from_js(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) {}

    fn ack(&self, memory_id: MemoryId, _memory: &mut MemoryAllocations) {
        self.acked_ids.borrow_mut().push(memory_id);
    }
}

impl ProtocolMethods<Vec<DomOp>> for MockProtocol {
    fn encode_and_write(
        &self,
        ops: Vec<DomOp>,
        _memory: &mut MemoryAllocations,
    ) -> (SendResult, *const u8, usize) {
        let op_count = ops.len();
        self.sent_batches.borrow_mut().push(ops);
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        (
            SendResult {
                memory_id: MemoryId::new(id, 0),
                op_count,
                encoded_bytes: 0, // nothing actually encoded
            },
            core::ptr::null(),
            0,
        )
    }

    fn handle_received(&self, _memory_id: MemoryId, _ptr: *const u8, _len: usize) -> HandleResult {
        Ok(Vec::new())
    }
}
