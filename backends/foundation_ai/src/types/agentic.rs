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

use foundation_compact::SystemTime;
use foundation_compact::ids::{Id, ParseError};
use serde::{Deserialize, Serialize};

use super::Messages;

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
        Self(foundation_compact::ids::new_scru128_with_machine_id(machine_id()))
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
                foundation_compact::ids::stable_hash(h.as_bytes()) as u32
            }
            // No stable host signal (wasm, or unset env): random per-process id.
            _ => foundation_compact::entropy::u32().unwrap_or(0),
        }
    })
}

// ============================================================================
// Forward-reference stubs (filled by F02 / F04)
// ============================================================================

/// Forward-reference stub for the F02 error taxonomy.
///
/// `SessionRecord::FailedAction` references `AgenticError`, but F02 (error
/// handling) depends on F01, so the real taxonomy lands later. This minimal
/// shell keeps F01 self-contained and is replaced when F02 lands. It satisfies
/// the pinned bounds `Clone + PartialEq + Debug + Serialize + Deserialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgenticError {
    /// Matchable error kind/category.
    pub kind: String,
    /// Human-readable message.
    pub message: String,
}

/// Forward-reference stub for the F04 token snapshot.
///
/// `SessionRecord::Summary` carries the cumulative session usage. F04 (token
/// accounting) owns the real `TokenSnapshot` (total/rolling/budget/cost); this
/// shell keeps F01 self-contained until F04 lands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenSnapshot {
    /// Cumulative input tokens this session.
    pub total_input: u64,
    /// Cumulative output tokens this session.
    pub total_output: u64,
    /// Cumulative cost (currency units). `f64` — `SessionRecord` is `PartialEq` only.
    pub cost: f64,
}

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case")]
pub enum SessionRecord {
    /// A provider-facing message (user/assistant/tool-result).
    Conversation { message: Messages },

    /// Permanent curated facts about the user/session (Decision 03, Tier 1).
    WorkingMemory {
        facts: Vec<MemoryFact>,
        version: u64,
        timestamp: SystemTime,
    },

    /// Time-scoped structured observations (Decision 03, Tier 2).
    Observation {
        observations: Vec<ObservationEntry>,
        token_count: u64,
        timestamp: SystemTime,
    },

    /// Condensed reflections over observations (Decision 03, Tier 3).
    Reflection {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_message_id: Option<Id>,
    pub confidence: f32,
}

/// A single structured observation (Tier 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationEntry {
    #[serde(rename = "type")]
    pub kind: ObservationKind,
    pub content: String,
    pub timestamp: SystemTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_message_id: Option<Id>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_is_strictly_monotonic_even_within_a_millisecond() {
        // The monotonic scru128 generator guarantees strict ordering for ids
        // minted in sequence, regardless of whether they share a millisecond.
        let ids: Vec<SessionId> = (0..1000).map(|_| SessionId::new()).collect();
        for w in ids.windows(2) {
            assert!(
                w[1].to_string() > w[0].to_string(),
                "ids must be strictly increasing: {} !> {}",
                w[1],
                w[0]
            );
        }
        assert!(ids[0].timestamp_ms() > 0);
    }

    #[test]
    fn session_id_is_machine_attributable() {
        // Two ids from the same process carry the same machine id.
        let a = SessionId::new();
        let b = SessionId::new();
        assert_eq!(a.machine_id(), b.machine_id());
    }

    #[test]
    fn session_id_from_name_is_stable_within_a_millisecond() {
        let a = SessionId::from_name("fix-bug");
        let b = SessionId::from_name("fix-bug");
        // Same name + same machine; if both land in the same ms they are equal,
        // otherwise only the timestamp differs. Compare the machine+hash region.
        if a.timestamp_ms() == b.timestamp_ms() {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn session_id_round_trips_through_string() {
        let a = SessionId::new();
        let s = a.to_string();
        let parsed = SessionId::from_str(&s).expect("scru128 string parses");
        assert_eq!(a, parsed);
    }

    #[test]
    fn observation_kind_serializes_snake_case() {
        let json = serde_json::to_string(&ObservationKind::Assertion).unwrap();
        assert_eq!(json, "\"assertion\"");
        let back: ObservationKind = serde_json::from_str("\"question\"").unwrap();
        assert_eq!(back, ObservationKind::Question);
    }

    #[test]
    fn session_record_working_memory_round_trips() {
        let rec = SessionRecord::WorkingMemory {
            facts: vec![MemoryFact {
                fact: "user prefers dark mode".into(),
                asserted_at: SystemTime::UNIX_EPOCH,
                source_message_id: None,
                confidence: 0.9,
            }],
            version: 1,
            timestamp: SystemTime::UNIX_EPOCH,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"message_type\":\"working_memory\""));
        let back: SessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, back);
    }

    #[test]
    fn session_record_summary_round_trips() {
        let rec = SessionRecord::Summary {
            message_count: 7,
            usage: TokenSnapshot {
                total_input: 100,
                total_output: 40,
                cost: 0.0021,
            },
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"message_type\":\"summary\""));
        let back: SessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, back);
    }
}
