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

### Priority queue

Messages here should **interrupt** current work. The agent loop checks this
queue when:
- `CancelCode::PauseForPriority` is set
- Between tool execution rounds
- Before starting a new generation

### Follow-up queue

Messages here are processed **after** current work completes. Used for:
- Multi-turn conversations (each user message is a follow-up)
- Agent-generated follow-up questions
- Deferred tool results

## 4. Using steering from AgentSession

```rust
// High-priority (interrupts current work)
session.steer(Messages::User { /* urgent message */ });
// → Sets CancelCode::PauseForPriority

// Follow-up (processed after current generation)
session.follow_up(Messages::User { /* next question */ });
// → Enqueued for next turn
```

## 5. Integration with the agentic loop

```
User message → session.follow_up(msg)
  → AgentLoop dequeues from follow_up queue
  → Generate from model
    → Text tokens → stream to user
    → Tool calls → execute → loop back
  → Check cancel_signal
    → PauseForPriority → drain priority queue
    → Abort → terminate
  → Check follow_up queue for next turn
```

## 6. Queue readiness (no spinning)

The agent loop uses `TaskStatus::Depends` with `QueueReadiness` to wait
for messages without busy-spinning:

```rust
// Wait for messages without spinning
TaskStatus::Depends(QueueReadiness::new(&priority_queue))
```

This parks the task until a message arrives, then resumes.

## 7. Session lifecycle with steering

```rust
// Create session
let session = AgentSession::builder(SessionId::new(), router).build()?;

// Start first turn
let stream = session.run_turn_stream(user_message("Hello"))?;

// While streaming, user interrupts:
session.steer(user_message("Stop, try a different approach"));
// → Current generation pauses, new message takes priority

// Follow-up for next turn
session.follow_up(user_message("Now explain it simply"));

// End session (flushes remaining queue messages)
session.end()?;
```

## 8. CancelCode reset

After handling a cancel signal, the loop resets it:

```rust
CancelCode::reset(&cancel_signal); // Back to None
```

This ensures the same cancel signal doesn't fire repeatedly.
