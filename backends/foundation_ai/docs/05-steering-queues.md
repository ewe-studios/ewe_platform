# Fundamentals 05 — Steering queues and the Depends protocol

How the agent prioritizes work, manages concurrent tasks, and coordinates
dependencies between operations.

---

## 1. The problem

An agent session has multiple things competing for attention:
- User messages (highest priority)
- Tool execution results
- Memory generation tasks
- Follow-up questions from the model
- System health checks

A simple FIFO queue doesn't work because some tasks depend on others
completing first.

## 2. PriorityQueue

Tasks are ordered by priority:

```rust
pub enum Priority {
    Critical,  // User messages, errors
    High,      // Tool results, memory triggers
    Normal,    // Follow-ups, periodic tasks
    Low,       // Cleanup, telemetry
}
```

The queue always dequeues the highest-priority item first. Within the same
priority level, FIFO ordering is preserved.

## 3. FollowUpQueue

For follow-up tasks generated during generation:

```rust
// Model generates: "I need to read the config file first"
// → FollowUp queued with dependency on the read operation
```

Follow-ups are processed after the current generation completes, in order.

## 4. Depends — readiness coordination

`Depends` tracks whether a task is ready to execute:

```rust
pub enum Depends {
    Ready,              // Can execute now
    WaitingOn(Vec<String>), // Waiting for these task IDs
}
```

The scheduler checks `Depends` before dequeuing:
- `Ready` → execute
- `WaitingOn(ids)` → skip until all `ids` are complete

## 5. Integration with the agentic loop

```
User message → enqueue(Critical)
  → dequeue → generate
    → tool calls → enqueue(High) for each tool
    → tool results → satisfy dependencies
    → follow-ups → enqueue(Normal)
  → memory triggers → enqueue(High)
  → loop detection → enqueue(Critical) if detected
```

The loop runs until the queue is empty AND generation is complete.

## 6. Backpressure

When the queue grows too large (> N pending items), the agent:
1. Pauses new generation
2. Processes pending tool calls
3. Clears follow-up queue
4. Resumes generation

This prevents memory exhaustion in long sessions with many tool calls.
