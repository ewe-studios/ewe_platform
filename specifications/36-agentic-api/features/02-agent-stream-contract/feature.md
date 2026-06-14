---
feature: "Agent Stream & Progress Contract"
description: "The agentic loop's streaming contract — pure SessionRecord on Stream::Next (errors as SessionRecord::FailedAction records, not a Result), thin AgentProgress status on Stream::Pending, mirroring the model layer's Stream<Messages, ModelState>"
status: "pending"
priority: "high"
depends_on: ["01-message-model"]
estimated_effort: "medium"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Feature 02: Agent Stream & Progress Contract

> **Review status (2026-06-14):** reviewed against live valtron + F01. Fixes folded: F01
> `Conversation` is a **struct variant** `{ message }`; the expect-next protocol is **advisory**
> (executor interleaves `Ignore`/`Wait`/`Delayed`; `Spread` multiplexes `Next`); streaming-partials
> decision added; `AgenticError` trait bounds pinned; session-final payload restored; `&'static str`
> → `Cow`. The `agentic` cargo feature is created by 00b (the wasm-surface marker); the agentic
> *code* lives unconditionally in `types/agentic.rs`/`agentic/` (per F01), not behind that feature.

> Implements Decision 11's streaming surface and resolves **TODO #9** ("what happened to the rich
> `Messages`? carry it on the stream, not thin events"). Pure contract + types; the loop that
> *produces* this stream is F27. Defines what every caller of `run_turn_stream` observes.

## WHY: Problem Statement

Decision 11's first draft modelled streaming as a big `AgentEvent` enum with
`MessageStart`/`MessageUpdate`/`MessageEnd`/`ToolCallStart`/… variants that **duplicated** the
content already in `foundation_ai`'s rich `Messages`. That is redundant and lossy.

The model layer already got this right: generation streams as
`StreamIterator<D = Messages, P = ModelState>` (`types/mod.rs:1438`) — the **rich `Messages`** ride
on `Stream::Next`, and thin status (`ModelState::{GeneratingTokens, Finished, Error}`) rides on
`Stream::Pending`. The agentic loop must **mirror** this one level up, not reinvent a parallel event
taxonomy.

Two more constraints:
- **Errors** flow as a **record, not a `Result`** (user, 2026-06-15). Rather than wrap `D` in
  `Result<…, AgenticError>`, add a **`SessionRecord::FailedAction`** variant carrying the failure (the
  concrete `AgenticError` kind + `trace: foundation_errstacks::StructuredErrorTrace`, the structured
  JSON-serializable projection of the errstack chain via `ErrorTrace::to_structured()` — the live
  `ErrorTrace`/`Report` is not `Deserialize` so it cannot sit in the enum; logged via `tracing` at the
  failure site, see F01 OD-1-FA) so it communicates *what failed and why*.
  `FailedAction` is **NOT persisted** to the Message API (a transient signal, not audit content) — so
  the **stream stays pure `SessionRecord`** (`D = SessionRecord`, no `Result` wrapper). Consumers match
  `SessionRecord::FailedAction`; F16 skips it on flush. (Supersedes the earlier CRIT-05 `Result`-in-`D`
  resolution; F01 adds the variant, F30 defines `AgenticError`.)
- **Memory records** (working/observation/reflection) are produced mid-loop and should be observable
  too — they are already modelled by F01's `SessionRecord`.

## WHAT: Solution

### The stream type

```rust
// The agentic loop is a TaskIterator; consumers see a StreamIterator of:
StreamIterator<D = SessionRecord, P = AgentProgress>   // pure SessionRecord — errors are SessionRecord::FailedAction
```

- **`Stream::Next(SessionRecord)`** — the rich record: `Conversation { message: Messages }` for
  assistant/tool-result/user, or `WorkingMemory`/`Observation`/`Reflection` when memory is generated.
  `SessionRecord::Conversation` (a **struct variant** per F01) wraps the exact `Messages` the model
  layer produces.
- **`Stream::Next(SessionRecord::FailedAction)`** — an error record (the `AgenticError` kind + `trace: StructuredErrorTrace`, errstack's serializable chain); NOT a `Result`, NOT persisted. (`AgenticError` is F30; see F01 OD-1-FA.)
- **`Stream::Pending(AgentProgress)`** — a thin status signal: lifecycle + progress only, **no
  content**. It tells the consumer *what kind of `Next` to expect* (the "expect-next" protocol).

> **OD-02-1 — RESOLVED (user):** `D = SessionRecord` (pure — conversation, memory, AND `FailedAction`
> on one stream; no `Result` wrapper). A conversation-only consumer matches
> `SessionRecord::Conversation { message }`; errors match `SessionRecord::FailedAction`.

### `AgentProgress` — thin status signals (the only "events")

```rust
/// Lifecycle + progress. Carries NO message content (that's on Stream::Next). Each variant is an
/// ADVISORY hint about a forthcoming `Stream::Next` (not a framing guarantee — see protocol).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentProgress {
    /// Loading context/memory; hint: recalled records or first assistant record.
    Initializing { step: Cow<'static, str> },
    /// Model is generating; hint: Conversation{Assistant Text|Thinking}.
    /// `tokens_so_far` = None until providers emit partial usage (today they don't — OD-02-4).
    Generating { model: ModelId, tokens_so_far: Option<u64> },
    /// Assistant requested a tool; hint: Conversation{Assistant content=ToolCall}.
    ToolCallRequested { name: String },
    /// Tools executing; hint: Conversation{ToolResult} (one per completion).
    ExecutingTools { total: usize, completed: usize },
    /// A tool call was cancelled by steering (so ExecutingTools consumers don't deadlock).
    ToolCallCancelled { id: String },
    /// Memory generation running; hint: Observation|Reflection|WorkingMemory record.
    ProcessingMemory { kind: MemoryKind },
    /// Flushing buffered records to storage; no Next follows from this.
    FlushingRecords { count: usize },
    /// Steering/interruption being applied; hint: Conversation{User role=System|Agent}.
    Steering { source: Cow<'static, str> },
    /// A model interaction / agent turn completed — carries the turn's usage stats (cloned from the
    /// model's UsageReport) so consumers see per-turn token/cost without re-summing (user, 2026-06-15).
    TurnComplete { usage: UsageReport },
    /// Turn/loop is ending; the final summary record is emitted just before stream end.
    SessionEnding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MemoryKind { Working, Observation, Reflection }
```

> **Summary + usage payload — RESOLVED (user, 2026-06-15):** add a **`SessionRecord::Summary {
> message_count, usage: TokenSnapshot }`** variant (F01), and the loop emits it **at every completed
> interaction/turn** (not only at session end) — so consumers see **running token usage** as the
> session progresses, plus a final one before the stream ends. Its `Stream::Pending` mirror is the
> **`AgentProgress::TurnComplete { usage }`** variant above (same per-turn usage, on the status
> channel) — so a UI can update token counters from `Pending` without waiting for the `Next` record.
> (`ToolCallCancelled` closes the steering/deadlock gap the review flagged.)

This **replaces** Decision 11's `AgentEvent` enum entirely. There is no `MessageStart/Update/End` —
streaming token deltas are the model layer's concern (it already yields incremental `Messages` /
`ModelState::GeneratingTokens`); the agent loop forwards them.

### The "expect-next" protocol

A `Stream::Pending(AgentProgress::X)` is a promise about the subsequent `Stream::Next`:

```mermaid
sequenceDiagram
    participant L as Agent Loop
    participant C as Caller
    L-->>C: Pending(Generating{model, 12})
    L-->>C: Next(Conversation(Assistant{Text}))
    L-->>C: Pending(ToolCallRequested{"read_file"})
    L-->>C: Next(Conversation(Assistant{ToolCall}))
    L-->>C: Pending(ExecutingTools{total:2, completed:0})
    L-->>C: Next(Conversation(ToolResult))      %% completed:1
    L-->>C: Next(Conversation(ToolResult))      %% completed:2
    L-->>C: Pending(ProcessingMemory{Observation})
    L-->>C: Next(Observation{..})
    L-->>C: Next(FailedAction{AgenticError::Generation(..), trace})
    L-->>C: Pending(SessionEnding)
```

**The protocol is ADVISORY, not a framing guarantee.** The executor freely interleaves `Ignore`,
`Wait`, and `Delayed` between any `Pending` and the eventual `Next` (`ConcurrentQueueStreamIterator`,
`streams.rs:370-398`); a single delivery point can emit **multiple** `Next` via `TaskStatus::Spread`
(`task.rs:217-227`); and a `Next(SessionRecord::FailedAction)` may substitute for the promised record. Therefore consumers
**must key off the record/error content of each `Next`, not item adjacency** — a UI updates its
"calling tool X" label on the `Pending` hint but must not assume the immediately-following item is
the promised record. Content-only callers ignore `Pending` entirely.

### Mapping to valtron `TaskStatus` — **valtron does this; F02 doesn't**

> **RESOLVED (user, 2026-06-15):** the `TaskStatus → Stream` conversion is **fully abstracted by
> valtron** (`From<TaskStatus> for Stream`) — F02 does **not** map it. We only declare the loop's
> associated types and emit `TaskStatus::{Ready, Pending, Init, Ignore, Wait, Delayed, Spawn, Depends,
> Spread}`; valtron turns them into the `Stream` the consumer sees.

So all F02 must state: the agent loop (F27) is a `TaskIterator` with
**`type Ready = SessionRecord`** (errors are `SessionRecord::FailedAction` records, not a `Result`)
and **`type Pending = AgentProgress`**. The `StreamIterator` `D`/`P` the consumer observes is the
post-`into_stream_iter()` view — valtron's conversion, not ours.

> **`AgenticError` trait-bound contract (pin now, F30 owns the type):** `Stream<D,P>`/`TaskStatus`
> only derive `PartialEq`/`Clone`/`Debug` when `D`/`P` do (`streams.rs:77`, `task.rs:142-150`). F01's
> `SessionRecord` is `Clone + PartialEq + Debug` (no `Eq`/`Hash`). So **`AgenticError` must be
> `Clone + PartialEq + Debug`** or the whole agent stream loses those derives. F30 must honor this.

### Relationship to the model stream

The loop wraps the model's `Stream<Messages, ModelState>` and lifts it:
- `model Next(Messages)` → `agent Next(SessionRecord::Conversation { message })`
- `model Pending(ModelState::GeneratingTokens(_))` → `agent Pending(AgentProgress::Generating{..})`
- `model Pending(ModelState::Error(s))` → `agent Next(SessionRecord::FailedAction{..})`

> **Streaming partials (OD-02-5) — RESOLVED (user, 2026-06-15; CONFIRMED against code).** The model
> layer is designed to **collect partials internally and deliver COMPLETE messages**, not raw partials:
> the providers accumulate (`accumulated_text`/`accumulated_thinking`/`AccumulatedToolCall`,
> `InputJsonDelta { partial_json }` — `anthropic_messages_provider.rs:919,937`) and emit complete
> `Messages`. Models are best placed to know partial vs complete, to chunk, and to **section under
> memory constraints to avoid OOM**. So the **agent loop forwards COMPLETE `SessionRecord::Conversation`
> records** — it does **not** deal with partials, and there is **no** `final: bool`/partial-record
> marker. Live token deltas (if a provider ever streams them) surface only via `AgentProgress::Generating`
> on the `Pending` channel; the `Next` stream is complete records only.

## HOW: Implementation Steps

1. Define `AgentProgress` + `MemoryKind` in `foundation_ai::agentic` (new module).
2. Define the loop's associated types: `type Ready = SessionRecord;   // errors are SessionRecord::FailedAction, not a Result
   type Pending = AgentProgress;` (the `AgenticError` shell may be a forward-declared stub until F30).
3. Document the expect-next protocol next to `AgentProgress` (each variant's doc states its Next).
4. Provide a `From<ModelState> for AgentProgress` and a helper lifting model
   `Stream<Messages, ModelState>` items into the agent stream (used by F27).
5. Unit-test the lift mapping (model item → agent item) and serde of `AgentProgress`.

## Open Decisions

- **OD-02-1 (user):** `SessionRecord` vs `Messages` as `Next`'s payload (see above). Rec: `SessionRecord`.
- **OD-02-2:** `tokens_so_far` source — **Resolved → `Option<u64>`**, `None` until providers thread
  partial usage. (Related: OD-02-4.)
- **OD-02-3:** `#[non_exhaustive]` — **Resolved → applied to both `AgentProgress` and `MemoryKind`.**
- **OD-02-4:** providers emit `GeneratingTokens(None)` today (verified) — change them to emit
  `Some(usage)` for live `tokens_so_far`, or accept turn-boundary-only updates? Rec: turn-boundary
  now; partial-usage is a separate provider enhancement.
- **OD-02-5 (user — heart of TODO #9):** forward streaming partials as N `Conversation` records, or
  **coalesce** to one final per turn? Rec: coalesce.
- **OD-02-6:** add `SessionRecord::Summary { message_count, usage }` (to F01) for the session-final
  payload, or have callers aggregate? Rec: add `Summary`.
- **OD-02-7 (dependency):** `AgenticError` (F30) is a forward-ref; until F30 lands, F02 uses a local
  stub with the pinned bounds (`Clone + PartialEq + Debug`). Track F30 as a soft dependency.

## Target Files

- `backends/foundation_ai/src/agentic/mod.rs` (new) — `AgentProgress`, `MemoryKind`, the stream type alias, the lift helper
- references F01 (`SessionRecord`), F30 (`AgenticError` — stub/forward until then)

## Tests

```bash
cargo test -p foundation_ai -- agentic::progress
cargo test -p foundation_ai -- agentic::stream_lift
```

## Verification

```bash
cargo build -p foundation_ai
cargo build -p foundation_ai --no-default-features --features agentic --target wasm32-unknown-unknown
cargo clippy -p foundation_ai -- -D warnings
cargo test -p foundation_ai
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: valtron's `Stream`/`TaskIterator` execution model & `TaskStatus`
→`Stream` mapping; streaming protocols (token deltas, SSE); **advisory vs framed** streaming
contracts; error-in-stream (`Result` payload) vs side channels; `Spread` multiplexing; how the agent
loop mirrors the model layer; trait-bound propagation (`Clone`/`PartialEq`) through generic streams.
(Task — see list.)

## Done When

- `AgentProgress` + the pure-`SessionRecord` / `AgentProgress` stream contract (errors via `FailedAction`) are
  defined and documented with the expect-next protocol.
- The model-stream lift mapping exists and is tested.
- No `AgentEvent`-style content-duplicating enum is introduced (TODO #9 honored).
- OD-02-1..3 resolved.
