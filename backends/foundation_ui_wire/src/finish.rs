use alloc::vec::Vec;

use flatbuffers::FlatBufferBuilder;

use crate::{
    Frame, FrameArgs, FrameKind, FrameKindT, FrameT, SignedStateT, WireError, FRAME_IDENTIFIER,
    STATE_IDENTIFIER,
};

mod aligned;

/// Packs an owned frame with its wire identifier and aligns the marked vectors.
///
/// # Errors
///
/// Returns a wire error if the frame has no kind.
pub fn finish_frame(frame: &FrameT) -> Result<Vec<u8>, WireError> {
    let mut builder = FlatBufferBuilder::new();
    let offset = match &frame.kind {
        FrameKindT::NONE => return Err(WireError::NoKind),
        FrameKindT::FrameRequest(_) => frame.pack(&mut builder),
        FrameKindT::FrameMessage(message) => {
            let kind = aligned::pack_message_frame(&mut builder, message);
            Frame::create(
                &mut builder,
                &FrameArgs {
                    schema_version: frame.schema_version,
                    stream_id: frame.stream_id,
                    sequence_id: frame.sequence_id,
                    kind_type: FrameKind::FrameMessage,
                    kind: Some(kind.as_union_value()),
                },
            )
        }
    };
    builder.finish(offset, Some(FRAME_IDENTIFIER));
    Ok(builder.finished_data().to_vec())
}

/// Packs a state that uses the optional `EWS1` envelope.
#[must_use]
pub fn finish_signed_state(state: &SignedStateT) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let offset = state.pack(&mut builder);
    builder.finish(offset, Some(STATE_IDENTIFIER));
    builder.finished_data().to_vec()
}
