---
feature: task-iterator
description: Internal TaskIterator implementation, ExecutionAction spawners, and feature-gated executor wrapper
status: pending
priority: high
depends_on:
  - valtron-utilities
  - foundation
  - connection
  - request-response
estimated_effort: medium
created: 2026-01-18
last_updated: 2026-01-24
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
      - ./templates/
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
---

# TaskIterator Feature

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this feature MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** in related modules to understand patterns
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific conventions
4. ✅ **Read parent specification** (`../requirements.md`) for high-level context
5. ✅ **Read module documentation** for modules this feature touches
6. ✅ **Check dependencies** by reading other feature files referenced in `depends_on`
7. ✅ **Follow discovered patterns** consistently with existing codebase

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume patterns based on typical practices without checking this codebase
- ❌ Implement without searching for similar features first
- ❌ Apply generic solutions without verifying project conventions
- ❌ Guess at naming conventions, file structures, or patterns
- ❌ Use pretraining knowledge without validating against actual project code

### Retrieval Checklist

Before implementing, answer these questions by reading code:
- [ ] What similar features exist in this project? (use Grep to find)
- [ ] What patterns do they follow? (read their implementations)
- [ ] What naming conventions are used? (observed from existing code)
- [ ] How are errors handled in similar code? (check error patterns)
- [ ] What testing patterns exist? (read existing test files)
- [ ] Are there existing helper functions I can reuse? (search thoroughly)

### Enforcement

- Show your retrieval steps in your work report
- Reference specific files/patterns you discovered
- Explain how your implementation matches existing patterns
- "I assumed..." responses will be rejected - only "I found in [file]..." accepted

---

## Overview

Create the internal async machinery for the HTTP 1.1 client. This feature implements the `TaskIterator` trait for HTTP requests, custom `ExecutionAction` spawners for redirects and TLS upgrades, and the feature-gated executor wrapper.

**IMPORTANT**: All types in this feature are INTERNAL. Users should NOT interact with TaskIterator, TaskStatus, or executor details directly.

## Dependencies

This feature depends on:

- `valtron-utilities` - Uses SpawnWithLift, SpawnWithSchedule, SpawnWithBroadcast (reusable ExecutionAction types), unified executor, state machine helpers
- `foundation` - Uses HttpClientError for errors
- `connection` - Uses HttpClientConnection, ParsedUrl
- `request-response` - Uses PreparedRequest, ResponseIntro

This feature is required by:

- `public-api` - Uses execute_task() internally

**Note on Action Types**: This feature defines custom `ExecutionAction` implementations (RedirectAction, TlsUpgradeAction) specific to HTTP client needs. These are different from the reusable action types in valtron-utilities (SpawnWithLift, SpawnWithSchedule, SpawnWithBroadcast) which provide general-purpose spawning strategies. The HttpClientAction enum can compose both custom and reusable actions as needed.

## Valtron Integration

Key Valtron types to use:

| Type                     | Purpose                                       |
| ------------------------ | --------------------------------------------- |
| `TaskIterator`           | Core iterator trait for async-like tasks      |
| `TaskStatus`             | Ready/Pending/Delayed/Spawn states            |
| `ExecutionAction`        | Trait for spawnable actions                   |
| `NoSpawner`/`NoAction`   | Default no-op spawner type                    |
| `DoNext`                 | Wrapper for TaskIterator as ExecutionIterator |
| `single::spawn()`        | Single-threaded task scheduling               |
| `multi::spawn()`         | Multi-threaded task scheduling                |
| `ExecutionEngine.lift()` | Priority task scheduling                      |

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

        spawn_builder(executor)
            .with_parent(parent_key.clone())
            .with_task(redirect_task)
            .lift()?;

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
        spawn_builder(executor)
            .with_parent(parent_key.clone())
            .with_task(tls_task)
            .lift()?;
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

| Platform | Feature | Executor Used     |
| -------- | ------- | ----------------- |
| WASM     | any     | `single` (always) |
| Native   | none    | `single`          |
| Native   | `multi` | `multi`           |

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

_Created: 2026-01-18_
_Last Updated: 2026-01-18_
