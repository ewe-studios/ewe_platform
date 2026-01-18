---
feature: task-iterator
completed: 0
uncompleted: 8
last_updated: 2026-01-18
tools:
  - Rust
  - cargo
---

# TaskIterator - Tasks

## Task List

### Custom Spawners (ExecutionAction)
- [ ] Create `client/actions.rs` - Module for custom ExecutionAction implementations
- [ ] Implement `RedirectAction` - spawns redirect request via engine.lift()
- [ ] Implement `TlsUpgradeAction` - spawns TLS handshake as priority task
- [ ] Implement `HttpClientAction` enum combining all actions

### Internal TaskIterator Implementation
- [ ] Create `client/task.rs` - HttpRequestTask implementing TaskIterator (internal)
- [ ] Implement state machine with all states including AwaitingRedirect

### Internal Executor Wrapper
- [ ] Create `client/executor.rs` - Feature-gated executor selection
- [ ] Implement `execute_task()` with WASM detection and feature-gated single/multi selection

## Implementation Order

1. **actions.rs** - ExecutionAction types (depends on request.rs, connection.rs)
2. **task.rs** - HttpRequestTask (depends on actions.rs, all previous)
3. **executor.rs** - Executor wrapper (depends on task.rs)

## Notes

### State Machine States
```rust
pub enum HttpRequestState {
    Init,              // Initial state
    Connecting,        // Resolving DNS, establishing TCP
    TlsHandshake,      // Upgrading to TLS (HTTPS only)
    SendingRequest,    // Writing request bytes
    ReceivingIntro,    // Reading status line
    ReceivingHeaders,  // Reading headers
    ReceivingBody,     // Reading body
    AwaitingRedirect,  // Waiting for spawned redirect task
    Done,              // Complete
    Error(HttpClientError), // Error occurred
}
```

### Redirect Handling Pattern
```rust
// In ReceivingHeaders state, when redirect detected:
if self.is_redirect() && self.remaining_redirects > 0 {
    let (sender, receiver) = create_response_channel();
    self.redirect_receiver = Some(receiver);
    self.state = HttpRequestState::AwaitingRedirect;

    let redirect_action = RedirectAction { ... };
    return Some(TaskStatus::Spawn(HttpClientAction::Redirect(redirect_action)));
}
```

### Executor Selection Pattern
```rust
#[cfg(target_arch = "wasm32")]
fn execute_task<T>(...) { execute_single(task) }

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_task<T>(...) { execute_multi(task) }

#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
fn execute_task<T>(...) { execute_single(task) }
```

### Key Valtron Imports
```rust
use crate::valtron::{
    TaskIterator, TaskStatus, ExecutionAction,
    BoxedExecutionEngine, GenericResult, DoNext,
    single, multi,
};
use crate::synca::Entry;
```

---
*Last Updated: 2026-01-18*
