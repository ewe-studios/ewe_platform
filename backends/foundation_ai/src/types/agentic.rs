//! Agentic session types — the record log substrate (Feature 01).
//!
//! WHY: The agentic loop persists conversation AND memory snapshots
//! (working memory, observations, reflections) to one ordered, replayable log.
//! Memory snapshots are NOT provider messages and must never be sent verbatim
//! to an LLM, so they cannot be `Messages` variants. This module introduces the
//! [`SessionRecord`] wrapper that keeps `Messages` provider-pure while giving the
//! Message API (F08) a single ordered log element.
//!
//! WHAT: [`SessionId`] (time-ordered, machine-attributable session identity),
//! [`SessionRecord`] (the log element), and the memory entry types
//! ([`MemoryFact`], [`ObservationEntry`], [`ReflectionEntry`]).
//!
//! HOW: ids are minted from `foundation_compact` scru128 (`Id`); timestamps use
//! `foundation_compact::SystemTime` (matches the existing `Messages::Assistant`).
//! `SessionRecord` derives only `PartialEq` (no `Eq`/`Hash`) — matching
//! `Messages` — so `f32` confidence/importance fields raise no conflict.

use std::fmt;
use std::str::FromStr;

use foundation_compact::ids::{Id, ParseError};
use foundation_compact::SystemTime;
use serde::{Deserialize, Serialize};

use super::base_types::Messages;

// ============================================================================
// SessionId — time-ordered, machine-attributable session identity (Decision 01)
// ============================================================================

/// Globally-unique, time-ordered session identity. Newtype over
/// `foundation_compact::Id` (scru128).
///
/// WHY: scru128 ids are lexicographically == chronologically sortable, so a
/// session log keyed by `SessionId` replays in creation order for free; folding
/// a machine id into the entropy region makes ids machine-attributable
/// (Decision 01) without a coordination service.
///
/// WHAT: `new()` mints a fresh time-ordered id; `from_name()` derives a stable,
/// sortable id from a human label (same name + machine + millisecond ⇒ same id).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Id);

impl SessionId {
    /// Fresh, **monotonically-ordered**, machine-attributable id.
    ///
    /// Uses `foundation_compact`'s thread-local scru128 `Generator` (via
    /// `new_scru128_with_machine_id`), so ids minted in sequence are strictly
    /// increasing even within the same millisecond — the generator's counter
    /// guarantees it. The machine id (Decision 01 / OD-7) is folded into the
    /// high bits of the entropy field by the generator, which does not disturb
    /// ordering (entropy is the least-significant field, below timestamp+counter).
    #[must_use]
    pub fn new() -> Self {
        Self(foundation_compact::ids::new_scru128_with_machine_id(
            machine_id(),
        ))
    }

    /// Deterministic, sortable id derived from a human-friendly name.
    ///
    /// Delegates to `foundation_compact::ids::scru128_from_name_with_machine_id`:
    /// the same name on the same machine within one millisecond reproduces the
    /// same id (used to resume a session by label). Cross-millisecond ordering is
    /// preserved by the timestamp; the machine id (OD-7) sits in the high entropy
    /// bits, matching [`new`](Self::new)'s layout. Different machines do not
    /// collide (OD-2 accepted: intra-ms same-machine collisions are the resume key).
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        Self(foundation_compact::ids::scru128_from_name_with_machine_id(
            name,
            machine_id(),
        ))
    }

    /// The 48-bit unix-ms timestamp embedded in this id.
    #[must_use]
    pub fn timestamp_ms(&self) -> u64 {
        self.0.timestamp()
    }

    /// The machine id folded into this id's entropy field (OD-7).
    #[must_use]
    pub fn machine_id(&self) -> u32 {
        foundation_compact::ids::machine_id_of(&self.0)
    }

    /// Borrow the underlying `foundation_compact::Id`.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Id implements Display as the 25-char scru128 string.
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Id::from_str(s).map(SessionId)
    }
}

/// Stable per-process machine identifier folded into id entropy (OD-7).
///
/// Native: derived from the hostname (`HOSTNAME`/`COMPUTERNAME` env, else a
/// per-process random fallback). Wasm: a per-process random value (no stable
/// host signal). Computed once and cached.
fn machine_id() -> u32 {
    use std::sync::OnceLock;
    static MACHINE_ID: OnceLock<u32> = OnceLock::new();
    *MACHINE_ID.get_or_init(|| {
        let host = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .ok();
        match host {
            Some(h) if !h.is_empty() => {
                // Intentional: fold the 64-bit hash down to a 32-bit machine id
                // (the generator masks it further to DEFAULT_MACHINE_ID_BITS).
                #[allow(clippy::cast_possible_truncation)]
                let mid = foundation_compact::ids::stable_hash(h.as_bytes()) as u32;
                mid
            }
            // No stable host signal (wasm, or unset env): random per-process id.
            _ => foundation_compact::entropy::u32().unwrap_or(0),
        }
    })
}

// ============================================================================
// Forward-reference stubs (filled by F02 / F04)
// ============================================================================

// `AgenticError` (the matchable error taxonomy carried by
// `SessionRecord::FailedAction`) is owned by F02 in `crate::agentic::errors` and
// re-exported here for the record types below.
pub use crate::agentic::errors::AgenticError;

// `TokenSnapshot` (the cumulative session usage carried by
// `SessionRecord::Summary`) is owned by F04 in `crate::agentic::token_ledger` and
// re-exported here for the record types below.
pub use crate::agentic::token_ledger::TokenSnapshot;

// ============================================================================
// SessionRecord — one entry in a session's ordered log (Decision 03, OD-3)
// ============================================================================

/// One entry in a session's ordered record log. Either a real conversation
/// message (sent to / received from the model) or an agentic memory snapshot
/// (persisted + replayed, never sent verbatim to a provider).
///
/// Internally tagged with `message_type` (Decision 03's tag key). `Conversation`
/// is a **struct variant** (not a newtype) because serde internal tagging cannot
/// wrap a newtype variant — a verified design constraint.
// `Conversation` wraps the large `Messages` enum; boxing it would complicate the
// serde internal-tagging contract for no real benefit (matches `Messages`'s own allow).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum SessionRecord {
    /// A provider-facing message (user/assistant/tool-result).
    Conversation { message: Messages },

    /// Discard the assistant output already streamed for this turn.
    ///
    /// WHY it has to exist: the agent loop can only judge a turn once it is
    /// complete, but a streaming consumer has already been handed every token
    /// by then. When the loop rejects that turn and asks the model again, the
    /// rejected text is sitting in the consumer's buffer — without this the
    /// retry's answer is appended to the junk it was meant to replace, and the
    /// user reads ".  Hello." instead of "Hello.".
    ///
    /// WHAT: consumers should drop the assistant messages they have collected
    /// for the current turn and keep whatever arrives after it. User messages
    /// and tool results are unaffected.
    Retracted {
        #[serde(default = "fresh_record_id")]
        id: Id,
        /// Why the turn was withdrawn, for logs and diagnostics.
        reason: String,
        timestamp: SystemTime,
    },

    /// Permanent curated facts about the user/session (Decision 03, Tier 1).
    WorkingMemory {
        #[serde(default = "fresh_record_id")]
        id: Id,
        facts: Vec<MemoryFact>,
        version: u64,
        timestamp: SystemTime,
    },

    /// Time-scoped structured observations (Decision 03, Tier 2).
    Observation {
        #[serde(default = "fresh_record_id")]
        id: Id,
        observations: Vec<ObservationEntry>,
        token_count: u64,
        timestamp: SystemTime,
    },

    /// Condensed reflections over observations (Decision 03, Tier 3).
    Reflection {
        #[serde(default = "fresh_record_id")]
        id: Id,
        reflections: Vec<ReflectionEntry>,
        generated_at: SystemTime,
        observation_token_count_before: u64,
        reflection_token_count_after: u64,
    },

    /// An error surfaced into the stream as a record rather than a `Result`
    /// (F03/F02). **Transient — NOT persisted** to the session log (F08 drops
    /// it); it exists only so the consumer-facing stream can deliver failures
    /// in-band. `trace` is the JSON-serializable projection of the live
    /// `foundation_errstacks` error chain (`ErrorTrace::to_structured()`).
    FailedAction {
        error: AgenticError,
        trace: foundation_errstacks::StructuredErrorTrace,
    },

    /// Per-interaction completion marker (F03): how many records the interaction
    /// produced and the cumulative session usage at that point. `usage` is F04's
    /// running rollup, not the per-turn delta.
    Summary {
        message_count: u64,
        usage: TokenSnapshot,
    },
}

// ============================================================================
// Memory entry types (Decision 03)
// ============================================================================

/// A single curated fact in working memory (Tier 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryFact {
    pub fact: String,
    pub asserted_at: SystemTime,
    pub source_message_id: Id,
    pub confidence: f32,
}

/// A single structured observation (Tier 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationEntry {
    #[serde(rename = "type")]
    pub kind: ObservationKind,
    pub content: String,
    pub timestamp: SystemTime,
    pub source_message_id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Whether an observation is an assertion or an open question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Assertion,
    Question,
}

/// A condensed reflection over observations (Tier 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionEntry {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
    pub observation_refs: Vec<Id>,
    pub importance: f32,
}

/// A `{from, to}` time window (matches Decision 03's object shape, not a 2-tuple).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: SystemTime,
    pub to: SystemTime,
}

fn fresh_record_id() -> Id {
    foundation_compact::ids::new_scru128()
}

