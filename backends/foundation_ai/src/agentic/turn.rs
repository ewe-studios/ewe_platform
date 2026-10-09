//! Turn results — `Turn`, `Answer`, `TurnStream` and `TurnEvent` (Proposal 15,
//! items 7 and 8).
//!
//! WHY: Reading an answer used to mean matching `SessionRecord` →
//! `Messages::Assistant` → `ModelOutput::Text`, and streaming consumers had to
//! handle valtron's scheduling states and undo printed text on `Retracted`
//! themselves. A turn that failed part-way lost everything it had produced.
//!
//! WHAT: [`Turn`] wraps the collected records (`Deref` to `Vec<SessionRecord>`,
//! so existing code keeps working) plus how the turn ended ([`TurnOutcome`]).
//! [`Answer`] is the text-only result of `AgentSession::ask`. [`TurnStream`]
//! is the raw stream iterator plus [`TurnStream::events`], which yields
//! [`TurnEvent`]s.
//!
//! HOW: all three follow the same order — the output produced before a failure
//! first, then the error that ended the turn. `Err` from the session methods
//! means the turn never started.

use std::collections::{HashMap, VecDeque};

use foundation_core::valtron::{DrivenStreamIterator, Stream, StreamSpread};
use foundation_db::traits::DocumentStore;
use foundation_errstacks::{ErrorTrace, StructuredErrorTrace};

use crate::agentic::agent_loop::AgentLoop;
use crate::agentic::errors::AgenticError;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::progress::AgentProgress;
use crate::agentic::token_ledger::TokenSnapshot;
use crate::types::{ArgType, Messages, ModelOutput, SessionRecord, UserModelContent};

// ---------------------------------------------------------------------------
// Flattening the raw stream
// ---------------------------------------------------------------------------

/// One item of the raw stream, without valtron's scheduling states. The
/// record is boxed: `SessionRecord` is much larger than `AgentProgress`.
pub(crate) enum TurnItem {
    Record(Box<SessionRecord>),
    Progress(AgentProgress),
}

/// The records and progress a raw stream item carries, in order.
pub(crate) fn flatten(item: Stream<SessionRecord, AgentProgress>) -> Vec<TurnItem> {
    match item {
        Stream::Next(record) => vec![TurnItem::Record(Box::new(record))],
        Stream::Pending(progress) => vec![TurnItem::Progress(progress)],
        Stream::Spread(items) => items
            .into_iter()
            .map(|s| match s {
                StreamSpread::Done(record) => TurnItem::Record(Box::new(record)),
                StreamSpread::Pending(progress) => TurnItem::Progress(progress),
            })
            .collect(),
        Stream::Init | Stream::Ignore | Stream::Wait | Stream::Delayed(_) => Vec::new(),
    }
}

fn is_assistant(record: &SessionRecord) -> bool {
    matches!(
        record,
        SessionRecord::Conversation {
            message: Messages::Assistant { .. }
        }
    )
}

// ---------------------------------------------------------------------------
// Turn
// ---------------------------------------------------------------------------

/// How a turn ended.
///
/// On failure the records produced before it are kept, in order, and the
/// error comes after them: partial output first, then the error that ended
/// the turn.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnOutcome {
    /// The turn ran to the end.
    Completed,
    /// A `FailedAction` ended the turn.
    Failed {
        error: AgenticError,
        trace: StructuredErrorTrace,
    },
}

/// The records one turn produced, and how it ended.
///
/// Derefs to `Vec<SessionRecord>` and iterates like one, so code written
/// against the old `Vec<SessionRecord>` return value keeps working. Output the
/// loop withdrew (`SessionRecord::Retracted`) is already dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    records: Vec<SessionRecord>,
    outcome: TurnOutcome,
}

impl Turn {
    /// Collect a turn from the raw stream: drop withdrawn assistant output,
    /// and stop at the first `FailedAction`, keeping what came before it.
    pub(crate) fn collect<I>(stream: I) -> Self
    where
        I: Iterator<Item = Stream<SessionRecord, AgentProgress>>,
    {
        let mut records = Vec::new();
        for item in stream {
            for flat in flatten(item) {
                let TurnItem::Record(record) = flat else {
                    continue;
                };
                match *record {
                    SessionRecord::FailedAction { error, trace } => {
                        return Self {
                            records,
                            outcome: TurnOutcome::Failed { error, trace },
                        };
                    }
                    // The loop withdrew the turn it had already streamed. Drop
                    // the assistant messages collected so far and keep whatever
                    // the retry produces; user turns and tool results stand.
                    SessionRecord::Retracted { reason, .. } => {
                        tracing::debug!(%reason, "run_turn: dropping a withdrawn turn");
                        records.retain(|kept| !is_assistant(kept));
                    }
                    other => records.push(other),
                }
            }
        }
        Self {
            records,
            outcome: TurnOutcome::Completed,
        }
    }

    /// Concatenated `ModelOutput::Text` of the assistant messages.
    #[must_use]
    pub fn text(&self) -> String {
        self.records.iter().map(Self::text_of).collect()
    }

    /// The assistant text in one record (`""` for anything else); `text()`
    /// joins these.
    #[must_use]
    pub fn text_of(record: &SessionRecord) -> String {
        match record {
            SessionRecord::Conversation {
                message:
                    Messages::Assistant {
                        content: ModelOutput::Text(t),
                        ..
                    },
            } => t.content.clone(),
            _ => String::new(),
        }
    }

    /// Each `ModelOutput::ToolCall` the model made: `(id, name, arguments)`.
    pub fn tool_calls(
        &self,
    ) -> impl Iterator<Item = (&str, &str, Option<&HashMap<String, ArgType>>)> {
        self.records.iter().filter_map(|record| match record {
            SessionRecord::Conversation {
                message:
                    Messages::Assistant {
                        content:
                            ModelOutput::ToolCall {
                                id,
                                name,
                                arguments,
                                ..
                            },
                        ..
                    },
            } => Some((id.as_str(), name.as_str(), arguments.as_ref())),
            _ => None,
        })
    }

    /// Each `Messages::ToolResult` produced this turn.
    pub fn tool_results(&self) -> impl Iterator<Item = &Messages> {
        self.records.iter().filter_map(|record| match record {
            SessionRecord::Conversation {
                message: message @ Messages::ToolResult { .. },
            } => Some(message),
            _ => None,
        })
    }

    /// The `SessionRecord::Summary` usage, if the turn reached it.
    #[must_use]
    pub fn usage(&self) -> Option<&TokenSnapshot> {
        self.records.iter().rev().find_map(|record| match record {
            SessionRecord::Summary { usage, .. } => Some(usage),
            _ => None,
        })
    }

    /// The records, without the outcome.
    #[must_use]
    pub fn into_records(self) -> Vec<SessionRecord> {
        self.records
    }

    /// `Some` when a `FailedAction` ended the turn; the records before it are
    /// kept.
    #[must_use]
    pub fn failure(&self) -> Option<&AgenticError> {
        match &self.outcome {
            TurnOutcome::Completed => None,
            TurnOutcome::Failed { error, .. } => Some(error),
        }
    }

    /// How the turn ended.
    #[must_use]
    pub fn outcome(&self) -> &TurnOutcome {
        &self.outcome
    }

    /// For callers that only want a completed turn: a failure becomes `Err`.
    ///
    /// # Errors
    /// The error that ended the turn.
    pub fn into_result(self) -> Result<Vec<SessionRecord>, ErrorTrace<AgenticError>> {
        match self.outcome {
            TurnOutcome::Completed => Ok(self.records),
            TurnOutcome::Failed { error, .. } => Err(ErrorTrace::new(error)),
        }
    }
}

impl std::ops::Deref for Turn {
    type Target = Vec<SessionRecord>;

    fn deref(&self) -> &Self::Target {
        &self.records
    }
}

impl IntoIterator for Turn {
    type Item = SessionRecord;
    type IntoIter = std::vec::IntoIter<SessionRecord>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.into_iter()
    }
}

impl<'a> IntoIterator for &'a Turn {
    type Item = &'a SessionRecord;
    type IntoIter = std::slice::Iter<'a, SessionRecord>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.iter()
    }
}

// ---------------------------------------------------------------------------
// Answer
// ---------------------------------------------------------------------------

/// The outcome of `AgentSession::ask`.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The turn finished; all assistant text.
    Complete(String),
    /// The turn hit a `FailedAction` after producing `partial_text`.
    Failed {
        partial_text: String,
        error: AgenticError,
        trace: StructuredErrorTrace,
    },
}

impl Answer {
    /// Collect the assistant text of a turn from the raw stream.
    pub(crate) fn collect<I>(stream: I) -> Self
    where
        I: Iterator<Item = Stream<SessionRecord, AgentProgress>>,
    {
        let mut text = String::new();
        for item in stream {
            for flat in flatten(item) {
                let TurnItem::Record(record) = flat else {
                    continue;
                };
                match *record {
                    SessionRecord::FailedAction { error, trace } => {
                        return Answer::Failed {
                            partial_text: text,
                            error,
                            trace,
                        };
                    }
                    // Withdrawn output: the retry replaces it.
                    SessionRecord::Retracted { .. } => text.clear(),
                    record => text.push_str(&Turn::text_of(&record)),
                }
            }
        }
        Answer::Complete(text)
    }

    /// The text either way: complete, or what was produced before the failure.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Answer::Complete(text) => text,
            Answer::Failed { partial_text, .. } => partial_text,
        }
    }

    /// True when the turn finished.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        matches!(self, Answer::Complete(_))
    }

    /// For callers that only want success: `Failed` becomes `Err`.
    ///
    /// # Errors
    /// The error that ended the turn.
    pub fn into_result(self) -> Result<String, ErrorTrace<AgenticError>> {
        match self {
            Answer::Complete(text) => Ok(text),
            Answer::Failed { error, .. } => Err(ErrorTrace::new(error)),
        }
    }
}

// ---------------------------------------------------------------------------
// TurnStream + TurnEvent
// ---------------------------------------------------------------------------

/// A summary of a completed turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnSummary {
    /// Records the interaction produced.
    pub message_count: u64,
    /// Cumulative session usage at the end of the turn.
    pub usage: TokenSnapshot,
}

/// What happened during a turn, as `TurnStream::events` reports it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TurnEvent {
    /// Assistant text as the record carried it (a delta for streaming
    /// providers).
    Text(String),
    /// Model reasoning, for providers that expose it.
    Thinking(String),
    /// The model asked for a tool.
    ToolCall {
        id: String,
        name: String,
        arguments: Option<HashMap<String, ArgType>>,
    },
    /// A tool's result, as the model will see it.
    ToolResult {
        tool_call_id: String,
        name: String,
        content: UserModelContent,
    },
    /// Drop the assistant text shown for this turn; the retry follows.
    Retract { reason: String },
    /// A progress hint from the loop.
    Progress(AgentProgress),
    /// Terminal: the events before it (the partial output) stand, and this
    /// ends the turn. No `Done` follows.
    Failed(AgenticError),
    /// Terminal: the turn completed.
    Done(TurnSummary),
}

impl TurnEvent {
    /// The events one record maps to. Memory records (`WorkingMemory`,
    /// `Observation`, `Reflection`) and user messages map to none; they stay
    /// visible on the raw iterator.
    fn from_record(record: SessionRecord) -> Option<Self> {
        match record {
            SessionRecord::Conversation { message } => match message {
                Messages::Assistant { content, .. } => match content {
                    ModelOutput::Text(t) => Some(TurnEvent::Text(t.content)),
                    ModelOutput::ThinkingContent { thinking, .. } => {
                        Some(TurnEvent::Thinking(thinking))
                    }
                    ModelOutput::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => Some(TurnEvent::ToolCall {
                        id,
                        name,
                        arguments,
                    }),
                    ModelOutput::Image(_) | ModelOutput::Embedding { .. } => None,
                },
                Messages::ToolResult {
                    tool_call_id,
                    name,
                    content,
                    ..
                } => Some(TurnEvent::ToolResult {
                    tool_call_id,
                    name,
                    content,
                }),
                Messages::User { .. } => None,
            },
            SessionRecord::Retracted { reason, .. } => Some(TurnEvent::Retract { reason }),
            SessionRecord::FailedAction { error, .. } => Some(TurnEvent::Failed(error)),
            SessionRecord::Summary {
                message_count,
                usage,
            } => Some(TurnEvent::Done(TurnSummary {
                message_count,
                usage,
            })),
            SessionRecord::WorkingMemory { .. }
            | SessionRecord::Observation { .. }
            | SessionRecord::Reflection { .. } => None,
        }
    }

    /// True for `Failed` and `Done`.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, TurnEvent::Failed(_) | TurnEvent::Done(_))
    }
}

/// A running turn: the raw stream iterator, plus [`events`](Self::events).
pub struct TurnStream<D, M>(DrivenStreamIterator<AgentLoop<D, M>>)
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static;

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> TurnStream<D, M> {
    pub(crate) fn new(inner: DrivenStreamIterator<AgentLoop<D, M>>) -> Self {
        Self(inner)
    }

    /// True when no item is ready right now (polling without blocking).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// True once the turn has finished producing items.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// The underlying valtron iterator.
    #[must_use]
    pub fn into_inner(self) -> DrivenStreamIterator<AgentLoop<D, M>> {
        self.0
    }

    /// The turn as [`TurnEvent`]s: text, tool calls and results, retractions
    /// and progress as they happen, then one terminal `Done` or `Failed`.
    #[must_use]
    pub fn events(self) -> TurnEvents<D, M> {
        TurnEvents {
            stream: self,
            pending: VecDeque::new(),
            finished: false,
        }
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> Iterator for TurnStream<D, M> {
    type Item = Stream<SessionRecord, AgentProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// The iterator [`TurnStream::events`] returns.
pub struct TurnEvents<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    stream: TurnStream<D, M>,
    pending: VecDeque<TurnEvent>,
    finished: bool,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> Iterator for TurnEvents<D, M> {
    type Item = TurnEvent;

    fn next(&mut self) -> Option<TurnEvent> {
        loop {
            if self.finished {
                return None;
            }
            if let Some(event) = self.pending.pop_front() {
                if event.is_terminal() {
                    // Nothing follows a terminal event.
                    self.finished = true;
                    self.pending.clear();
                }
                return Some(event);
            }
            let item = self.stream.next()?;
            for flat in flatten(item) {
                let event = match flat {
                    TurnItem::Record(record) => TurnEvent::from_record(*record),
                    TurnItem::Progress(progress) => Some(TurnEvent::Progress(progress)),
                };
                self.pending.extend(event);
            }
        }
    }
}
