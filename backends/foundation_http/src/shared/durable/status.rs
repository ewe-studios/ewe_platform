//! The Durable Streams status table and header names.

use foundation_core::durable::StoreError;

/// Map one core refusal to its HTTP status.
#[must_use]
pub fn status_for(error: &StoreError) -> u16 {
    match error {
        StoreError::NotFound => 404,
        StoreError::Gone | StoreError::Trimmed => 410,
        StoreError::Fenced { .. } => 403,
        StoreError::InvalidEpochSeq { .. } | StoreError::BadRequest(_) => 400,
        StoreError::SequenceGap { .. }
        | StoreError::StreamSeqConflict { .. }
        | StoreError::ContentTypeMismatch { .. }
        | StoreError::ConfigConflict
        | StoreError::Closed { .. } => 409,
        StoreError::RateLimited { .. } => 429,
        StoreError::Unsupported => 501,
        StoreError::Storage(_) => 500,
    }
}

/// The protocol header names.
pub mod header {
    /// The next offset for a read.
    pub const STREAM_NEXT_OFFSET: &str = "Stream-Next-Offset";
    /// The CDN collapsing cursor.
    pub const STREAM_CURSOR: &str = "Stream-Cursor";
    /// The caught-up flag.
    pub const STREAM_UP_TO_DATE: &str = "Stream-Up-To-Date";
    /// The closed flag.
    pub const STREAM_CLOSED: &str = "Stream-Closed";
    /// The application writer sequence.
    pub const STREAM_SEQ: &str = "Stream-Seq";
    /// The sliding TTL.
    pub const STREAM_TTL: &str = "Stream-TTL";
    /// The absolute expiry.
    pub const STREAM_EXPIRES_AT: &str = "Stream-Expires-At";
    /// The producer identifier.
    pub const PRODUCER_ID: &str = "Producer-Id";
    /// The producer epoch.
    pub const PRODUCER_EPOCH: &str = "Producer-Epoch";
    /// The producer sequence.
    pub const PRODUCER_SEQ: &str = "Producer-Seq";
    /// The expected producer sequence.
    pub const PRODUCER_EXPECTED_SEQ: &str = "Producer-Expected-Seq";
    /// The received producer sequence.
    pub const PRODUCER_RECEIVED_SEQ: &str = "Producer-Received-Seq";
    /// The fork source path.
    pub const STREAM_FORKED_FROM: &str = "Stream-Forked-From";
    /// The fork divergence offset.
    pub const STREAM_FORK_OFFSET: &str = "Stream-Fork-Offset";
    /// The fork sub-offset.
    pub const STREAM_FORK_SUB_OFFSET: &str = "Stream-Fork-Sub-Offset";
    /// The SSE base64 marker.
    pub const STREAM_SSE_DATA_ENCODING: &str = "Stream-SSE-Data-Encoding";
}
