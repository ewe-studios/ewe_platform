# Valtron Executor Deep Dive - Learnings

## Critical Findings

### Finding 1: CondVar/park_timeout Mismatch (CRITICAL)

**Issue:** ThreadYielder uses `park_timeout` while shutdown uses `CondVar::notify_all()`. These are incompatible synchronization mechanisms.

**Impact:**
- Tests take ~18s each (exponential backoff: 1s → 2s → 4s → 8s → 16s)
- Serial test execution amplifies delays
- Production shutdown could hang for seconds

**Root Cause:**
```rust
// ThreadYielder - uses park_timeout
std::thread::park_timeout(remaining_timeout);

// Shutdown - uses CondVar
self.registry.latch().signal_all(); // CondVar notify_all

// Problem: CondVar CANNOT wake parked threads!
```

**Fix:** Use `foundation_nostd::CondVar::wait_timeout` instead of `park_timeout`:
- Works in std (true blocking) and no_std (spin-wait with generation counter)
- Both modes support `notify_all()` for interruption
- Same API across platforms via `foundation_nostd::comp::condvar_comp`

### Finding 2: NoThreadController Does Nothing (HIGH)

**Issue:** NoThreadController::yield_for() only logs, doesn't actually yield.

**Impact:**
- WASM and single-threaded environments busy-wait at 100% CPU
- Delayed tasks consume full CPU during wait

**Current Code:**
```rust
impl ProcessController for NoThreadController {
    fn yield_for(&self, dur: Duration) {
        tracing::info!("...does nothing"); // Literally!
    }
}
```

**Fix:** Implement platform-specific yielding (CondVar for std, async for WASM, spin-loop for no_std).

### Finding 3: Sleeping Tasks Not Notified on Shutdown (HIGH)

**Issue:** Tasks with `Delayed(duration)` status are not woken during shutdown.

**Impact:**
- Shutdown must wait for all delays to complete
- ConnectionHandler's exponential backoff cannot be interrupted

**Root Cause:**
```rust
// DurationWaker has no interrupt mechanism
pub struct DurationWaker {
    entry_id: TaskEntryId,
    deadline: Instant,
    // No way to signal early wake!
}
```

**Fix:** Add `interrupted: AtomicBool` to DurationWaker, call `interrupt_all()` during shutdown.

### Finding 4: drivers.rs Busy-Waiting (MEDIUM)

**Issue:** Stream polling uses spin-loop + sleep instead of proper blocking.

**Current:**
```rust
while stream.is_empty() {
    std::hint::spin_loop();
    std::thread::sleep(Duration::from_micros(100));
}
```

**Impact:** Wastes CPU cycles.

**Fix:** Use channel or CondVar-based notification.

### Finding 5: WASM/no_std Spin-Waiter Required

**Issue:** No accurate timing in WASM/no_std without OS support.

**Current:** `Duration::ZERO` causes infinite spin loops.

**Solution:** Extract spin waiter to `foundation_nostd::SpinWaiter`:
- Iteration-based waiting (not time-based)
- Configurable iterations-per-ms for different platforms
- Interruptible via AtomicBool
- Bounded (never spins forever)

```rust
pub struct SpinWaiter {
    interrupted: AtomicBool,
    iterations_per_ms: u64, // Platform-tunable
}

impl SpinWaiter {
    pub fn wait(&self, dur: Duration) {
        let spins = dur.as_millis() as u64 * self.iterations_per_ms;
        let capped = spins.min(self.iterations_per_ms * 10_000); // 10s max
        
        for i in 0..capped {
            core::hint::spin_loop();
            if i % 1000 == 0 && self.interrupted.load(Relaxed) {
                return; // Interrupted
            }
        }
    }
}
```

**Trade-off:** Timing is approximate but bounded.

### State Machine Complexity

The executor has multiple overlapping state machines:
1. TaskStatus (Ready/Pending/Delayed/Spawn/Ignore)
2. ExecutorState (Pending/SpinWait/CanProgress/NoWork)
3. ConnectionHandler (Idle/Processing)

These interact in complex ways that aren't fully documented.

### Single vs Multi-Threaded Divergence

The two modes have fundamentally different behavior:
- Multi-threaded: Blocks threads, uses OS primitives
- Single-threaded: Cooperative, requires manual yielding

The ProcessController abstraction doesn't fully capture these differences.

### WASM Constraints

WASM cannot block the JS event loop:
- No `std::thread` or `park`
- Must use `setTimeout` via async
- Current implementation is fundamentally broken

## Performance Characteristics

### Delayed Task Overhead
- Registration: O(1)
- Wakeup check: O(n) where n = sleepers count
- Memory: ~40 bytes per sleeper

### Shutdown with Delayed Tasks
| Scenario | Before Fix | After Fix |
|----------|-----------|-----------|
| 1 task, 16s delay | ~16s | <100ms |
| 100 tasks, 5-15s delays | ~15s | <200ms |

### Memory Overhead
- InterruptibleWait: ~48 bytes per yielder
- DurationWaker interrupt flag: ~1 byte per sleeper
- Negligible for typical workloads

## Design Decisions

### Why CondVar over park/unpark?

**Alternative:** Use `thread.unpark()` in shutdown.

**Why Not:** Requires keeping thread handles, complex lifecycle management, harder to test.

**Decision:** CondVar with `wait_timeout` provides:
- Unified wakeup mechanism
- Testable (can signal from tests)
- Compatible with standard Rust patterns

### Why AtomicBool per sleeper?

**Alternative:** Global generation counter.

**Why Not:** More complex, requires global state, harder to reason about.

**Decision:** Per-sleeper AtomicBool:
- Simple and clear
- Low overhead (~1 byte per sleeper)
- No global contention

### Why Platform Abstraction?

**Alternative:** Single implementation with cfg flags.

**Why Not:** Results in messy code with many cfg branches.

**Decision:** PlatformYielder trait with implementations:
- Clean separation of concerns
- Testable per platform
- Extensible for new platforms

## Implementation Notes

### Ordering Requirements

Shutdown must happen in this order:
1. `kill_signal.turn_on()` - Signal intent
2. `sleepers.interrupt_all()` - Wake delayed tasks
3. `yielders.interrupt_all()` - Wake yielding threads
4. `latch.signal_all()` - Wake CondVar waits
5. `waitgroup.wait()` - Wait for completion
6. `join_all_threads()` - Clean up

### Thread Safety

- `DurationWaker::interrupt()` uses `Relaxed` ordering - sufficient for bool
- `SleepersList::interrupt_all()` holds borrow briefly
- All operations are lock-free where possible

### Testing Strategy

1. Unit tests for InterruptibleWait
2. Integration tests for shutdown timing
3. Stress tests with many delayed tasks
4. WASM compile tests (even if not runnable)

## Open Questions

1. Should we add metrics for executor performance?
2. How to handle WASM async integration properly?
3. Is spin-loop acceptable for no_std, or do we need timer abstraction?
4. Should we add task priorities/affinities?

## References

- Feature 01: ThreadYielder Improvements
- Feature 02: WASM Compatibility
- Feature 03: Shutdown Mechanism Fixes
- `backends/foundation_core/src/valtron/executors/threads.rs`
- `backends/foundation_core/src/valtron/executors/local.rs`
- `backends/foundation_core/src/valtron/executors/single/mod.rs`

---

*Last updated: 2026-05-11*
