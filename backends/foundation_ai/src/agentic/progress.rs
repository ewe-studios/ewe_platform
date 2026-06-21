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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TextContent, UserModelContent};

    fn user_message() -> Messages {
        Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: crate::types::MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "hi".into(),
                signature: None,
            }),
            signature: None,
        }
    }

    #[test]
    fn agent_progress_serde_round_trips() {
        let progresses = vec![
            AgentProgress::Initializing {
                step: Cow::Borrowed("loading"),
            },
            AgentProgress::ToolCallRequested {
                name: "read".into(),
            },
            AgentProgress::ExecutingTools {
                total: 2,
                completed: 1,
            },
            AgentProgress::ProcessingMemory {
                kind: MemoryKind::Observation,
            },
            AgentProgress::SessionEnding,
        ];
        for p in progresses {
            let json = serde_json::to_string(&p).unwrap();
            let back: AgentProgress = serde_json::from_str(&json).unwrap();
            assert_eq!(p, back);
        }
    }

    #[test]
    fn lift_next_message_becomes_conversation() {
        let model = ModelId::Name("m".into(), None);
        let lifted = lift_model_item(Stream::Next(user_message()), &model);
        match lifted {
            Stream::Next(SessionRecord::Conversation { message }) => {
                assert!(matches!(message, Messages::User { .. }));
            }
            other => panic!("expected Conversation Next, got {other:?}"),
        }
    }

    #[test]
    fn lift_model_error_becomes_failed_action_record() {
        let model = ModelId::Name("m".into(), None);
        let lifted = lift_model_item(Stream::Pending(ModelState::Error("boom".into())), &model);
        match lifted {
            Stream::Next(SessionRecord::FailedAction { error, trace }) => {
                assert_eq!(error, AgenticError::Unexpected("boom".into()));
                // The structured trace's current context is the error's Display.
                assert!(trace.current_context.contains("boom"));
            }
            other => panic!("expected FailedAction Next, got {other:?}"),
        }
    }

    #[test]
    fn lift_generating_tokens_becomes_generating_progress() {
        let model = ModelId::Name("m".into(), None);
        let usage = UsageReport {
            input: 10.0,
            output: 5.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 15.0,
            cost: crate::types::UsageCosting::zero(crate::types::CostStatus::Actual),
        };
        let lifted = lift_model_item(
            Stream::Pending(ModelState::GeneratingTokens(Some(usage))),
            &model,
        );
        match lifted {
            Stream::Pending(AgentProgress::Generating {
                model: m,
                tokens_so_far,
            }) => {
                assert_eq!(m, model);
                assert_eq!(tokens_so_far, Some(15));
            }
            other => panic!("expected Generating progress, got {other:?}"),
        }
    }

    #[test]
    fn lift_control_variants_pass_through() {
        let model = ModelId::Name("m".into(), None);
        assert!(matches!(
            lift_model_item(Stream::Ignore, &model),
            Stream::Ignore
        ));
        assert!(matches!(
            lift_model_item(Stream::Wait, &model),
            Stream::Wait
        ));
        assert!(matches!(
            lift_model_item(Stream::Init, &model),
            Stream::Init
        ));
    }
}
