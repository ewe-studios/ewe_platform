use alloc::string::ToString;

use flatbuffers::{Follow, InvalidFlatbuffer, Verifiable, Verifier, VerifierOptions};

use crate::{
    Frame, FrameKind, FrameKindName, FrameKindT, FrameT, SignedState, WireError, WireLimits,
};
use crate::{FRAME_IDENTIFIER, STATE_IDENTIFIER};

/// Verifies a frame and returns a borrowed view.
///
/// # Errors
///
/// Returns a wire error if the frame exceeds a limit or fails verification.
pub fn read_frame<'a>(bytes: &'a [u8], limits: &WireLimits) -> Result<Frame<'a>, WireError> {
    let frame = read_root::<Frame<'_>>(bytes, limits, limits.frame_bytes(), FRAME_IDENTIFIER)?;
    if frame.kind_type() == FrameKind::NONE {
        return Err(WireError::NoKind);
    }
    if frame
        .kind_as_frame_request()
        .is_some_and(|request| request.action().is_some())
    {
        check_size(bytes.len(), limits.action_request_bytes())?;
    }
    Ok(frame)
}

/// Verifies the structure of an `EWS1` envelope with the action request size limit.
///
/// # Errors
///
/// Returns a wire error if the state exceeds a limit or fails verification.
pub fn read_signed_state<'a>(
    bytes: &'a [u8],
    limits: &WireLimits,
) -> Result<SignedState<'a>, WireError> {
    read_root::<SignedState<'_>>(
        bytes,
        limits,
        limits.action_request_bytes(),
        STATE_IDENTIFIER,
    )
}

/// Returns the named kind of an owned frame.
#[must_use]
pub fn frame_kind(frame: &FrameT) -> Option<FrameKindName> {
    match frame.kind {
        FrameKindT::NONE => None,
        FrameKindT::FrameRequest(_) => Some(FrameKindName::Request),
        FrameKindT::FrameMessage(_) => Some(FrameKindName::Message),
    }
}

/// Checks that an owned frame has the kind the caller expects.
///
/// # Errors
///
/// Returns a wire error if the frame has no kind or holds a different kind.
pub fn expect_kind(frame: &FrameT, expected: FrameKindName) -> Result<(), WireError> {
    let found = frame_kind(frame).ok_or(WireError::NoKind)?;
    if expected != found {
        return Err(WireError::WrongKind { expected, found });
    }
    Ok(())
}

fn check_size(actual: usize, limit: usize) -> Result<(), WireError> {
    if actual > limit {
        return Err(WireError::TooLarge { limit, actual });
    }
    Ok(())
}

fn read_root<'a, T: Follow<'a> + Verifiable + 'a>(
    bytes: &'a [u8],
    limits: &WireLimits,
    size_limit: usize,
    identifier: &'static str,
) -> Result<T::Inner, WireError> {
    check_size(bytes.len(), size_limit)?;
    let Some(&[_, _, _, _, first, second, third, fourth, ..]) = bytes.get(..8) else {
        return Err(WireError::TooShort {
            actual: bytes.len(),
        });
    };
    let found = [first, second, third, fourth];
    if found != identifier.as_bytes() {
        return Err(WireError::WrongIdentifier {
            expected: identifier,
            found,
        });
    }
    let options = VerifierOptions {
        max_depth: limits.depth(),
        max_tables: limits.tables(),
        ..VerifierOptions::default()
    };
    flatbuffers::root_with_opts::<T>(&options, bytes).map_err(|error| WireError::Invalid {
        detail: error.to_string(),
    })
}

/// Checks an unknown union's offset and table header without interpreting the payload.
pub(crate) fn verify_opaque_union(
    verifier: &mut Verifier<'_, '_>,
    position: usize,
) -> Result<(), InvalidFlatbuffer> {
    let offset = verifier.get_uoffset(position)? as usize;
    if offset == 0 {
        return InvalidFlatbuffer::new_missing_required("opaque union table offset");
    }
    verifier
        .visit_table(position.saturating_add(offset))?
        .finish();
    Ok(())
}
