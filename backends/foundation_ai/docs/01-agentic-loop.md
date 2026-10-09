# Fundamentals 01 — The agentic loop

How one agent turn runs end to end. Read this before you change the loop, add
a guard, or debug odd agent behaviour.

Source: `src/agentic/session.rs`, `src/agentic/agent_loop.rs`.

---

## 1. The big picture

```
session.run_turn_stream(prompt)
  └─ push prompt onto the follow-up queue
  └─ build a fresh AgentLoop (shares the session's components)
  └─ schedule it on the Valtron executor → DrivenStreamIterator

AgentLoop (one TaskIterator step per next_status call)
  Initializing
  └─ OuterBoundary ── abort? ──────────────────────────────┐
       drain priority queue, then follow-up queue          │
       nothing queued ─────────────────────────────────────┤
       │                                                   │
       └─ InnerAssemble                                    │
            abort? / priority? / budget exhausted?         │
            router.get_model(current_model)                │
            hydrate memory, assemble context               │
            preflight compression + context-pressure note  │
            model.stream(interaction, params)              │
          InnerGenerate   (pump one stream item per step)  │
            priority arrived? → discard, re-assemble       │
            stream done → loop / vacuous-answer checks,    │
                          persist assistant turn,          │
                          record usage in the ledger       │
          InnerToolCalls → InnerExecuting → InnerEmitResults
            (tool results persisted, back to InnerAssemble)│
          no tool calls → OutputProcessing (memory triggers)
       back to OuterBoundary                               │
  Ending  ◄────────────────────────────────────────────────┘
    emit SessionRecord::Summary
  Done
```

Everything is synchronous from the caller's point of view. The loop never
blocks the executor: each `next_status` call does one step of work and returns.

## 2. Running a turn

```rust
let agent = AgentSession::<Doc, Mem>::builder(SessionId::new(), router)
    .with_model(ModelId::Name("claude-sonnet-4-6".into(), None))
    .with_system_prompt("You are a coding assistant.")
    .build()?;

// Streaming — the primary API.
for item in agent.run_turn_stream(prompt)? {
    match item {
        Stream::Next(record) => { /* SessionRecord */ }
        Stream::Pending(progress) => { /* AgentProgress */ }
        _ => {}
    }
}

// Or collect the whole turn.
let records: Vec<SessionRecord> = agent.run_turn(prompt)?;
```

`run_turn` drains the stream for you. It returns `Err` on the first
`SessionRecord::FailedAction`, and when it sees `SessionRecord::Retracted` it
drops the assistant messages it has collected so far (the loop is retrying that
answer). Streaming consumers must handle `Retracted` themselves.

The process must be running a Valtron executor (`#[valtron] fn main`, or the
test helpers in `foundation_testing`).

## 3. The states

| State | What happens |
|---|---|
| `Initializing` | Emits `AgentProgress::Initializing`, moves to `OuterBoundary`. |
| `OuterBoundary` | Honours an abort, enforces `max_outer_iterations`, drains the priority queue first, then the follow-up queue. Each drained message is persisted to the `MessageApi`. Nothing queued → `Ending`. |
| `InnerAssemble` | Honours abort, folds in any priority messages, stops on an exhausted budget (`AgenticError::BudgetExhausted`), resolves the model, assembles context, starts `model.stream(...)`. |
| `InnerGenerate { stream, collected }` | Pumps one stream item per step. A newly-arrived priority message discards the generation and re-assembles. When the stream ends, runs `on_generation_complete`. |
| `InnerToolCalls { calls }` | Tool calls extracted from the assistant output. |
| `InnerExecuting { .. }` | Runs each call through `ToolCallManager::execute_one`, wrapped in a `CancellableFutureTask`. A priority message cancels in-flight tools. |
| `InnerEmitResults { .. }` | Emits each `Messages::ToolResult` as a record, persists it, returns to `InnerAssemble`. Enforces `max_inner_iterations`. |
| `OutputProcessing` | Calls `MemoryHierarchy::check_triggers()`; emits `ProcessingMemory` when a threshold is crossed (see Doc 11 — generation itself is not wired). |
| `Ending` | Emits `SessionRecord::Summary { message_count, usage }`. |
| `Done` | Terminal. |

## 4. Context assembly

`ContextProvider::assemble_from_memory` builds the message list in this order:

1. Working memory (if any) as a system-role message
2. Reflection (if any)
3. Observation — only when it is newer than the latest reflection
   (`ContextConfig::inject_newer_observations`)
4. The last `ContextConfig::recent_message_count` (default 20) records from the
   `MessageApi`

The system prompt rides separately on `ModelInteraction::system_prompt`. The
tools come from `ToolCallManager::build_toolshed()` at every assemble, so tools
registered mid-session show up on the next generation.

> **Known issue.** `MessageApi::recent` returns records newest-first and the
> context keeps that order, so the model sees recent messages in reverse. See
> Doc 00, "Known limitations".

Two budget-driven adjustments run before the request is sent. Both measure the
context's estimated tokens against the **session token budget**
(`TokenLedger::budget()`), not the model's context window, and both do nothing
when there is no budget:

- **Preflight compression** (`preflight_compression_threshold`, default 0.85):
  drops the oldest messages until the estimate fits under
  `threshold × budget`.
- **Context pressure** (`context_pressure_threshold`, default 0.70): appends a
  "Context is at N% capacity — prefer concise responses" note to the system
  prompt.

`max_tokens` for the request is clamped to the budget's remaining tokens
(`TokenLedger::effective_max_tokens`).

## 5. After generation: checks and persistence

When the stream finishes, `on_generation_complete`:

1. Runs `LoopDetector::check` on every assistant output. On a loop it
   escalates:
   - `Redirect` → injects a system message ("Loop detected. Please try a
     different approach…") and re-assembles.
   - `SwitchModelOrTemperature` → moves to the next fallback model via the
     circuit breaker (temperature is **not** changed), injects a system
     message, re-assembles.
   - `Terminate` → `AgenticError::LoopDetected` as a `FailedAction`. A
     repeated *empty* answer is passed through instead of failing.
2. If the turn called no tool, judges the assembled answer with
   `LoopDetector::check_answer`. A vacuous answer (a lone `.`, an empty
   reply, or a bare number to a question that didn't ask for one) is retried:
   the loop emits `SessionRecord::Retracted` and asks again. Out of retries,
   the weak answer is passed through rather than turned into an error.
3. Resets the detector after a good turn.
4. Persists the assistant messages to the `MessageApi`.
5. Records the last assistant message's `UsageReport` in the `TokenLedger`
   (streaming backends report cumulative usage, so only the last one counts).
6. Extracts tool calls; none → `OutputProcessing`.

The detector is built with `LoopDetectorConfig::default()` (window 5,
similarity 0.9, 3 tool-call repeats, 3 redirects). It is not configurable
through `AgentSession` today.

## 6. Error handling

Errors become `AgenticError` and go through `ErrorPolicy::classify`, which
returns an `AgentAction`:

| Error | Default action | What the loop does |
|---|---|---|
| `Generation` with `GenKind::ContextOverflow` | `RetryWithReducedContext` | Re-assembles (compression applies if a budget is set) |
| `Generation` with `GenKind::RateLimit` | `SwitchModel` | `CircuitBreaker::on_failure()`; next fallback model, or terminate when none are left |
| `ToolCall`, `LoopDetected` | `Continue` | Keeps going |
| Everything else | `Terminate(err)` | Emits `FailedAction`, ends the turn |

`GenKind` is detected at the boundary: context overflow by matching provider
error text (`Messages::is_context_overflow`), rate limits by `"rate limit"`,
`"429"` or `"too many requests"` in the message. Providers retry 429/5xx
internally before an error reaches the loop.

Override the policy with a closure:

```rust
let policy = ErrorPolicy::customize(|error| match error {
    AgenticError::ToolCall { ref tool_name, .. } if tool_name == "deploy" => {
        AgentAction::Terminate(error)
    }
    other => ErrorPolicy::new().classify(other),
});
let agent = builder.with_error_policy(policy).build()?;
```

A tool that fails normally does **not** reach the policy: its error becomes a
`Messages::ToolResult` (with `error_detail`) that the model reads and reacts
to. A tool that panics is caught at the boundary and reported the same way.

### Circuit breaker

`CircuitBreaker::new(threshold, fallbacks)` counts consecutive failures. Once
the count reaches `threshold` (`AgentConfig::circuit_breaker_threshold`,
default 3), each further `on_failure()` returns the next model from
`fallback_models`; `on_success()` resets the count.

## 7. Token accounting

`TokenLedger` is shared by the session, the loop and the memory hierarchy.

| Method | Meaning |
|---|---|
| `record(&UsageReport)` | Add a generation's usage (called by the loop) |
| `total()`, `input()`, `output()`, `cost()` | Session totals |
| `rolling()` / `reset_rolling()` | Tokens since the last observation (drives the memory trigger) |
| `budget()`, `remaining()`, `is_exhausted()` | Hard budget, set from `SessionAccessProvider::token_budget` at `build()` |
| `snapshot()` | A `TokenSnapshot` (also carried by `SessionRecord::Summary`) |

To price usage yourself, use `costing::calculate_cost(&pricing, &usage,
CostStatus::…)` and `CostAccumulator`.

## 8. Limits and defaults (`AgentConfig`)

| Field | Default | Meaning |
|---|---|---|
| `primary_model` | empty name — set it with `with_model` | Model for generation |
| `fallback_models` | `[]` | Circuit-breaker fallbacks |
| `memory_model` | `None` | Recorded, but nothing generates memory yet |
| `max_inner_iterations` | 25 | Tool-call rounds per outer iteration |
| `max_outer_iterations` | 10 | Queue-drain rounds per turn |
| `circuit_breaker_threshold` | 3 | Failures before switching models |
| `preflight_compression_threshold` | 0.85 | Fraction of the budget; `0.0` disables |
| `context_pressure_threshold` | 0.70 | Fraction of the budget; `0.0` disables |
| `model_params` | `ModelParams::default()` | Sampling parameters for every request |

Doc 09 covers tuning these.
