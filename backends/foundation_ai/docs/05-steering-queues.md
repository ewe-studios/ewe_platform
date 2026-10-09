# Fundamentals 05 — Steering queues and cancellation

How the agent handles interruptions, priority messages, and session cancellation.

---

## 1. The problem

An agent session must be interruptible. The user might want to:
- Stop the current generation
- Inject an urgent message
- Abort the session entirely

Meanwhile, the agent is streaming tokens and executing tools. The steering
system handles this without race conditions.

## 2. CancelCode — the cancellation signal

```rust
#[repr(u32)]
pub enum CancelCode {
    None = 0,            // Normal execution
    PauseForPriority = 1, // Pause, handle priority messages
    Abort = 2,           // Abort session entirely
}
```

Carried by an `Arc<AtomicU32>`. The agent loop reads this atomically:

```rust
let code = CancelCode::load(&cancel_signal);
match code {
    CancelCode::None => { /* continue */ }
    CancelCode::PauseForPriority => { /* check priority queue */ }
    CancelCode::Abort => { /* terminate */ }
}
```

## 3. SteeringQueues — two queues + cancel signal

```rust
pub struct SteeringQueues {
    pub priority: Arc<ConcurrentQueue<Messages>>,     // High-priority (interrupts)
    pub follow_up: Arc<ConcurrentQueue<Messages>>,     // Deferred (after current work)
    pub cancel_signal: Arc<AtomicU32>,                // Cancellation state
}
```

`push_priority` also stores `CancelCode::PauseForPriority` in the cancel
signal; `abort()` stores `CancelCode::Abort`.

### Where the loop checks them

| Point in the loop | Priority message waiting | Abort |
|---|---|---|
| `OuterBoundary` | Drain it (before the follow-up queue), persist, start a new inner round | End the turn |
| `InnerAssemble` | Fold it into history before generating | End the turn |
| `InnerGenerate` (each stream step) | **Discard** the in-flight generation and re-assemble | — |
| `InnerExecuting` | **Cancel** every in-flight tool future, then re-assemble | — |

Follow-up messages are only read at `OuterBoundary`, after the current inner
round is done. Each user prompt you pass to `run_turn` is itself pushed onto
the follow-up queue.

Every drained message is persisted to the `MessageApi` and reaches the model
through normal context assembly. `max_outer_iterations` (default 10) bounds
how many queue drains one turn performs.

## 4. Using steering from AgentSession

```rust
// Interrupt current work (from another thread while a turn runs):
agent.steer(user_message("Stop — use the staging database instead."));

// Queue the next message; it runs after the current work in the same turn,
// or in the next run_turn if nothing is running:
agent.follow_up(user_message("Then summarise what changed."));

// Hard stop: the loop ends the turn at its next boundary and emits Summary.
agent.abort();
```

`AgentSession` is cheap to clone (`Arc` inside), so hand a clone to the thread
that steers.

## 5. Queue readiness

`SteeringQueues::priority_readiness()` / `followup_readiness()` return
`EventReadiness` handles for Valtron's `TaskStatus::Depends`, so a task can park
until a message arrives. The agent loop does not use them today — it checks
the queues at the points in §3 instead.

## 6. Ending a session

`AgentSession::end()` flushes the `MessageApi`, drains both queues and
persists whatever was still queued as conversation records, flushes again, and
resets the cancel signal.

## 7. CancelCode reset

After handling a cancel signal, the loop resets it:

```rust
CancelCode::reset(&cancel_signal); // Back to None
```

This ensures the same cancel signal doesn't fire repeatedly.
