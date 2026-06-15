---
feature: "Agentic Loop — inner/outer loop as a valtron TaskIterator"
description: "The orchestrator: a valtron TaskIterator running the nested inner (tool calls + steering) / outer (follow-up) loop, consuming F03's stream contract (rich SessionRecord on Next, AgentProgress on Pending), wiring input/output processors, memory triggers, loop detection, and the circuit breaker, with PriorityQueue interruption (front-inject) and FollowUpQueue continuation"
status: "pending"
priority: "high"
depends_on: ["03-agent-stream-contract", "11-toolcall-execution-dag", "12-model-provider-router", "13-steering-queues-depends", "14-input-output-processors", "17-loop-detection", "02-error-handling"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature 19: Agentic Loop

> **Review status (2026-06-14) — self-review against code (subagent unavailable):**
> 1. **The stream contract is F03's, and F03 already RESOLVED Decision 11's "this is all stupid" TODO**
>    (Decision 11 line 145). F03 pins it:
>    `StreamIterator<D = SessionRecord, P = AgentProgress>` — **pure `SessionRecord`
>    on `Stream::Next`** (errors are `SessionRecord::FailedAction` records, NOT a `Result`), thin
>    `AgentProgress` on `Pending`. So F19 emits
>    `TaskStatus::Ready(SessionRecord::Conversation{ message })` for real messages (NOT the
>    `AgentEvent{MessageUpdate{content:String}}` Decision 08/11 sketched — those `String`-payload events
>    are SUPERSEDED). `AgentProgress` (F03:79) carries the "expect-next" status only. Decision 08/11's
>    `AgentEvent` enum is **dead** — do not implement it.
> 2. **The loop is a `TaskIterator`** (`task.rs:392`): `type Ready = SessionRecord` (errors are
>    `SessionRecord::FailedAction` records, not a `Result`), `type Pending = AgentProgress`,
>    `type Spawner = <object-safe action>` (Decision 08 line 137 used `BoxedSendExecutionAction` — verify
>    the real spawner type), `fn next_status(&mut self) -> Option<TaskStatus<..>>` (:412). It returns
>    `Init` on setup, `Pending(AgentProgress)` while working, `Ready(SessionRecord)` per record (a failure
>    is `Ready(SessionRecord::FailedAction{..})`), **`Depends`** when waiting on queues/tools (F13/F11),
>    never a `Pending` spin.
> 3. **The loop ORCHESTRATES; it owns almost no logic.** It sequences: F14 input pipeline → F12 generate/
>    stream → extract tool calls → F11 execute workflow → F14 output pipeline (which fires F15 memory +
>    F17 loop detection) → F13 queue checks. Decision 05 §Agent Loop Integration + Decision 11 §Loop
>    Structure are the exact control flow. F19 must NOT re-implement memory/tools/detection — it drives them.
> 4. **Inner loop = tool calls + steering; outer loop = follow-up** (Decision 05/11). Inner: drain
>    PriorityQueue at FRONT (Decision 11 line 43) → if steering, F11 cancel → input pipeline → generate →
>    tool calls? → F11 execute → repeat-inner-if-more-tools. Outer: after inner settles, run output
>    pipeline, check FollowUpQueue → continue-outer-if-messages else end. **PriorityQueue front-inject is
>    load-bearing** (interrupt overrides current direction).
> 5. **Mid-generation interruption uses F13's sequenced composition** (Decision 05): the LLM generate/
>    stream runs as a child task sequenced under the loop so the loop checks PriorityQueue between LLM
>    steps and sets `CancelCode::PauseForPriority`. With F12's **boxed** stream (`Box<dyn StreamIterator>`,
>    F12 OD-12-2), the loop pumps the stream and interleaves queue checks. (OD-19-3.)
> 6. **Errors via `Stream::Next(SessionRecord::FailedAction{ error, trace })`** (F02/Decision 16, F01 §5): generation errors → F02 circuit
>    breaker (`handle_error` → Continue/RetryReducedContext/SwitchModel/Terminate); tool errors already
>    became `ToolResult{error_detail}` in F11 (back to LLM, loop continues); loop detection → redirect
>    from memory (F17). The loop's `handle_error` is Decision 16's `AgentAction` dispatcher.
> 7. **Circuit breaker needs the multi-provider router** (F12 + Decision 16 §Circuit Breaker): on repeated
>    generation failure, switch `current_model` to a fallback via F12. `AgentConfig{ primary_model,
>    fallback_models, memory_model, circuit_breaker_threshold, .. }` (Decision 16 line 257). F12's `Vec`
>    routes enable same-model multi-provider fallback later.
> 8. **No blocking, ever.** Generate/stream pumped step-wise; tool execution is F11's task; memory/detect
>    are spawned. Waits are `TaskStatus::Depends`; backoff is `Delayed` (never `SleepIterator`); on wasm
>    the valtron engine yields to the JS loop. (requirements §8, memory `feedback_async_iterators`.)

> Implements Decision 11 (inner/outer loop) + Decision 08 (valtron `TaskIterator`) on F03's stream
> contract. The orchestrator that drives the whole turn: processors → model (routed) → tool DAG →
> processors, with steering interruption, follow-up continuation, memory triggers, loop detection, and
> the circuit breaker. Emits rich `SessionRecord` on `Next`, thin `AgentProgress` on `Pending`.

## WHY: Problem Statement

Everything else in this spec is a component; nothing yet runs a *turn*. The loop is the spine: it must
assemble context, call the (routed) model, extract and execute tool calls in dependency order, persist
everything, feed results back, distil memory, detect loops, accept steering mid-flight, continue on
follow-up, and stream rich records + thin progress to the caller — all as a non-blocking valtron task
that survives interruption and resumes deterministically. This feature is that orchestrator.

## WHAT: Solution

```rust
// backends/foundation_ai/src/agentic/loop.rs
pub struct AgentLoop {
    session: Arc<SessionInner>,   // F20 bundle: message_api, context(F16), tools(F11), queues(F13),
                                  // router(F12), memory(F15), detector(F17), processors(F14), ledger(F04)
    state: AgentLoopState,
    current_model: ModelId,
    failure_count: u32,
    config: AgentConfig,
}

pub enum AgentLoopState {
    Initializing,
    OuterBoundary,                      // check FollowUpQueue / end
    InnerAssemble,                      // run input processors → AgentContext
    InnerGenerate { stream: Box<dyn StreamIterator<D = Messages, P = ModelState>> }, // F12, pumped
    InnerToolCalls { calls: Vec<ToolCallRequest> },
    InnerExecuting { task: ToolCallExecTask },   // F11
    OutputProcessing { turn: TurnOutput },        // run output processors (memory/detect/save)
    Ending,
}

impl TaskIterator for AgentLoop {
    type Ready   = SessionRecord;   // F03 contract — errors are SessionRecord::FailedAction, not a Result
    type Pending = AgentProgress;                          // F03
    type Spawner = BoxedSendExecutionAction;               // verify real type

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // state machine implementing Decision 11 §Loop Structure (see HOW)
    }
}
```

### Control flow (Decision 05/11)

```text
Initializing -> Init ; load context (resume = F20)
OuterBoundary:
    drain FollowUpQueue (F13) -> if msgs, append as pending User, -> InnerAssemble
    else -> Ending
InnerAssemble:
    drain PriorityQueue at FRONT (F13) -> if steering: F11.cancel(); inject at front
    InputPipeline.run(&mut ctx) (F14)  -> InnerGenerate
InnerGenerate (sequenced under loop for interruption, F13/OD-19-3):
    pump F12 stream; Pending(AgentProgress::Generating); on each msg -> Ready(Conversation{message:msg})
    between steps: check PriorityQueue -> set PauseForPriority -> break to InnerAssemble
    on done: extract tool calls
        has tools -> InnerToolCalls ; none -> OutputProcessing
InnerToolCalls -> F11.execute_workflow -> InnerExecuting
InnerExecuting:
    drive ToolCallExecTask; Ready(Conversation{message:ToolResult}) per result (already persisted, F11)
    on complete -> InnerAssemble (feed results back to LLM)   # inner loop repeats
OutputProcessing:
    OutputPipeline.run (F14) -> fires F15 memory triggers, F17 loop detect, F08 save (spawned)
    loop detected? -> handle (F17 redirect / escalate)
    -> OuterBoundary
Ending -> Ready(SessionRecord::Summary{..}) (F03 OD-03-6) ; flush (F08/F20)
```

### Error / circuit breaker (F02 / Decision 16)

```rust
// `report: ErrorTrace<AgenticError>` (foundation_errstacks) — `classify` reads the context kind.
fn handle_error(&mut self, report: ErrorTrace<AgenticError>) -> TaskStatus<...> {
    match self.session.errors.classify(&report) {   // F02
        AgentAction::Continue              => /* tool error already a ToolResult; keep going */,
        AgentAction::RetryWithReducedContext => /* Messages::is_context_overflow → trim, retry */,
        AgentAction::SwitchModel           => { self.current_model = self.next_fallback()?; /* F12 */ },
        AgentAction::Terminate(report)     => return Some(TaskStatus::Ready(SessionRecord::FailedAction{
                                                  error: report.current_context().clone(),
                                                  trace: report.to_structured() })),
    }
}
```

### Waiting without spinning

When parked on steering/follow-up/tool readiness, return `TaskStatus::Depends(readiness)` (F13
`QueueReadiness` / F11 result readiness) — never `Pending` in a tight loop. Backoff (retry) is F11's
`Delayed`.

## Architecture

```mermaid
graph TD
    INIT[Initializing -> Init] --> OB[OuterBoundary]
    OB -->|FollowUp msgs| IA[InnerAssemble]
    OB -->|empty| END[Ending -> Summary + flush]
    IA -->|drain Priority FRONT + F14 input| GEN[InnerGenerate pump F12 stream]
    GEN -->|Next Ok Conversation| C[caller]
    GEN -->|priority mid-gen| IA
    GEN -->|tool calls| TC[InnerToolCalls -> F11 DAG]
    GEN -->|no tools| OP[OutputProcessing F14]
    TC --> EX[InnerExecuting -> ToolResult persisted]
    EX --> IA
    OP -->|F15 memory + F17 detect + F08 save| OB
    OP -->|loop detected| RD[F17 redirect/escalate]
    GEN -->|error| HE[handle_error F02 circuit breaker -> F12 fallback]
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: the agentic loop (inner tool-call loop vs outer follow-up loop, why
the nesting maps to interrupt-now vs defer); the ReAct-style generate→tool→observe cycle; **driving an
LLM stream step-wise as a valtron `TaskIterator`** (pumping `Box<dyn StreamIterator>`, emitting rich
`SessionRecord` on `Next` + `AgentProgress` on `Pending`, the expect-next protocol); cooperative
interruption (front-injection, sequenced composition, cancel signal) and zero-spin waiting
(`Depends`); orchestration-vs-logic (the loop sequences components, owns none); circuit-breaker model
fallback; non-blocking discipline (no `sleep`, `Delayed`/`Depends`, wasm JS-loop yielding). (Task —
see list.)

## HOW: Implementation Steps

1. `AgentLoop` + `AgentLoopState`; `TaskIterator` impl (`Ready = SessionRecord` — errors are
   `SessionRecord::FailedAction`, `Pending = AgentProgress`) — verify the real `Spawner` type.
2. `Initializing` (load context / F20 resume) → `Init`.
3. `OuterBoundary`: FollowUpQueue drain (F13) → continue or `Ending`.
4. `InnerAssemble`: PriorityQueue front-drain + F11 cancel on steering; F14 input pipeline.
5. `InnerGenerate`: pump F12 stream sequenced for interruption; emit `Ready(Conversation)` +
   `Pending(Generating)`; mid-gen priority check (OD-19-3); extract tool calls.
6. `InnerToolCalls`/`InnerExecuting`: F11 workflow; emit persisted `ToolResult`s; loop back to assemble.
7. `OutputProcessing`: F14 output pipeline (F15 memory, F17 detect, F08 save); handle loop detection.
8. `handle_error` (F02 `AgentAction`) + circuit breaker model switch via F12.
9. `Ending`: emit `Summary` (F03 OD-03-6), flush (F08/F20).
10. Waits via `Depends`; never block; backoff via F11 `Delayed`.
11. Tests (mostly via F21 MockModelProvider): full turn no-tools; turn with parallel tool calls;
    inner-loop repeats until no tools; PriorityQueue interrupts mid-generation (front-inject);
    FollowUpQueue continues outer; memory trigger fires at threshold; loop detection redirects; circuit
    breaker switches model on repeated failure; error surfaces as `Next(FailedAction)`; resume mid-session;
    `Depends` parks (no spin); wasm build.

## Open Decisions

- **OD-19-1 — Ready payload:** `SessionRecord` (F03 rec) vs `Messages`. F03 already chose `SessionRecord`
  (superset: conversation + memory on one stream). Confirm alignment with F03 OD-03-1.
- **OD-19-2 — AgentEvent is dead:** confirm Decision 08/11's `AgentEvent{MessageUpdate{String}}` enum is
  NOT implemented (superseded by rich `SessionRecord` on `Next`, F03). Rec: dead — delete from scope.
- **OD-19-3 — mid-gen interruption (load-bearing):** sequenced composition pumping F12's boxed stream
  with between-step PriorityQueue checks. Rec: yes; confirm the boxed stream is pumpable step-wise.
- **OD-19-4 — inner-loop termination:** inner repeats while the LLM emits tool calls; a max-iterations
  guard prevents runaway (independent of F17 loop detection). Rec: add `config.max_inner_iterations`.
- **OD-19-5 — Summary record:** emit `SessionRecord::Summary{ message_count, usage }` at end (F03
  OD-03-6 adds the variant to F01). Confirm F01 has it.

## Target Files

- `backends/foundation_ai/src/agentic/loop.rs` (new) — `AgentLoop`, `AgentLoopState`
- coordinates F03 (stream contract), F04 (ledger), F08 (save/recent), F16 (context), F15 (memory),
  F11 (tool DAG), F12 (router), F13 (queues/cancel/sequenced), F14 (processors), F17 (loop detection),
  F02 (errors/circuit breaker), F20 (session bundle + resume)

## Tests

```bash
cargo test -p foundation_ai -- agentic::loop
cargo build -p foundation_ai --no-default-features --features agentic --target wasm32-unknown-unknown
```

## Verification

```bash
cargo build -p foundation_ai
cargo build -p foundation_ai --no-default-features --features agentic --target wasm32-unknown-unknown
cargo clippy -p foundation_ai -- -D warnings
cargo test  -p foundation_ai -- agentic::loop
```

## Done When

- `AgentLoop` is a valtron `TaskIterator` emitting `Stream::Next(SessionRecord)` + `Pending(AgentProgress)`
  (errors as `Next(SessionRecord::FailedAction)`), running the inner (tools+steering) / outer (follow-up) loop per Decision 05/11;
  PriorityQueue interrupts mid-generation with front-injection; FollowUpQueue continues; F14 processors,
  F15 memory, F17 detection, F02 circuit breaker all wired; waits via `Depends` (no spin); the dead
  `AgentEvent` enum is NOT implemented; builds native + wasm.
- OD-19-1..5 resolved (OD-19-2/27-3 flagged); fundamentals authored.
