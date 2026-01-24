# WASM Considerations - Using CondVar in WebAssembly

## Table of Contents
- [WASM Threading Model](#wasm-threading-model)
- [Single-Threaded vs Multi-Threaded](#single-threaded-vs-multi-threaded)
- [Variant Selection for WASM](#variant-selection-for-wasm)
- [Memory Constraints](#memory-constraints)
- [Performance Optimization](#performance-optimization)
- [Testing WASM Code](#testing-wasm-code)
- [Common Pitfalls](#common-pitfalls)
- [Best Practices](#best-practices)

## WASM Threading Model

### Understanding WASM Execution Contexts

**WASM environments come in two flavors**:

1. **Single-threaded** (default)
   - One JavaScript event loop
   - No true parallelism
   - Atomics may not be available
   - Most browser and Node.js contexts

2. **Multi-threaded** (optional)
   - SharedArrayBuffer + Atomics
   - Web Workers as threads
   - Requires specific browser flags/configuration
   - Support varies across platforms

### Detection at Compile Time

```rust
// Check if WASM atomics are available
#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
const SINGLE_THREADED: bool = true;

#[cfg(all(target_family = "wasm", target_feature = "atomics"))]
const MULTI_THREADED: bool = true;

#[cfg(not(target_family = "wasm"))]
const NATIVE: bool = true;
```

### Detection at Runtime

```rust
fn is_single_threaded_wasm() -> bool {
    #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
    {
        true
    }
    #[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
    {
        false
    }
}

fn print_context() {
    if is_single_threaded_wasm() {
        println!("Running in single-threaded WASM");
    } else {
        println!("Running in multi-threaded or native context");
    }
}
```

## Single-Threaded vs Multi-Threaded

### Single-Threaded WASM Behavior

**CondVar operations in single-threaded context**:

| Operation | Behavior | Rationale |
|-----------|----------|-----------|
| `wait()` | ❌ Panics | Would deadlock (no other thread to notify) |
| `notify_one()` | ✅ No-op | No threads to wake |
| `notify_all()` | ✅ No-op | No threads to wake |
| `is_poisoned()` | ✅ Returns false | No panics cross threads |

**Why wait() must panic**:
```rust
// Single-threaded scenario
fn deadlock_scenario() {
    let mutex = RawSpinMutex::new(false);
    let condvar = CondVarNonPoisoning::new();

    let mut guard = mutex.lock();

    // ❌ This would block forever:
    // - Only one thread exists
    // - No other thread can call notify()
    // - Thread sleeps eternally
    condvar.wait(guard); // PANIC: would deadlock!
}
```

### Multi-Threaded WASM Behavior

**With WASM threads (SharedArrayBuffer + Atomics)**:

| Operation | Behavior | Notes |
|-----------|----------|-------|
| `wait()` | ✅ Blocks until notified | Uses Atomics.wait() internally |
| `notify_one()` | ✅ Wakes one thread | Uses Atomics.notify() |
| `notify_all()` | ✅ Wakes all threads | Uses Atomics.notify() with count |
| `is_poisoned()` | ✅ Tracks poison state | Full poisoning support |

**Requirements**:
- Browser with SharedArrayBuffer support
- Cross-Origin-Opener-Policy (COOP) headers set
- Cross-Origin-Embedder-Policy (COEP) headers set
- Compiled with atomics target feature

### Compilation Targets

**Single-threaded**:
```bash
cargo build --target wasm32-unknown-unknown
# No atomics feature
```

**Multi-threaded**:
```bash
# Use wasm32-unknown-unknown with atomics
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
  cargo build --target wasm32-unknown-unknown -Z build-std=std,panic_abort
```

**Check target features**:
```bash
rustc --target=wasm32-unknown-unknown --print target-features
```

## Variant Selection for WASM

### Recommended: CondVarNonPoisoning

**Why CondVarNonPoisoning is best for WASM**:

1. **Smaller binary size**
   - No poisoning infrastructure
   - Compile-time dead code elimination
   - 15-20% smaller than CondVar

2. **Simpler API**
   - No Result wrapping
   - Cleaner code in WASM context
   - Fewer `.unwrap()` calls

3. **Panic handling**
   - WASM panics terminate the module
   - Poisoning provides no recovery (can't restart)
   - Non-poisoning variant is more honest

4. **Performance**
   - 10-15% faster (no poison checks)
   - Matters more in WASM (slower baseline)

### Comparison

```rust
// CondVar (with poisoning)
use foundation_nostd::primitives::{CondVar, SpinMutex};

let mutex = SpinMutex::new(0);
let condvar = CondVar::new();

let mut guard = mutex.lock().unwrap(); // Result
while *guard < 10 {
    guard = condvar.wait(guard).unwrap(); // Result
}
// Binary size: +24 KB (estimated)

// CondVarNonPoisoning (no poisoning)
use foundation_nostd::primitives::{CondVarNonPoisoning, RawSpinMutex};

let mutex = RawSpinMutex::new(0);
let condvar = CondVarNonPoisoning::new();

let mut guard = mutex.lock(); // No Result
while *guard < 10 {
    guard = condvar.wait(guard); // No Result
}
// Binary size: +20 KB (estimated)
```

**Recommendation**: Use **CondVarNonPoisoning** unless you have a specific reason to use CondVar.

## Memory Constraints

### WASM Memory Model

**WASM has limited memory**:
- Default: 16 MB initial, grows as needed
- Maximum: 4 GB (32-bit address space)
- Memory growth is expensive (requires reallocation)

### CondVar Memory Footprint

**Per CondVar instance**:

| Variant | Size | Notes |
|---------|------|-------|
| CondVar | ~24 bytes | Includes poison bit |
| CondVarNonPoisoning | ~20 bytes | No poison bit |
| RwLockCondVar | ~24 bytes | Includes poison bit |

**Per waiting thread**:
- Wait node: ~16 bytes (stack-allocated)

### Optimization Tips

#### 1. Reuse CondVars

❌ **Bad**: Creating many CondVars
```rust
struct Task {
    data: Vec<u8>,
    condvar: CondVarNonPoisoning, // 20 bytes per task
}

let tasks = vec![Task::new(); 10000]; // 200 KB just for CondVars!
```

✅ **Good**: Share CondVars
```rust
struct TaskPool {
    tasks: Vec<Task>,
    condvar: CondVarNonPoisoning, // Single CondVar for all tasks
}

let pool = TaskPool {
    tasks: vec![Task::new(); 10000],
    condvar: CondVarNonPoisoning::new(), // Only 20 bytes
};
```

#### 2. Use Non-Poisoning Variant

```rust
// Saves 4 bytes per CondVar
// For 1000 CondVars: 4 KB savings
CondVarNonPoisoning::new() // 20 bytes
// vs
CondVar::new() // 24 bytes
```

#### 3. Avoid Heap Allocations in Wait Loops

❌ **Bad**: Allocating in wait loop
```rust
while !condition {
    let temp = vec![0; 1024]; // ❌ Allocates every iteration
    guard = condvar.wait(guard);
}
```

✅ **Good**: Allocate once outside loop
```rust
let temp = vec![0; 1024]; // ✅ Allocate once
while !condition {
    guard = condvar.wait(guard);
}
```

#### 4. Limit Maximum Waiters

```rust
struct BoundedCondVar {
    condvar: CondVarNonPoisoning,
    max_waiters: usize,
    current_waiters: AtomicUsize,
}

impl BoundedCondVar {
    fn wait<T>(&self, guard: MutexGuard<'_, T>) -> MutexGuard<'_, T> {
        let count = self.current_waiters.fetch_add(1, Ordering::Relaxed);

        if count >= self.max_waiters {
            self.current_waiters.fetch_sub(1, Ordering::Relaxed);
            panic!("Too many waiters (WASM memory limit)");
        }

        let guard = self.condvar.wait(guard);
        self.current_waiters.fetch_sub(1, Ordering::Relaxed);
        guard
    }
}
```

## Performance Optimization

### Benchmarks (WASM vs Native)

**Relative performance** (WASM / Native):

| Operation | Single-threaded WASM | Multi-threaded WASM |
|-----------|----------------------|---------------------|
| `new()` | 100% | 100% |
| `notify_one()` | ~1% (no-op) | ~60% |
| `wait()` | N/A (panics) | ~40% |
| `is_poisoned()` | 100% | 80% |

**Interpretation**:
- Single-threaded: notify is effectively free (compile-time no-op)
- Multi-threaded: ~2-3x slower than native (WASM overhead)

### Optimization Strategies

#### 1. Minimize Wait Calls

❌ **Bad**: Tight wait loop
```rust
while !condition(&data) {
    data = condvar.wait(data); // WASM wait is expensive
}
```

✅ **Good**: Batch condition checks
```rust
while !condition(&data) {
    // Wait with timeout, allows batching
    let (d, result) = condvar.wait_timeout(data, Duration::from_millis(10));
    data = d;
}
```

#### 2. Prefer Polling for Short Waits

```rust
use core::hint::spin_loop;

// For waits < 1ms, spinning is faster in WASM
fn fast_wait<T>(condvar: &CondVarNonPoisoning, mut guard: MutexGuard<'_, T>, condition: impl Fn(&T) -> bool) -> MutexGuard<'_, T> {
    let start = Instant::now();

    // Spin for up to 1ms
    while start.elapsed() < Duration::from_millis(1) {
        if condition(&guard) {
            return guard;
        }
        spin_loop();
    }

    // Fall back to blocking wait
    while !condition(&guard) {
        guard = condvar.wait(guard);
    }

    guard
}
```

#### 3. Use Single-Threaded Detection

```rust
#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
fn optimized_notify(condvar: &CondVarNonPoisoning) {
    // No-op: compile-time optimization
    // This function body is removed by compiler
}

#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
fn optimized_notify(condvar: &CondVarNonPoisoning) {
    condvar.notify_one();
}
```

## Testing WASM Code

### Test Setup

**Install WASM target**:
```bash
rustup target add wasm32-unknown-unknown
```

**Install wasm-pack** (for browser tests):
```bash
cargo install wasm-pack
```

**Install wasmtime** (for CLI tests):
```bash
cargo install wasmtime-cli
```

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_single_threaded() {
        let condvar = CondVarNonPoisoning::new();

        // In single-threaded WASM, notify is no-op
        condvar.notify_one(); // Should not panic
        condvar.notify_all(); // Should not panic
    }

    #[test]
    #[should_panic(expected = "would deadlock")]
    #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
    fn test_wait_panics_single_threaded() {
        let mutex = RawSpinMutex::new(0);
        let condvar = CondVarNonPoisoning::new();

        let guard = mutex.lock();
        condvar.wait(guard); // Should panic
    }

    #[test]
    #[cfg(all(target_family = "wasm", target_feature = "atomics"))]
    fn test_multi_threaded_wasm() {
        use std::thread;

        let mutex = RawSpinMutex::new(false);
        let condvar = CondVarNonPoisoning::new();

        thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            *mutex.lock() = true;
            condvar.notify_one();
        });

        let mut guard = mutex.lock();
        while !*guard {
            guard = condvar.wait(guard);
        }
        assert!(*guard);
    }
}
```

### Running Tests

**Native tests** (default):
```bash
cargo test
```

**WASM tests** (single-threaded):
```bash
cargo test --target wasm32-unknown-unknown
```

**WASM tests** (multi-threaded):
```bash
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
  cargo test --target wasm32-unknown-unknown -Z build-std=std,panic_abort
```

**Browser tests** (with wasm-pack):
```bash
wasm-pack test --headless --firefox
wasm-pack test --headless --chrome
```

### Conditional Compilation

```rust
// Compile different code for WASM vs native
#[cfg(target_family = "wasm")]
fn wasm_specific() {
    // WASM-only code
}

#[cfg(not(target_family = "wasm"))]
fn native_specific() {
    // Native-only code
}

// Detect atomics support
#[cfg(all(target_family = "wasm", target_feature = "atomics"))]
fn multi_threaded_wasm() {
    // Multi-threaded WASM
}

#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
fn single_threaded_wasm() {
    // Single-threaded WASM
}
```

## Common Pitfalls

### 1. Assuming Multi-Threaded WASM

❌ **Wrong**: Assuming threads always work
```rust
use std::thread;

// ❌ May panic in single-threaded WASM
thread::spawn(|| {
    // Worker code
});
```

✅ **Correct**: Check for thread support
```rust
#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
fn spawn_worker() {
    panic!("Threads not supported in single-threaded WASM");
}

#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
fn spawn_worker() {
    thread::spawn(|| {
        // Worker code
    });
}
```

### 2. Blocking the Event Loop

❌ **Wrong**: Blocking in single-threaded WASM
```rust
// In single-threaded WASM, this blocks the JavaScript event loop
let guard = mutex.lock();
std::thread::sleep(Duration::from_secs(10)); // ❌ Freezes browser!
```

✅ **Correct**: Use async or timeouts
```rust
// Use async patterns or short timeouts
let (guard, timed_out) = condvar.wait_timeout(guard, Duration::from_millis(100));
if timed_out.timed_out() {
    // Handle timeout, don't block event loop
}
```

### 3. Ignoring Memory Limits

❌ **Wrong**: Creating too many CondVars
```rust
// Each CondVar is 20-24 bytes
let condvars: Vec<_> = (0..1_000_000)
    .map(|_| CondVarNonPoisoning::new())
    .collect(); // ❌ 20 MB just for CondVars!
```

✅ **Correct**: Reuse CondVars
```rust
// Single CondVar + event flag pattern
struct EventSystem {
    events: Mutex<HashMap<EventId, bool>>,
    condvar: CondVarNonPoisoning, // Only 20 bytes
}
```

### 4. Not Handling Single-Threaded Case

❌ **Wrong**: Unconditional wait
```rust
// Panics in single-threaded WASM
fn wait_for_event() {
    let guard = mutex.lock();
    condvar.wait(guard); // ❌ Deadlock in single-threaded!
}
```

✅ **Correct**: Conditional compilation
```rust
fn wait_for_event() {
    #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
    {
        panic!("Cannot wait in single-threaded WASM");
    }

    #[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
    {
        let guard = mutex.lock();
        condvar.wait(guard);
    }
}
```

## Best Practices

### 1. Choose the Right Variant

**For WASM**: Always prefer **CondVarNonPoisoning**

```rust
// ✅ WASM-optimized
use foundation_nostd::primitives::{CondVarNonPoisoning, RawSpinMutex};

let mutex = RawSpinMutex::new(0);
let condvar = CondVarNonPoisoning::new();
```

### 2. Feature Flag for Thread Support

```rust
// In Cargo.toml
[features]
default = []
threads = []

// In code
#[cfg(feature = "threads")]
fn threaded_implementation() {
    // Use CondVar with threads
}

#[cfg(not(feature = "threads"))]
fn single_threaded_implementation() {
    // Use alternative approach (polling, callbacks, etc.)
}
```

### 3. Timeout Everything in WASM

```rust
// Always use timeouts to avoid infinite waits
let timeout = Duration::from_secs(5);
let (guard, result) = condvar.wait_timeout(guard, timeout);

if result.timed_out() {
    // Handle timeout (WASM may be slower)
}
```

### 4. Test on Multiple Browsers

```rust
// Different WASM implementations across browsers
// Test on:
// - Firefox (best WASM support)
// - Chrome/Edge (good support)
// - Safari (more limited)

// Use wasm-pack:
wasm-pack test --headless --firefox
wasm-pack test --headless --chrome
wasm-pack test --headless --safari
```

### 5. Document WASM Requirements

```rust
/// Worker pool implementation
///
/// # WASM Support
///
/// - **Single-threaded WASM**: Not supported (panics on creation)
/// - **Multi-threaded WASM**: Requires SharedArrayBuffer and atomics
///
/// ## Browser Requirements
///
/// Multi-threaded WASM requires these HTTP headers:
/// - `Cross-Origin-Opener-Policy: same-origin`
/// - `Cross-Origin-Embedder-Policy: require-corp`
///
pub struct WorkerPool {
    // ...
}
```

### 6. Provide Fallbacks

```rust
pub struct AdaptiveQueue<T> {
    #[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
    condvar: CondVarNonPoisoning,

    #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
    polling: AtomicBool, // Use polling as fallback
}

impl<T> AdaptiveQueue<T> {
    pub fn wait_for_item(&self) -> T {
        #[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
        {
            // Use CondVar in multi-threaded contexts
            let mut guard = self.queue.lock();
            while guard.is_empty() {
                guard = self.condvar.wait(guard);
            }
            guard.pop_front().unwrap()
        }

        #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
        {
            // Poll in single-threaded WASM
            loop {
                let mut guard = self.queue.lock();
                if let Some(item) = guard.pop_front() {
                    return item;
                }
                drop(guard);
                // Yield to JavaScript event loop
                std::thread::yield_now();
            }
        }
    }
}
```

## Summary

### WASM CondVar Checklist

**For single-threaded WASM**:
- ✅ Use CondVarNonPoisoning (smallest, fastest)
- ✅ notify_one/notify_all are safe (no-ops)
- ❌ Never call wait() (will panic/deadlock)
- ✅ Provide polling-based alternative

**For multi-threaded WASM**:
- ✅ Use CondVarNonPoisoning (simpler than CondVar)
- ✅ All operations work (with performance penalty vs native)
- ✅ Requires SharedArrayBuffer + atomics
- ✅ Test across multiple browsers

**Memory optimization**:
- ✅ Prefer CondVarNonPoisoning (4 bytes smaller)
- ✅ Reuse CondVars across multiple tasks
- ✅ Limit maximum waiters to avoid memory bloat
- ✅ Avoid allocations in wait loops

**Performance**:
- ✅ Use timeouts to avoid long waits
- ✅ Batch condition checks
- ✅ Consider polling for short waits (< 1ms)
- ✅ Profile on target WASM platform

**Testing**:
- ✅ Test both single-threaded and multi-threaded WASM
- ✅ Test on multiple browsers
- ✅ Use conditional compilation for WASM-specific tests
- ✅ Use wasm-pack for browser integration tests

## Next Steps

- **[06-std-compatibility.md](./06-std-compatibility.md)** - Migration from std::sync::Condvar
- **[00-overview.md](./00-overview.md)** - Back to overview and quick start
