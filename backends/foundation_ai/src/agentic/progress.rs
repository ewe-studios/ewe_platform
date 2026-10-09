//! Agent stream & progress contract (Feature 03).
//!
//! WHY: The agent loop must stream its work without inventing a parallel,
//! content-duplicating event taxonomy. The model layer already got this right —
//! it streams `Stream<Messages, ModelState>`: the rich `Messages` ride on
//! `Stream::Next`, thin status on `Stream::Pending`. The agent loop mirrors this
//! one level up: rich [`SessionRecord`]s on `Next`, thin [`AgentProgress`] on
//! `Pending`.
//!
//! WHAT: [`AgentProgress`] (lifecycle/progress status, NO content),
//! [`MemoryKind`], the [`AgentStream`] type alias, `From<ModelState>` for
//! `AgentProgress`, and [`lift_model_item`] — the helper that maps a model
//! `Stream<Messages, ModelState>` item up to an agent
//! `Stream<SessionRecord, AgentProgress>` item.
//!
//! HOW: errors travel as records, not `Result`s — a model error becomes
//! `Stream::Next(SessionRecord::FailedAction { .. })`. The "expect-next" protocol
//! (each `Pending` hints at a forthcoming `Next`) is **advisory**: the executor
//! freely interleaves `Ignore`/`Wait`/`Delayed` and may `Spread` multiple `Next`,
//! so consumers must key off each `Next`'s content, not item adjacency.

use std::borrow::Cow;

use foundation_core::valtron::Stream;
use serde::{Deserialize, Serialize};

use crate::agentic::errors::AgenticError;
use crate::types::{Messages, ModelId, ModelState, SessionRecord, UsageReport};

/// The stream a consumer of the agent loop observes.
///
/// `D = SessionRecord` (pure — conversation, memory, AND `FailedAction` on one
/// stream; no `Result` wrapper). `P = AgentProgress` (thin status). A
/// conversation-only consumer matches `SessionRecord::Conversation { message }`;
/// errors match `SessionRecord::FailedAction`.
pub type AgentStream = Stream<SessionRecord, AgentProgress>;

/// Lifecycle + progress signal. Carries **no** message content (that rides on
/// `Stream::Next`). Each variant is an **advisory** hint about a forthcoming
/// `Stream::Next` — not a framing guarantee (the executor may interleave
/// `Ignore`/`Wait`/`Delayed`, and `Spread` can multiplex several `Next`s).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentProgress {
    /// Loading context/memory. Hint: recalled records or the first assistant record.
    Initializing { step: Cow<'static, str> },
    /// Model is generating. Hint: `Conversation{Assistant Text|Thinking}`.
    /// `tokens_so_far` is the live in-flight count for this turn (input +
    /// output-so-far); `None` only in the brief pre-first-delta window.
    Generating {
        model: ModelId,
        tokens_so_far: Option<u64>,
    },
    /// Assistant requested a tool. Hint: `Conversation{Assistant content=ToolCall}`.
    ToolCallRequested { name: String },
    /// Tools executing. Hint: `Conversation{ToolResult}` (one per completion).
    ExecutingTools { total: usize, completed: usize },
    /// A tool call was cancelled by steering (so `ExecutingTools` consumers don't deadlock).
    ToolCallCancelled { id: String },
    /// Memory generation running. Hint: `Observation|Reflection|WorkingMemory` record.
    ProcessingMemory { kind: MemoryKind },
    /// Flushing buffered records to storage; no `Next` follows from this.
    FlushingRecords { count: usize },
    /// Steering/interruption being applied. Hint: `Conversation{User role=System|Agent}`.
    Steering { source: Cow<'static, str> },
    /// A model interaction / agent turn completed — carries the turn's usage stats
    /// (cloned from the model's `UsageReport`) so consumers see per-turn token/cost
    /// without re-summing. Mirrors the `SessionRecord::Summary` `Next` record.
    TurnComplete { usage: UsageReport },
    /// Turn/loop is ending; the final summary record is emitted just before stream end.
    SessionEnding,
}

/// Which memory tier is being generated (mirror of `SessionRecord`'s memory variants).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MemoryKind {
    Working,
    Observation,
    Reflection,
}

impl From<&ModelState> for AgentProgress {
    /// Lift a model status into an agent progress signal. `Error` is intentionally
    /// NOT mapped here — a model error becomes a `SessionRecord::FailedAction`
    /// record on the `Next` channel, not a `Pending` status (see [`lift_model_item`]).
    fn from(state: &ModelState) -> Self {
        match state {
            ModelState::GeneratingTokens(usage) => AgentProgress::Generating {
                // We don't have the ModelId here; the loop fills it. Use a neutral
                // placeholder so the conversion is total. Callers that know the
                // model should construct `Generating` directly.
                model: ModelId::Name(String::new(), None),
                // Token counts are non-negative and within u64 range.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                tokens_so_far: usage.as_ref().map(|u| (u.input + u.output) as u64),
            },
            ModelState::GeneratingEmbeddings => AgentProgress::Initializing {
                step: Cow::Borrowed("generating_embeddings"),
            },
            ModelState::Finished => AgentProgress::SessionEnding,
            // Error has no Pending representation — see lift_model_item.
            ModelState::Error(msg) => AgentProgress::Initializing {
                step: Cow::Owned(format!("error: {msg}")),
            },
        }
    }
}

/// Lift one model-stream item (`Stream<Messages, ModelState>`) up to one
/// agent-stream item (`Stream<SessionRecord, AgentProgress>`).
///
/// Mapping (F03 §"Relationship to the model stream"):
/// - `Next(Messages)` → `Next(SessionRecord::Conversation { message })`
/// - `Pending(ModelState::Error(s))` → `Next(SessionRecord::FailedAction { .. })`
///   (an error becomes a **record**, not a status)
/// - `Pending(ModelState::GeneratingTokens(_))` → `Pending(AgentProgress::Generating{..})`
/// - other `Pending(state)` → `Pending(AgentProgress::from(state))`
/// - control variants (`Init`/`Ignore`/`Wait`/`Delayed`) pass through unchanged
/// - `Spread` is forwarded by lifting each element
///
/// `model` is the id to stamp on `Generating` progress (the model layer doesn't
/// carry it in `ModelState`).
#[must_use]
pub fn lift_model_item(item: Stream<Messages, ModelState>, model: &ModelId) -> AgentStream {
    match item {
        Stream::Next(message) => Stream::Next(SessionRecord::Conversation { message }),
        Stream::Pending(ModelState::Error(msg)) => {
            // Model error → a FailedAction record on the Next channel.
            Stream::Next(AgenticError::Unexpected(msg).into_failed_action())
        }
        Stream::Pending(ModelState::GeneratingTokens(usage)) => {
            Stream::Pending(AgentProgress::Generating {
                model: model.clone(),
                // Token counts are non-negative and within u64 range.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                tokens_so_far: usage.as_ref().map(|u| (u.input + u.output) as u64),
            })
        }
        Stream::Pending(state) => Stream::Pending(AgentProgress::from(&state)),
        Stream::Init => Stream::Init,
        Stream::Ignore => Stream::Ignore,
        Stream::Wait => Stream::Wait,
        Stream::Delayed(d) => Stream::Delayed(d),
        Stream::Spread(items) => {
            // Lift each spread element; Spread carries StreamSpread<D,P> = Done(D)|Pending(P).
            use foundation_core::valtron::StreamSpread;
            let lifted = items
                .into_iter()
                .map(|s| match s {
                    StreamSpread::Done(message) => {
                        StreamSpread::Done(SessionRecord::Conversation { message })
                    }
                    StreamSpread::Pending(state) => {
                        StreamSpread::Pending(AgentProgress::from(&state))
                    }
                })
                .collect();
            Stream::Spread(lifted)
        }
    }
}

