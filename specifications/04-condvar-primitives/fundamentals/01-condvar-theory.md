# Condition Variables - Theory and Concepts

## Table of Contents
- [What Problem Do Condition Variables Solve?](#what-problem-do-condition-variables-solve)
- [The Classic Producer-Consumer Problem](#the-classic-producer-consumer-problem)
- [Wait/Notify Semantics](#waitnotify-semantics)
- [Spurious Wakeups](#spurious-wakeups)
- [Memory Ordering and Synchronization](#memory-ordering-and-synchronization)
- [Comparison with Other Primitives](#comparison-with-other-primitives)
- [Theoretical Foundations](#theoretical-foundations)

## What Problem Do Condition Variables Solve?

### The Coordination Problem

In concurrent programming, threads often need to **wait for a condition** before proceeding. For example:
- A consumer waits for data to be available in a queue
- A worker thread waits for work to be submitted
- Multiple threads wait at a synchronization barrier

Without condition variables, you have two bad options:

#### Option 1: Busy-Waiting (CPU Intensive)

```rust
// BAD: Busy-wait loop
loop {
    let data = queue.lock().unwrap();
    if !data.is_empty() {
        // Process data
        break;
    }
    drop(data); // Release lock
    // Loop immediately, burning CPU cycles
}
```

**Problems**:
- ❌ Wastes 100% CPU while waiting
- ❌ Increases contention on the lock
- ❌ Poor energy efficiency (important for mobile/embedded)
- ❌ Prevents other threads from running efficiently

#### Option 2: Periodic Polling (Inefficient)

```rust
// BAD: Sleep and poll
loop {
    let data = queue.lock().unwrap();
    if !data.is_empty() {
        break;
    }
    drop(data);
    std::thread::sleep(Duration::from_millis(10)); // ❌ Arbitrary delay
}
```

**Problems**:
- ❌ Adds unnecessary latency (minimum 10ms in this example)
- ❌ Still wastes some CPU on repeated checks
- ❌ Hard to tune: too short → CPU waste, too long → high latency

#### The Solution: Condition Variables

```rust
// GOOD: Efficient waiting with CondVar
let mut data = queue.lock().unwrap();
while data.is_empty() {
    data = condvar.wait(data).unwrap(); // ✅ Sleeps until notified
}
// Process data immediately when available
```

**Benefits**:
- ✅ Zero CPU usage while waiting
- ✅ Wakes up immediately when condition changes (no polling delay)
- ✅ Atomically releases lock and sleeps (no race conditions)
- ✅ Energy efficient

### The Key Insight: Atomic Lock Release

The critical property of `wait()` is that it **atomically**:
1. Releases the mutex
2. Puts the thread to sleep
3. Adds thread to the wait queue

This atomicity prevents the **lost wakeup problem**:

```rust
// WRONG: Manual implementation has race condition
let guard = mutex.lock();
if !condition {
    drop(guard);          // ⚠️ Release lock
    // ← RACE: notify() could happen here!
    park();               // ⚠️ Then we sleep - miss the notification!
}

// CORRECT: CondVar.wait() does this atomically
while !condition {
    guard = condvar.wait(guard); // ✅ Atomic release + sleep
}
```

**Why atomicity matters**: If there's a gap between releasing the lock and sleeping, another thread could acquire the lock, change the condition, call `notify()`, and release the lock—all before we start sleeping. We'd then sleep forever waiting for a notification that already happened.

## The Classic Producer-Consumer Problem

Condition variables elegantly solve the producer-consumer problem:

### Problem Statement

- **Producers** generate data and add it to a shared queue
- **Consumers** process data from the queue
- Queue has bounded capacity
- Producers must wait when queue is full
- Consumers must wait when queue is empty

### Solution with Two CondVars

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::VecDeque;

struct BoundedQueue<T> {
    data: SpinMutex<VecDeque<T>>,
    not_empty: CondVar, // Signals consumers: data available
    not_full: CondVar,  // Signals producers: space available
    capacity: usize,
}

impl<T> BoundedQueue<T> {
    fn new(capacity: usize) -> Self {
        Self {
            data: SpinMutex::new(VecDeque::with_capacity(capacity)),
            not_empty: CondVar::new(),
            not_full: CondVar::new(),
            capacity,
        }
    }

    /// Producer: Add item to queue
    fn push(&self, item: T) {
        let mut queue = self.data.lock().unwrap();

        // Wait until queue has space
        while queue.len() >= self.capacity {
            queue = self.not_full.wait(queue).unwrap();
        }

        queue.push_back(item);

        // Notify one waiting consumer that data is available
        drop(queue);
        self.not_empty.notify_one();
    }

    /// Consumer: Remove item from queue
    fn pop(&self) -> T {
        let mut queue = self.data.lock().unwrap();

        // Wait until queue has data
        while queue.is_empty() {
            queue = self.not_empty.wait(queue).unwrap();
        }

        let item = queue.pop_front().unwrap();

        // Notify one waiting producer that space is available
        drop(queue);
        self.not_full.notify_one();

        item
    }
}
```

**Key design points**:
1. **Two condition variables**: One for "not empty", one for "not full"
2. **While loops**: Handle spurious wakeups and multiple waiters
3. **Drop lock before notify**: Reduces contention (optional optimization)
4. **notify_one()**: Only one waiter needs to wake up

### Why Two CondVars?

Using separate CondVars for different conditions is more efficient than one:

```rust
// SUBOPTIMAL: Single CondVar
fn push(&self, item: T) {
    // ...
    self.condvar.notify_all(); // ❌ Wakes ALL threads (producers + consumers)
}

// OPTIMAL: Two CondVars
fn push(&self, item: T) {
    // ...
    self.not_empty.notify_one(); // ✅ Wakes only one consumer
}
```

**Why it matters**: With one CondVar, every change wakes all threads (both producers and consumers), causing **thundering herd**—many threads wake up but only one can proceed, wasting CPU on context switches.

## Wait/Notify Semantics

### The Wait Operation

```rust
pub fn wait<'a, T>(
    &self,
    guard: MutexGuard<'a, T>
) -> LockResult<MutexGuard<'a, T>>
```

**What `wait()` does**:
1. **Atomically release** the mutex (drops guard)
2. **Add thread** to the wait queue
3. **Block thread** (park/sleep)
4. When woken up:
   - **Re-acquire** the mutex (may block if contended)
   - **Return** the mutex guard

**Atomicity guarantee**: Steps 1-3 happen atomically—no other thread can acquire the lock and modify state between releasing and sleeping.

**Memory ordering**: `wait()` has:
- **Release semantics** when dropping the lock (step 1)
- **Acquire semantics** when re-acquiring the lock (step 4)

This ensures all writes before `wait()` are visible to the thread that calls `notify()`, and all writes before `notify()` are visible after `wait()` returns.

### The Notify Operations

#### notify_one()

```rust
pub fn notify_one(&self)
```

**Behavior**:
- Wakes up **at most one** thread from the wait queue
- If no threads are waiting, does nothing (notification is "lost")
- FIFO ordering (first thread to wait is first to be woken)

**Use when**: Only one thread needs to act on the condition change.

**Example**: Producer-consumer queue - one new item means one consumer can proceed.

#### notify_all()

```rust
pub fn notify_all(&self)
```

**Behavior**:
- Wakes up **all** threads in the wait queue
- If no threads are waiting, does nothing
- All woken threads will compete to re-acquire the lock

**Use when**: The condition change is relevant to all waiting threads.

**Example**: Shutdown signal - all worker threads should wake up and exit.

### When Notifications Are "Lost"

**Important**: Notifications are **not queued**. If you call `notify_one()` when no threads are waiting, the notification is lost:

```rust
// Thread 1
condvar.notify_one(); // ← Nobody is waiting yet

// Thread 2 (starts later)
let mut guard = mutex.lock().unwrap();
condvar.wait(guard); // ← Will NOT see the earlier notification
                     // Will sleep until a NEW notification arrives
```

**Implication**: Always check the condition in a shared variable protected by the mutex. The CondVar is just a hint that the condition *might* have changed.

```rust
// CORRECT pattern
let mut guard = mutex.lock().unwrap();
while !condition_is_true(&guard) {
    guard = condvar.wait(guard).unwrap();
}
// Condition is definitely true now
```

## Spurious Wakeups

### What Are Spurious Wakeups?

A **spurious wakeup** occurs when `wait()` returns without any thread calling `notify_one()` or `notify_all()`.

**Why they happen**:
1. **OS thread scheduler** may wake threads for internal reasons
2. **Signal handling** on Unix systems can interrupt waits
3. **Implementation optimization** may batch wakeups
4. **Multicore race conditions** in lock implementation

**Key fact**: Spurious wakeups are **explicitly allowed** by the Rust and POSIX specifications for condition variables.

### Handling Spurious Wakeups

**Always use a `while` loop** to re-check the condition:

```rust
// ❌ WRONG: Only checks once
if !ready {
    guard = condvar.wait(guard)?;
}
// Guard here might have !ready == true (spurious wakeup)

// ✅ CORRECT: Re-checks after wakeup
while !ready {
    guard = condvar.wait(guard)?;
}
// Guard here is guaranteed to have ready == true
```

**Why while, not if**: The `while` loop ensures that even if `wait()` returns spuriously, we'll immediately wait again. Only when the condition is actually true will we exit the loop.

### Example: Handling Spurious Wakeups

```rust
struct Counter {
    value: SpinMutex<usize>,
    condvar: CondVar,
}

impl Counter {
    fn wait_for_value(&self, target: usize) {
        let mut value = self.value.lock().unwrap();

        // While loop handles spurious wakeups
        while *value < target {
            // Re-check condition after every wakeup
            value = self.condvar.wait(value).unwrap();
        }

        // Guaranteed: *value >= target
    }

    fn increment(&self) {
        let mut value = self.value.lock().unwrap();
        *value += 1;
        drop(value);
        self.condvar.notify_all();
    }
}
```

Even if `wait()` returns spuriously (condition not met), the `while` loop catches it and waits again.

### Frequency of Spurious Wakeups

**In practice**: Spurious wakeups are rare on modern systems (< 0.1% of wakeups in typical workloads).

**However**: You must still handle them correctly with a `while` loop. Treating spurious wakeups as bugs will lead to subtle, hard-to-reproduce race conditions.

**Best practice**: Always assume spurious wakeups can happen, even if they're rare in your testing.

## Memory Ordering and Synchronization

### Synchronization Guarantees

Condition variables provide strong memory ordering guarantees:

```rust
// Thread A
let mut data = shared.mutex.lock().unwrap();
*data = 42;                          // Write
drop(data);
shared.condvar.notify_one();         // Synchronizes-with wait()

// Thread B
let mut data = shared.mutex.lock().unwrap();
while *data == 0 {
    data = shared.condvar.wait(data).unwrap(); // Happens-after notify()
}
assert_eq!(*data, 42);              // ✅ Guaranteed to see 42
```

**Happens-before relationship**:
- All writes before `notify_*()` **happen-before** the corresponding `wait()` returns
- This is enforced by the mutex's acquire/release semantics

### Why Mutex + CondVar?

The mutex provides the memory ordering:

1. **Releasing the mutex** before notify has **Release semantics**
   - All prior writes are visible to the thread that next acquires the mutex

2. **Acquiring the mutex** after wait has **Acquire semantics**
   - All writes from the notifying thread are visible

Without the mutex, you'd need explicit memory barriers and atomic operations to ensure visibility.

### Memory Ordering in wait()

```rust
impl CondVar {
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>)
        -> LockResult<MutexGuard<'a, T>>
    {
        // 1. Release mutex (Release memory ordering)
        let mutex = MutexGuard::unlock(guard);

        // 2. Add to wait queue
        self.wait_queue.push_current_thread();

        // 3. Park thread (suspend execution)
        thread::park();

        // 4. Re-acquire mutex (Acquire memory ordering)
        let guard = mutex.lock();

        guard
    }
}
```

The **Release** on mutex unlock ensures the notifying thread sees all our writes. The **Acquire** on mutex lock ensures we see all the notifying thread's writes.

## Comparison with Other Primitives

### CondVar vs Semaphore

| Feature | CondVar | Semaphore |
|---------|---------|-----------|
| **Purpose** | Wait for arbitrary condition | Count-based signaling |
| **Requires Mutex** | Yes (always paired) | No (self-contained) |
| **Notification** | Not queued (lost if no waiters) | Counted (accumulated) |
| **Use Case** | Complex conditions | Simple counting (permits) |

**Example where CondVar is better**:
```rust
// Wait for "data.len() > 5 AND data[0] == 'A'"
// ✅ Easy with CondVar + mutex
while !(data.len() > 5 && data[0] == 'A') {
    guard = condvar.wait(guard)?;
}

// ❌ Impossible with semaphore alone
```

**Example where Semaphore is better**:
```rust
// Limited resource pool (e.g., 10 database connections)
// Semaphore tracks available count naturally
semaphore.acquire(); // Block if count == 0
use_resource();
semaphore.release(); // Increment count
```

### CondVar vs Channel

| Feature | CondVar | Channel |
|---------|---------|---------|
| **Communication** | Shared state + notification | Message passing |
| **Ownership** | Shared (mutex-protected) | Transfer (send/recv) |
| **Flexibility** | Arbitrary conditions | Fixed message types |
| **Buffering** | No (just notification) | Yes (bounded/unbounded) |

**Use CondVar when**: Multiple threads coordinate on shared state (e.g., work queue, barrier)

**Use Channel when**: Transferring ownership of data between threads (e.g., task submission, results)

### CondVar vs Atomic Variables

| Feature | CondVar | Atomic |
|---------|---------|--------|
| **Waiting** | Sleeps (efficient) | Busy-spin (CPU waste) |
| **Complexity** | Handles complex state | Simple flags/counters |
| **Latency** | ~1-10µs (thread wake) | ~10-100ns (no sleep) |

**Use Atomic when**: Very short waits (< 1µs), simple flag checks

**Use CondVar when**: Waits could be long, need to avoid busy-spinning

### CondVar vs Barrier

| Feature | CondVar | Barrier |
|---------|---------|---------|
| **Purpose** | General waiting | Synchronize N threads |
| **Reusability** | Infinite | Once (or reusable variant) |
| **Flexibility** | Any condition | Fixed thread count |

**Note**: Barriers are often *implemented* using CondVars internally!

```rust
// Barrier implementation using CondVar
struct Barrier {
    mutex: Mutex<(usize, usize)>, // (count, total)
    condvar: CondVar,
}

impl Barrier {
    fn wait(&self) {
        let mut state = self.mutex.lock().unwrap();
        state.0 += 1;
        if state.0 == state.1 {
            state.0 = 0;
            self.condvar.notify_all();
        } else {
            while state.0 != 0 {
                state = self.condvar.wait(state).unwrap();
            }
        }
    }
}
```

## Theoretical Foundations

### Monitors and Mesa-Style Semantics

Condition variables follow **Mesa-style** monitor semantics (named after the Mesa programming language):

**Mesa-style** (what Rust uses):
- `wait()` returns with lock held, but condition **not guaranteed**
- Woken thread must re-check condition (hence `while` loop)
- Allows spurious wakeups

**Hoare-style** (alternative, not used in Rust):
- `wait()` returns with condition **guaranteed to be true**
- No spurious wakeups allowed
- More expensive to implement (requires immediate transfer of control)

**Why Mesa-style?**:
1. More efficient on modern hardware
2. Simpler implementation (less scheduling overhead)
3. Composable (allows multiple conditions on one CondVar)

### Fairness

Most CondVar implementations provide **FIFO fairness**: the first thread to call `wait()` is the first to be woken by `notify_one()`.

**This library guarantees FIFO ordering** through the wait queue implementation.

**Why FIFO?**:
- Prevents starvation (thread waiting longest gets priority)
- Predictable behavior (easier to reason about)
- Matches user expectation (fair queueing)

**However**: Due to scheduling jitter, a woken thread might not run immediately. Another thread could acquire the lock first, even if it wasn't the earliest waiter.

### Correctness Properties

A correct CondVar implementation must guarantee:

1. **Safety**: No lost wakeups
   - If `notify()` is called while threads wait, at least one thread must wake

2. **Liveness**: No deadlock
   - Threads don't wait forever if condition becomes true

3. **Atomicity**: Atomic release + sleep
   - No race between releasing lock and sleeping

4. **Memory ordering**: Synchronization
   - Writes before `notify()` visible after `wait()` (via mutex)

**This library ensures all four properties** through careful use of atomics and thread parking.

## Summary

**Key Takeaways**:
- CondVars solve the efficient waiting problem without busy-spinning
- Always paired with a mutex to protect the condition
- `wait()` atomically releases lock and sleeps (prevents lost wakeups)
- Notifications are not queued (lost if nobody waiting)
- **Always use `while` loop** to handle spurious wakeups
- Memory ordering guaranteed through mutex acquire/release
- Choose CondVar over semaphores for complex conditions
- FIFO fairness prevents starvation

**Mental model**: Think of CondVar as a "wake-up service"—you tell it to wake you when something interesting happens, but you still check if that thing actually happened (spurious wakeups).

## Next Steps

- **[02-implementation-details.md](./02-implementation-details.md)** - See how this library implements these concepts using bit-masking and thread parking
- **[03-variants-comparison.md](./03-variants-comparison.md)** - Compare the three CondVar variants
- **[04-usage-patterns.md](./04-usage-patterns.md)** - More practical examples and patterns
- **[05-wasm-considerations.md](./05-wasm-considerations.md)** - WASM-specific behavior
- **[06-std-compatibility.md](./06-std-compatibility.md)** - Migration from std::sync::Condvar
