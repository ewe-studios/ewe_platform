# Condition Variables - Overview and Quick Start

## Introduction

**Condition variables** (CondVars) are synchronization primitives that allow threads to wait for a condition to become true before proceeding. They work in combination with mutexes to coordinate access to shared state and enable efficient waiting without busy-spinning.

### Why Condition Variables?

Consider the classic producer-consumer problem: producers generate data, consumers process it. Without condition variables, you'd have to either:
- **Busy-wait** (constantly check if data is available) - wastes CPU
- **Sleep** (periodically wake up and check) - inefficient and adds latency

**Condition variables solve this** by allowing threads to:
1. **Wait efficiently** - Sleep until explicitly woken up
2. **Avoid busy-spinning** - No CPU wasted on repeated checks
3. **Enable precise coordination** - Wake exactly when the condition changes

### How They Work

The fundamental operations are:
- **`wait()`** - Atomically release mutex and sleep until notified
- **`notify_one()`** - Wake up one waiting thread
- **`notify_all()`** - Wake up all waiting threads

**Key insight**: CondVars always work with a mutex. The waiting pattern is:
```rust
let mut guard = mutex.lock().unwrap();
while !condition_is_true(&guard) {
    guard = condvar.wait(guard).unwrap();
}
// Condition is now true, guard still held
```

The `while` loop handles **spurious wakeups** - cases where `wait()` returns without a notification. More on this in [01-condvar-theory.md](./01-condvar-theory.md).

## Quick Start

### Example: Producer-Consumer Queue

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::VecDeque;

struct SharedQueue<T> {
    mutex: SpinMutex<VecDeque<T>>,
    condvar: CondVar,
}

impl<T> SharedQueue<T> {
    fn new() -> Self {
        Self {
            mutex: SpinMutex::new(VecDeque::new()),
            condvar: CondVar::new(),
        }
    }

    fn push(&self, item: T) {
        let mut queue = self.mutex.lock().unwrap();
        queue.push_back(item);
        drop(queue); // Release lock before notifying
        self.condvar.notify_one(); // Wake one waiting consumer
    }

    fn pop(&self) -> T {
        let mut queue = self.mutex.lock().unwrap();
        // Wait until queue is non-empty
        while queue.is_empty() {
            queue = self.condvar.wait(queue).unwrap();
        }
        queue.pop_front().unwrap()
    }
}
```

**Key points**:
- Lock mutex before checking condition
- Use `while` loop to handle spurious wakeups
- Release lock before notifying (optional but improves performance)
- `notify_one()` wakes exactly one waiter

## Three Variants - Which to Use?

This library provides three CondVar variants for different use cases:

| Variant | Poisoning | Use Case | Integration |
|---------|-----------|----------|-------------|
| **`CondVar`** | ✅ Yes | Standard usage, std compatibility | `SpinMutex<T>` |
| **`CondVarNonPoisoning`** | ❌ No | WASM, embedded, panic=abort contexts | `RawSpinMutex<T>` |
| **`RwLockCondVar`** | ✅ Yes | Coordination with read-write locks | `SpinRwLock<T>` |

### Decision Tree

```
Do you need RwLock integration (readers/writers)?
├─ YES → Use RwLockCondVar
└─ NO  → Is your code running in WASM or embedded (panic=abort)?
          ├─ YES → Use CondVarNonPoisoning
          └─ NO  → Do you need std::sync::Condvar compatibility?
                    ├─ YES → Use CondVar
                    └─ NO  → Use CondVarNonPoisoning (simpler, slightly faster)
```

### When to Use Each Variant

#### CondVar (Standard, with Poisoning)

**Use when**:
- You need drop-in replacement for `std::sync::Condvar`
- Running in std environment where panics can unwind
- Want panic safety (poisoning) to detect corrupted state
- Default choice for general-purpose code

**Example**:
```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};

let mutex = SpinMutex::new(false);
let condvar = CondVar::new();

// Thread 1: Wait for condition
let mut ready = mutex.lock().unwrap();
while !*ready {
    ready = condvar.wait(ready).unwrap(); // Handles poisoning
}

// Thread 2: Signal condition
*mutex.lock().unwrap() = true;
condvar.notify_one();
```

#### CondVarNonPoisoning (Simplified, No Poisoning)

**Use when**:
- Targeting WASM (single-threaded or multi-threaded)
- Embedded systems with `panic = "abort"`
- Poisoning overhead not needed
- Want simpler API without Result wrapping

**Example**:
```rust
use foundation_nostd::primitives::{CondVarNonPoisoning, RawSpinMutex};

let mutex = RawSpinMutex::new(false);
let condvar = CondVarNonPoisoning::new();

// Thread 1: Wait (no Result wrapping)
let mut ready = mutex.lock();
while !*ready {
    ready = condvar.wait(ready); // Returns guard directly
}

// Thread 2: Signal
*mutex.lock() = true;
condvar.notify_one();
```

#### RwLockCondVar (Read-Write Lock Integration)

**Use when**:
- Coordinating with RwLock (multiple readers, single writer)
- Need to wait on read or write access
- Want to notify waiting readers or writers

**Example**:
```rust
use foundation_nostd::primitives::{RwLockCondVar, SpinRwLock};

struct Cache {
    lock: SpinRwLock<Option<String>>,
    condvar: RwLockCondVar,
}

impl Cache {
    fn wait_for_data(&self) -> String {
        let mut data = self.lock.write().unwrap();
        while data.is_none() {
            data = self.condvar.wait_write(data).unwrap();
        }
        data.clone().unwrap()
    }

    fn set_data(&self, value: String) {
        *self.lock.write().unwrap() = Some(value);
        self.condvar.notify_all(); // Wake all waiting readers/writers
    }
}
```

## API Compatibility with std

### CondVar vs std::sync::Condvar

`CondVar` provides **full API compatibility** with `std::sync::Condvar`:

| std Method | foundation_nostd Equivalent | Notes |
|------------|----------------------------|-------|
| `new()` | `CondVar::new()` | ✅ Identical |
| `wait(guard)` | `wait(guard)` | ✅ Identical |
| `wait_while(guard, f)` | `wait_while(guard, f)` | ✅ Identical |
| `wait_timeout(guard, dur)` | `wait_timeout(guard, dur)` | ✅ Identical |
| `wait_timeout_while(guard, dur, f)` | `wait_timeout_while(guard, dur, f)` | ✅ Identical |
| `notify_one()` | `notify_one()` | ✅ Identical |
| `notify_all()` | `notify_all()` | ✅ Identical |

**Drop-in replacement**:
```rust
// Replace std imports
// use std::sync::{Condvar, Mutex};
use foundation_nostd::primitives::{CondVar as Condvar, SpinMutex as Mutex};

// Code works identically
let mutex = Mutex::new(false);
let condvar = Condvar::new();
```

See [06-std-compatibility.md](./06-std-compatibility.md) for detailed migration guide.

## Common Patterns

### 1. Event Notification

Wake threads when an event occurs:

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};

struct EventFlag {
    flag: SpinMutex<bool>,
    condvar: CondVar,
}

impl EventFlag {
    fn wait_for_event(&self) {
        let mut flag = self.flag.lock().unwrap();
        while !*flag {
            flag = self.condvar.wait(flag).unwrap();
        }
        *flag = false; // Reset flag
    }

    fn signal_event(&self) {
        *self.flag.lock().unwrap() = true;
        self.condvar.notify_all();
    }
}
```

### 2. Barrier Synchronization

Wait for N threads to reach a point:

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};

struct Barrier {
    mutex: SpinMutex<(usize, usize)>, // (waiting, total)
    condvar: CondVar,
}

impl Barrier {
    fn new(n: usize) -> Self {
        Self {
            mutex: SpinMutex::new((0, n)),
            condvar: CondVar::new(),
        }
    }

    fn wait(&self) {
        let mut state = self.mutex.lock().unwrap();
        let (waiting, total) = &mut *state;
        *waiting += 1;

        if *waiting == *total {
            // Last thread - wake everyone
            *waiting = 0;
            drop(state);
            self.condvar.notify_all();
        } else {
            // Wait for last thread
            while *waiting != 0 {
                state = self.condvar.wait(state).unwrap();
            }
        }
    }
}
```

### 3. Thread Pool Work Queue

Distribute work to worker threads:

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::VecDeque;

struct WorkQueue<T> {
    queue: SpinMutex<VecDeque<T>>,
    condvar: CondVar,
}

impl<T> WorkQueue<T> {
    fn submit(&self, work: T) {
        self.queue.lock().unwrap().push_back(work);
        self.condvar.notify_one(); // Wake one idle worker
    }

    fn get_work(&self) -> Option<T> {
        let mut queue = self.queue.lock().unwrap();
        while queue.is_empty() {
            queue = self.condvar.wait(queue).unwrap();
        }
        queue.pop_front()
    }
}
```

## Anti-Patterns (What NOT to Do)

### ❌ Don't Forget the While Loop

```rust
// WRONG - Susceptible to spurious wakeups
let mut ready = mutex.lock().unwrap();
if !*ready {
    ready = condvar.wait(ready).unwrap(); // ❌ Only checks once
}

// CORRECT - Handles spurious wakeups
let mut ready = mutex.lock().unwrap();
while !*ready { // ✅ Re-checks after wakeup
    ready = condvar.wait(ready).unwrap();
}
```

**Why**: `wait()` can return without a notification (spurious wakeup). Always re-check the condition.

### ❌ Don't Hold Lock While Notifying (Performance)

```rust
// SUBOPTIMAL - Holding lock during notify
let mut data = mutex.lock().unwrap();
*data = new_value;
condvar.notify_one(); // ❌ Lock still held
drop(data); // Lock released here

// BETTER - Release lock before notify
let mut data = mutex.lock().unwrap();
*data = new_value;
drop(data); // ✅ Release lock first
condvar.notify_one(); // Waking thread can acquire lock immediately
```

**Why**: If lock is held during `notify_one()`, the woken thread will immediately block trying to acquire it. Releasing first reduces contention.

### ❌ Don't Mix Variants

```rust
// WRONG - Mixing poisoning and non-poisoning
let mutex = SpinMutex::new(0); // Poisoning variant
let condvar = CondVarNonPoisoning::new(); // Non-poisoning variant
// ❌ Type mismatch - won't compile

// CORRECT - Use matching types
let mutex = SpinMutex::new(0);
let condvar = CondVar::new(); // ✅ Both support poisoning
```

### ❌ Don't Busy-Wait Instead

```rust
// WRONG - Busy-waiting wastes CPU
loop {
    let ready = mutex.lock().unwrap();
    if *ready {
        break;
    }
    drop(ready);
    // ❌ Constantly re-checking, burning CPU cycles
}

// CORRECT - Use CondVar
let mut ready = mutex.lock().unwrap();
while !*ready {
    ready = condvar.wait(ready).unwrap(); // ✅ Sleeps efficiently
}
```

## Performance Characteristics

### Memory Overhead

| Variant | Size (bytes) | Notes |
|---------|--------------|-------|
| `CondVar` | 32-64 | Includes poisoning state |
| `CondVarNonPoisoning` | 32-48 | No poisoning overhead |
| `RwLockCondVar` | 32-64 | Includes poisoning state |

**WASM optimization**: In single-threaded WASM, some operations become no-ops, reducing overhead.

### Operation Latency (Uncontended)

| Operation | Typical Latency | Notes |
|-----------|-----------------|-------|
| `notify_one()` | < 100ns | Fast, just sets flag and wakes thread |
| `notify_all()` | O(N waiters) | Scales linearly with waiting threads |
| `wait()` | ~1-10µs | Includes thread park/unpark overhead |
| `wait_timeout()` | ~1-10µs + timeout | Adds timer overhead |

**Contention impact**: With high contention (many threads), latency increases due to lock spinning and thread scheduling.

### Scalability

- **`notify_one()`** - O(1) complexity, wakes one thread
- **`notify_all()`** - O(N) complexity, wakes all N waiting threads
- **Wait queue** - FIFO ordering for fairness

## WASM Considerations

### Single-Threaded WASM

In single-threaded WASM:
- **`wait()` may panic** or return immediately (no other threads to wake it)
- **`notify_*()` is a no-op** (no other threads exist)
- **Use `CondVarNonPoisoning`** for better WASM compatibility

### Multi-Threaded WASM (Atomics + Threads)

With WASM threads support:
- All operations work normally
- Requires `--target-feature +atomics`
- Requires `SharedArrayBuffer` support in browser

**Recommendation**: Use `CondVarNonPoisoning` for WASM to avoid poisoning overhead in panic=abort environments.

See [05-wasm-considerations.md](./05-wasm-considerations.md) for detailed WASM guide.

## Next Steps

- **[01-condvar-theory.md](./01-condvar-theory.md)** - Deep dive into condition variable theory, spurious wakeups, and memory ordering
- **[02-implementation-details.md](./02-implementation-details.md)** - How this library implements CondVars using bit-masking and thread parking
- **[03-variants-comparison.md](./03-variants-comparison.md)** - Detailed comparison table of all three variants
- **[04-usage-patterns.md](./04-usage-patterns.md)** - More real-world patterns and examples
- **[05-wasm-considerations.md](./05-wasm-considerations.md)** - WASM-specific behavior and optimization
- **[06-std-compatibility.md](./06-std-compatibility.md)** - Migration guide from std::sync::Condvar

## Summary

**Key Takeaways**:
- CondVars enable efficient waiting without busy-spinning
- Always use with a mutex and check condition in a `while` loop
- Three variants: `CondVar` (standard), `CondVarNonPoisoning` (WASM/embedded), `RwLockCondVar` (RwLock integration)
- Full API compatibility with `std::sync::Condvar`
- Release lock before notifying for better performance
- WASM users should prefer `CondVarNonPoisoning`

**Choose your variant**:
- General purpose → `CondVar`
- WASM/embedded → `CondVarNonPoisoning`
- RwLock coordination → `RwLockCondVar`

Happy synchronizing! 🦀
