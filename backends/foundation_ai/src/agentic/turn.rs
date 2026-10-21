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
//! is the raw valtron stream (for composing inside valtron tasks) plus its
//! async views: [`TurnStream::events`] ([`TurnEvent`]s) and
//! [`TurnStream::into_text_stream`] (new text only).
//!
//! HOW: async by default. Every consumer view waits by parking on the model
//! stream, so the same code runs natively and on wasm (where a synchronous
//! drain would spin and starve the `fetch` it waits on). Synchronous callers
//! wrap them with valtron: `block_on_future(session.ask(..))`,
//! `execute(from_stream(stream.events()))`. All views follow the same order —
//! the output produced before a failure first, then the error that ended the
//! turn. `Err` from the session methods means the turn never started.

use std::collections::VecDeque;

use foundation_core::valtron::{DrivenStreamIterator, Stream, StreamSpread};
use foundation_db::traits::DocumentStore;
use foundation_errstacks::{ErrorTrace, StructuredErrorTrace};

use crate::agentic::agent_loop::AgentLoop;
use crate::agentic::errors::AgenticError;
use crate::agentic::memory_store::MemoryStore;
use crate::agentic::progress::AgentProgress;
use crate::agentic::token_ledger::TokenSnapshot;
use crate::types::{Messages, ModelOutput, SessionRecord, ToolArguments, UserModelContent};

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

/// The raw turn as an async stream: every `Stream<SessionRecord,
/// AgentProgress>` item, parking (not polling) while the loop waits.
pub(crate) type RawTurnItems<D, M> = foundation_compact::SendWrapper<
    foundation_core::valtron::StreamParkingFutureStream<DrivenStreamIterator<AgentLoop<D, M>>>,
>;

/// The next raw item of `items`.
async fn next_item<D, M>(items: &mut RawTurnItems<D, M>) -> Option<Stream<SessionRecord, AgentProgress>>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    std::future::poll_fn(|cx| futures_lite::Stream::poll_next(std::pin::Pin::new(&mut **items), cx)).await
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
    pub(crate) async fn collect<D, M>(stream: TurnStream<D, M>) -> Self
    where
        D: DocumentStore + 'static,
        M: MemoryStore + 'static,
    {
        let mut items = stream.into_raw_items();
        let mut records = Vec::new();
        while let Some(item) = next_item(&mut items).await {
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

    /// The assistant text of the turn.
    ///
    /// Each run of consecutive streamed text records is joined the way the
    /// agent loop joins them: snapshot chunks (each re-sends the text so far, as
    /// the HTTP backends do) collapse to the last one, delta chunks concatenate.
    #[must_use]
    pub fn text(&self) -> String {
        Self::text_of_records(&self.records)
    }

    /// The assistant text of a turn's records, joined like [`Turn::text`].
    ///
    /// For callers that drive the record stream themselves (e.g. a Worker that
    /// awaits `into_collect_future`) and so have the records but no `Turn`.
    #[must_use]
    pub fn text_of_records<'a>(records: impl IntoIterator<Item = &'a SessionRecord>) -> String {
        let mut text = TurnText::default();
        for record in records {
            text.push(record);
        }
        text.finish()
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
    pub fn tool_calls(&self) -> impl Iterator<Item = (&str, &str, Option<&ToolArguments>)> {
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

/// Builds a turn's assistant text from its streamed records.
///
/// Consecutive assistant text records form one run (one generation's chunks);
/// any other record ends the run. Each run is joined with the agent loop's
/// rule ([`join_text_chunks`]): snapshots collapse to the last, deltas
/// concatenate.
///
/// [`join_text_chunks`]: crate::agentic::agent_loop::join_text_chunks
#[derive(Default)]
struct TurnText {
    done: String,
    run: Vec<String>,
}

impl TurnText {
    fn push(&mut self, record: &SessionRecord) {
        match record {
            SessionRecord::Conversation {
                message:
                    Messages::Assistant {
                        content: ModelOutput::Text(t),
                        ..
                    },
            } => self.run.push(t.content.clone()),
            _ => self.flush(),
        }
    }

    fn flush(&mut self) {
        let pieces: Vec<&str> = self.run.iter().map(String::as_str).collect();
        self.done
            .push_str(&crate::agentic::agent_loop::join_text_chunks(&pieces));
        self.run.clear();
    }

    fn clear(&mut self) {
        self.done.clear();
        self.run.clear();
    }

    fn finish(mut self) -> String {
        self.flush();
        self.done
    }
}

impl Answer {
    /// Collect the assistant text of a turn from the raw stream.
    pub(crate) async fn collect<D, M>(stream: TurnStream<D, M>) -> Self
    where
        D: DocumentStore + 'static,
        M: MemoryStore + 'static,
    {
        let mut items = stream.into_raw_items();
        let mut text = TurnText::default();
        while let Some(item) = next_item(&mut items).await {
            for flat in flatten(item) {
                let TurnItem::Record(record) = flat else {
                    continue;
                };
                match *record {
                    SessionRecord::FailedAction { error, trace } => {
                        return Answer::Failed {
                            partial_text: text.finish(),
                            error,
                            trace,
                        };
                    }
                    // Withdrawn output: the retry replaces it.
                    SessionRecord::Retracted { .. } => text.clear(),
                    record => text.push(&record),
                }
            }
        }
        Answer::Complete(text.finish())
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
        arguments: Option<ToolArguments>,
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

/// A running turn: the raw valtron stream, plus its async views.
///
/// - **Consume it async** (every target, wasm included):
///   [`events`](Self::events) for [`TurnEvent`]s,
///   [`into_text_stream`](Self::into_text_stream) for new text only, or
///   `AgentSession::run_turn` / `ask` for the collected result.
/// - **Compose it inside valtron**: it is the raw stream. It implements
///   `Iterator<Item = Stream<SessionRecord, AgentProgress>>`, so valtron's
///   blanket impl makes it a [`StreamIterator`](foundation_core::valtron::StreamIterator)
///   with `D = SessionRecord`, `P = AgentProgress`, and every
///   [`StreamIteratorExt`](foundation_core::valtron::StreamIteratorExt)
///   combinator applies. A task that pulls it must park on `Stream::Wait`
///   (`valtron::park_on`), never loop on it — a synchronous drain spins
///   forever on wasm.
/// - [`inner`](Self::inner) / [`into_inner`](Self::into_inner) hand over the
///   underlying [`DrivenStreamIterator`].
///
/// ```ignore
/// use futures_lite::StreamExt;
///
/// let mut events = agent.run_turn_stream("Hi")?.events();
/// while let Some(event) = events.next().await {
///     if let TurnEvent::Text(text) = event { print!("{text}"); }
/// }
///
/// // Synchronous code: the same future, driven by valtron.
/// let answer = foundation_core::valtron::block_on_future(agent.ask("Again"))?;
/// ```
pub struct TurnStream<D, M>(foundation_compact::SendWrapper<DrivenStreamIterator<AgentLoop<D, M>>>)
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static;

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> TurnStream<D, M> {
    pub(crate) fn new(inner: DrivenStreamIterator<AgentLoop<D, M>>) -> Self {
        Self(foundation_compact::SendWrapper::new(inner))
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
        self.0.into_inner()
    }

    /// Borrow the underlying valtron iterator.
    #[must_use]
    pub fn inner(&self) -> &DrivenStreamIterator<AgentLoop<D, M>> {
        &self.0
    }

    /// Mutably borrow the underlying valtron iterator — e.g. to drive it
    /// directly and keep the `TurnStream` afterwards.
    pub fn inner_mut(&mut self) -> &mut DrivenStreamIterator<AgentLoop<D, M>> {
        &mut self.0
    }

    /// The raw items as an async stream that parks while the loop waits.
    pub(crate) fn into_raw_items(self) -> RawTurnItems<D, M> {
        use foundation_core::valtron::StreamIteratorExt;
        foundation_compact::SendWrapper::new(self.into_inner().into_future_stream())
    }

    /// The turn as an async stream of [`TurnEvent`]s: text, tool calls and
    /// results, retractions and progress as they happen, then one terminal
    /// `Done` or `Failed`. Waiting parks on the model stream, so it works the
    /// same on every target (a Worker included). A synchronous caller drives
    /// it with valtron (`execute(from_stream(events))`, or
    /// `block_on_future` around a collecting future).
    #[must_use]
    pub fn events(self) -> TurnEvents<D, M> {
        TurnEvents {
            items: self.into_raw_items(),
            pending: VecDeque::new(),
            finished: false,
        }
    }

    /// The assistant's text as an async stream of the **new** text only —
    /// what a chat UI appends.
    ///
    /// Each chunk follows the turn's text rule ([`Turn::text`]): a chunk that
    /// extends the text shown so far is a snapshot (the HTTP backends) and
    /// only its new suffix is yielded; any other chunk is a delta (the local
    /// backends) and is yielded whole. Empty chunks are skipped. The stream
    /// ends with the turn (completed or failed).
    #[must_use]
    pub fn into_text_stream(self) -> TurnTextStream<D, M> {
        TurnTextStream {
            events: self.events(),
            shown: String::new(),
        }
    }
}

/// See [`TurnStream::into_text_stream`].
pub struct TurnTextStream<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    events: TurnEvents<D, M>,
    /// The text of the current run of text chunks shown so far (reset by any
    /// other output, e.g. a tool call, as [`TurnText`] does).
    shown: String,
}

impl<D, M> TurnTextStream<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    /// The new text `event` adds, if any.
    fn new_text(&mut self, event: TurnEvent) -> Option<String> {
        match event {
            TurnEvent::Text(chunk) => {
                let new = if !self.shown.is_empty() && chunk.starts_with(self.shown.as_str()) {
                    let suffix = chunk[self.shown.len()..].to_string();
                    self.shown = chunk;
                    suffix
                } else {
                    self.shown.push_str(&chunk);
                    chunk
                };
                (!new.is_empty()).then_some(new)
            }
            TurnEvent::Progress(_) | TurnEvent::Thinking(_) => None,
            _ => {
                self.shown.clear();
                None
            }
        }
    }
}

impl<D, M> futures_lite::Stream for TurnTextStream<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    type Item = String;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<String>> {
        let this = self.get_mut();
        loop {
            match std::pin::Pin::new(&mut this.events).poll_next(cx) {
                std::task::Poll::Ready(Some(event)) => {
                    if let Some(text) = this.new_text(event) {
                        return std::task::Poll::Ready(Some(text));
                    }
                }
                std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> Iterator for TurnStream<D, M> {
    type Item = Stream<SessionRecord, AgentProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> From<TurnStream<D, M>>
    for DrivenStreamIterator<AgentLoop<D, M>>
{
    fn from(stream: TurnStream<D, M>) -> Self {
        stream.0.into_inner()
    }
}

/// The async stream [`TurnStream::events`] returns.
pub struct TurnEvents<D, M>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    items: RawTurnItems<D, M>,
    pending: VecDeque<TurnEvent>,
    finished: bool,
}

impl<D: DocumentStore + 'static, M: MemoryStore + 'static> futures_lite::Stream
    for TurnEvents<D, M>
{
    type Item = TurnEvent;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<TurnEvent>> {
        use std::task::Poll;
        let this = self.get_mut();
        loop {
            if this.finished {
                return Poll::Ready(None);
            }
            if let Some(event) = this.pending.pop_front() {
                if event.is_terminal() {
                    // Nothing follows a terminal event.
                    this.finished = true;
                    this.pending.clear();
                }
                return Poll::Ready(Some(event));
            }
            match futures_lite::Stream::poll_next(std::pin::Pin::new(&mut *this.items), cx) {
                Poll::Ready(Some(item)) => {
                    for flat in flatten(item) {
                        let event = match flat {
                            TurnItem::Record(record) => TurnEvent::from_record(*record),
                            TurnItem::Progress(progress) => Some(TurnEvent::Progress(progress)),
                        };
                        this.pending.extend(event);
                    }
                }
                Poll::Ready(None) => {
                    this.finished = true;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
