//! The JSON message schema of the Durable Streams WebSocket transport.
//!
//! A client sends one request in one WebSocket text frame. A request is a JSON
//! object with an `id` and an `op`. The server answers with a JSON object that
//! carries the same `id`. A read that stays open sends one frame per page and a
//! final `end` frame.
//!
//! The transport owns no protocol state. It maps each operation to
//! [`DurableService`](foundation_core::durable::DurableService). The core keeps
//! no WebSocket type.
//!
//! # Message schema
//!
//! A request:
//!
//! ```json
//! {"id":1,"op":"create","stream":"events","content_type":"text/plain"}
//! {"id":2,"op":"append","stream":"events","data":"aGVsbG8=","content_type":"text/plain"}
//! {"id":3,"op":"read","stream":"events","offset":"now","live":true}
//! {"id":4,"op":"cancel","read":3}
//! ```
//!
//! A reply:
//!
//! ```json
//! {"id":1,"ok":true,"op":"create","status":201,"created":true,"offset":"0000000000000000"}
//! {"id":2,"ok":false,"error":{"kind":"not_found","status":404,"message":"stream not found"}}
//! ```
//!
//! Byte payloads are base64 in the standard alphabet with padding. A read reply
//! carries `chunks` as a list, so a JSON stream keeps its message boundaries.
//!
//! A frame that is not a valid request answers with `id` 0 and a `bad_request`
//! error. The connection stays open.

use std::fmt;

use base64::Engine as _;
use foundation_core::durable::{
    AppendKind, AppendOutcome, CreateOutcome, DeleteOutcome, ReadPage, StoreError, StreamHead,
    StreamOffset,
};
use serde::{Deserialize, Serialize};

/// A transport failure on one WebSocket connection.
#[derive(Debug)]
pub enum WsError {
    /// The connection failed.
    Io(std::io::Error),
    /// The peer broke the WebSocket framing rules.
    Protocol(String),
    /// The peer sent a request this transport cannot read.
    BadRequest(String),
    /// The store refused the operation.
    Store(StoreError),
    /// A reply is larger than the configured frame cap.
    FrameTooLarge {
        /// The reply size in bytes.
        size: usize,
        /// The configured cap in bytes.
        cap: usize,
    },
}

impl fmt::Display for WsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "websocket I/O error: {error}"),
            Self::Protocol(message) => write!(f, "websocket protocol error: {message}"),
            Self::BadRequest(message) => write!(f, "bad request: {message}"),
            Self::Store(error) => write!(f, "store error: {error}"),
            Self::FrameTooLarge { size, cap } => {
                write!(f, "reply of {size} bytes exceeds the {cap}-byte frame cap")
            }
        }
    }
}

impl std::error::Error for WsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for WsError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<StoreError> for WsError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// One request from the client.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WsRequest {
    /// The request identifier. The reply repeats it.
    pub id: u64,
    /// The operation and its arguments.
    #[serde(flatten)]
    pub op: WsOp,
}

/// The operations this transport carries.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WsOp {
    /// Create a stream.
    Create {
        /// The stream name.
        stream: String,
        /// The media type. The store default when absent.
        #[serde(default)]
        content_type: Option<String>,
        /// A sliding time to live (TTL) in milliseconds.
        #[serde(default)]
        ttl_ms: Option<u64>,
        /// An absolute expiry as UNIX milliseconds.
        #[serde(default)]
        expires_at_ms: Option<i64>,
        /// True to create the stream closed.
        #[serde(default)]
        closed: bool,
        /// Initial content, base64.
        #[serde(default)]
        initial: Option<String>,
    },
    /// Append one chunk.
    Append {
        /// The stream name.
        stream: String,
        /// The chunk, base64. Absent or empty means a close-only request.
        #[serde(default)]
        data: Option<String>,
        /// The chunk media type. Required when `data` is not empty.
        #[serde(default)]
        content_type: Option<String>,
        /// The application writer sequence.
        #[serde(default)]
        stream_seq: Option<String>,
        /// True to close the stream after the append.
        #[serde(default)]
        close: bool,
    },
    /// Close a stream.
    Close {
        /// The stream name.
        stream: String,
        /// A final chunk, base64.
        #[serde(default)]
        data: Option<String>,
    },
    /// Return stream metadata.
    Head {
        /// The stream name.
        stream: String,
    },
    /// Delete a stream.
    Delete {
        /// The stream name.
        stream: String,
    },
    /// Create a fork.
    Fork {
        /// The new stream name.
        stream: String,
        /// The source stream name.
        from: String,
        /// The divergence offset. The source tail when absent.
        #[serde(default)]
        offset: Option<String>,
        /// The sub-position past the divergence offset.
        #[serde(default)]
        sub_offset: u32,
    },
    /// Read a bounded page, or stay open and push pages.
    Read {
        /// The stream name.
        stream: String,
        /// The start offset. `-1` or absent is the start. `now` is the tail.
        #[serde(default)]
        offset: Option<String>,
        /// True to keep the read open and push pages.
        #[serde(default)]
        live: bool,
        /// The page byte cap.
        #[serde(default)]
        max_bytes: Option<usize>,
        /// The page chunk cap.
        #[serde(default)]
        max_chunks: Option<usize>,
    },
    /// Cancel an open live read.
    Cancel {
        /// The `id` of the live read request.
        read: u64,
    },
}

impl WsRequest {
    /// Parse one request from a text frame.
    ///
    /// # Errors
    ///
    /// Returns [`WsError::BadRequest`] when the text is not a request object.
    pub fn parse(text: &str) -> Result<Self, WsError> {
        serde_json::from_str(text)
            .map_err(|error| WsError::BadRequest(format!("invalid JSON request: {error}")))
    }
}

/// The error body of a failed reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WsErrorBody {
    /// A stable error name.
    pub kind: &'static str,
    /// The status the Hypertext Transfer Protocol (HTTP) adapter would use.
    pub status: u16,
    /// A readable message.
    pub message: String,
}

/// One reply from the server.
///
/// Fields that do not fit the operation are absent from the textual output.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WsReply {
    /// The request identifier this reply answers.
    pub id: u64,
    /// True when the operation succeeded.
    pub ok: bool,
    /// The operation this reply answers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<&'static str>,
    /// The status the Hypertext Transfer Protocol (HTTP) adapter would use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// True when a create made a new stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<bool>,
    /// True when an append was a duplicate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate: Option<bool>,
    /// True when a delete was soft, because a fork still reads the stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_deleted: Option<bool>,
    /// The stream media type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// The tail offset or the next read offset, as wire text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
    /// True when a page has all data available at response time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_to_date: Option<bool>,
    /// True when the stream is closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    /// True when a live read ended because the client cancelled it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled: Option<bool>,
    /// The page chunks, each base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks: Option<Vec<String>>,
    /// The error body of a failed reply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WsErrorBody>,
}

impl WsReply {
    /// The reply to a failed request.
    #[must_use]
    pub fn error(id: u64, error: &StoreError) -> Self {
        Self {
            id,
            ok: false,
            status: Some(crate::shared::durable::status::status_for(error)),
            error: Some(WsErrorBody {
                kind: error_kind(error),
                status: crate::shared::durable::status::status_for(error),
                message: error.to_string(),
            }),
            ..Self::default()
        }
    }

    /// A transport-level failure that no store operation caused.
    #[must_use]
    pub fn transport_error(id: u64, kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            error: Some(WsErrorBody {
                kind,
                status: 400,
                message: message.into(),
            }),
            ..Self::default()
        }
    }

    /// The reply to a create or fork.
    #[must_use]
    pub fn create(op: &'static str, id: u64, outcome: &CreateOutcome) -> Self {
        Self {
            id,
            ok: true,
            op: Some(op),
            status: Some(if outcome.created { 201 } else { 200 }),
            created: Some(outcome.created),
            content_type: Some(outcome.head.content_type.as_str().to_string()),
            offset: Some(offset_text(&outcome.head.tail)),
            closed: Some(outcome.head.closed),
            ..Self::default()
        }
    }

    /// The reply to an append or a close.
    #[must_use]
    pub fn append(op: &'static str, id: u64, outcome: &AppendOutcome) -> Self {
        Self {
            id,
            ok: true,
            op: Some(op),
            status: Some(204),
            duplicate: Some(matches!(outcome.kind, AppendKind::Duplicate)),
            offset: Some(offset_text(&outcome.cursor.next)),
            up_to_date: Some(outcome.cursor.up_to_date || outcome.cursor.closed),
            closed: Some(outcome.closed),
            ..Self::default()
        }
    }

    /// The reply to a head.
    #[must_use]
    pub fn head(id: u64, head: &StreamHead) -> Self {
        Self {
            id,
            ok: true,
            op: Some("head"),
            content_type: Some(head.content_type.as_str().to_string()),
            offset: Some(offset_text(&head.tail)),
            closed: Some(head.closed),
            ..Self::default()
        }
    }

    /// The reply to a delete.
    #[must_use]
    pub fn delete(id: u64, outcome: DeleteOutcome) -> Self {
        Self {
            id,
            ok: true,
            op: Some("delete"),
            status: Some(204),
            soft_deleted: Some(outcome.soft_deleted),
            ..Self::default()
        }
    }

    /// One read page. `op` is `read` for the first page of a read and `data` for
    /// a later page of a live read.
    #[must_use]
    pub fn page(op: &'static str, id: u64, page: &ReadPage) -> Self {
        Self {
            id,
            ok: true,
            op: Some(op),
            chunks: Some(
                page.chunks
                    .iter()
                    .map(|chunk| encode(chunk.as_bytes()))
                    .collect(),
            ),
            offset: Some(offset_text(&page.cursor.next)),
            up_to_date: Some(page.cursor.up_to_date || page.cursor.closed),
            closed: Some(page.cursor.closed),
            ..Self::default()
        }
    }

    /// The end of a live read that reached the closed end of the stream.
    #[must_use]
    pub fn end_closed(id: u64) -> Self {
        Self {
            id,
            ok: true,
            op: Some("end"),
            closed: Some(true),
            ..Self::default()
        }
    }

    /// The end of a live read the client cancelled.
    #[must_use]
    pub fn end_cancelled(id: u64) -> Self {
        Self {
            id,
            ok: true,
            op: Some("end"),
            cancelled: Some(true),
            ..Self::default()
        }
    }

    /// Render the reply as its JSON text.
    ///
    /// # Errors
    ///
    /// Returns [`WsError::Protocol`] when the reply cannot be serialized.
    pub fn to_text(&self) -> Result<String, WsError> {
        serde_json::to_string(self)
            .map_err(|error| WsError::Protocol(format!("reply encoding failed: {error}")))
    }
}

/// The stable error name for a store refusal.
#[must_use]
pub fn error_kind(error: &StoreError) -> &'static str {
    match error {
        StoreError::NotFound => "not_found",
        StoreError::Gone => "gone",
        StoreError::Trimmed => "trimmed",
        StoreError::Fenced { .. } => "fenced",
        StoreError::InvalidEpochSeq { .. } => "invalid_epoch_seq",
        StoreError::SequenceGap { .. } => "sequence_gap",
        StoreError::StreamSeqConflict { .. } => "stream_seq_conflict",
        StoreError::ContentTypeMismatch { .. } => "content_type_mismatch",
        StoreError::ConfigConflict => "config_conflict",
        StoreError::Closed { .. } => "closed",
        StoreError::BadRequest(_) => "bad_request",
        StoreError::RateLimited { .. } => "rate_limited",
        StoreError::Unsupported => "unsupported",
        StoreError::Storage(_) => "storage",
    }
}

/// Encode bytes as standard base64 with padding.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Decode standard base64 with padding.
///
/// # Errors
///
/// Returns [`StoreError::BadRequest`] with
/// [`BadRequestKind::Body`](foundation_core::durable::BadRequestKind::Body)
/// when the text is malformed.
pub fn decode(text: &str) -> Result<Vec<u8>, StoreError> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| StoreError::BadRequest(foundation_core::durable::BadRequestKind::Body))
}

/// The wire text of an offset.
#[must_use]
pub fn offset_text(offset: &StreamOffset) -> String {
    foundation_core::durable::wire::offset_text(offset)
}
