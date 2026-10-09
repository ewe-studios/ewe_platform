//! The `TryClone` contract.
//!
//! WHY: `TryClone` exists because `Clone` is all-or-nothing — one non-clonable
//! variant disqualifies the whole type, pushing the "which states may be
//! duplicated?" decision out to every call site. The contract that makes it
//! useful is that a refusal is *reported* rather than silently degraded, and
//! that cloning does not consume the source: a reconnect loop replays the same
//! body on every attempt, so a `take`-style implementation would work once and
//! then quietly send nothing.
//!
//! WHAT: the trait is implementable by a downstream type, both outcomes are
//! observable, repeated clones are stable, and `TryCloneError` renders a message
//! that names what refused.
//!
//! HOW: a local type standing in for the real shape (owned data plus a
//! single-shot handle). No I/O.

use foundation_core::traits::{TryClone, TryCloneError};

/// Mirrors the real shape: some states own their data, one wraps a stream that
/// can only be read once.
#[derive(Debug, PartialEq, Eq)]
enum Payload {
    Empty,
    Owned(Vec<u8>),
    Streaming,
}

impl TryClone for Payload {
    type Error = TryCloneError;

    fn try_clone(&self) -> Result<Self, Self::Error> {
        match self {
            Self::Empty => Ok(Self::Empty),
            Self::Owned(bytes) => Ok(Self::Owned(bytes.clone())),
            Self::Streaming => Err(TryCloneError::NotReplayable("Payload::Streaming")),
        }
    }
}

#[test]
fn an_owned_value_clones_with_its_contents() {
    let cloned = Payload::Owned(vec![1, 2, 3])
        .try_clone()
        .expect("owned data is replayable");
    assert_eq!(cloned, Payload::Owned(vec![1, 2, 3]));
}

#[test]
fn an_empty_value_clones() {
    assert_eq!(Payload::Empty.try_clone().expect("empty"), Payload::Empty);
}

#[test]
fn a_single_shot_value_is_refused() {
    let err = Payload::Streaming
        .try_clone()
        .expect_err("a single-shot source must not be duplicated");
    assert_eq!(err, TryCloneError::NotReplayable("Payload::Streaming"));
}

#[test]
fn cloning_does_not_consume_the_source() {
    // The contract callers depend on: a reconnect loop clones the same value on
    // every attempt. An implementation that moved the data out would satisfy the
    // first clone and silently yield nothing afterwards — the exact bug this
    // trait was introduced to prevent.
    let payload = Payload::Owned(b"body".to_vec());
    let first = payload.try_clone().expect("first clone");
    let second = payload.try_clone().expect("second clone");
    assert_eq!(first, second);
    assert_eq!(
        payload,
        Payload::Owned(b"body".to_vec()),
        "the original must remain usable after cloning"
    );
}

#[test]
fn the_error_names_what_refused() {
    // This string is what shows up in a log when a replay silently stops; it has
    // to identify the source, not just say "cannot clone".
    let text = TryCloneError::NotReplayable("Payload::Streaming").to_string();
    assert!(
        text.contains("Payload::Streaming"),
        "the message must name the refusing variant, got: {text}"
    );
    assert!(
        text.contains("single-shot"),
        "the message must explain why, got: {text}"
    );
}

#[test]
fn the_error_is_a_std_error() {
    fn assert_is_error<E: std::error::Error>(_: &E) {}
    assert_is_error(&TryCloneError::NotReplayable("x"));
}

#[test]
fn the_trait_is_usable_generically() {
    // The point of living in foundation_core: code can accept "anything
    // fallibly clonable" without knowing the concrete type.
    fn duplicate<T: TryClone>(value: &T) -> Option<T> {
        value.try_clone().ok()
    }
    assert!(duplicate(&Payload::Owned(vec![7])).is_some());
    assert!(duplicate(&Payload::Streaming).is_none());
}
