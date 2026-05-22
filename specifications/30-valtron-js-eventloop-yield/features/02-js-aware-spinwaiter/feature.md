---
feature: "JS Event Loop Yield — End to End"
description: "Cooperative spin mutex, ProcessController YieldSignal, JSThreadYielder with wasm-bindgen and foundation_wasm backends, executor stop logic, consumer registration"
status: "draft"
priority: "critical"
depends_on: ["01-notify-queue-contract-fix"]
estimated_effort: "large"
created: 2026-05-22
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 15
  total: 15
  completion_percentage: 0%
---

# JS Event Loop Yield — End to End

## Overview

Merge features 02-06 into one end-to-end feature. The valtron executor needs to yield to the JS event loop on wasm32 instead of spinning. This is achieved through:

1. **CooperativeSpinWaiter** (`foundation_nostd`) — generic cooperative spin mutex, returns `WaitStatus::Waiting` immediately
2. **ProcessController::YieldSignal** — associated type on the trait, default `()`, native ignores, JS uses it to stop
3. **JSThreadYielder** (`foundation_core/src/wasm/`) — two backends gated by features, isolated in a `wasm/` submodule:
   - `js-wasmbindgen` — uses `wasm-bindgen` + `web-sys` for `setTimeout`
   - `js-foundation-wasm` — uses `foundation_wasm` FFI for JS-side timer scheduling
4. **Executor stop logic** — `run_until` and `block_until_finished` break on `WaitStatus::Waiting`

## Module Structure

```
foundation_core/src/
├── wasm/
│   ├── mod.rs              # Feature-gated module selection + re-exports
│   ├── wasm_bindgen/       # wasm-bindgen + web-sys backend
│   │   └── mod.rs          # JSThreadYielder implementation
│   └── foundation_wasm/    # foundation_wasm FFI backend
│       └── mod.rs          # JSThreadYielder implementation
├── valtron/
│   ├── executors/
│   │   ├── controller.rs   # ProcessController trait (updated with YieldSignal)
│   │   └── local.rs        # run_until, block_until_finished (updated stop logic)
│   └── ...
└── lib.rs                  # pub mod wasm;
```

**`wasm/mod.rs`** — selects and exposes the correct backend based on feature flags:

```rust
#[cfg(feature = "js-wasmbindgen")]
mod wasm_bindgen;
#[cfg(feature = "js-wasmbindgen")]
pub use wasm_bindgen::JSThreadYielder;
#[cfg(feature = "js-wasmbindgen")]
pub use wasm_bindgen::JS_WAIT_CHECK_INTERVAL;

#[cfg(feature = "js-foundation-wasm")]
mod foundation_wasm;
#[cfg(feature = "js-foundation-wasm")]
pub use foundation_wasm::JSThreadYielder;
#[cfg(feature = "js-foundation-wasm")]
pub use foundation_wasm::JS_WAIT_CHECK_INTERVAL;
```

Backend-specific imports (`wasm_bindgen`, `web_sys`, `foundation_wasm`) never leak outside `wasm/`. The public API is always `JSThreadYielder`, regardless of which feature is active.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Application (cf-login-app, etc.)                           │
│    │                                                        │
│    └─ LocalThreadExecutor<JSThreadYielder>                  │
│         │                                                   │
│         ├─ run_until(checker)                               │
│         │   │                                               │
│         │   ├─ schedule_and_do_work()                       │
│         │   ├─ yielder.yield_for(dur) → YieldSignal         │
│         │   │   │                                           │
│         │   │   └─ if matches!(signal, WaitStatus::Waiting) │
│         │   │                                               │
│         │   └─ loop breaks                                  │
│         │                                                   │
│  foundation_core/src/wasm/                                  │
│    │                                                        │
│    ├─ JSThreadYielder::yield_for(dur)                       │
│    │   └─ CooperativeSpinWaiter::wait(dur)                  │
│    │       ├─ schedule_resume(dur, resume_closure)          │
│    │       │   │                                            │
│    │       │   ├─ [wasm_bindgen] window.setTimeout(closure)        │
│    │       │   └─ [foundation_wasm] register_schedule(dur, closure) │
│    │       │                                                │
│    │       └─ returns WaitStatus::Waiting                   │
│    │                                                        │
│    └─ executor sees Waiting → breaks loop → returns to JS   │
│                                                             │
│  JS Event Loop                                              │
│    │                                                        │
│    ├─ setTimeout fires after `dur` ms                       │
│    │                                                        │
│    └─ closure: {                                            │
│         resume_closure() → AtomicBool=true                  │
│         single::run_until(checker) → re-invoke executor     │
│       }                                                     │
│       │                                                     │
│       └─ executor continues from where it left off          │
└─────────────────────────────────────────────────────────────┘
```

## Solution — Layer by Layer

### Layer 1: CooperativeSpinWaiter (foundation_nostd)

**File**: `backends/foundation_nostd/src/primitives/cooperative_spin_waiter.rs`

A new type — generic name, no JS awareness.

```rust
use core::sync::atomic::{AtomicBool, Ordering};

/// Status returned by CooperativeSpinWaiter::wait().
pub enum WaitStatus {
    /// The runtime has been told to schedule a resume.
    /// Caller should stop processing and yield.
    Waiting,
}

/// Signature for the schedule_resume callback.
/// The consumer provides the external timer mechanism (setTimeout, host FFI, etc.).
pub type ScheduleResumeFn = fn(core::time::Duration, Box<dyn FnOnce()>);

/// Cooperative spin waiter that tells the runtime "wake me up after X"
/// and returns Waiting immediately — no spinning.
pub struct CooperativeSpinWaiter {
    resumed: AtomicBool,
    iterations_per_ms: u64,
    schedule_resume: ScheduleResumeFn,
}

impl CooperativeSpinWaiter {
    pub const fn new(iterations_per_ms: u64, schedule_resume: ScheduleResumeFn) -> Self {
        Self {
            resumed: AtomicBool::new(false),
            iterations_per_ms,
            schedule_resume,
        }
    }

    /// Tell the runtime to wake me up after `dur`.
    /// Returns `Waiting` immediately — no spinning.
    pub fn wait(&self, dur: core::time::Duration) -> WaitStatus {
        let resumed = &self.resumed;
        (self.schedule_resume)(dur, Box::new(move || {
            resumed.store(true, Ordering::Release);
        }));
        WaitStatus::Waiting
    }

    pub fn has_resumed(&self) -> bool {
        self.resumed.load(Ordering::Acquire)
    }

    pub fn reset(&self) {
        self.resumed.store(false, Ordering::Release);
    }

    pub const fn iterations_per_ms(&self) -> u64 {
        self.iterations_per_ms
    }
}
```

### Layer 2: ProcessController::YieldSignal (controller.rs)

**File**: `backends/foundation_core/src/valtron/executors/controller.rs`

Add an associated type `YieldSignal` defaulting to `()`. Native impls return `()` and callers ignore it. JS impl returns `WaitStatus` — caller sees it and stops.

```rust
use std::time;

pub trait ProcessController {
    type YieldSignal;

    fn yield_for(&self, dur: time::Duration) -> Self::YieldSignal;
}
```

**Update existing impls:**

**NoThreadController** (`single/mod.rs`):
```rust
impl ProcessController for NoThreadController {
    type YieldSignal = ();

    fn yield_for(&self, dur: std::time::Duration) {
        self.waiter.wait(dur);
    }
}
```

**ThreadYielder** (`multi/threads.rs`):
```rust
impl ProcessController for ThreadYielder {
    type YieldSignal = ();

    fn yield_for(&self, dur: std::time::Duration) {
        // ... existing CondVar wait code unchanged ...
    }
}
```

**NoYielder** (tests in `local.rs`):
```rust
impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _: time::Duration) {}
}
```

### Layer 3: JSThreadYielder (foundation_core/src/wasm/)

#### Feature: `js-wasmbindgen`

**File**: `backends/foundation_core/src/wasm/wasm_bindgen/mod.rs`

Uses `wasm-bindgen` + `web-sys` directly. All imports stay inside this module.

The setTimeout closure calls a module-level helper `js_yield_and_continue()`, which encapsulates the full re-invocation logic: set the resumed flag, then call back into valtron's executor. No registration, no storage — the thread-local provides global access.

```rust
use crate::valtron::{ProgressIndicator, State};
use foundation_nostd::primitives::cooperative_spin_waiter::{
    CooperativeSpinWaiter, ScheduleResumeFn, WaitStatus,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use std::time;

/// JS-specific timeout for "queue empty, check again soon" scenarios.
/// 4ms: browser minimum (clamped), CF Workers fire ~1ms but 4ms is safe.
/// Balances responsiveness with event loop throughput.
pub const JS_WAIT_CHECK_INTERVAL: time::Duration = time::Duration::from_millis(4);

/// Module-level checker used by both the executor's run_until and the
/// setTimeout callback's re-invocation. Returns true when the executor
/// should stop and yield back to JS.
fn js_should_stop(signal: &ProgressIndicator) -> bool {
    match signal {
        // No tasks available — stop and return to JS
        ProgressIndicator::NoWork => true,
        // Task requested a specific wait duration → yield
        ProgressIndicator::SpinWait(_) => true,
        // Queue empty (ProgressIndicator::Wait from Feature 01) → yield
        ProgressIndicator::Wait => true,
        // Task wants to reschedule → yield, let JS do other work
        ProgressIndicator::CanProgress(Some(State::Reschedule)) => true,
        _ => false,
    }
}

/// Module-level function called from the setTimeout closure.
/// After the timer fires and the resumed flag is set, this re-invokes
/// the valtron executor to continue processing. It uses the same
/// js_should_stop checker so the executor yields again if nothing
/// has changed.
fn js_yield_and_continue() {
    crate::valtron::single::run_until(js_should_stop);
}

fn wasm_bindgen_schedule_resume(dur: time::Duration, resume: Box<dyn FnOnce()>) {
    let window = web_sys::window().expect("no window");
    let closure = Closure::once(Box::new(move || {
        // Step 1: Signal that the wait period is over
        resume();
        // Step 2: Re-invoke valtron's executor via the thread-local
        js_yield_and_continue();
    }) as Box<dyn FnOnce()>);
    window
        .set_timeout_with_callback_and_timeout_millis(
            closure.as_ref().unchecked_ref(),
            dur.as_millis() as i32,
        )
        .expect("set_timeout failed");
    closure.forget();
}

pub struct JSThreadYielder {
    spin_waiter: CooperativeSpinWaiter,
}

impl JSThreadYielder {
    pub fn new() -> Self {
        Self {
            spin_waiter: CooperativeSpinWaiter::new(100_000, wasm_bindgen_schedule_resume),
        }
    }
}

impl crate::valtron::executors::controller::ProcessController for JSThreadYielder {
    type YieldSignal = WaitStatus;

    fn yield_for(&self, dur: time::Duration) -> WaitStatus {
        self.spin_waiter.wait(dur)
    }
}

```

#### Feature: `js-foundation-wasm`

**File**: `backends/foundation_core/src/wasm/foundation_wasm/mod.rs`

Uses `foundation_wasm`'s existing `host_runtime::web::register_schedule(timing, f)` which already registers the callback in the global `SCHEDULED_CALLBACKS` and calls `schedule_timeout`. No separate registry needed.

```rust
use crate::valtron::{ProgressIndicator, State};
use foundation_nostd::primitives::cooperative_spin_waiter::{
    CooperativeSpinWaiter, ScheduleResumeFn, WaitStatus,
};
use std::time;

/// JS-specific timeout for "queue empty, check again soon" scenarios.
/// 4ms: browser minimum (clamped), CF Workers fire ~1ms but 4ms is safe.
/// Balances responsiveness with event loop throughput.
pub const JS_WAIT_CHECK_INTERVAL: time::Duration = time::Duration::from_millis(4);

/// Module-level checker used by both the executor's run_until and the
/// setTimeout callback's re-invocation. Returns true when the executor
/// should stop and yield back to JS.
fn js_should_stop(signal: &ProgressIndicator) -> bool {
    match signal {
        // No tasks available — stop and return to JS
        ProgressIndicator::NoWork => true,
        // Task requested a specific wait duration → yield
        ProgressIndicator::SpinWait(_) => true,
        // Queue empty (ProgressIndicator::Wait from Feature 01) → yield
        ProgressIndicator::Wait => true,
        // Task wants to reschedule → yield, let JS do other work
        ProgressIndicator::CanProgress(Some(State::Reschedule)) => true,
        _ => false,
    }
}

/// Module-level function called from the ScheduleRegistry callback.
/// After the timer fires and the resumed flag is set, this re-invokes
/// the valtron executor to continue processing.
fn js_yield_and_continue() {
    crate::valtron::single::run_until(js_should_stop);
}

fn foundation_wasm_schedule_resume(dur: time::Duration, resume: Box<dyn FnOnce()>) {
    // register_schedule registers the callback and calls schedule_timeout
    // for us — no need for our own GLOBAL_SCHEDULE.
    foundation_wasm::host_runtime::web::register_schedule(
        dur.as_millis() as f64,
        move || {
            resume();
            js_yield_and_continue();
        },
    );
}

pub struct JSThreadYielder {
    spin_waiter: CooperativeSpinWaiter,
}

impl JSThreadYielder {
    pub fn new() -> Self {
        Self {
            spin_waiter: CooperativeSpinWaiter::new(100_000, foundation_wasm_schedule_resume),
        }
    }
}

impl crate::valtron::executors::controller::ProcessController for JSThreadYielder {
    type YieldSignal = WaitStatus;

    fn yield_for(&self, dur: time::Duration) -> WaitStatus {
        self.spin_waiter.wait(dur)
    }
}
```

### Layer 4: Executor Stop Logic (local.rs)

**File**: `backends/foundation_core/src/valtron/executors/local.rs`

Feature 01 adds `ProgressIndicator::Wait` returned from `schedule_and_do_work` when `State::Wait` is hit. On JS, intercept this and yield instead of tight-spinning.

**`run_until`** — updated:
```rust
let response = self.schedule_and_do_work();

// JS: intercept ProgressIndicator::Wait → yield to event loop with short timer
#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
if matches!(&response, ProgressIndicator::Wait) {
    tracing::debug!("run_until: Wait → yielding to JS event loop ({}ms)",
        crate::wasm::JS_WAIT_CHECK_INTERVAL.as_millis());
    self.yielder.yield_for(crate::wasm::JS_WAIT_CHECK_INTERVAL);
    break;
}

// JS: intercept SpinWait → yield with task-specified duration
#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
if let ProgressIndicator::SpinWait(spin_duration) = &response {
    let _signal = self.yielder.yield_for(*spin_duration);
    if matches!(_signal, crate::wasm::WaitStatus::Waiting) {
        tracing::debug!("run_until: SpinWait({}ms) → yielding to JS event loop",
            spin_duration.as_millis());
        break;
    }
}

// JS: intercept NoWork → stop and return to JS
#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
if matches!(&response, ProgressIndicator::NoWork) {
    break;
}

// JS: intercept Reschedule → yield, let JS do other work
#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
if matches!(&response, ProgressIndicator::CanProgress(Some(State::Reschedule))) {
    tracing::debug!("run_until: Reschedule → yielding to JS event loop");
    break;
}

if checker(response) {
    break;
}
continue;
```

**`block_until_finished`** — updated:
```rust
match response {
    ProgressIndicator::SpinWait(duration) => {
        #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
        {
            let _signal = self.yielder.yield_for(duration);
            if matches!(_signal, crate::wasm::WaitStatus::Waiting) {
                break;  // JS event loop will resolve tasks
            }
        }
        #[cfg(not(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")))]
        self.yielder.yield_for(duration);
    }
    ProgressIndicator::Wait => {
        #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
        {
            let _signal = self.yielder.yield_for(crate::wasm::JS_WAIT_CHECK_INTERVAL);
            if matches!(_signal, crate::wasm::WaitStatus::Waiting) {
                break;  // JS event loop will check for new tasks
            }
        }
    }
    ProgressIndicator::NoWork | ProgressIndicator::CanProgress(Some(State::Reschedule)) => {
        #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
        {
            break;  // Nothing to do, return to JS
        }
    }
    _ => { /* handled normally */ }
}
```

### Layer 5: Feature Flags

**`foundation_core/Cargo.toml`:**
```toml
[features]
# ... existing features ...
js-wasmbindgen = ["dep:wasm-bindgen", "dep:web-sys"]
js-foundation-wasm = ["dep:foundation_wasm"]
```

Mutually exclusive — only one should be enabled at a time. The `wasm/` module is only compiled when one of these features is active.

## Complete Signal Flow

```
1. App creates LocalThreadExecutor<JSThreadYielder>
2. valtron executor runs tasks via run_until() or block_until_finished()
3. Two paths to yielding:

   Path A — Task has a specific wait duration (SpinWait):
     a. Task returns ProgressIndicator::SpinWait(dur)
     b. executor calls yielder.yield_for(dur)
     c. JSThreadYielder calls CooperativeSpinWaiter::wait(dur)
     d. wait() calls schedule_resume(dur, resume_closure)
     e. schedule_resume sets up JS setTimeout with: Closure { resume_closure(); single::run_until(...) }
     f. wait() returns WaitStatus::Waiting → yield_for returns Waiting
     g. executor breaks loop, returns to JS caller
     h. setTimeout fires after `dur` ms → flag set + executor re-invoked

   Path B — Queue empty, check again soon (State::Wait):
     a. NotifyQueue returns Some(NotificationItem::None) → Stream::Wait → State::Wait
     b. schedule_and_do_work returns CanProgress(Some(State::Wait))
     c. JS intercept: yielder.yield_for(JS_WAIT_CHECK_INTERVAL) [4ms]
     d. setTimeout set for 4ms → executor breaks loop, returns to JS caller
     e. setTimeout fires after 4ms → flag set + executor re-invoked
     f. Next executor invocation finds tasks available

4. JS event loop runs freely between yields — Promises resolve, timers fire
5. setTimeout callback: resume_closure() + single::run_until() → executor restarts
```

## Implementation Phases

### Phase 1: CooperativeSpinWaiter (foundation_nostd)

1. Add `WaitStatus` enum to `foundation_nostd/src/primitives/cooperative_spin_waiter.rs`
2. Add `ScheduleResumeFn` type alias
3. Add `CooperativeSpinWaiter` struct with `new()`, `wait() -> WaitStatus`, `has_resumed()`, `reset()`, `iterations_per_ms()`
4. Add `pub mod cooperative_spin_waiter` to `foundation_nostd/src/primitives/mod.rs`
5. Add tests for cooperative_spin_waiter

### Phase 2: ProcessController YieldSignal

6. Update `ProcessController` trait in `controller.rs` — add `type YieldSignal`, change `yield_for` to return it
7. Update `NoThreadController` in `single/mod.rs` — `type YieldSignal = ()`
8. Update `ThreadYielder` in `multi/threads.rs` — `type YieldSignal = ()`
9. Update `NoYielder` in tests — `type YieldSignal = ()`
10. Verify all existing native tests pass

### Phase 3: wasm/ Module + Both Backends

11. Create `foundation_core/src/wasm/mod.rs` with feature-gated re-exports
12. Create `foundation_core/src/wasm/wasm_bindgen/mod.rs` with `JSThreadYielder` using `web_sys`
13. Create `foundation_core/src/wasm/foundation_wasm/mod.rs` with `JSThreadYielder` using `foundation_wasm`'s `register_schedule`
15. Add `pub mod wasm;` to `foundation_core/src/lib.rs` (feature-gated)
16. Add feature flags to `foundation_core/Cargo.toml`: `js-wasmbindgen`, `js-foundation-wasm`

### Phase 4: Executor Stop Logic

17. Update `run_until` in `local.rs` — check `yield_for` return, break on `WaitStatus::Waiting`
18. Update `block_until_finished` in `local.rs` — break inner loop on `WaitStatus::Waiting`

### Phase 5: Integration

19. Add `JSThreadYielder` as an alternative to `NoThreadController` in `initialize_pool()` when JS yield feature is active
20. Update `cf-login-app` example to use `JSThreadYielder`

## Tests

1. `test_cooperative_spin_waiter_returns_waiting()` — `wait()` returns `WaitStatus::Waiting` immediately
2. `test_cooperative_spin_waiter_resume_sets_flag()` — calling resume closure sets `has_resumed()` to true
3. `test_cooperative_spin_waiter_reset_clears_flag()` — `reset()` clears the resumed flag
4. `test_process_controller_native_returns_unit()` — native impls return `()`
5. `test_js_thread_yielder_returns_waiting()` — `JSThreadYielder::yield_for()` returns `WaitStatus::Waiting`
6. `test_run_until_breaks_on_waiting()` — `run_until` loop breaks when `yield_for` returns `Waiting`
7. `test_block_until_finished_breaks_inner_loop_on_waiting()` — inner loop breaks, outer loop re-checks
8. `test_native_yield_continues_normally()` — native impls don't break the loop
9. `test_js_eventloop_yield_has_no_effect_on_native()` — features don't affect native builds

## Success Criteria

- `CooperativeSpinWaiter` exists in `foundation_nostd` with generic name (no "JS" reference)
- `WaitStatus::Waiting` variant exists
- `CooperativeSpinWaiter::wait()` returns immediately with `Waiting`, no spinning
- `ProcessController` has `type YieldSignal` associated type (default `()`)
- `yield_for()` returns `Self::YieldSignal`
- All native impls return `()` — zero behavior change on native
- `foundation_core/src/wasm/` module exists with feature-gated backend selection
- `wasm/wasm_bindgen/mod.rs` implements `JSThreadYielder` using `web_sys`
- `wasm/foundation_wasm/mod.rs` implements `JSThreadYielder` using `foundation_wasm::register_schedule`
- setTimeout/ScheduleRegistry callback calls `single::run_until()` directly after setting flag
- `run_until` / `block_until_finished` break on `WaitStatus::Waiting`
- `foundation_nostd` remains `no_std`-compatible
- All existing tests pass on native
- `cf-login-app` works with JS yield enabled

## Verification Commands

```bash
# Native tests (no features)
cargo test -p foundation_nostd -- cooperative_spin_waiter
cargo test -p foundation_core -- process_controller
cargo test -p foundation_core -- valtron

# wasm-bindgen backend
cargo build -p foundation_core --target wasm32-unknown-unknown --features js-wasmbindgen
cargo clippy -p foundation_core --target wasm32-unknown-unknown --features js-wasmbindgen -- -D warnings

# foundation_wasm backend
cargo build -p foundation_core --target wasm32-unknown-unknown --features js-foundation-wasm
cargo clippy -p foundation_core --target wasm32-unknown-unknown --features js-foundation-wasm -- -D warnings

# No_std compatibility
cargo build -p foundation_nostd --target wasm32-unknown-unknown
```
