//! The transport driver trait and the normalized observation.
//!
//! A driver maps the business operations to one transport. The suite calls the
//! same operations on every driver and compares the [`Observation`] values.
//! An observation holds the protocol outcome only. It holds no transport
//! encoding: the offset text may differ, so it records presence, not text.

use core::fmt;
use core::time::Duration;

use serde::{Deserialize, Serialize};

/// A producer identity and position in one epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Producer {
    /// The producer identifier.
    pub id: String,
    /// The producer epoch.
    pub epoch: u64,
    /// The sequence within the epoch.
    pub seq: u64,
}

/// The protocol outcome of one step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// The logical status. A success uses the status the protocol assigns.
    Status(u16),
    /// A typed refusal.
    Error(ErrorKind),
}

/// A transport-neutral error kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    /// The stream is absent.
    NotFound,
    /// The stream is soft-deleted.
    Gone,
    /// The stream is closed.
    Closed,
    /// The request conflicts with the stream state.
    Conflict,
    /// The request is malformed.
    BadRequest,
    /// The transport cannot express the operation.
    Unsupported,
    /// Any other refusal, with a readable name.
    Other(String),
}

/// One normalized step result.
///
/// The fields are the ones the scenario suite compares. A transport may encode
/// them differently, but a driver decodes them to this form.
// The four booleans are independent protocol facts, not one state.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// The logical status or the typed refusal.
    pub outcome: Outcome,
    /// The body bytes of a read.
    pub body: Vec<u8>,
    /// True when the reply carries a next offset.
    pub next_offset_present: bool,
    /// True when a read reached the tail.
    pub up_to_date: bool,
    /// True when the reply says the stream is closed.
    pub closed: bool,
    /// The number of read chunks. A write has no chunks.
    pub chunks: usize,
    /// True when the store treated the write as a duplicate.
    pub duplicate: bool,
}

impl Observation {
    /// A success with the given logical status and no body.
    #[must_use]
    pub fn status(status: u16) -> Self {
        Self {
            outcome: Outcome::Status(status),
            body: Vec::new(),
            next_offset_present: false,
            up_to_date: false,
            closed: false,
            chunks: 0,
            duplicate: false,
        }
    }

    /// A typed refusal with no body.
    #[must_use]
    pub fn error(kind: ErrorKind) -> Self {
        Self {
            outcome: Outcome::Error(kind),
            body: Vec::new(),
            next_offset_present: false,
            up_to_date: false,
            closed: false,
            chunks: 0,
            duplicate: false,
        }
    }

    /// Set the next-offset presence.
    #[must_use]
    pub fn with_next_offset(mut self, present: bool) -> Self {
        self.next_offset_present = present;
        self
    }

    /// Set the up-to-date fact.
    #[must_use]
    pub fn with_up_to_date(mut self, value: bool) -> Self {
        self.up_to_date = value;
        self
    }

    /// Set the closed fact.
    #[must_use]
    pub fn with_closed(mut self, value: bool) -> Self {
        self.closed = value;
        self
    }

    /// Set the duplicate fact.
    #[must_use]
    pub fn with_duplicate(mut self, value: bool) -> Self {
        self.duplicate = value;
        self
    }

    /// Set the body. A non-empty body is one chunk for a binary stream.
    #[must_use]
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.chunks = usize::from(!body.is_empty());
        self.body = body;
        self
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("not_found"),
            Self::Gone => formatter.write_str("gone"),
            Self::Closed => formatter.write_str("closed"),
            Self::Conflict => formatter.write_str("conflict"),
            Self::BadRequest => formatter.write_str("bad_request"),
            Self::Unsupported => formatter.write_str("unsupported"),
            Self::Other(name) => formatter.write_str(name),
        }
    }
}

/// One step result: the observation and the raw next offset.
///
/// The raw offset is not compared. It lets a later step start where an earlier
/// one stopped, which proves the offset order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepResult {
    /// The normalized observation.
    pub observation: Observation,
    /// The raw next offset, used only to chain steps.
    pub next_offset: Option<String>,
}

impl StepResult {
    /// Build one step result.
    #[must_use]
    pub fn new(observation: Observation, next_offset: Option<String>) -> Self {
        Self {
            observation,
            next_offset,
        }
    }
}

/// The operations every transport driver implements.
///
/// The suite calls these methods on each driver with the same arguments. A
/// transport that cannot express an operation returns
/// [`ErrorKind::Unsupported`]; it does not fake a success.
#[allow(async_fn_in_trait)]
pub trait Driver {
    /// Create a stream with an optional sliding time to live (TTL) in
    /// milliseconds.
    async fn create(&mut self, stream: &str, content_type: &str, ttl_ms: Option<u64>)
        -> StepResult;

    /// Append one chunk.
    async fn append(
        &mut self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult;

    /// Close a stream, with an optional final chunk.
    async fn close(&mut self, stream: &str, data: &[u8], content_type: &str) -> StepResult;

    /// Read a bounded page. `None` starts at the beginning.
    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult;

    /// Create a fork at `at`. `None` forks at the source tail.
    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult;

    /// Wait on the injected timer.
    ///
    /// The model and every host read the same clock, so a test can advance the
    /// wall clock with one bounded wait.
    async fn wait(&mut self, duration: Duration);
}
