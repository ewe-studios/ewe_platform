//! Validate and encode frames for one client.

mod columnar;
mod config;
mod error;
mod gate;
mod messages;
mod sequencer;
mod table;

use alloc::{boxed::Box, vec::Vec};

use crate::{finish_frame, FrameKindT, FrameMessageT, FrameT, MessageT, SCHEMA_VERSION};

#[cfg(feature = "reference")]
pub(crate) use columnar::decode_batch;
pub use columnar::{columnar_batch, columnar_rows, ColumnarError};
pub use config::{ClientCaps, WriterConfig};
pub use error::WriteError;
pub use gate::gate;
pub use messages::{write_arrow_ipc, write_caps, write_error, write_messagepack, write_messages};
pub use sequencer::{FrameHeader, StreamSequencer};
pub use table::{table_frames, TableFrames};

/// Identifies the part of a message that the gate removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilteredPart {
    Message,
    Web,
    Mobile,
    NestedOperation,
}

/// Gives the reason that the gate removed a message or field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterReason {
    OtherPlatform,
    UnknownVocabulary,
}

/// Locates a removal in the input message list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilteredMessage {
    pub message_index: usize,
    pub part: FilteredPart,
    pub reason: FilterReason,
}

/// Records removals and fallback node kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteReport {
    pub filtered: Vec<FilteredMessage>,
    /// Input indices of kept messages with a fallback kind, in increasing order.
    pub replaced_kinds: Vec<usize>,
}

fn finish_message_frame(
    messages: Vec<MessageT>,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    let frame = FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: header.stream_id,
        sequence_id: header.sequence_id,
        kind: FrameKindT::FrameMessage(Box::new(FrameMessageT {
            ack: header.ack,
            subscription_id: 0,
            messages: Some(messages),
        })),
    };
    let bytes = finish_frame(&frame)?;
    let limit = config.wire_limits().frame_bytes();
    if bytes.len() > limit {
        return Err(WriteError::TooLarge {
            limit,
            actual: bytes.len(),
        });
    }
    Ok(bytes)
}
