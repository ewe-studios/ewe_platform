---
feature: task-iterator
description: Internal TaskIterator implementation, ExecutionAction spawners, and feature-gated executor wrapper
status: pending
depends_on:
  - valtron-utilities
  - foundation
  - connection
  - request-response
estimated_effort: medium
created: 2026-01-18
last_updated: 2026-01-19
---

# TaskIterator Feature

## Overview

Create the internal async machinery for the HTTP 1.1 client. This feature implements the `TaskIterator` trait for HTTP requests, custom `ExecutionAction` spawners for redirects and TLS upgrades, and the feature-gated executor wrapper.

**IMPORTANT**: All types in this feature are INTERNAL. Users should NOT interact with TaskIterator, TaskStatus, or executor details directly.

## Dependencies

This feature depends on:
- `valtron-utilities` - Uses reusable ExecutionAction types, unified executor, state machine helpers
- `foundation` - Uses HttpClientError for errors
- `connection` - Uses HttpClientConnection, ParsedUrl
- `request-response` - Uses PreparedRequest, ResponseIntro

This feature is required by:
- `public-api` - Uses execute_task() internally

## Valtron Integration

Key Valtron types to use:

| Type | Purpose |
|------|---------|
| `TaskIterator` | Core iterator trait for async-like tasks |
| `TaskStatus` | Ready/Pending/Delayed/Spawn states |
| `ExecutionAction` | Trait for spawnable actions |
| `NoSpawner`/`NoAction` | Default no-op spawner type |
| `DoNext` | Wrapper for TaskIterator as ExecutionIterator |
| `single::spawn()` | Single-threaded task scheduling |
| `multi::spawn()` | Multi-threaded task scheduling |
| `ExecutionEngine.lift()` | Priority task scheduling |

## Requirements

### Custom Spawners (ExecutionAction)

#### RedirectAction

```rust
pub struct RedirectAction<R: DnsResolver + Send + 'static> {
    pub request: PreparedRequest,
    pub resolver: R,
    pub remaining_redirects: u8,
    pub response_sender: Option<ResponseSender>,
}

impl<R: DnsResolver + Send + 'static> ExecutionAction for RedirectAction<R> {
    fn apply(self, parent_key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        let redirect_task = HttpRequestTask::new_redirect(...);
        engine.lift(Box::new(DoNext::new(redirect_task)), Some(parent_key))?;
        Ok(())
    }
}
```

#### TlsUpgradeAction

```rust
pub struct TlsUpgradeAction {
    pub connection: Connection,
    pub sni: String,
    pub on_complete: TlsCompletionSender,
}

impl ExecutionAction for TlsUpgradeAction {
    fn apply(self, parent_key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        let tls_task = TlsHandshakeTask::new(...);
        engine.lift(Box::new(DoNext::new(tls_task)), Some(parent_key))?;
        Ok(())
    }
}
```

#### HttpClientAction Enum

```rust
pub enum HttpClientAction<R: DnsResolver + Send + 'static> {
    None,
    Redirect(RedirectAction<R>),
    TlsUpgrade(TlsUpgradeAction),
}

impl<R: DnsResolver + Send + 'static> ExecutionAction for HttpClientAction<R> {
    fn apply(self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        match self {
            Self::None => Ok(()),
            Self::Redirect(action) => action.apply(key, engine),
            Self::TlsUpgrade(action) => action.apply(key, engine),
        }
    }
}
```

### HttpRequestTask (TaskIterator)

```rust
pub struct HttpRequestTask<R: DnsResolver + Send + 'static> {
    state: HttpRequestState,
    resolver: R,
    request: PreparedRequest,
    remaining_redirects: u8,
    redirect_receiver: Option<ResponseReceiver>,
}

pub enum HttpRequestState {
    Init,
    Connecting,
    TlsHandshake,
    SendingRequest,
    ReceivingIntro,
    ReceivingHeaders,
    ReceivingBody,
    AwaitingRedirect,
    Done,
    Error(HttpClientError),
}

impl<R: DnsResolver + Send + 'static> TaskIterator for HttpRequestTask<R> {
    type Pending = HttpRequestState;
    type Ready = ClientResponse;
    type Spawner = HttpClientAction<R>;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}
```

### Feature-Gated Executor Wrapper

```rust
pub(crate) fn execute_task<T>(task: T) -> Result<T::Ready, ExecutorError>
where
    T: TaskIterator + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    { execute_single(task) }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        { execute_multi(task) }

        #[cfg(not(feature = "multi"))]
        { execute_single(task) }
    }
}
```

| Platform | Feature | Executor Used |
|----------|---------|---------------|
| WASM | any | `single` (always) |
| Native | none | `single` |
| Native | `multi` | `multi` |

## Implementation Details

### File Structure

```
client/
├── actions.rs   (NEW - ExecutionAction implementations)
├── task.rs      (NEW - HttpRequestTask, HttpRequestState)
├── executor.rs  (NEW - Feature-gated executor selection)
└── ...
```

## Success Criteria

- [ ] `RedirectAction` implements ExecutionAction correctly
- [ ] `TlsUpgradeAction` implements ExecutionAction correctly
- [ ] `HttpClientAction` enum combines all actions
- [ ] `HttpRequestTask` implements TaskIterator
- [ ] State machine handles all states correctly
- [ ] Redirect spawning via TaskStatus::Spawn works
- [ ] `execute_single()` works with valtron::single
- [ ] `execute_multi()` works with valtron::multi (feature-gated)
- [ ] WASM always uses single executor
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- task
cargo test --package foundation_core -- executor
cargo build --package foundation_core
cargo build --package foundation_core --features multi
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** foundation, connection, request-response features are complete
- **MUST READ** `valtron/executors/task.rs` for TaskIterator trait
- **MUST READ** `valtron/executors/executor.rs` for ExecutionAction, TaskStatus
- **MUST READ** `valtron/executors/single/mod.rs` for single::spawn() usage
- **MUST READ** `valtron/executors/multi/mod.rs` for multi::spawn() usage

### Implementation Guidelines
- All types are INTERNAL (pub(crate) or private)
- Use generic type parameters for DnsResolver
- State machine must be non-blocking
- Redirects use TaskStatus::Spawn, not blocking loops
- Feature gates for multi-threaded executor

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
