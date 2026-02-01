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

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification/feature MUST follow token and context optimization protocols.**

### Machine-Optimized Prompts (Rule 14)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification/feature finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

### Context Compaction (Rule 15)

**Sub-Agents MUST** (before starting work):
1. Read machine_prompt.md and PROGRESS.md
2. Generate `COMPACT_CONTEXT.md`:
   - Embed machine_prompt.md content for current task
   - Extract current status from PROGRESS.md
   - List files for current task only (500-800 tokens)
3. CLEAR entire context
4. RELOAD from COMPACT_CONTEXT.md only
5. Proceed with 97% context reduction (180K→5K tokens)

**After PROGRESS.md Updates**:
- Regenerate COMPACT_CONTEXT.md (re-embed machine_prompt content)
- Clear and reload
- Maintain minimal context

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task
- Contains ONLY current task (no history)
- Deleted when task completes
- Rewritten from scratch for next task

**See**:
- Rule 14: .agents/rules/14-machine-optimized-prompts.md
- Rule 15: .agents/rules/15-instruction-compaction.md

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
    pub request: Option<PreparedRequest>,
    pub resolver: R,
    pub remaining_redirects: u8,
    pub response_sender: Option<ResponseSender>,
}

impl<R: DnsResolver + Send + 'static> ExecutionAction for RedirectAction<R> {
    fn apply(
        &mut self,
        parent_key: Entry,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(request) = self.request.take() {
            let redirect_task = HttpRequestTask::new_redirect(
                request,
                self.resolver.clone(),
                self.remaining_redirects,
            );

            spawn_builder(engine)
                .with_parent(parent_key)
                .with_task(redirect_task)
                .lift()?;
        }
        Ok(())
    }
}
```

**Key Points**:
- `&mut self` (not `self`) - allows multiple apply calls, idempotent via Option::take()
- `engine: BoxedExecutionEngine` (not `executor`)
- Fields use `Option` for take() pattern
- Use `.lift()` for priority spawning

#### TlsUpgradeAction

```rust
pub struct TlsUpgradeAction {
    pub connection: Option<Connection>,
    pub sni: String,
    pub on_complete: Option<TlsCompletionSender>,
}

impl ExecutionAction for TlsUpgradeAction {
    fn apply(
        &mut self,
        parent_key: Entry,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let (Some(connection), Some(sender)) = (self.connection.take(), self.on_complete.take()) {
            let tls_task = TlsHandshakeTask::new(connection, self.sni.clone(), sender);

            spawn_builder(engine)
                .with_parent(parent_key)
                .with_task(tls_task)
                .lift()?;
        }
        Ok(())
    }
}
```

**Key Points**:
- `&mut self` (not `self`) - allows multiple apply calls
- `engine: BoxedExecutionEngine` (not `executor`)
- Fields use `Option` for take() pattern (idempotent)
- Use `.lift()` for priority spawning

#### HttpClientAction Enum

```rust
pub enum HttpClientAction<R: DnsResolver + Send + 'static> {
    None,
    Redirect(RedirectAction<R>),
    TlsUpgrade(TlsUpgradeAction),
}

impl<R: DnsResolver + Send + 'static> ExecutionAction for HttpClientAction<R> {
    fn apply(
        &mut self,
        key: Entry,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<()> {
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

**CORRECTED PATTERN** (from valtron/executors/unified.rs):

```rust
use crate::synca::mpp::RecvIterator;
use crate::valtron::{single, TaskIterator, TaskStatus, ExecutionAction, GenericResult};

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use crate::valtron::multi;

/// Execute a task using the appropriate executor for the current platform/features.
///
/// Returns an iterator over TaskStatus values. Users must drive execution by calling:
/// - `single::run_once()` - Make progress once (for manual control)
/// - `single::run_until_complete()` - Run until all tasks done
///
/// Use `ReadyValues::new(iter)` to filter for only Ready values.
pub(crate) fn execute<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        execute_single(task)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            execute_multi(task)
        }

        #[cfg(not(feature = "multi"))]
        {
            execute_single(task)
        }
    }
}

/// Execute using single-threaded executor.
fn execute_single<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use std::time::Duration;

    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(5))?;

    Ok(iter)
}

/// Execute using multi-threaded executor (feature-gated).
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use std::time::Duration;

    let iter = multi::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(1))?;

    Ok(iter)
}
```

**Key Points**:
- Returns `RecvIterator<TaskStatus<...>>` (not direct Ready value)
- Use `.schedule_iter(Duration)` to spawn task and get iterator
- Users drive execution with `single::run_once()` or `single::run_until_complete()`
- Use `ReadyValues::new(iter)` to filter for Ready values only
- All functions are internal (pub(crate) or private)
```

| Platform | Feature | Executor Used     |
| -------- | ------- | ----------------- |
| WASM     | any     | `single` (always) |
| Native   | none    | `single`          |
| Native   | `multi` | `multi`           |

### Usage Pattern (from unified.rs tests)

**Returns RecvIterator** (not direct Ready value):
```rust
use crate::valtron::ReadyValues;
use crate::valtron::single;

let task = HttpRequestTask::new(...);

// Execute returns iterator over TaskStatus values
let status_iter = execute(task)?;

// Option 1: Drive manually with run_once()
let mut ready_values = ReadyValues::new(status_iter);
loop {
    single::run_once();  // Make progress
    if let Some(value) = ready_values.next() {
        // Process ready value
        break;
    }
}

// Option 2: Run until complete
let status_iter = execute(task)?;
single::run_until_complete();  // Drive to completion
let values: Vec<_> = ReadyValues::new(status_iter)
    .filter_map(|item| item.inner())
    .collect();
```

**Key Points**:
- `execute()` returns `RecvIterator<TaskStatus<...>>` (not `T::Ready`)
- Users call `single::run_once()` or `single::run_until_complete()` to drive
- Use `ReadyValues::new(iter)` wrapper to filter for Ready values
- `.schedule_iter(Duration)` spawns task and returns iterator

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
- **MUST READ** `valtron/executors/unified.rs` for execute() wrapper pattern (PRIMARY REFERENCE)
- **MUST READ** `valtron/executors/actions.rs` for ExecutionAction patterns (PRIMARY REFERENCE)
- **MUST READ** `valtron/executors/task.rs` for TaskIterator trait
- **MUST READ** `valtron/executors/executor.rs` for ExecutionAction, TaskStatus
- **MUST READ** `valtron/executors/single/mod.rs` for single::spawn() usage
- **MUST READ** `valtron/executors/multi/mod.rs` for multi::spawn() usage

### Implementation Guidelines

**CRITICAL - ExecutionAction Signature** (Updated 2026-02-01):

The correct `ExecutionAction::apply()` signature is:
```rust
fn apply(
    &mut self,           // Mutable borrow (NOT self - allows reuse)
    key: Entry,          // Parent task entry
    engine: BoxedExecutionEngine,  // Execution engine (NOT executor)
) -> GenericResult<()>
```

**Pattern**: Use `Option` fields with `.take()` for idempotent apply:
```rust
pub struct MyAction {
    data: Option<SomeData>,  // Option allows take()
}

impl ExecutionAction for MyAction {
    fn apply(&mut self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        if let Some(data) = self.data.take() {  // Take makes it idempotent
            let task = SomeTask::new(data);
            spawn_builder(engine)  // Use 'engine' parameter
                .with_parent(key)
                .with_task(task)
                .lift()?;  // Or .schedule() or .broadcast()
        }
        Ok(())
    }
}
```

**Reference Implementation**: See `valtron/executors/actions.rs`:
- `SpawnWithBroadcast::apply()` - uses `.broadcast()` for global queue
- `SpawnWithSchedule::apply()` - uses `.schedule()` for local queue
- Both use `&mut self`, `engine` parameter, `Option::take()` pattern

**Spawn Methods**:
- `.lift()` - Priority spawn (use for redirects, important tasks)
- `.schedule()` - Normal spawn (use for regular tasks)
- `.broadcast()` - Global queue spawn (use for cross-thread tasks)

**Implementation Guidelines**:
- All types are INTERNAL (pub(crate) or private)
- Use generic type parameters for DnsResolver
- State machine must be non-blocking
- Redirects use TaskStatus::Spawn, not blocking loops
- Feature gates for multi-threaded executor
- Follow Option::take() pattern from valtron/executors/actions.rs

---

_Created: 2026-01-18_
_Last Updated: 2026-02-01 (Updated ExecutionAction signatures and executor wrapper with unified.rs patterns)_
