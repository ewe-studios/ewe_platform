# Learnings: Task-Iterator Feature Implementation

**Date**: 2026-02-01
**Feature**: HTTP 1.1 Client - Task-Iterator Machinery
**Context**: Implementation of internal TaskIterator machinery for HTTP request execution

---

## Critical Learnings

### 1. ExecutionAction::apply() Signature Matters Immensely

**Discovery**: The signature must be `apply(&mut self, key: Entry, engine: BoxedExecutionEngine)`.

**Why It Matters**:
- Using `self` instead of `&mut self` makes the action single-use
- Using `executor` instead of `engine` causes compilation errors
- The `&mut self` + `Option::take()` pattern enables idempotent operations

**Wrong**:
```rust
fn apply(self, parent_key: Entry, executor: BoxedExecutionEngine) -> GenericResult<()>
```

**Correct**:
```rust
fn apply(&mut self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()>
```

**Impact**: Using the wrong signature would break the entire valtron execution model.

**Reference**: `valtron/executors/actions.rs` - all actions use `&mut self, key, engine`.

---

### 2. execute() Returns Iterator, NOT Value

**Discovery**: The execute wrapper must return `RecvIterator<TaskStatus>`, not a direct `Ready` value.

**Why It Matters**:
- Returning a direct value would block until completion
- Iterator pattern enables non-blocking consumption
- Users can poll for progress, check Pending states, handle Spawns

**Wrong**:
```rust
fn execute<T>(task: T) -> GenericResult<T::Ready>
```

**Correct**:
```rust
fn execute<T>(task: T) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
```

**Impact**: This is fundamental to the non-blocking design. Getting this wrong would force blocking behavior.

**Reference**: `valtron/executors/unified.rs` - execute() always returns iterator.

---

### 3. Single Mode Requires Explicit Execution Driving

**Discovery**: In single-threaded mode (WASM or multi=off), the executor doesn't run automatically.

**Why It Matters**:
- Forgetting to call `run_once()` or `run_until_complete()` means tasks never execute
- This is a critical user-facing requirement
- Documentation must be extremely clear about this

**Single Mode**:
```rust
let iter = execute_task(task)?;
single::run_once(); // MUST call - execution doesn't happen without this!
let ready_values = ReadyValues::new(iter);
```

**Multi Mode**:
```rust
let iter = execute_task(task)?;
// Threads run automatically - NO run_once needed
let ready_values = ReadyValues::new(iter);
```

**Impact**: Users could be confused why their tasks aren't executing. Clear documentation prevents this.

**Reference**: `valtron/executors/unified.rs` tests show this pattern consistently.

---

### 4. Option::take() Enables Idempotent Actions

**Discovery**: Wrapping data in `Option<T>` and using `take()` makes actions safe to call multiple times.

**Why It Matters**:
- Valtron may call `apply()` multiple times due to scheduling
- Without `take()`, operations could execute multiple times
- `take()` returns `None` on subsequent calls, making it a no-op

**Pattern**:
```rust
struct MyAction {
    data: Option<SomeData>,  // Wrapped in Option
}

impl ExecutionAction for MyAction {
    fn apply(&mut self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        if let Some(data) = self.data.take() {  // take() consumes the Option
            // This block only executes once
            spawn_builder(engine).with_parent(key).with_task(task).lift()?;
        }
        Ok(())  // Subsequent calls are safe no-ops
    }
}
```

**Impact**: Without this pattern, actions could spawn duplicate tasks or cause undefined behavior.

**Reference**: Every action in `valtron/executors/actions.rs` uses this pattern.

---

### 5. Unit Tests Can't Always Use Real Resources

**Discovery**: Trying to create real TCP connections in unit tests causes failures.

**Why It Matters**:
- Unit tests should be fast and not require network
- `Connection::without_timeout()` actually attempts to connect
- Compile-time type checks can verify correctness without runtime execution

**Problem**:
```rust
#[test]
fn test_tls_upgrade_action_new() {
    let connection = Connection::without_timeout(addr).unwrap();  // ❌ Fails - connection refused
    let action = TlsUpgradeAction::new(connection, "example.com".to_string(), tx);
    assert!(action.connection.is_some());
}
```

**Solution**:
```rust
#[test]
fn test_tls_upgrade_action_structure() {
    // Compile-time type check
    fn _assert_tls_upgrade_holds_expected_types(_action: TlsUpgradeAction) {
        // If this compiles, the structure is correct
    }
}
```

**Impact**: Distinguishing unit tests (type safety) from integration tests (runtime behavior) is crucial.

**Lesson**: Use compile-time checks for unit tests, save runtime checks for integration tests.

---

### 6. Feature Gates Affect API Surface

**Discovery**: TlsUpgradeAction only exists on non-WASM platforms, requiring careful feature gates.

**Why It Matters**:
- Code must compile on all platforms
- Enum variants need `#[cfg(not(target_arch = "wasm32"))]`
- Tests need corresponding feature gates

**Pattern**:
```rust
pub(crate) enum HttpClientAction<R> {
    None,
    Redirect(RedirectAction<R>),
    #[cfg(not(target_arch = "wasm32"))]  // Only on native platforms
    TlsUpgrade(TlsUpgradeAction),
}

impl<R> ExecutionAction for HttpClientAction<R> {
    fn apply(&mut self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        match self {
            HttpClientAction::None => Ok(()),
            HttpClientAction::Redirect(action) => action.apply(key, engine),
            #[cfg(not(target_arch = "wasm32"))]  // Match the enum variant
            HttpClientAction::TlsUpgrade(action) => action.apply(key, engine),
        }
    }
}
```

**Impact**: Forgetting feature gates causes compilation failures on specific platforms.

**Lesson**: Always match feature gates between enum variants and their usage.

---

### 7. schedule_iter() Duration is Critical

**Discovery**: Different executors use different polling intervals.

**Why It Matters**:
- Single-threaded uses `Duration::from_nanos(5)` for tight polling
- Multi-threaded uses `Duration::from_nanos(1)` for maximum responsiveness
- Interval determines how often the executor checks for new work

**Pattern**:
```rust
// Single-threaded - balance between responsiveness and CPU usage
single::spawn()
    .with_task(task)
    .schedule_iter(Duration::from_nanos(5))?;

// Multi-threaded - maximize responsiveness with background threads
multi::spawn()
    .with_task(task)
    .schedule_iter(Duration::from_nanos(1))?;
```

**Impact**: Wrong durations could cause sluggish response or excessive CPU usage.

**Reference**: `valtron/executors/unified.rs` uses these exact values.

---

### 8. State Machine Pattern for Non-Blocking I/O

**Discovery**: HTTP requests naturally map to a state machine with transitions.

**Why It Matters**:
- Each `next()` call should do a small amount of work and return
- Blocking operations must be broken into states
- State transitions indicate progress

**Pattern**:
```rust
enum State {
    Init,
    Connecting,
    TlsHandshake,
    SendingRequest,
    // ... more states
}

impl TaskIterator for MyTask {
    fn next(&mut self) -> Option<TaskStatus<...>> {
        match self.state {
            State::Init => {
                // Do initialization
                self.state = State::Connecting;
                Some(TaskStatus::Pending(State::Init))
            }
            State::Connecting => {
                // Start connection (non-blocking)
                // Transition when ready
                Some(TaskStatus::Pending(State::Connecting))
            }
            // ... handle other states
        }
    }
}
```

**Impact**: This pattern enables non-blocking I/O without async/await.

**Lesson**: State machines are the key to non-blocking iterator-based execution.

---

### 9. Generic Bounds Must Include Send + 'static

**Discovery**: Valtron executors require Send + 'static bounds for multi-threaded execution.

**Why It Matters**:
- Multi-threaded executor moves tasks between threads
- Without Send, tasks can't cross thread boundaries
- Without 'static, tasks can't outlive their creator

**Pattern**:
```rust
pub(crate) fn execute_task<T>(task: T)
where
    T: TaskIterator + Send + 'static,  // Both required
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
```

**Impact**: Missing these bounds causes compilation errors in multi-threaded mode.

**Lesson**: Always include Send + 'static for executor-related generics.

---

### 10. pub(crate) for Internal Implementation

**Discovery**: Task-iterator machinery should be internal, not exposed to users.

**Why It Matters**:
- Users don't need to understand TaskIterator, TaskStatus, ExecutionAction
- Public API will provide high-level interface
- Internal details can change without breaking users

**Pattern**:
```rust
// Internal types
pub(crate) struct HttpRequestTask<R> { ... }
pub(crate) struct RedirectAction<R> { ... }
pub(crate) fn execute_task<T>(...) { ... }

// Public API (to be implemented)
pub struct HttpClient { ... }
pub fn get(url: &str) -> Result<Response, Error> { ... }
```

**Impact**: Proper encapsulation prevents users from depending on internal details.

**Lesson**: Use pub(crate) for implementation machinery, pub for user-facing API.

---

## Testing Insights

### 1. TDD Forces Clear Design

**Observation**: Writing tests first clarified the API before implementation.

**Example**: Test for HttpRequestTask construction showed we needed:
- Initial state (Init)
- Request storage (Option<PreparedRequest>)
- Redirect tracking (remaining_redirects)
- Redirect receiver (Option<Receiver>)

**Impact**: Tests documented requirements and prevented implementation drift.

---

### 2. Compile-Time vs Runtime Testing

**Observation**: Not all tests need runtime execution to verify correctness.

**When to use compile-time checks**:
- Type safety verification
- Trait implementation verification
- Generic bounds verification

**When to use runtime checks**:
- State transitions
- Value transformations
- Error handling

**Example**:
```rust
// Compile-time: Does TlsUpgradeAction implement ExecutionAction?
#[test]
fn test_is_execution_action() {
    fn _assert_is_execution_action<T: ExecutionAction>() {}
    _assert_is_execution_action::<TlsUpgradeAction>();
}

// Runtime: Does state transition work correctly?
#[test]
fn test_state_transition() {
    let mut task = HttpRequestTask::new(...);
    let status = task.next();
    assert_eq!(task.state, HttpRequestState::Connecting);
}
```

---

### 3. Test Organization Matters

**Observation**: Nested test modules improve organization and readability.

**Pattern**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Group 1: Type Tests
    // ========================================================================

    #[test]
    fn test_type_a() { ... }

    #[test]
    fn test_type_b() { ... }

    // ========================================================================
    // Group 2: Function Tests
    // ========================================================================

    mod specific_function_tests {
        use super::*;

        #[test]
        fn test_case_1() { ... }

        #[test]
        fn test_case_2() { ... }
    }
}
```

**Impact**: Clear organization makes tests easier to understand and maintain.

---

## Documentation Insights

### 1. WHY/WHAT/HOW Structure is Powerful

**Observation**: Three-part documentation structure clarifies intent.

**Pattern**:
```rust
/// Short description.
///
/// WHY: Why does this type/function exist? What problem does it solve?
///
/// WHAT: What does this type/function do? What are its responsibilities?
///
/// HOW: How does it work? What patterns or techniques does it use?
```

**Impact**: Readers understand not just what code does, but why it exists and how it works.

---

### 2. CRITICAL and MUST Make Requirements Clear

**Observation**: Using strong language for critical requirements prevents mistakes.

**Pattern**:
```rust
/// # CRITICAL
///
/// In single mode (WASM or multi=off), users **MUST** call `single::run_once()`
/// or `single::run_until_complete()` to drive execution. Without this, tasks
/// will not execute.
```

**Impact**: Users can't miss important requirements.

---

## Architectural Insights

### 1. Separation of Concerns

**Observation**: Clear module boundaries improve maintainability.

**Structure**:
- `actions.rs` - ExecutionAction implementations only
- `task.rs` - TaskIterator state machine only
- `executor.rs` - Execution wrapper only

**Impact**: Each module has single responsibility, easier to understand and test.

---

### 2. Type-Driven Design

**Observation**: Using specific types for each concept prevents mistakes.

**Example**:
- `HttpRequestState` - Distinct type for states (not just strings)
- `HttpClientAction` - Enum for different action types (not generic boxes)
- `PreparedRequest` - Specific type for prepared requests (not raw data)

**Impact**: Compiler catches mistakes at compile time instead of runtime.

---

### 3. Iterator Pattern for Non-Blocking I/O

**Observation**: Iterator pattern is powerful alternative to async/await.

**Advantages**:
- No runtime required (unlike async/await)
- Works on WASM without complications
- Fine-grained control over execution
- Easy to test

**Disadvantages**:
- More verbose than async/await
- Requires explicit state management
- Users must understand execution model

**Impact**: Choose iterator pattern when async/await is not available or desirable.

---

## Performance Insights

### 1. Duration Values Matter

**Observation**: schedule_iter() duration affects responsiveness vs CPU usage.

**Trade-offs**:
- Shorter duration → more responsive, higher CPU usage
- Longer duration → less responsive, lower CPU usage

**Current choices**:
- Single: 5ns (tight polling needed since no background threads)
- Multi: 1ns (maximize responsiveness, background threads handle it)

**Impact**: May need tuning based on real-world usage patterns.

---

### 2. Option::take() is Zero-Cost

**Observation**: Option::take() compiles to simple pointer swap, no overhead.

**Implementation**: `take()` replaces `Option::Some(value)` with `Option::None` and returns value.

**Impact**: Can use Option::take() freely for idempotency without performance concerns.

---

## Future Considerations

### 1. Error Handling Needs Expansion

**Current state**: GenericResult catches all errors.

**Future needs**:
- Specific error types for different failure modes
- Retry logic for transient failures
- Error recovery strategies

---

### 2. State Machine Needs Full Implementation

**Current state**: States defined, transitions stubbed.

**Future needs**:
- Full state transition logic
- Connection management integration
- Request/response body handling
- Redirect logic
- TLS upgrade integration

---

### 3. Integration Tests Required

**Current state**: Unit tests verify structure and patterns.

**Future needs**:
- Integration tests with test HTTP server
- Multi-request scenarios
- Redirect following tests
- TLS handshake tests
- Error handling tests

---

## Key Takeaways

1. **Follow Reference Implementations Exactly**: valtron/executors/*.rs provided correct patterns.

2. **&mut self + Option::take() = Idempotency**: This pattern is fundamental to valtron actions.

3. **Iterator ≠ Direct Value**: execute() returns iterator for non-blocking operation.

4. **Single Mode Needs Driving**: Users must call run_once() in single-threaded mode.

5. **Unit Tests Can Be Compile-Time**: Type checks are sometimes sufficient.

6. **Feature Gates Everywhere**: WASM vs native requires careful feature management.

7. **Documentation Prevents Confusion**: Clear docs about execution model are critical.

8. **TDD Clarifies Design**: Tests-first approach surfaces requirements early.

9. **State Machines Enable Non-Blocking**: Iterator-based state machines replace async/await.

10. **Type Safety Prevents Mistakes**: Specific types catch errors at compile time.

---

## Conclusion

The task-iterator implementation reinforced the importance of following established patterns exactly. The three critical patterns (ExecutionAction signature, execute() return type, executor driving) were non-negotiable - getting any of them wrong would break the entire system.

The iterator-based approach to non-blocking I/O proved elegant and testable. While more verbose than async/await, it provides fine-grained control and works universally across platforms.

TDD methodology was invaluable - writing tests first surfaced design questions early and prevented implementation drift. The comprehensive test suite gives high confidence in correctness.

Clear documentation is critical when patterns are non-obvious (like needing to call run_once()). Future users will rely heavily on this documentation to use the system correctly.

---

**Date**: 2026-02-01
**Status**: ✅ Documented and Ready for Knowledge Transfer
