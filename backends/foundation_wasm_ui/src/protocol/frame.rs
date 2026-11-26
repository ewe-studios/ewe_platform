//! Send generated operations as complete frames.

use super::{ship, SendResult};
use alloc::{string::String, vec::Vec};
use core::{cell::RefCell, fmt};
use foundation_ui_wire::{
    ops, read_frame,
    writer::{write_messages, ClientCaps, FrameHeader, WriteError, WriterConfig},
    MessageT, OperationT, WireError, WireLimits,
};
use foundation_wasm::{MemoryAllocationError, MemoryAllocations, MemoryId, ProtocolHandler};

pub const PROTOCOL_FRAME: u8 = 3;

/// Errors raised before a batch reaches its destination.
#[derive(Debug)]
pub enum ProtocolError {
    Writer(WriteError),
    Wire(WireError),
    Allocation(MemoryAllocationError),
    UnsupportedOperation,
    SequenceExhausted,
    Delivery(String),
}

impl From<WriteError> for ProtocolError {
    fn from(error: WriteError) -> Self {
        Self::Writer(error)
    }
}
impl From<WireError> for ProtocolError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}
impl From<MemoryAllocationError> for ProtocolError {
    fn from(error: MemoryAllocationError) -> Self {
        Self::Allocation(error)
    }
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Writer(e) => e.fmt(f),
            Self::Wire(e) => e.fmt(f),
            Self::Allocation(e) => e.fmt(f),
            Self::UnsupportedOperation => {
                f.write_str("The selected protocol cannot carry this operation.")
            }
            Self::SequenceExhausted => f.write_str("The stream used its last sequence number."),
            Self::Delivery(detail) => f.write_str(detail),
        }
    }
}
impl core::error::Error for ProtocolError {}

pub type HandleResult = Result<Vec<OperationT>, ProtocolError>;
/// The slot, start address and byte count for one complete batch.
pub type WrittenBatch = Result<(SendResult, *const u8, usize), ProtocolError>;

/// Encode generated operations for a host. Send only after the arena lock is released.
pub trait ProtocolMethods<T>: ProtocolHandler {
    /// # Errors
    /// Returns a validation or allocation error before sending any bytes.
    fn encode_and_write(&self, data: T, memory: &mut MemoryAllocations) -> WrittenBatch;

    /// # Errors
    /// Returns the encoding error from the selected protocol.
    fn encode_and_send(
        &self,
        data: T,
        memory: &mut MemoryAllocations,
    ) -> Result<SendResult, ProtocolError> {
        let (result, ptr, len) = self.encode_and_write(data, memory)?;
        self.send_to_js(result.memory_id, ptr, len);
        Ok(result)
    }

    /// # Errors
    /// Returns an error if the payload cannot be decoded as operations.
    fn handle_received(&self, bytes: &[u8]) -> HandleResult;
}

/// Decode one operation-only frame with the supplied limits.
///
/// # Errors
/// Rejects malformed frames, requests, and control or data messages.
pub fn decode_frame(bytes: &[u8], limits: &WireLimits) -> HandleResult {
    let frame = read_frame(bytes, limits)?;
    let body = frame
        .kind_as_frame_message()
        .ok_or(ProtocolError::UnsupportedOperation)?;
    body.messages()
        .into_iter()
        .flatten()
        .map(|message| {
            if message.control().is_some() || message.data().is_some() {
                return Err(ProtocolError::UnsupportedOperation);
            }
            message
                .operation()
                .map(|op| op.unpack())
                .ok_or(ProtocolError::UnsupportedOperation)
        })
        .collect()
}

/// Write one bare frame per flush. Successful writes advance this stream's sequence.
pub struct FrameV1 {
    stream: u64,
    sequence: RefCell<u64>,
    caps: ClientCaps,
    config: WriterConfig,
}

impl FrameV1 {
    #[must_use]
    pub fn new(stream_id: u64) -> Self {
        Self::with_config(
            stream_id,
            ClientCaps::web_current(),
            WriterConfig::default(),
        )
    }
    #[must_use]
    pub fn with_config(stream: u64, caps: ClientCaps, config: WriterConfig) -> Self {
        Self {
            stream,
            sequence: RefCell::new(0),
            caps,
            config,
        }
    }

    /// Encode a batch and hand it off. Advance the sequence only on success.
    ///
    /// # Errors
    /// Returns validation, allocation or delivery errors before advancing the sequence.
    pub fn deliver<R>(
        &self,
        operations: Vec<OperationT>,
        ack: u64,
        send: impl FnOnce(Vec<u8>) -> Result<R, ProtocolError>,
    ) -> Result<R, ProtocolError> {
        let mut sequence = self.sequence.try_borrow_mut().map_err(|_| {
            ProtocolError::Delivery("Frame delivery is already in progress.".into())
        })?;
        let next = sequence
            .checked_add(1)
            .ok_or(ProtocolError::SequenceExhausted)?;
        let header = FrameHeader {
            stream_id: self.stream,
            sequence_id: next,
            ack,
        };
        let messages: Vec<MessageT> = operations.into_iter().map(ops::message).collect();
        let (bytes, _report) = write_messages(messages, &self.caps, header, &self.config)?;
        let result = send(bytes)?;
        *sequence = next;
        Ok(result)
    }
}

impl ProtocolHandler for FrameV1 {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_FRAME
    }
    fn version(&self) -> u8 {
        1
    }
    fn send_to_js(&self, id: MemoryId, ptr: *const u8, len: usize) {
        ship(id, ptr, len);
    }
    fn handle_from_js(&self, _: MemoryId, _: *const u8, _: usize) {}
}

impl ProtocolMethods<Vec<OperationT>> for FrameV1 {
    fn encode_and_write(
        &self,
        ops: Vec<OperationT>,
        memory: &mut MemoryAllocations,
    ) -> WrittenBatch {
        let op_count = ops.len();
        self.deliver(ops, 0, |bytes| {
            let len = bytes.len();
            let id = memory.allocate(0)?;
            let result = (|| {
                let slot = memory.get(id)?;
                let ptr = slot
                    .into_with(|buf| {
                        *buf = bytes;
                        buf.as_ptr()
                    })
                    .ok_or(MemoryAllocationError::NoMemoryAllocation)?;
                Ok((
                    SendResult {
                        memory_id: id,
                        op_count,
                        encoded_bytes: len,
                    },
                    ptr,
                    len,
                ))
            })();
            if result.is_err() {
                memory.deallocate(id)?;
            }
            result
        })
    }
    fn handle_received(&self, bytes: &[u8]) -> HandleResult {
        decode_frame(bytes, self.config.wire_limits())
    }
}

/// Send one bare frame to an application callback, without allocating a browser slot.
pub struct FrameOutput<F> {
    frame: FrameV1,
    send: F,
}

impl<F: Fn(Vec<u8>) -> Result<(), ProtocolError>> FrameOutput<F> {
    #[must_use]
    pub fn new(frame: FrameV1, send: F) -> Self {
        Self { frame, send }
    }
}

impl<F> ProtocolHandler for FrameOutput<F> {
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

impl<F: Fn(Vec<u8>) -> Result<(), ProtocolError>> ProtocolMethods<Vec<OperationT>>
    for FrameOutput<F>
{
    fn encode_and_write(&self, ops: Vec<OperationT>, _: &mut MemoryAllocations) -> WrittenBatch {
        let op_count = ops.len();
        self.frame.deliver(ops, 0, |bytes| {
            let encoded_bytes = bytes.len();
            (self.send)(bytes)?;
            Ok((
                SendResult {
                    memory_id: MemoryId::new(0, 0),
                    op_count,
                    encoded_bytes,
                },
                core::ptr::null(),
                0,
            ))
        })
    }
    fn handle_received(&self, bytes: &[u8]) -> HandleResult {
        self.frame.handle_received(bytes)
    }
}
