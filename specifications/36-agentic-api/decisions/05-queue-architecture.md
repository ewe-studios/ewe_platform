# Decision 05: Queue Architecture — PriorityQueue vs FollowUpQueue

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

Users need two distinct ways to influence an ongoing agent session:
1. **Immediate interruption** — stop what the agent is doing now and respond to new input
2. **Deferred steering** — queue instructions for after the current loop finishes

A single queue cannot serve both purposes because interruption requires cancelling in-progress work, while deferred steering must wait for natural completion.

## Decision

Two separate queue-backed delivery mechanisms with distinct semantics:

| Queue | Interrupts LLM? | Interrupts ToolCallManager? | When processed | Use case |
|-------|-----------------|----------------------------|----------------|----------|
| **PriorityQueue** | ✅ Yes — sets cancel signal checked each LLM valtron iteration | ✅ Yes — signals cancellation | Between each LLM valtron step | User says "stop, do this instead" |
| **FollowUpQueue** | ❌ No — waits for LLM turn to complete | ❌ No — waits for all tool calls done | At outer loop boundary | User says "after that, also do this" |

### Queue Types

Both use `Arc<ConcurrentQueue<Messages>>` — the Agent task holds references to both. Steering messages are **always** `Messages::User` — whether from a human user, system prompt, or another LLM agent:

```rust
pub struct AgentSession {
    pub priority_queue: Arc<ConcurrentQueue<Messages>>,  // Messages::User from any source
    pub followup_queue: Arc<ConcurrentQueue<Messages>>,  // Messages::User from any source
    pub cancel_signal: Arc<AtomicU32>,  // signal codes for LLM task
    // ... other shared state
}

pub enum CancelCode: u32 {
    None = 0,
    PauseForPriority = 1,   // LLM should pause, check priority queue
    Abort = 2,              // LLM should abort entirely
}
```

A `Messages::User` can come from:
- **Human user** → `role: MessageRole::User` — direct input
- **System** → `role: MessageRole::System` — instructions, loop redirect
- **Another LLM agent** → `role: MessageRole::Agent` — inter-agent steering/guidance

The **queue determines urgency**, not the message type. Same `Messages::User` routed to PriorityQueue interrupts immediately; routed to FollowUpQueue, it waits for the outer loop boundary.

### How PriorityQueue Interrupts the LLM

The LLM task in foundation_ai is a valtron task. The Agent task drives it using valtron's **sequenced** execution mode (`DualSequeunceChildAndParentLinkedTask`):

```
Agent Task (parent) ──sequenced──▶ LLM Task (child)

Each executor step:
1. LLM task runs one valtron iteration → yields token / pending / done
2. Agent task runs one valtron iteration → checks priority queue
3. If priority queue has messages:
   a. Agent sets cancel_signal = PauseForPriority
   b. LLM task sees signal on next iteration → pauses/aborts
   c. Agent drains priority queue, injects at front of message list
   d. Agent continues with new instruction
```

**Why sequenced (`DualSequeunceChildAndParentLinkedTask`)?**

From valtron's `dependent_lift.rs` — `DualSequeunceChildAndParentLinkedTask` polls both child and parent on each executor step:

```rust
// Each next() call:
// 1. Poll child (LLM task) → get token/pending/done
// 2. Poll parent (Agent task) → check queues, set signals
// 3. Return child's state
// Both run in lockstep — Agent can interleave between each LLM step
```

This gives the Agent a chance to check the priority queue **between every LLM valtron iteration** — not just after the full response.

### How FollowUpQueue Works

FollowUpQueue is checked at **outer loop boundary** — after the LLM turn completes, all tool calls finish, and inner loop has no more tool calls:

```
Outer loop boundary:
├── LLM turn complete (no more tool calls)
├── Check FollowUpQueue
│   ├── If messages → move to pending, continue outer loop
│   └── If empty → emit(agent_end)
└── Agent loop restarts with follow-up instructions
```

The FollowUpQueue does NOT set cancel_signal — it waits for natural completion.

### Valtron Task Composition Options

The Agent task can compose with the LLM task using different valtron spawn modes:

| Spawn Mode | Executor Type | Behavior | Use for |
|-----------|--------------|----------|---------|
| **Sequenced** (`sequenced()`) | `DualSequeunceChildAndParentLinkedTask` | Child + parent run lockstep each executor iteration | **LLM task** — Agent checks queues between each LLM step |
| **Lifted** (`lift()`) | `FinishChildBeforeParentTask` | Child runs to exhaustion, then parent resumes | **Tool execution sub-tasks** — run tool, then Agent processes result |
| **Broadcast** (`broadcast()`) | Global queue → another thread | Runs on different thread | **Embedding generation** — independent background work |
| **Scheduled** (`schedule()`) | Bottom of local queue | Runs after all local tasks complete | **Memory generation** — deferred, non-urgent |

### Agent Loop Integration

```
Agent.prompt("Fix the bug")
└─ runLoop()
   │
   ├─ emit(AgentEvent::SessionStart)
   │
   ├─ OUTER LOOP (follow-up continuation)
   │   │
   │   ├─ Check FollowUpQueue → if messages, add as pending, continue outer loop
   │   │
   │   ├─ LLM Task (spawned via sequenced with Agent as parent)
   │   │   │
   │   │   ├─ Each executor step:
   │   │   │   ├── LLM task iteration → yield token/pending
   │   │   │   └── Agent task iteration → check PriorityQueue
   │   │   │       └── If priority messages:
   │   │   │           ├── cancel_signal = PauseForPriority
   │   │   │           ├── LLM sees signal → pauses
   │   │   │           ├── Agent drains PriorityQueue → inject at front
   │   │   │           └── Agent continues with new instruction
   │   │   │
   │   │   └─ When LLM completes → emit(MessageEnd)
   │   │
   │   ├─ Extract tool calls
   │   ├─ If tool calls:
   │   │   ├─ ToolCallManager.submit(tool_calls) — spawned via lifted
   │   │   │   ├── Each tool runs as child task
   │   │   │   ├── Agent checks PriorityQueue between tool executions
   │   │   │   └── If priority: cancel remaining tool calls
   │   │   └── Results → Message API → agent loop
   │   │
   │   └─ Check FollowUpQueue → if messages, continue outer loop
   │
   └─ emit(AgentEvent::SessionEnd)
```

### Rationale

**Why sequenced execution for LLM + Agent?**
- Gives Agent a chance to check queues between every LLM valtron iteration
- LLM can be interrupted mid-stream, not just after completion
- Uses valtron's built-in `DualSequeunceChildAndParentLinkedTask` — no custom coordination needed
- Agent's `next()` runs every step — can set `cancel_signal`, check queues, emit progress

**Why AtomicU32 cancel signal instead of AtomicBool?**
- Single bool only supports on/off — can't distinguish "pause" from "abort"
- U32 allows multiple signal codes: PauseForPriority, Abort, Resume, etc.
- Extensible — new signal types added without API changes

**Why shared `Arc<ConcurrentQueue>` instead of valtron `NotifyQueue`?**
- Agent task owns the queues and polls them directly in its `next()` iteration
- No separate consumer task needed — Agent is the consumer
- `ConcurrentQueue` is sufficient — no CondVar blocking needed (Agent polls, doesn't block)
- `NotifyQueue` is for producer-consumer where consumer blocks waiting — not our pattern

**Why two queues instead of priority levels?**
- Two distinct check points: PriorityQueue (between LLM steps), FollowUpQueue (outer loop boundary)
- Priority levels in a single queue would still need a decision: "do I check this now or wait?"
- Separate queues make the intent explicit at the API level

## Alternatives Considered

### Single queue with priority levels
- **Pros:** Simpler API (one queue)
- **Cons:** Still needs cancellation logic at every check point; unclear semantics for "medium priority"
- **Rejected because:** Two distinct use cases (interrupt vs defer) are better served by explicit separation

### NotifyQueue + BoolSignal for wakeup
- **Pros:** Uses valtron's existing notification infrastructure
- **Cons:** Agent task owns the queues and polls them directly — no separate consumer needs wakeup
- **Rejected because:** Our pattern is inline polling in Agent's valtron iteration, not blocking consumer

### Channel-based queues (mpsc)
- **Pros:** Built-in async support, backpressure
- **Cons:** `mpsc` is single-consumer; we need the Agent to poll at specific points
- **Rejected because:** `ConcurrentQueue` + inline polling matches the Agent's execution model better
