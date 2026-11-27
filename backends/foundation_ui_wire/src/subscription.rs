//! Encode the messages that resume a logical subscription.
use crate::{
    finish_frame, read_frame, ControlT, FrameKindT, FrameMessageT, FrameRequestT, FrameT, MessageT,
    ReplayExpiredT, ResumeSubscriptionT, WireError, WireLimits, SCHEMA_VERSION,
};
use alloc::{boxed::Box, vec, vec::Vec};
use core::num::NonZeroU64;

/// Encodes a resume request for an existing delivery session.
///
/// # Errors
/// Returns the frame encoding error.
pub fn resume_subscription(
    session: NonZeroU64,
    subscription: NonZeroU64,
    stream: NonZeroU64,
    after: u64,
) -> Result<Vec<u8>, WireError> {
    let value = ResumeSubscriptionT {
        delivery_session: session.get(),
        subscription_id: subscription.get(),
        stream_id: stream.get(),
        after,
    };
    let request = FrameRequestT {
        resume: Some(Box::new(value)),
        ..Default::default()
    };
    finish_frame(&FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: session.get(),
        kind: FrameKindT::FrameRequest(Box::new(request)),
        ..Default::default()
    })
}

/// Returns a replay frame for one subscriber. Keeps the stream sequence and operation order.
///
/// # Errors
/// Rejects malformed frames, action replies, and frames above the configured byte limit.
pub fn tag_replay(bytes: &[u8], id: NonZeroU64, limits: &WireLimits) -> Result<Vec<u8>, WireError> {
    let mut frame = read_frame(bytes, limits)?.unpack();
    let FrameKindT::FrameMessage(ref mut body) = frame.kind else {
        return Err(WireError::Invalid {
            detail: "Expected a pushed message".into(),
        });
    };
    if body.ack != 0 || body.subscription_id != 0 {
        return Err(WireError::Invalid {
            detail: "Expected an untagged stream update".into(),
        });
    }
    body.subscription_id = id.get();
    let tagged = finish_frame(&frame)?;
    if tagged.len() > limits.frame_bytes() {
        return Err(WireError::TooLarge {
            limit: limits.frame_bytes(),
            actual: tagged.len(),
        });
    }
    Ok(tagged)
}

/// Tells a subscriber to request a new snapshot.
///
/// # Errors
/// Returns the frame encoding error.
pub fn replay_expired(
    id: NonZeroU64,
    stream: NonZeroU64,
    after: u64,
) -> Result<Vec<u8>, WireError> {
    let expired = ReplayExpiredT {
        stream_id: stream.get(),
        after,
    };
    let control = ControlT {
        replay_expired: Some(Box::new(expired)),
        ..Default::default()
    };
    let message = MessageT {
        control: Some(Box::new(control)),
        ..Default::default()
    };
    let body = FrameMessageT {
        messages: Some(vec![message]),
        subscription_id: id.get(),
        ..Default::default()
    };
    finish_frame(&FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: stream.get(),
        kind: FrameKindT::FrameMessage(Box::new(body)),
        ..Default::default()
    })
}
