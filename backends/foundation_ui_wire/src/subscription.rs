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
    let mut value = ResumeSubscriptionT::default();
    value.delivery_session = session.get();
    value.subscription_id = subscription.get();
    value.stream_id = stream.get();
    value.after = after;
    let mut request = FrameRequestT::default();
    request.resume = Some(Box::new(value));
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = session.get();
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    finish_frame(&frame)
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
    let mut expired = ReplayExpiredT::default();
    expired.stream_id = stream.get();
    expired.after = after;
    let mut control = ControlT::default();
    control.replay_expired = Some(Box::new(expired));
    let mut message = MessageT::default();
    message.control = Some(Box::new(control));
    let mut body = FrameMessageT::default();
    body.messages = Some(vec![message]);
    body.subscription_id = id.get();
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = stream.get();
    frame.kind = FrameKindT::FrameMessage(Box::new(body));
    finish_frame(&frame)
}
