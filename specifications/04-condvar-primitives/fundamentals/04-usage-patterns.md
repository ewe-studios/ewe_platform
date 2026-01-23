# Usage Patterns - Practical CondVar Examples

## Table of Contents
- [Producer-Consumer Queue](#producer-consumer-queue)
- [Thread Pool Work Distribution](#thread-pool-work-distribution)
- [Barrier Implementation](#barrier-implementation)
- [Event Notification](#event-notification)
- [State Machine Synchronization](#state-machine-synchronization)
- [Timeout Patterns](#timeout-patterns)
- [Error Handling and Recovery](#error-handling-and-recovery)
- [Advanced Patterns](#advanced-patterns)

## Producer-Consumer Queue

### Problem

Multiple producers generate data, multiple consumers process it. Producers wait when queue is full, consumers wait when queue is empty.

### Single CondVar Solution (Suboptimal)

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::VecDeque;

struct Queue<T> {
    data: SpinMutex<VecDeque<T>>,
    condvar: CondVar,
    capacity: usize,
}

impl<T> Queue<T> {
    fn new(capacity: usize) -> Self {
        Self {
            data: SpinMutex::new(VecDeque::with_capacity(capacity)),
            condvar: CondVar::new(),
            capacity,
        }
    }

    fn push(&self, item: T) {
        let mut queue = self.data.lock().unwrap();

        // Wait for space
        while queue.len() >= self.capacity {
            queue = self.condvar.wait(queue).unwrap();
        }

        queue.push_back(item);
        drop(queue);

        // ❌ Wakes ALL threads (producers + consumers)
        self.condvar.notify_all();
    }

    fn pop(&self) -> T {
        let mut queue = self.data.lock().unwrap();

        // Wait for data
        while queue.is_empty() {
            queue = self.condvar.wait(queue).unwrap();
        }

        let item = queue.pop_front().unwrap();
        drop(queue);

        // ❌ Wakes ALL threads again
        self.condvar.notify_all();
        item
    }
}
```

**Problem**: `notify_all()` wakes producers AND consumers, causing **thundering herd**.

### Two CondVar Solution (Optimal)

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::VecDeque;

struct Queue<T> {
    data: SpinMutex<VecDeque<T>>,
    not_empty: CondVar,  // Signals consumers
    not_full: CondVar,   // Signals producers
    capacity: usize,
}

impl<T> Queue<T> {
    fn new(capacity: usize) -> Self {
        Self {
            data: SpinMutex::new(VecDeque::with_capacity(capacity)),
            not_empty: CondVar::new(),
            not_full: CondVar::new(),
            capacity,
        }
    }

    /// Producer: Add item (blocks if full)
    fn push(&self, item: T) {
        let mut queue = self.data.lock().unwrap();

        // Wait for space
        while queue.len() >= self.capacity {
            queue = self.not_full.wait(queue).unwrap();
        }

        queue.push_back(item);
        drop(queue);

        // ✅ Wake ONE consumer (only relevant threads)
        self.not_empty.notify_one();
    }

    /// Consumer: Remove item (blocks if empty)
    fn pop(&self) -> T {
        let mut queue = self.data.lock().unwrap();

        // Wait for data
        while queue.is_empty() {
            queue = self.not_empty.wait(queue).unwrap();
        }

        let item = queue.pop_front().unwrap();
        drop(queue);

        // ✅ Wake ONE producer
        self.not_full.notify_one();
        item
    }

    /// Try to pop without blocking
    fn try_pop(&self) -> Option<T> {
        let mut queue = self.data.lock().unwrap();

        if queue.is_empty() {
            None
        } else {
            let item = queue.pop_front();
            drop(queue);
            self.not_full.notify_one();
            item
        }
    }
}
```

**Benefits**:
- ✅ Only relevant threads wake up
- ✅ `notify_one()` is sufficient (exactly one thread can proceed)
- ✅ Eliminates thundering herd
- ✅ 2x-5x better throughput under contention

### WASM-Friendly Non-Poisoning Version

```rust
use foundation_nostd::primitives::{CondVarNonPoisoning, RawSpinMutex};
use alloc::collections::VecDeque;

struct QueueNonPoisoning<T> {
    data: RawSpinMutex<VecDeque<T>>,
    not_empty: CondVarNonPoisoning,
    not_full: CondVarNonPoisoning,
    capacity: usize,
}

impl<T> QueueNonPoisoning<T> {
    fn new(capacity: usize) -> Self {
        Self {
            data: RawSpinMutex::new(VecDeque::with_capacity(capacity)),
            not_empty: CondVarNonPoisoning::new(),
            not_full: CondVarNonPoisoning::new(),
            capacity,
        }
    }

    fn push(&self, item: T) {
        let mut queue = self.data.lock();

        // No Result wrapping needed
        while queue.len() >= self.capacity {
            queue = self.not_full.wait(queue);
        }

        queue.push_back(item);
        drop(queue);
        self.not_empty.notify_one();
    }

    fn pop(&self) -> T {
        let mut queue = self.data.lock();

        while queue.is_empty() {
            queue = self.not_empty.wait(queue);
        }

        let item = queue.pop_front().unwrap();
        drop(queue);
        self.not_full.notify_one();
        item
    }
}
```

**Benefits for WASM**:
- ✅ Simpler API (no `.unwrap()`)
- ✅ Smaller binary size
- ✅ Slightly faster (no poison checks)

## Thread Pool Work Distribution

### Problem

Multiple worker threads wait for work items. Work can be submitted from multiple threads. Shutdown signal must wake all workers.

### Implementation

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::VecDeque;

enum Message<T> {
    Work(T),
    Shutdown,
}

struct ThreadPool<T> {
    queue: SpinMutex<VecDeque<Message<T>>>,
    condvar: CondVar,
}

impl<T: Send + 'static> ThreadPool<T> {
    fn new(num_workers: usize) -> Self {
        let pool = Self {
            queue: SpinMutex::new(VecDeque::new()),
            condvar: CondVar::new(),
        };

        // Spawn worker threads
        for id in 0..num_workers {
            let pool_ref = &pool as *const Self;
            std::thread::spawn(move || {
                Self::worker_loop(unsafe { &*pool_ref }, id);
            });
        }

        pool
    }

    /// Submit work to the pool
    fn submit(&self, work: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(Message::Work(work));
        drop(queue);

        // Wake one worker
        self.condvar.notify_one();
    }

    /// Shutdown all workers
    fn shutdown(&self) {
        let mut queue = self.queue.lock().unwrap();

        // Send shutdown message to all workers
        for _ in 0..queue.len() {
            queue.push_back(Message::Shutdown);
        }
        drop(queue);

        // ✅ Use notify_all for shutdown (all workers must exit)
        self.condvar.notify_all();
    }

    fn worker_loop(pool: &Self, id: usize) {
        loop {
            let mut queue = pool.queue.lock().unwrap();

            // Wait for work
            while queue.is_empty() {
                queue = pool.condvar.wait(queue).unwrap();
            }

            let message = queue.pop_front().unwrap();
            drop(queue);

            match message {
                Message::Work(work) => {
                    // Process work
                    Self::process_work(work, id);
                }
                Message::Shutdown => {
                    println!("Worker {} shutting down", id);
                    break;
                }
            }
        }
    }

    fn process_work(work: T, worker_id: usize) {
        // Actual work processing
        println!("Worker {} processing work", worker_id);
    }
}
```

**Key points**:
- `notify_one()` for work submission (only one worker needed)
- `notify_all()` for shutdown (all workers must exit)
- Separate message types for work vs control

### With Timeout (Handle Idle Workers)

```rust
use core::time::Duration;

impl<T> ThreadPool<T> {
    fn worker_loop_with_timeout(pool: &Self, id: usize) {
        let idle_timeout = Duration::from_secs(60); // 1 minute

        loop {
            let mut queue = pool.queue.lock().unwrap();

            // Wait with timeout
            while queue.is_empty() {
                let (q, timeout_result) = pool.condvar
                    .wait_timeout(queue, idle_timeout)
                    .unwrap();

                queue = q;

                if timeout_result.timed_out() {
                    println!("Worker {} idle timeout, exiting", id);
                    return; // Exit idle worker
                }
            }

            let message = queue.pop_front().unwrap();
            drop(queue);

            match message {
                Message::Work(work) => Self::process_work(work, id),
                Message::Shutdown => break,
            }
        }
    }
}
```

**Benefits**:
- Workers exit after idle timeout (reduces thread count)
- Useful for dynamic thread pools

## Barrier Implementation

### Problem

N threads must all reach a synchronization point before any can proceed.

### Implementation with CondVar

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};

struct Barrier {
    mutex: SpinMutex<BarrierState>,
    condvar: CondVar,
    num_threads: usize,
}

struct BarrierState {
    count: usize,       // Threads that have arrived
    generation: usize,  // Barrier cycle number (for reuse)
}

impl Barrier {
    fn new(num_threads: usize) -> Self {
        Self {
            mutex: SpinMutex::new(BarrierState {
                count: 0,
                generation: 0,
            }),
            condvar: CondVar::new(),
            num_threads,
        }
    }

    /// Wait for all threads to arrive
    fn wait(&self) {
        let mut state = self.mutex.lock().unwrap();
        let generation = state.generation;

        state.count += 1;

        if state.count == self.num_threads {
            // Last thread arrives
            state.count = 0;
            state.generation += 1;
            drop(state);

            // ✅ Wake all waiting threads
            self.condvar.notify_all();
        } else {
            // Not last thread, wait
            while state.generation == generation {
                state = self.condvar.wait(state).unwrap();
            }
        }
    }
}

// Usage example
fn example_barrier() {
    let barrier = std::sync::Arc::new(Barrier::new(5));

    for i in 0..5 {
        let barrier_clone = barrier.clone();
        std::thread::spawn(move || {
            println!("Thread {} starting work", i);
            // Do some work...
            std::thread::sleep(Duration::from_millis(i * 100));

            println!("Thread {} reached barrier", i);
            barrier_clone.wait();

            println!("Thread {} continuing after barrier", i);
        });
    }
}
```

**Key points**:
- Generation counter allows barrier reuse
- Last thread wakes all others with `notify_all()`
- Handles spurious wakeups via `while` loop checking generation

### Reusable Barrier Pattern

```rust
struct ReusableBarrier {
    mutex: SpinMutex<(usize, usize)>, // (count, generation)
    condvar: CondVar,
    num_threads: usize,
}

impl ReusableBarrier {
    fn wait(&self) {
        let mut state = self.mutex.lock().unwrap();
        let local_gen = state.1;
        state.0 += 1;

        if state.0 == self.num_threads {
            // Reset for next cycle
            state.0 = 0;
            state.1 += 1;
            drop(state);
            self.condvar.notify_all();
        } else {
            // Wait until generation changes
            while state.1 == local_gen {
                state = self.condvar.wait(state).unwrap();
            }
        }
    }
}
```

## Event Notification

### Problem

Wait for specific events to occur, with multiple event types.

### Multi-Event Pattern

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};

bitflags::bitflags! {
    struct EventFlags: u32 {
        const READY    = 0b0001;
        const ERROR    = 0b0010;
        const COMPLETE = 0b0100;
        const CANCELED = 0b1000;
    }
}

struct EventWaiter {
    flags: SpinMutex<EventFlags>,
    condvar: CondVar,
}

impl EventWaiter {
    fn new() -> Self {
        Self {
            flags: SpinMutex::new(EventFlags::empty()),
            condvar: CondVar::new(),
        }
    }

    /// Wait for specific event(s)
    fn wait_for(&self, events: EventFlags) {
        let mut flags = self.flags.lock().unwrap();

        // Wait until any of the desired events occur
        while !flags.intersects(events) {
            flags = self.condvar.wait(flags).unwrap();
        }

        // Clear the flags we waited for
        flags.remove(events);
    }

    /// Wait for ALL specified events
    fn wait_for_all(&self, events: EventFlags) {
        let mut flags = self.flags.lock().unwrap();

        // Wait until ALL desired events have occurred
        while !flags.contains(events) {
            flags = self.condvar.wait(flags).unwrap();
        }

        flags.remove(events);
    }

    /// Set event flag and notify
    fn signal(&self, event: EventFlags) {
        let mut flags = self.flags.lock().unwrap();
        flags.insert(event);
        drop(flags);

        // Wake all waiters (they'll check which event occurred)
        self.condvar.notify_all();
    }

    /// Check if event is set (non-blocking)
    fn is_set(&self, event: EventFlags) -> bool {
        let flags = self.flags.lock().unwrap();
        flags.contains(event)
    }
}

// Usage
fn example_events() {
    let waiter = std::sync::Arc::new(EventWaiter::new());

    // Thread 1: Wait for READY or ERROR
    let waiter1 = waiter.clone();
    std::thread::spawn(move || {
        println!("Waiting for READY or ERROR...");
        waiter1.wait_for(EventFlags::READY | EventFlags::ERROR);
        println!("Event occurred!");
    });

    // Thread 2: Signal READY
    std::thread::sleep(Duration::from_secs(1));
    waiter.signal(EventFlags::READY);
}
```

**Benefits**:
- Multiple threads can wait for different event combinations
- Flexible event handling (any vs all)
- Bit flags are compact and efficient

## State Machine Synchronization

### Problem

Coordinate state transitions across multiple threads, ensuring state changes are observed correctly.

### Implementation

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Starting,
    Running,
    Stopping,
    Stopped,
}

struct StateMachine {
    state: SpinMutex<State>,
    condvar: CondVar,
}

impl StateMachine {
    fn new() -> Self {
        Self {
            state: SpinMutex::new(State::Idle),
            condvar: CondVar::new(),
        }
    }

    /// Wait until state matches predicate
    fn wait_for_state<F>(&self, predicate: F)
    where
        F: Fn(State) -> bool,
    {
        let mut state = self.state.lock().unwrap();

        while !predicate(*state) {
            state = self.condvar.wait(state).unwrap();
        }
    }

    /// Transition to new state and notify waiters
    fn transition_to(&self, new_state: State) {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
        drop(state);

        // Wake all threads waiting for state changes
        self.condvar.notify_all();
    }

    /// Get current state
    fn get_state(&self) -> State {
        *self.state.lock().unwrap()
    }
}

// Usage example
fn example_state_machine() {
    let sm = std::sync::Arc::new(StateMachine::new());

    // Worker thread
    let sm_worker = sm.clone();
    std::thread::spawn(move || {
        // Wait for Running state
        sm_worker.wait_for_state(|s| s == State::Running);
        println!("Started working!");

        // Do work...
        std::thread::sleep(Duration::from_secs(2));

        sm_worker.transition_to(State::Stopped);
    });

    // Controller thread
    sm.transition_to(State::Starting);
    std::thread::sleep(Duration::from_millis(500));
    sm.transition_to(State::Running);

    // Wait for completion
    sm.wait_for_state(|s| s == State::Stopped);
    println!("Work complete!");
}
```

## Timeout Patterns

### Timed Wait with Retry

```rust
use core::time::Duration;
use foundation_nostd::primitives::{CondVar, SpinMutex};

struct TimedOperation {
    ready: SpinMutex<bool>,
    condvar: CondVar,
}

impl TimedOperation {
    fn wait_with_retry(&self, timeout: Duration, max_retries: usize) -> bool {
        for attempt in 0..max_retries {
            let mut ready = self.ready.lock().unwrap();

            // Try waiting with timeout
            let (r, timeout_result) = self.condvar
                .wait_timeout_while(
                    ready,
                    timeout,
                    |ready| !*ready,
                )
                .unwrap();

            ready = r;

            if *ready {
                return true; // Success
            }

            if timeout_result.timed_out() {
                println!("Attempt {} timed out", attempt + 1);
                // Could implement exponential backoff here
                drop(ready);
                continue;
            }
        }

        false // All retries exhausted
    }
}
```

### Deadline-Based Waiting

```rust
use std::time::Instant;

impl TimedOperation {
    fn wait_until(&self, deadline: Instant) -> bool {
        let mut ready = self.ready.lock().unwrap();

        loop {
            if *ready {
                return true;
            }

            let now = Instant::now();
            if now >= deadline {
                return false; // Deadline exceeded
            }

            let remaining = deadline - now;
            let (r, timeout_result) = self.condvar
                .wait_timeout(ready, remaining)
                .unwrap();

            ready = r;

            if timeout_result.timed_out() {
                return false;
            }
        }
    }
}
```

## Error Handling and Recovery

### Poison Error Recovery

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex, PoisonError};

struct ResilientQueue<T> {
    data: SpinMutex<Vec<T>>,
    condvar: CondVar,
}

impl<T> ResilientQueue<T> {
    fn pop(&self) -> Result<T, String> {
        let result = self.data.lock();

        // Handle poison error
        let mut queue = match result {
            Ok(q) => q,
            Err(poisoned) => {
                eprintln!("Queue was poisoned, recovering...");
                // Recover by extracting the guard
                poisoned.into_inner()
            }
        };

        // Wait for data with poison handling
        while queue.is_empty() {
            let wait_result = self.condvar.wait(queue);

            queue = match wait_result {
                Ok(q) => q,
                Err(poisoned) => {
                    eprintln!("CondVar was poisoned during wait");
                    poisoned.into_inner()
                }
            };
        }

        Ok(queue.pop().unwrap())
    }

    fn push_with_panic_safety(&self, item: T) {
        // Even if we panic here, lock will be poisoned
        let mut queue = self.data.lock().unwrap();

        // Potentially panicking operation
        if queue.len() > 1000 {
            panic!("Queue overflow!");
        }

        queue.push(item);
        drop(queue);
        self.condvar.notify_one();
    }
}
```

### Graceful Degradation

```rust
impl<T> ResilientQueue<T> {
    fn try_pop_with_fallback(&self) -> Option<T> {
        // Try to lock without blocking
        let result = self.data.try_lock();

        let mut queue = match result {
            Ok(q) => q,
            Err(_) => {
                // Lock contended, use fallback strategy
                return None;
            }
        };

        // Check if poisoned
        if self.condvar.is_poisoned() {
            eprintln!("CondVar is poisoned, clearing state");
            // Could reset state here
        }

        queue.pop()
    }
}
```

## Advanced Patterns

### Priority Queue with Multiple CondVars

```rust
use foundation_nostd::primitives::{CondVar, SpinMutex};
use std::collections::BinaryHeap;

struct PriorityQueue<T: Ord> {
    high: SpinMutex<Vec<T>>,
    low: SpinMutex<Vec<T>>,
    high_ready: CondVar,
    low_ready: CondVar,
}

impl<T: Ord> PriorityQueue<T> {
    fn push_high(&self, item: T) {
        let mut queue = self.high.lock().unwrap();
        queue.push(item);
        drop(queue);
        self.high_ready.notify_one();
    }

    fn push_low(&self, item: T) {
        let mut queue = self.low.lock().unwrap();
        queue.push(item);
        drop(queue);
        self.low_ready.notify_one();
    }

    fn pop(&self) -> T {
        // Try high priority first
        let mut high = self.high.lock().unwrap();

        if !high.is_empty() {
            return high.pop().unwrap();
        }

        drop(high);

        // Wait on low priority
        let mut low = self.low.lock().unwrap();
        while low.is_empty() {
            low = self.low_ready.wait(low).unwrap();
        }

        low.pop().unwrap()
    }
}
```

### Broadcast Channel

```rust
struct BroadcastChannel<T: Clone> {
    subscribers: SpinMutex<Vec<usize>>,
    message: SpinMutex<Option<T>>,
    condvar: CondVar,
}

impl<T: Clone> BroadcastChannel<T> {
    fn broadcast(&self, msg: T) {
        let mut message = self.message.lock().unwrap();
        *message = Some(msg);
        drop(message);

        // Wake all subscribers
        self.condvar.notify_all();
    }

    fn receive(&self) -> T {
        let mut message = self.message.lock().unwrap();

        while message.is_none() {
            message = self.condvar.wait(message).unwrap();
        }

        message.clone().unwrap()
    }
}
```

## Summary

### Pattern Selection Guide

| Pattern | Best For | CondVar Usage |
|---------|----------|---------------|
| Producer-Consumer | Queue-like workloads | Two CondVars (not_empty, not_full) |
| Thread Pool | Task distribution | One CondVar + control messages |
| Barrier | N-thread synchronization | One CondVar + generation counter |
| Event Notification | Multiple event types | One CondVar + bit flags |
| State Machine | State transitions | One CondVar + state enum |
| Timeout | Deadline-based waits | wait_timeout variants |
| Priority Queue | Prioritized work | Multiple CondVars per priority |

### Common Mistakes to Avoid

1. ❌ **Using `if` instead of `while`**
   ```rust
   // WRONG
   if !ready { condvar.wait(guard)?; }

   // CORRECT
   while !ready { condvar.wait(guard)?; }
   ```

2. ❌ **Forgetting to drop guard before notify**
   ```rust
   // WRONG (holds lock during notify)
   queue.push(item);
   condvar.notify_one();
   drop(queue);

   // CORRECT (releases lock first)
   queue.push(item);
   drop(queue);
   condvar.notify_one();
   ```

3. ❌ **Using single CondVar when multiple are better**
   - Use separate CondVars for different conditions
   - Avoids thundering herd

4. ❌ **Not handling poison errors**
   - Use `.unwrap()` only in tests or when panics are acceptable
   - Implement recovery logic for production code

## Next Steps

- **[05-wasm-considerations.md](./05-wasm-considerations.md)** - WASM-specific patterns and optimizations
- **[06-std-compatibility.md](./06-std-compatibility.md)** - std::sync::Condvar migration guide
