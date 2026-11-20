use alloc::vec::Vec;

use flatbuffers::FlatBufferBuilder;

use crate::{FrameKindT, FrameT, SignedStateT, WireError, FRAME_IDENTIFIER, STATE_IDENTIFIER};

/// Packs an owned frame with its wire identifier.
///
/// # Errors
///
/// Returns a wire error if the frame has no kind.
pub fn finish_frame(frame: &FrameT) -> Result<Vec<u8>, WireError> {
    if matches!(frame.kind, FrameKindT::NONE) {
        return Err(WireError::NoKind);
    }
    let mut builder = FlatBufferBuilder::new();
    let offset = frame.pack(&mut builder);
    builder.finish(offset, Some(FRAME_IDENTIFIER));
    Ok(builder.finished_data().to_vec())
}

/// Packs an owned signed state with its wire identifier.
#[must_use]
pub fn finish_signed_state(state: &SignedStateT) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let offset = state.pack(&mut builder);
    builder.finish(offset, Some(STATE_IDENTIFIER));
    builder.finished_data().to_vec()
}
