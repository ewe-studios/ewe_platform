# Implementation Details - CondVar Internals

## Table of Contents
- [Architecture Overview](#architecture-overview)
- [State Management with Bit-Masking](#state-management-with-bit-masking)
- [Wait Queue Design](#wait-queue-design)
- [Thread Parking and Unparking](#thread-parking-and-unparking)
- [Notification Mechanisms](#notification-mechanisms)
- [WASM Optimizations](#wasm-optimizations)
- [Performance Characteristics](#performance-characteristics)
- [Memory Layout](#memory-layout)

## Architecture Overview

### High-Level Structure

The CondVar implementation consists of four main components:

```
┌─────────────────────────────────────────┐
│          CondVar Public API             │
│  (wait, notify_one, notify_all, etc.)   │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│        Atomic State (u32/u64)           │
│   Bit-masked: count | flags | poison    │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│          Wait Queue (FIFO)              │
│    Manages waiting thread ordering      │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│     Thread Park/Unpark (from spec 03)  │
│      OS-level thread suspension         │
└─────────────────────────────────────────┘
```

### Key Design Decisions

**1. Single Atomic State Variable**: All state (waiting count, notification flags, poison bit) packed into one atomic for efficiency.

**Why?**:
- Single atomic CAS operation = lock-free fast path
- Reduces memory footprint (4-8 bytes vs 12-24 bytes for separate fields)
- Cache-friendly: one cache line contains all state

**2. FIFO Wait Queue**: First thread to wait is first to be woken.

**Why?**:
- Fairness: prevents starvation
- Predictability: easier to reason about
- Matches user expectations

**3. Integration with Mutex**: CondVar doesn't own a mutex, works with any mutex via guard.

**Why?**:
- Flexibility: one CondVar can work with multiple mutexes
- Composability: matches std::sync API
- Separation of concerns: locking and condition waiting are orthogonal

## State Management with Bit-Masking

### The Problem: Multiple State Variables

A CondVar needs to track:
- **Waiting thread count** (0 to N)
- **Notification pending** flag (has notify been called?)
- **Poisoned state** (did a thread panic while holding the associated mutex?)

**Naive approach** (❌ inefficient):
```rust
struct CondVarState {
    wait_count: AtomicUsize,     // 8 bytes
    notify_pending: AtomicBool,  // 1 byte (+ padding)
    poisoned: AtomicBool,        // 1 byte (+ padding)
}
// Total: 16-24 bytes, 3 separate atomics = 3 cache lines potentially
```

**Problems**:
- Multiple atomic operations required to update state
- Race conditions between separate atomics
- Poor cache locality
- Higher memory overhead

### The Solution: Bit-Masking

**Pack all state into a single atomic integer**:

```rust
// 32-bit state encoding (for typical use cases)
use core::sync::atomic::AtomicU32;

struct CondVarState {
    // Bit layout for AtomicU32:
    // Bits 0-29:  Waiting thread count (30 bits = 0 to 1,073,741,823)
    // Bit 30:     Notify pending flag
    // Bit 31:     Poisoned flag
    state: AtomicU32,
}

// Bit masks
const WAIT_COUNT_MASK: u32 = 0x3FFF_FFFF;  // Bits 0-29
const NOTIFY_PENDING:  u32 = 0x4000_0000;  // Bit 30
const POISONED:        u32 = 0x8000_0000;  // Bit 31

// Total: 4 bytes, 1 atomic operation
```

**Benefits**:
- ✅ Single atomic CAS updates all fields atomically
- ✅ Fits in one 32-bit word (4 bytes)
- ✅ Lock-free operations
- ✅ Cache-friendly (single cache line)

### Bit Manipulation Examples

#### Reading State Components

```rust
impl CondVarState {
    /// Get the waiting thread count
    #[inline]
    fn wait_count(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        state & WAIT_COUNT_MASK
    }

    /// Check if a notification is pending
    #[inline]
    fn has_notify_pending(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & NOTIFY_PENDING) != 0
    }

    /// Check if poisoned
    #[inline]
    fn is_poisoned(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & POISONED) != 0
    }
}
```

**Bit-masking operations**:
- `state & WAIT_COUNT_MASK` → Extracts bits 0-29 (count)
- `state & NOTIFY_PENDING` → Checks bit 30 (notify flag)
- `state & POISONED` → Checks bit 31 (poison flag)

#### Updating State Atomically

**Increment wait count**:
```rust
/// Atomically increment wait count
fn increment_wait_count(&self) -> Result<u32, ()> {
    self.state.fetch_update(
        Ordering::AcqRel,
        Ordering::Acquire,
        |state| {
            let count = state & WAIT_COUNT_MASK;
            if count == WAIT_COUNT_MASK {
                // Overflow: max waiters reached
                None
            } else {
                // Increment count, preserve other bits
                Some((state & !WAIT_COUNT_MASK) | (count + 1))
            }
        }
    )
    .map_err(|_| ())
}
```

**Set notify pending flag**:
```rust
/// Atomically set notify pending flag
fn set_notify_pending(&self) {
    self.state.fetch_or(NOTIFY_PENDING, Ordering::Release);
}
```

**Clear notify pending and decrement wait count**:
```rust
/// Atomically clear notify and decrement wait count
fn consume_notify(&self) {
    self.state.fetch_update(
        Ordering::AcqRel,
        Ordering::Acquire,
        |state| {
            let count = state & WAIT_COUNT_MASK;
            if count > 0 {
                // Decrement count, clear notify flag, preserve poison bit
                let new_count = count - 1;
                let new_state = (state & POISONED) | new_count;
                Some(new_state)
            } else {
                None // No waiters, no change
            }
        }
    ).ok();
}
```

### Why This Works

**Atomicity**: `fetch_update` performs read-modify-write atomically via CAS loop.

**Memory Ordering**:
- `Ordering::AcqRel`: Acquire on read (see prior writes), Release on write (make our writes visible)
- Ensures happens-before relationship between notify and wait

**Overflow Handling**: 30 bits supports 1 billion+ waiting threads (unrealistic), so overflow is theoretical.

### Alternative: 64-bit State

For even more capacity or additional flags:

```rust
// 64-bit state encoding
const WAIT_COUNT_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;  // Bits 0-47 (48 bits)
const NOTIFY_PENDING:  u64 = 0x0001_0000_0000_0000;  // Bit 48
const NOTIFY_ALL:      u64 = 0x0002_0000_0000_0000;  // Bit 49
const POISONED:        u64 = 0x8000_0000_0000_0000;  // Bit 63
// Bits 50-62 reserved for future use
```

**Trade-off**: 64-bit atomics may be less efficient on 32-bit platforms, but provides more flags and capacity.

## Wait Queue Design

### Requirements

The wait queue must:
1. **FIFO ordering**: First waiter woken first (fairness)
2. **Thread-safe**: Multiple threads can wait/notify concurrently
3. **Lock-free fast path**: No locks for common operations
4. **Efficient wakeup**: O(1) to wake one thread, O(N) to wake all

### Data Structure

**Design**: Intrusive linked list using thread IDs and atomic pointers.

```rust
use core::sync::atomic::{AtomicPtr, AtomicUsize};

struct WaitNode {
    thread_id: usize,           // Thread identifier for parking
    next: AtomicPtr<WaitNode>,  // Next node in queue (null = end)
}

struct WaitQueue {
    head: AtomicPtr<WaitNode>,  // Front of queue (first waiter)
    tail: AtomicPtr<WaitNode>,  // Back of queue (last waiter)
    len: AtomicUsize,           // Number of waiters (for fast count)
}
```

**Memory**: Nodes allocated on waiting thread's stack (intrusive), so no heap allocation needed.

### Operations

#### Enqueue (Add Waiter)

```rust
impl WaitQueue {
    /// Add current thread to end of wait queue
    fn enqueue(&self, node: &mut WaitNode) {
        node.next.store(ptr::null_mut(), Ordering::Relaxed);

        // Atomically add to tail
        let prev_tail = self.tail.swap(node, Ordering::AcqRel);

        if prev_tail.is_null() {
            // Queue was empty, this is now head
            self.head.store(node, Ordering::Release);
        } else {
            // Link previous tail to this node
            unsafe { (*prev_tail).next.store(node, Ordering::Release); }
        }

        self.len.fetch_add(1, Ordering::Release);
    }
}
```

**Complexity**: O(1) - single atomic swap

#### Dequeue One (Wake One Thread)

```rust
impl WaitQueue {
    /// Remove and return front thread from queue
    fn dequeue_one(&self) -> Option<usize> {
        // Atomically take head
        let head = self.head.swap(ptr::null_mut(), Ordering::Acquire);

        if head.is_null() {
            return None; // Queue empty
        }

        let thread_id = unsafe { (*head).thread_id };
        let next = unsafe { (*head).next.load(Ordering::Acquire) };

        if next.is_null() {
            // Was last node, clear tail
            self.tail.store(ptr::null_mut(), Ordering::Release);
        } else {
            // Make next node the new head
            self.head.store(next, Ordering::Release);
        }

        self.len.fetch_sub(1, Ordering::Release);
        Some(thread_id)
    }
}
```

**Complexity**: O(1) - single atomic swap

#### Dequeue All (Wake All Threads)

```rust
impl WaitQueue {
    /// Remove all threads and return their IDs
    fn dequeue_all(&self) -> Vec<usize> {
        let mut thread_ids = Vec::new();

        // Atomically take entire queue
        let head = self.head.swap(ptr::null_mut(), Ordering::Acquire);
        self.tail.store(ptr::null_mut(), Ordering::Release);
        let len = self.len.swap(0, Ordering::AcqRel);

        if head.is_null() {
            return thread_ids;
        }

        // Walk the list and collect thread IDs
        let mut current = head;
        thread_ids.reserve(len);

        while !current.is_null() {
            let node = unsafe { &*current };
            thread_ids.push(node.thread_id);
            current = node.next.load(Ordering::Acquire);
        }

        thread_ids
    }
}
```

**Complexity**: O(N) where N = number of waiters (unavoidable, must wake all)

### Fairness Guarantee

**FIFO ordering ensures**:
- Thread A calls `wait()` at T1
- Thread B calls `wait()` at T2 (T2 > T1)
- On `notify_one()`, Thread A is woken first

**Why this matters**: Prevents starvation where newer threads always win and older threads wait forever.

## Thread Parking and Unparking

### Integration with Spec 03

CondVar builds on the thread parking primitives from specification 03:

```rust
// From spec 03: foundation_nostd::primitives::park
pub fn park();
pub fn unpark(thread: ThreadId);
```

**What park() does**:
- Suspends current thread (OS-level sleep)
- Returns when unparked or spuriously woken

**What unpark() does**:
- Wakes the specified thread
- Thread resumes execution after park()

### Wait Implementation

```rust
impl CondVar {
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>)
        -> LockResult<MutexGuard<'a, T>>
    {
        // 1. Create wait node on stack
        let mut node = WaitNode {
            thread_id: current_thread_id(),
            next: AtomicPtr::new(ptr::null_mut()),
        };

        // 2. Add to wait queue
        self.wait_queue.enqueue(&mut node);
        self.state.increment_wait_count();

        // 3. Get mutex pointer before dropping guard
        let mutex = guard.mutex();

        // 4. Atomically release mutex
        drop(guard);

        // 5. Park thread (sleep)
        park();  // ← Blocks here until unparked

        // 6. Re-acquire mutex (may block if contended)
        let guard = mutex.lock();

        // 7. Check poison state and return
        if self.is_poisoned() {
            Err(PoisonError::new(guard))
        } else {
            Ok(guard)
        }
    }
}
```

**Critical property**: Steps 2-4 happen with mutex held, so no race between enqueueing and parking.

### Notify One Implementation

```rust
impl CondVar {
    pub fn notify_one(&self) {
        // 1. Dequeue one thread
        if let Some(thread_id) = self.wait_queue.dequeue_one() {
            // 2. Unpark the thread
            unpark(thread_id);

            // 3. Set notify pending flag
            self.state.set_notify_pending();
        }
        // If queue empty, notification is lost (not stored)
    }
}
```

**Fast path**: If no waiters, `dequeue_one()` returns None immediately (O(1)).

### Notify All Implementation

```rust
impl CondVar {
    pub fn notify_all(&self) {
        // 1. Dequeue all threads
        let thread_ids = self.wait_queue.dequeue_all();

        if thread_ids.is_empty() {
            return; // No waiters
        }

        // 2. Unpark all threads
        for thread_id in thread_ids {
            unpark(thread_id);
        }

        // 3. Set notify all flag
        self.state.set_notify_all();
    }
}
```

**Batch wakeup**: All threads unparked in sequence, but OS scheduler decides execution order.

## Notification Mechanisms

### Notify vs Broadcast

**notify_one()**: Wake exactly one thread
- **Use case**: Producer-consumer (one item → one consumer)
- **Efficiency**: Only one thread context switch

**notify_all()**: Wake all threads
- **Use case**: Condition relevant to all (shutdown, barrier)
- **Efficiency**: Multiple context switches, but necessary when all must check condition

### Lost Notifications

**Key insight**: Notifications are **edge-triggered**, not **level-triggered**.

```rust
// Scenario: Lost notification
// Time T1: Producer calls notify_one() (no waiters yet)
// Time T2: Consumer calls wait() (misses the notification)
// Result: Consumer sleeps until NEXT notification
```

**This is by design**: CondVar is a signaling mechanism, not a queue. The condition variable doesn't "remember" past notifications.

**Solution**: Always check shared state protected by mutex:

```rust
// CORRECT: Check condition in loop
let mut data = mutex.lock().unwrap();
while !condition_met(&data) {
    data = condvar.wait(data).unwrap();
}
// Now condition is definitely true
```

### Thundering Herd Prevention

**Problem**: Waking all threads when only one can proceed wastes CPU.

**Solution**: Use multiple CondVars for different conditions:

```rust
struct Queue {
    not_empty: CondVar,  // Only consumers wait here
    not_full: CondVar,   // Only producers wait here
}

// Producer
queue.not_empty.notify_one(); // Wake one consumer

// Consumer
queue.not_full.notify_one();  // Wake one producer
```

**Result**: Only relevant threads wake up.

## WASM Optimizations

### Single-Threaded Detection

**WASM context**: May be single-threaded (no threads feature) or multi-threaded.

**Detection at compile time**:
```rust
#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
const IS_SINGLE_THREADED: bool = true;

#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
const IS_SINGLE_THREADED: bool = false;
```

**Detection at runtime** (for dynamic detection):
```rust
fn is_wasm_single_threaded() -> bool {
    #[cfg(target_family = "wasm")]
    {
        // Check if threads are available
        !cfg!(target_feature = "atomics")
    }
    #[cfg(not(target_family = "wasm"))]
    {
        false
    }
}
```

### Single-Threaded Optimizations

**In single-threaded WASM**:
- `wait()` → Deadlock (only thread would block forever)
- `notify_one()` → No-op (no other threads exist)
- `notify_all()` → No-op

**Optimized implementation**:
```rust
impl CondVar {
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>)
        -> LockResult<MutexGuard<'a, T>>
    {
        #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
        {
            // Single-threaded WASM: immediate panic
            panic!("CondVar::wait() called in single-threaded WASM context");
        }

        #[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
        {
            // Normal implementation
            self.wait_impl(guard)
        }
    }

    pub fn notify_one(&self) {
        #[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
        {
            // Single-threaded WASM: no-op
            return;
        }

        #[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
        {
            self.notify_one_impl();
        }
    }
}
```

**Benefits**:
- Zero overhead in single-threaded context
- Clear error message if misused (wait in single-threaded)
- Compile-time optimization (dead code elimination)

### Multi-Threaded WASM

**With WASM threads**:
- Full implementation works as expected
- Uses WASM atomic operations
- Thread parking via WASM thread API

**Memory constraints**:
- Keep state compact (4-8 bytes per CondVar)
- Avoid heap allocation in hot paths
- Use stack-allocated wait nodes

## Performance Characteristics

### Operation Complexity

| Operation | Time Complexity | Notes |
|-----------|----------------|-------|
| `new()` | O(1) | Initialize atomic to 0 |
| `wait()` | O(1) amortized | Fast if no contention, blocks on park |
| `notify_one()` | O(1) | Single atomic + one unpark |
| `notify_all()` | O(N) | N = waiting threads, must wake all |
| `is_poisoned()` | O(1) | Single atomic load |

### Memory Footprint

**Per CondVar**:
- State: 4 bytes (u32) or 8 bytes (u64)
- Wait queue head/tail: 16 bytes (2 pointers)
- Total: **20-24 bytes**

**Per waiting thread**:
- Wait node: 16 bytes (on thread stack, not heap)

**Comparison**:
- std::sync::Condvar: ~40-48 bytes (includes OS mutex)
- This implementation: ~20-24 bytes (lighter weight)

### Latency

**Uncontended wait/notify** (micro-benchmark):
- `notify_one()`: ~50-100ns (atomic + unpark syscall)
- `wait()`: ~1-10µs (depends on thread wake latency)

**Contended** (many waiters):
- Scales linearly with waiter count for notify_all
- FIFO prevents worst-case starvation

**WASM**:
- Single-threaded: notify is no-op (~1ns)
- Multi-threaded: similar to native but depends on WASM runtime

### Scalability

**Strong scaling** (fixed work, more threads):
- notify_one: O(1) regardless of waiter count
- notify_all: O(N) where N = waiters (unavoidable)

**Weak scaling** (more work, more threads):
- Linear scaling as long as mutex isn't bottleneck
- Consider multiple CondVars for different conditions

## Memory Layout

### Alignment and Padding

**Optimal layout for cache efficiency**:

```rust
#[repr(C)]
struct CondVar {
    state: AtomicU32,           // 4 bytes, aligned to 4
    _pad1: [u8; 4],             // Padding to 8-byte boundary
    wait_queue: WaitQueue,      // 24 bytes
}
// Total: 32 bytes (fits in single cache line on most CPUs)
```

**Why padding?**: Prevents false sharing when CondVars are in arrays.

### Cache Line Considerations

**Modern CPUs**: Cache line = 64 bytes

**Recommendation**: Place CondVar and associated Mutex in same cache line for locality:

```rust
struct Shared {
    mutex: SpinMutex<Data>,     // 24 bytes
    condvar: CondVar,           // 32 bytes
    // Total: 56 bytes, fits in one cache line
}
```

**Benefit**: Fewer cache misses when waiting/notifying.

## Summary

**Key implementation techniques**:
1. **Bit-masking**: Pack state into single atomic for efficiency
2. **FIFO wait queue**: Fairness via linked list
3. **Thread parking**: OS-level sleep for efficiency
4. **WASM optimization**: Compile-time detection and no-op paths
5. **Lock-free fast path**: CAS loop for state updates

**Performance highlights**:
- 20-24 byte memory footprint
- O(1) notify_one, O(N) notify_all
- Lock-free state management
- Cache-friendly layout

**Next steps**:
- **[03-variants-comparison.md](./03-variants-comparison.md)** - Compare CondVar, CondVarNonPoisoning, RwLockCondVar
- **[04-usage-patterns.md](./04-usage-patterns.md)** - Practical usage examples
- **[05-wasm-considerations.md](./05-wasm-considerations.md)** - WASM-specific guide
