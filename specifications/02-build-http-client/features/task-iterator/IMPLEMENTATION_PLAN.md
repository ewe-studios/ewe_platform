# task-iterator Implementation Plan

## Overview

Complete the HTTP client task-iterator feature by implementing all TODO stubs. Use `ExecutionTaskIteratorBuilder` with `.lift_ready_iter()` / `.schedule_iter()` methods instead of manual channel management.

**Phase 1 Goal**: Basic HTTP GET requests working for `http://` URLs only.

## Key Insights from Valtron

### ExecutionTaskIteratorBuilder Pattern

The builder provides clean APIs for spawning tasks and getting results:

```rust
// Returns RecvIterator<TaskStatus<Ready, Pending, Action>>
let iter = spawn_builder::<ConnectionTask, NoAction>(engine)
    .with_parent(key)
    .with_task(task)
    .lift_ready_iter(Duration::from_nanos(5))?;

// Consume Ready values from iterator
for status in iter {
    if let TaskStatus::Ready(connection) = status {
        // Use connection
    }
}
```

**Methods Available**:
- `.lift_ready_iter(wait)` - Only Ready values, priority queue
- `.schedule_ready_iter(wait)` - Only Ready values, normal queue
- `.lift_iter(wait)` - All TaskStatus values, priority queue
- `.schedule_iter(wait)` - All TaskStatus values, normal queue
- `.stream_lift_iter(wait)` - Simplified Stream<Ready, Pending> type

**No manual channels needed!** The builder internally uses `ReadyConsumingIter` or `ConsumingIter` which manage the queue.

## Revised Implementation Approach

### Connection Pattern (NEW - Simpler)

```rust
// State: Connecting - spawn ConnectionTask
HttpRequestState::Connecting => {
    let url = self.request.as_ref().unwrap().url.clone();
    let task = ConnectionTask::new(url, self.resolver.clone(), Some(Duration::from_secs(30)));

    // Get engine from somewhere... need to think about this
    let iter = spawn_builder::<ConnectionTask<R>, NoAction>(engine)
        .with_parent(parent_key)
        .with_task(task)
        .lift_ready_iter(Duration::from_nanos(5))?;

    self.connection_iter = Some(iter);
    self.state = HttpRequestState::WaitingConnection;
    Some(TaskStatus::Pending(HttpRequestState::WaitingConnection))
}

// State: WaitingConnection - poll iterator
HttpRequestState::WaitingConnection => {
    if let Some(ref mut iter) = self.connection_iter {
        if let Some(TaskStatus::Ready(connection)) = iter.next() {
            self.connection = Some(connection);
            self.connection_iter = None;
            self.state = HttpRequestState::SendingRequest;
            Some(TaskStatus::Pending(HttpRequestState::SendingRequest))
        } else {
            // Still waiting
            Some(TaskStatus::Pending(HttpRequestState::WaitingConnection))
        }
    } else {
        // No iterator? Error
        self.state = HttpRequestState::Error;
        None
    }
}
```

### Problem: Where does `engine` and `parent_key` come from?

**TaskIterator::next()** signature is:
```rust
fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
```

No access to `engine` or `parent_key`!

### Solution: Use TaskStatus::Spawn with custom action

The **proper pattern** is to return `TaskStatus::Spawn(action)` where the action has the engine/key:

```rust
// State: Connecting - create spawn action
HttpRequestState::Connecting => {
    let url = self.request.as_ref().unwrap().url.clone();
    let task = ConnectionTask::new(url, self.resolver.clone(), Some(Duration::from_secs(30)));

    // Create action that will spawn the task
    let action = HttpClientAction::SpawnConnection(SpawnConnectionAction {
        task: Some(task),
        // Store reference to get results later?
    });

    self.state = HttpRequestState::WaitingConnection;
    Some(TaskStatus::Spawn(action))
}
```

But how do we get the results back from the spawned task?

## Key Insight: No Channels, No Custom Actions Needed!

**CRITICAL SIMPLIFICATION**: We DON'T need:
- ❌ SpawnConnectionAction (unnecessary abstraction)
- ❌ Channels/Sender/Receiver (over-complicating)
- ❌ Custom ExecutionAction implementations (not needed here)

**Why?** Because:
1. **Spawning happens OUTSIDE HttpRequestTask** - The caller spawns ConnectionTask
2. **RecvIterator passed directly** - No need to pass it through actions/channels
3. **HttpRequestTask just consumes** - It polls the iterator, doesn't spawn

### The Simplified Pattern

```rust
// OUTSIDE HttpRequestTask (in the code that creates it):
let connection_task = ConnectionTask::new(url.clone(), resolver.clone(), timeout);
let connection_iter = spawn_builder::<ConnectionTask<R>, NoAction>(engine)
    .with_task(connection_task)
    .lift_ready_iter(Duration::from_nanos(100))?;

// Create HttpRequestTask WITH the iterator
let http_task = HttpRequestTask::new_with_connection_iter(
    request,
    resolver,
    max_redirects,
    connection_iter, // Pass directly!
);

// HttpRequestTask stores RecvIterator and polls it
```

**Inside HttpRequestTask**:
```rust
pub struct HttpRequestTask<R: DnsResolver + Send + 'static> {
    state: HttpRequestState,
    resolver: R,
    request: Option<PreparedRequest>,
    connection: Option<HttpClientConnection>,

    // Just store the iterator - no channels!
    connection_iter: Option<RecvIterator<TaskStatus<HttpClientConnection, (), NoAction>>>,
}

impl<R> TaskIterator for HttpRequestTask<R> {
    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state {
            HttpRequestState::WaitingConnection => {
                // Just poll the iterator!
                if let Some(ref mut iter) = self.connection_iter {
                    match iter.next() {
                        Some(TaskStatus::Ready(connection)) => {
                            self.connection = Some(connection);
                            self.connection_iter = None;
                            self.state = HttpRequestState::SendingRequest;
                            Some(TaskStatus::Pending(HttpRequestState::SendingRequest))
                        }
                        Some(_) => {
                            // Still pending
                            Some(TaskStatus::Pending(HttpRequestState::WaitingConnection))
                        }
                        None => {
                            // Connection failed
                            self.state = HttpRequestState::Error;
                            None
                        }
                    }
                } else {
                    self.state = HttpRequestState::Error;
                    None
                }
            }
            // ... other states
        }
    }
}
```

### Benefits

✅ **No unnecessary abstractions** - No SpawnConnectionAction wrapper
✅ **No channel complexity** - Direct iterator passing
✅ **Clearer data flow** - Obvious where iterator comes from
✅ **Less code** - Fewer types, fewer files
✅ **Easier to understand** - Straightforward pattern

### General Principle (Clean Code)

**Avoid unnecessary abstraction layers**:
- If data flows directly from A → B, pass it directly
- Don't create wrapper actions/channels unless they add value
- Ask: "Does this abstraction solve a real problem or just add complexity?"

**When to use ExecutionAction**:
- ✅ When you need to spawn INSIDE a TaskIterator (no other way to get engine)
- ✅ When spawning logic is complex and reusable
- ❌ When spawning happens outside TaskIterator (just call spawn_builder directly)
- ❌ When it's just wrapping a single spawn call with no additional logic

### Better Approach: Simpler MVP

For Phase 1, let's keep it **really simple**:

1. **Don't spawn connection as separate task** - just do it inline (blocking is OK for MVP)
2. Focus on getting the state machine structure right
3. Later phases can optimize with proper task spawning

```rust
// Phase 1: Blocking connection (simple)
HttpRequestState::Connecting => {
    match HttpClientConnection::connect(
        &self.request.as_ref().unwrap().url,
        &self.resolver,
        Some(Duration::from_secs(30)),
    ) {
        Ok(connection) => {
            self.connection = Some(connection);
            self.state = HttpRequestState::SendingRequest;
            Some(TaskStatus::Pending(HttpRequestState::SendingRequest))
        }
        Err(e) => {
            tracing::error!("Connection failed: {}", e);
            self.state = HttpRequestState::Error;
            None
        }
    }
}
```

**Phase 2** can add proper task spawning using the ExecutionTaskIteratorBuilder pattern.

## Phase 1 Implementation (Revised - Two Approaches)

### Approach A: Full Valtron Pattern (Recommended)

**Spawn connection externally, pass RecvIterator to HttpRequestTask**

**Pros**:
- ✅ Proper non-blocking pattern
- ✅ No manual channels
- ✅ Follows valtron best practices
- ✅ Connection runs on executor threads

**Cons**:
- Requires changing how HttpRequestTask is created
- Slightly more complex initial setup

**Implementation**:
```rust
// External code spawns connection
let connection_task = ConnectionTask::new(url, resolver.clone(), timeout);
let connection_iter = spawn_builder::<ConnectionTask<R>, NoAction>(engine)
    .with_task(connection_task)
    .lift_ready_iter(Duration::from_nanos(100))?;

// Pass iterator to HttpRequestTask
let http_task = HttpRequestTask::new_with_connection_iter(
    request,
    resolver,
    max_redirects,
    Some(connection_iter),
);

// HttpRequestTask starts in WaitingConnection state, polls iterator
```

### Approach B: Blocking Connection (Simple MVP)

**Connect inline (blocking)**

**Pros**:
- ✅ Simplest to implement
- ✅ Fewer states
- ✅ Self-contained HttpRequestTask

**Cons**:
- ❌ Blocks executor thread during connection
- ❌ Not ideal for production
- ❌ Doesn't showcase valtron patterns

**Implementation**:
```rust
// In HttpRequestTask::next()
HttpRequestState::Connecting => {
    match HttpClientConnection::connect(&url, &self.resolver, timeout) {
        Ok(connection) => {
            self.connection = Some(connection);
            self.state = HttpRequestState::SendingRequest;
            Some(TaskStatus::Pending(HttpRequestState::SendingRequest))
        }
        Err(e) => {
            self.state = HttpRequestState::Error;
            None
        }
    }
}
```

### Recommendation: Use Approach A

**Why**:
- Demonstrates proper valtron usage
- Better foundation for Phase 2+
- Only slightly more complex
- Production-ready pattern

### States (Approach A - Phase 1)
1. **Init** - Validate request
2. **WaitingConnection** - Poll connection_iter for Ready
3. **SendingRequest** - Write HTTP request bytes
4. **ReceivingIntro** - Read and parse status line
5. **Done** - Complete

### States (Approach B - Phase 1)
1. **Init** - Validate request
2. **Connecting** - Connect (blocking)
3. **SendingRequest** - Write HTTP request bytes
4. **ReceivingIntro** - Read and parse status line
5. **Done** - Complete

### Files to Create

**None** - Use existing connection.rs for ConnectionTask (defer to Phase 2)

### Files to Modify

1. **task.rs**:
   - Update HttpRequestTask struct (add connection field)
   - Implement 5 state handlers in next()
   - Add WASM cfg guard at file level

2. **actions.rs**:
   - Add WASM cfg guard at file level
   - Keep existing TODO stubs (implement in Phase 2)

3. **mod.rs**:
   - Add WASM cfg guard for task module export

### Implementation Tasks

#### Task 1: Add WASM guards

**Files**: task.rs, actions.rs, mod.rs

Add at top after imports:
```rust
#![cfg(not(target_arch = "wasm32"))]
```

#### Task 2: Update HttpRequestTask struct

**File**: task.rs

```rust
pub struct HttpRequestTask<R: DnsResolver + Send + 'static> {
    state: HttpRequestState,
    resolver: R,
    request: Option<PreparedRequest>,
    remaining_redirects: u8,

    // NEW
    connection: Option<HttpClientConnection>,
}

// Update enum to only have Phase 1 states
pub enum HttpRequestState {
    Init,
    Connecting,
    SendingRequest,
    ReceivingIntro,
    Done,
    Error,
}
```

#### Task 3: Implement state machine

**File**: task.rs, in HttpRequestTask::next()

See detailed state implementations in "Simplified Pattern" section above.

Key points:
- Use existing HttpClientConnection::connect() (blocking is fine)
- Use existing Http11::request().http_render_string() for rendering
- Use existing HttpMachine for parsing response
- Keep states simple and focused

### Verification

```bash
# Build
cargo build --package foundation_core

# Test
cargo test --package foundation_core -- simple_http::client::task

# Clippy
cargo clippy --package foundation_core -- -D warnings

# Format
cargo fmt -- --check
```

### Success Criteria

- [ ] 5 states implemented (Init, Connecting, SendingRequest, ReceivingIntro, Done)
- [ ] HTTP GET request to http://example.com returns ResponseIntro
- [ ] HTTPS returns error (not implemented yet)
- [ ] WASM cfg guards added
- [ ] No clippy warnings
- [ ] All tests pass

## Future Phases

**Phase 2**: Use ExecutionTaskIteratorBuilder for non-blocking connection spawning
**Phase 3**: Add HTTPS/TLS support
**Phase 4**: Add redirects
**Phase 5**: Add body/header parsing

## Time Estimate

**Phase 1**: 2-3 hours (simple blocking HTTP)

---

_Created: 2026-02-02_
_Last Updated: 2026-02-02_
