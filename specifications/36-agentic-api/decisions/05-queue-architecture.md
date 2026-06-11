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

Two separate concurrent-queue-backed delivery queues with distinct semantics:

| Queue | Interrupts ToolCallManager? | Interrupts LLM? | When processed | Use case |
|-------|----------------------------|-----------------|----------------|----------|
| **PriorityQueue** | ✅ Yes — cancels in-progress tool calls | ✅ Yes — next turn | Immediately at next check | User says "stop, do this instead" |
| **FollowUpQueue** | ❌ No — lets tool calls complete | ❌ No — waits for loop end | At outer loop boundary | User says "after that, also do this" |

### PriorityQueue

**Purpose:** Immediate user interruption and steering.

**Behavior:**
1. User sends a message → enqueued to PriorityQueue
2. Agent loop checks PriorityQueue at **every inner loop iteration**
3. **If PriorityQueue has messages:**
   - Signal ToolCallManager to cancel in-progress tool calls
   - Inject priority messages at the **front** of the message list
   - LLM **must** respond to the priority message first
   - Original task is abandoned or paused

**Processing order:**
```
Inner loop iteration:
├── Drain PriorityQueue → inject at FRONT of message list
│   └── "STOP. Instead, fix the auth bug in the login handler."
├── streamAssistantResponse() with modified message list
│   └── LLM responds to the priority message
└── Continue inner loop with LLM's response
```

**Cancellation cascade:**
```
PriorityQueue has messages
├── ToolCallManager.cancel() called
│   ├── cancellation_signal.store(true)
│   ├── In-progress tool calls check signal → abort
│   └── Partial results persisted to Message API
├── Agent loop processes priority message
│   └── New tool calls may be generated (fresh execution)
└── Original tool calls are abandoned (results still persisted for audit)
```

### FollowUpQueue

**Purpose:** Deferred steering — instructions for the next iteration.

**Behavior:**
1. User sends a message → enqueued to FollowUpQueue
2. Agent loop checks FollowUpQueue only at **outer loop boundary** (after all tool calls complete, inner loop has no more tool calls)
3. **If FollowUpQueue has messages:**
   - Move messages to the pending message list
   - Continue outer loop with follow-up instructions as the next task
   - Does **NOT** interrupt ToolCallManager or LLM

**Processing order:**
```
Outer loop boundary (inner loop exhausted, no more tool calls):
├── Drain FollowUpQueue → add as pending messages
│   └── "Now that the auth bug is fixed, also update the tests."
├── Set as pending, continue outer loop
└── Inner loop restarts with follow-up instructions
```

### Queue Implementation

Both queues use `concurrent_queue::ConcurrentQueue` wrapped in `Arc`:

```rust
pub struct PriorityQueue {
    queue: Arc<ConcurrentQueue<SteeringMessage>>,
    notifier: Arc<Notifier>, // wakes the agent task when messages arrive
}

pub struct FollowUpQueue {
    queue: Arc<ConcurrentQueue<SteeringMessage>>,
    notifier: Arc<Notifier>,
}

pub struct SteeringMessage {
    pub content: String,
    pub source: MessageSource, // user, system, external
    pub enqueued_at: u128,     // scru128 timestamp
}
```

### Agent Loop Integration

```
agent.prompt("Fix the bug")
└─ runLoop()
   │
   ├─ emit(agent_start)
   ├─ emit(turn_start)
   │
   ├─ OUTER LOOP (follow-up continuation)
   │   │
   │   ├─ Check FollowUpQueue → if messages, add as pending, continue outer loop
   │   │
   │   ├─ INNER LOOP (tool calls + steering)
   │   │   ├─ Drain PriorityQueue → inject at FRONT of message list
   │   │   │   └── If PriorityQueue had messages:
   │   │   │       └── ToolCallManager.cancel() → abort in-progress tool calls
   │   │   │
   │   │   ├─ streamAssistantResponse()
   │   │   ├─ Extract tool calls
   │   │   ├─ If tool calls:
   │   │   │   ├─ ToolCallManager.submit(tool_calls)
   │   │   │   ├─ ToolCallManager executes
   │   │   │   └── Results → Message API → agent loop
   │   │   └─ Check PriorityQueue again → repeat inner loop if priority messages
   │   │
   │   └─ Check FollowUpQueue → if messages, continue outer loop
   │
   └─ emit(agent_end)
```

### Rationale

**Why two queues instead of one?**  
- A single queue with priority levels still requires a decision point: "do I cancel or wait?"
- Separate queues make the intent explicit at the API level — callers choose the right queue
- PriorityQueue is checked at every inner loop iteration; FollowUpQueue only at outer loop boundary
- Cancellation semantics are different — PriorityQueue cancels tool calls, FollowUpQueue does not

**Why check PriorityQueue at every inner loop iteration?**  
- User interruption should be responsive — the agent shouldn't continue executing irrelevant tool calls
- Tool calls can be expensive (API calls, file operations) — cancelling early saves resources
- The LLM should respond to the user's latest intent, not stale instructions

**Why check FollowUpQueue only at outer loop boundary?**  
- Follow-up instructions are for the **next** task, not the current one
- Interrupting mid-task wastes work and creates confusing LLM behavior
- Natural boundary: "current task done, what's next?"

**Why use ConcurrentQueue + Notifier?**  
- `ConcurrentQueue` provides lock-free enqueue/dequeue
- `Notifier` (from foundation_core::synca or similar) allows the agent task to wake immediately when a message arrives, rather than polling
- This matches valtron's `EventReadiness` pattern — the queue implements `EventReadiness` and the agent task uses `TaskStatus::Depends(signal)`

## Alternatives Considered

### Single queue with priority levels
- **Pros:** Simpler API (one queue)
- **Cons:** Still needs cancellation logic at every check point; unclear semantics for "medium priority"
- **Rejected because:** Two distinct use cases (interrupt vs defer) are better served by explicit separation

### Channel-based queues (mpsc)
- **Pros:** Built-in async support, backpressure
- **Cons:** `mpsc` is single-consumer; we need multiple listeners (agent loop, monitoring tasks)
- **Rejected because:** `ConcurrentQueue` + `Broadcaster` supports multiple consumers

### Signal-based interruption only (no queue)
- **Pros:** Simpler — just a flag to interrupt
- **Cons:** Loses the actual steering message content; agent knows to stop but not what to do instead
- **Rejected because:** Interruption must carry the new instruction, not just a stop signal
