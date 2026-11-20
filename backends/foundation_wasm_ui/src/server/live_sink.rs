//! Publish one complete application update to a live channel.
use super::{LiveError, LiveTx};
use crate::protocol::{
    HandleResult, ProtocolError, ProtocolMethods, SendResult, WrittenBatch, PROTOCOL_FRAME,
};
use foundation_ui_wire::{ops, OperationT};
use foundation_wasm::{MemoryAllocations, MemoryId, ProtocolHandler};
use std::prelude::rust_2021::*;

/// A protocol sink that delivers operations directly to the server's live channel.
pub struct FrameBroadcastSink(LiveTx);

impl FrameBroadcastSink {
    #[must_use]
    pub fn new(sender: LiveTx) -> Self {
        Self(sender)
    }
}

impl ProtocolHandler for FrameBroadcastSink {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_FRAME
    }
    fn version(&self) -> u8 {
        1
    }
    fn send_to_js(&self, _: MemoryId, _: *const u8, _: usize) {}
    fn handle_from_js(&self, _: MemoryId, _: *const u8, _: usize) {}
    fn ack(&self, _: MemoryId, _: &mut MemoryAllocations) {}
}

impl ProtocolMethods<Vec<OperationT>> for FrameBroadcastSink {
    fn encode_and_write(&self, ops: Vec<OperationT>, _: &mut MemoryAllocations) -> WrittenBatch {
        let op_count = ops.len();
        let (sequence, encoded_bytes) = self
            .0
            .publish(ops.into_iter().map(ops::message).collect())
            .map_err(|e| match e {
                LiveError::Write(source) => ProtocolError::Writer(source),
                LiveError::SequenceExhausted => ProtocolError::SequenceExhausted,
                error => ProtocolError::Delivery(error.to_string()),
            })?;
        Ok((
            SendResult {
                memory_id: MemoryId::from(sequence),
                op_count,
                encoded_bytes,
            },
            core::ptr::null(),
            0,
        ))
    }
    fn handle_received(&self, _: &[u8]) -> HandleResult {
        Ok(Vec::new())
    }
}
