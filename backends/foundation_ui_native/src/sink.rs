//! Deliver complete generated operation batches after the instruction receiver releases its arena.
use crate::{core::Push, NativeCore};
use foundation_ui_wire::OperationT;
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};
use foundation_wasm_ui::protocol::{
    decode_frame, HandleResult, ProtocolError, ProtocolMethods, SendResult, WrittenBatch,
    PROTOCOL_FRAME,
};
use std::{cell::RefCell, collections::BTreeMap, sync::Arc};

pub struct CoreSink {
    core: Arc<NativeCore>,
    pending: RefCell<BTreeMap<u64, Push>>,
}

impl CoreSink {
    #[must_use]
    pub fn new(core: Arc<NativeCore>) -> Self {
        Self {
            core,
            pending: RefCell::default(),
        }
    }
}

impl ProtocolHandler for CoreSink {
    fn version(&self) -> u8 {
        1
    }
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_FRAME
    }
    fn send_to_js(&self, id: MemoryId, _ptr: *const u8, _len: usize) {
        let delivery = self.pending.borrow_mut().remove(&id.as_u64());
        if let Some(push) = delivery {
            self.core.publish(push);
        }
    }
    fn ack(&self, id: MemoryId, _arena: &mut MemoryAllocations) {
        self.pending.borrow_mut().remove(&id.as_u64());
    }
    fn handle_from_js(&self, _id: MemoryId, _ptr: *const u8, _len: usize) {}
}
impl ProtocolMethods<Vec<OperationT>> for CoreSink {
    fn encode_and_write(
        &self,
        ops: Vec<OperationT>,
        _arena: &mut MemoryAllocations,
    ) -> WrittenBatch {
        let op_count = ops.len();
        let push = self
            .core
            .prepare_push(ops)
            .map_err(|e| ProtocolError::Delivery(e.to_string()))?;
        let len = push.bytes.len();
        let ptr = push.bytes.as_ptr();
        let receipt = SendResult {
            memory_id: MemoryId::from_u64(push.ticket),
            op_count,
            encoded_bytes: len,
        };
        self.pending.borrow_mut().insert(push.ticket, push);
        Ok((receipt, ptr, len))
    }
    fn handle_received(&self, bytes: &[u8]) -> HandleResult {
        decode_frame(bytes, self.core.limits())
    }
}
