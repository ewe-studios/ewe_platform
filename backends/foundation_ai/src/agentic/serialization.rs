//! Dual serialization for session records (Feature 05).
//!
//! WHY: `SessionRecord`s need two serializations — **JSON/NDJSON** (human-readable,
//! append-friendly storage for the `DocumentStore`, `jq`/`fff`-friendly) and
//! **Arrow** columnar (analytics: token sums, filter-by-type, ordered scans; and
//! efficient bulk transport). A single JSON `content` column wastes Arrow's
//! columnar power, so we **promote** the searchable fields to real columns and
//! keep the full record as a JSON `content` blob for fidelity (Decision 10 / TODO #3).
//!
//! WHAT: [`SessionRecordRow`] — the flat, promoted-column row that derives the
//! `foundation_arrow` traits — plus [`to_record_batch`] / [`from_record_batch`]
//! over `&[SessionRecord]` and small columnar analytics helpers.
//!
//! HOW: JSON is `SessionRecord`'s own serde. Arrow goes through the flat row:
//! `SessionRecord → SessionRecordRow → RecordBatch` and back. The `content`
//! column holds the JSON of the full record (no fidelity loss); promoted columns
//! carry the bits worth filtering/aggregating on. `FlatBuffers` is intentionally
//! NOT used (Arrow rides `FlatBuffers` internally and builds on wasm — TODO #4).

use foundation_arrow::arrow_array::RecordBatch;
use foundation_arrow::{FromArrow, ToArrow};
use foundation_macros::{ArrowSchema, FromArrow, ToArrow};
use serde::{Deserialize, Serialize};

use crate::types::{Messages, ModelOutput, SessionRecord};

/// Serialization error for the dual JSON/Arrow codec.
///
/// WHY: `SessionRecord` supports two serialization formats (JSON for
/// storage, Arrow for analytics). Failures in either path must be
/// distinguishable so callers can log which codec broke and whether
/// the data is still readable via the other format.
///
/// WHAT: Two variants — `Json` (`serde_json` failure) and `Arrow`
/// (arrow-rs batch conversion failure), each carrying a diagnostic string.
///
/// HOW: Returned by `to_record_batch` / `from_record_batch` and the JSON
/// round-trip helpers. Implements `Display` and `Error` for `?` propagation.
#[derive(Debug)]
pub enum SerError {
    /// JSON (de)serialization failed.
    Json(String),
    /// Arrow batch (de)serialization failed.
    Arrow(String),
}

impl core::fmt::Display for SerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SerError::Json(m) => write!(f, "json serialization: {m}"),
            SerError::Arrow(m) => write!(f, "arrow serialization: {m}"),
        }
    }
}

impl std::error::Error for SerError {}

/// The flat, promoted-column projection of a [`SessionRecord`] for Arrow.
///
/// Promoted columns mirror F06's `DocumentStore` columns (consistency) so the same
/// `record_type`/`title`/`summary` + token/model analytics fields exist in both
/// the columnar batch and the document index. `content` keeps the full JSON
/// record so nothing is lost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ArrowSchema, ToArrow, FromArrow)]
pub struct SessionRecordRow {
    /// scru128 id (sortable/searchable). For `Conversation` this is the wrapped
    /// message's id; for memory/summary records it's a synthesized stable key.
    pub id: String,
    /// The record discriminant ("`conversation"/"working_memory"/"observation`"/
    /// "`reflection"/"failed_action"/"summary`").
    pub record_type: String,
    /// `MessageRole` wire string for conversation records, else empty.
    pub role: String,
    /// A short title/label (memory summaries); null otherwise.
    pub title: Option<String>,
    /// A summary/first-line for memory records; null otherwise.
    pub summary: Option<String>,
    /// Model id for assistant conversation records; null otherwise.
    pub model: Option<String>,
    /// Input tokens (assistant turns); null otherwise — enables `SUM(input_tokens)`.
    pub input_tokens: Option<u32>,
    /// Output tokens (assistant turns); null otherwise.
    pub output_tokens: Option<u32>,
    /// scru128 timestamp in ms (for time filters / `ORDER BY`). 0 when unknown.
    pub created_at: u64,
    /// The full record encoded as JSON (fidelity — reconstructs the exact record).
    pub content: String,
}

impl SessionRecordRow {
    /// Project a `SessionRecord` into its flat promoted-column row.
    pub fn from_record(record: &SessionRecord) -> Result<Self, SerError> {
        let content = serde_json::to_string(record).map_err(|e| SerError::Json(e.to_string()))?;
        let mut row = Self {
            id: String::new(),
            record_type: record_type_of(record).to_string(),
            role: String::new(),
            title: None,
            summary: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: 0,
            content,
        };

        match record {
            SessionRecord::Conversation { message } => {
                row.id = message.id().to_string();
                row.created_at = message.id().timestamp();
                match message {
                    Messages::User { role, .. } => row.role = role.as_wire().to_string(),
                    Messages::ToolResult { .. } => row.role = "tool".to_string(),
                    Messages::Assistant {
                        model,
                        usage,
                        content,
                        ..
                    } => {
                        row.role = "assistant".to_string();
                        row.model = Some(model.to_string());
                        // Token counts are clamped non-negative and within u32 range.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        {
                            row.input_tokens = Some(usage.input.max(0.0).round() as u32);
                            row.output_tokens = Some(usage.output.max(0.0).round() as u32);
                        }
                        // Surface a tool-call name as the title for quick scanning.
                        if let ModelOutput::ToolCall { name, .. } = content {
                            row.title = Some(name.clone());
                        }
                    }
                }
            }
            SessionRecord::WorkingMemory { facts, .. } => {
                row.summary = facts.first().map(|f| f.fact.clone());
                row.title = Some(format!("{} fact(s)", facts.len()));
            }
            SessionRecord::Observation { observations, .. } => {
                row.summary = observations.first().map(|o| o.content.clone());
                row.title = Some(format!("{} observation(s)", observations.len()));
            }
            SessionRecord::Reflection { reflections, .. } => {
                row.summary = reflections.first().map(|r| r.summary.clone());
                row.title = Some(format!("{} reflection(s)", reflections.len()));
            }
            SessionRecord::FailedAction { error, .. } => {
                row.summary = Some(error.to_string());
                row.title = Some("failed_action".to_string());
            }
            SessionRecord::Summary {
                message_count,
                usage,
            } => {
                row.title = Some(format!("summary ({message_count} msgs)"));
                // Clamped to u32::MAX so truncation is safe.
                #[allow(clippy::cast_possible_truncation)]
                {
                    row.input_tokens = Some(usage.input.min(u64::from(u32::MAX)) as u32);
                    row.output_tokens = Some(usage.output.min(u64::from(u32::MAX)) as u32);
                }
            }
        }
        Ok(row)
    }

    /// Reconstruct the exact `SessionRecord` from the JSON `content` blob.
    pub fn into_record(self) -> Result<SessionRecord, SerError> {
        serde_json::from_str(&self.content).map_err(|e| SerError::Json(e.to_string()))
    }
}

/// Encode a slice of records to a columnar Arrow `RecordBatch` (promoted columns
/// + content blob).
pub fn to_record_batch(records: &[SessionRecord]) -> Result<RecordBatch, SerError> {
    let rows: Vec<SessionRecordRow> = records
        .iter()
        .map(SessionRecordRow::from_record)
        .collect::<Result<_, _>>()?;
    SessionRecordRow::to_arrow_batch(&rows).map_err(|e| SerError::Arrow(e.to_string()))
}

/// Decode a columnar Arrow `RecordBatch` back to records (via the content blob).
pub fn from_record_batch(batch: &RecordBatch) -> Result<Vec<SessionRecord>, SerError> {
    let rows =
        SessionRecordRow::from_arrow_batch(batch).map_err(|e| SerError::Arrow(e.to_string()))?;
    rows.into_iter()
        .map(SessionRecordRow::into_record)
        .collect()
}

/// The `record_type` discriminant string for a record (matches serde's tag).
fn record_type_of(record: &SessionRecord) -> &'static str {
    match record {
        SessionRecord::Conversation { .. } => "conversation",
        SessionRecord::WorkingMemory { .. } => "working_memory",
        SessionRecord::Observation { .. } => "observation",
        SessionRecord::Reflection { .. } => "reflection",
        SessionRecord::FailedAction { .. } => "failed_action",
        SessionRecord::Summary { .. } => "summary",
    }
}

// ---------------------------------------------------------------------------
// Columnar analytics helpers (demonstrate the value of promoted columns)
// ---------------------------------------------------------------------------

/// Sum of `output_tokens` across rows (skips nulls) — the columnar analog of
/// `SELECT SUM(output_tokens)`.
#[must_use]
pub fn sum_output_tokens(rows: &[SessionRecordRow]) -> u64 {
    rows.iter()
        .filter_map(|r| r.output_tokens)
        .map(u64::from)
        .sum()
}

/// Sum of `input_tokens` across rows (skips nulls).
#[must_use]
pub fn sum_input_tokens(rows: &[SessionRecordRow]) -> u64 {
    rows.iter()
        .filter_map(|r| r.input_tokens)
        .map(u64::from)
        .sum()
}

/// Filter rows by `record_type` — the columnar analog of
/// `WHERE record_type = ?`.
#[must_use]
pub fn filter_by_type<'a>(
    rows: &'a [SessionRecordRow],
    record_type: &str,
) -> Vec<&'a SessionRecordRow> {
    rows.iter()
        .filter(|r| r.record_type == record_type)
        .collect()
}

