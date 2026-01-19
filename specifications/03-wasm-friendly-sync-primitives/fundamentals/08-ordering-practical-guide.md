# Practical Guide to Atomic Ordering

This document provides practical, actionable guidance for choosing the correct `Ordering` in Rust's atomic operations. It focuses on **how to think about ordering** rather than deep theory.

---

## The Golden Rule

**Ask yourself: "Is this atomic protecting access to other (non-atomic) data?"**

- **No** → Use `Relaxed`
- **Yes** → Keep reading

---

## Quick Reference Card

Print this and keep it handy:

```
┌─────────────────────────────────────────────────────────────────┐
│                    ORDERING QUICK REFERENCE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  RELAXED     Counter, stats, progress indicator                  │
│              (atomic doesn't protect other data)                 │
│                                                                  │
│  ACQUIRE     After this load, I will READ protected data         │
│              "I'm acquiring access to shared state"              │
│                                                                  │
│  RELEASE     Before this store, I WROTE protected data           │
│              "I'm releasing/publishing my changes"               │
│                                                                  │
│  ACQ_REL     Read-modify-write that does BOTH                    │
│              (e.g., fetch_add in ref counting, CAS in locks)     │
│                                                                  │
│  SEQ_CST     When you need TOTAL ordering across all threads     │
│              (rare - usually Acquire/Release is enough)          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## The Mental Model

Think of orderings as **barriers** that prevent reordering:

```
                    YOUR CODE
                        │
    ════════════════════╪════════════════════
                        │
         ACQUIRE ───────┼───────  (barrier AFTER)
                        │         Operations below cannot
                        │         move above the acquire
                        ▼
                   [your reads]
                        │
                        │
                   [your writes]
                        │
                        ▲
                        │         Operations above cannot
         RELEASE ───────┼───────  move below the release
                        │         (barrier BEFORE)
    ════════════════════╪════════════════════
                        │
```

**Acquire** = "Nothing after me moves before me"
**Release** = "Nothing before me moves after me"

---

## Scenario-Based Guide

### Scenario 1: Simple Counter (No Protection)

**Situation**: You have a counter for statistics, progress, or metrics. The counter value doesn't protect access to any other data.

```rust
use core::sync::atomic::{AtomicU64, Ordering};

static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

fn handle_request() {
    // ✅ CORRECT: Relaxed - just counting, not protecting anything
    REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

    // ... handle the request ...
}

fn get_stats() -> u64 {
    // ✅ CORRECT: Relaxed - just reading, no synchronization needed
    REQUEST_COUNT.load(Ordering::Relaxed)
}
```

**Why Relaxed?** The counter value doesn't gate access to any other memory. We just want atomic increments.

---

### Scenario 2: Flag to Signal "Data is Ready"

**Situation**: One thread writes data, then sets a flag. Another thread checks the flag, then reads the data.

```rust
use core::sync::atomic::{AtomicBool, Ordering};

static DATA: AtomicU64 = AtomicU64::new(0);
static READY: AtomicBool = AtomicBool::new(false);

// Producer thread
fn produce() {
    // Step 1: Write the data
    DATA.store(42, Ordering::Relaxed);  // Can be Relaxed

    // Step 2: Signal that data is ready
    // ✅ RELEASE: "I wrote data, now I'm publishing it"
    READY.store(true, Ordering::Release);
}

// Consumer thread
fn consume() -> u64 {
    // Step 1: Wait for ready signal
    // ✅ ACQUIRE: "I'm acquiring access to the published data"
    while !READY.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }

    // Step 2: Read the data (guaranteed to see producer's write)
    DATA.load(Ordering::Relaxed)  // Can be Relaxed now
}
```

**The Pattern**:
- Writer: `Relaxed` writes, then `Release` store on flag
- Reader: `Acquire` load on flag, then `Relaxed` reads

**Why?**
- `Release` on the flag ensures DATA write happens before flag write
- `Acquire` on the flag ensures DATA read happens after flag read
- Together they form a **synchronizes-with** relationship

---

### Scenario 3: Spin Lock

**Situation**: Implementing a lock where acquiring the lock means "I can now access protected data."

```rust
use core::sync::atomic::{AtomicBool, Ordering};

struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    fn lock(&self) {
        // Keep trying until we successfully set locked = true
        while self.locked
            .compare_exchange_weak(
                false,              // expected: unlocked
                true,               // desired: locked
                Ordering::Acquire,  // ✅ SUCCESS: Acquire - we're acquiring the lock
                Ordering::Relaxed,  // ✅ FAILURE: Relaxed - just retry, no sync needed
            )
            .is_err()
        {
            // Spin-wait optimization
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        // After this point, we can safely access protected data
    }

    fn unlock(&self) {
        // ✅ RELEASE: We're done with protected data, release the lock
        self.locked.store(false, Ordering::Release);
    }
}
```

**Why these orderings?**

| Operation | Ordering | Reason |
|-----------|----------|--------|
| CAS success | `Acquire` | After acquiring lock, we read protected data |
| CAS failure | `Relaxed` | Failed attempt, no data access, just retry |
| Unlock store | `Release` | Before releasing, we wrote protected data |

---

### Scenario 4: Reference Counting (Arc-style)

**Situation**: Multiple owners share data. Last owner deallocates.

```rust
use core::sync::atomic::{AtomicUsize, Ordering, fence};

struct MyArc<T> {
    ptr: *mut ArcInner<T>,
}

struct ArcInner<T> {
    count: AtomicUsize,
    data: T,
}

impl<T> Clone for MyArc<T> {
    fn clone(&self) -> Self {
        unsafe {
            // ✅ RELAXED: We already have access, just incrementing count
            (*self.ptr).count.fetch_add(1, Ordering::Relaxed);
        }
        MyArc { ptr: self.ptr }
    }
}

impl<T> Drop for MyArc<T> {
    fn drop(&mut self) {
        unsafe {
            // ✅ RELEASE: Our writes to data must be visible before potential dealloc
            if (*self.ptr).count.fetch_sub(1, Ordering::Release) == 1 {
                // We were the last owner

                // ✅ ACQUIRE fence: See all writes from other threads before dealloc
                fence(Ordering::Acquire);

                // Now safe to deallocate
                drop(Box::from_raw(self.ptr));
            }
        }
    }
}
```

**Why these orderings?**

| Operation | Ordering | Reason |
|-----------|----------|--------|
| Clone (increment) | `Relaxed` | Already have a reference, no sync needed |
| Drop (decrement) | `Release` | Our modifications must be visible before dealloc |
| Before dealloc | `Acquire` fence | Must see all other threads' modifications |

**The Acquire fence is crucial**: Without it, we might deallocate while another thread's writes to the data are still in flight.

---

### Scenario 5: One-Time Initialization

**Situation**: Initialize something exactly once, all threads see the result.

```rust
use core::sync::atomic::{AtomicU8, Ordering};

const UNINIT: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;

struct Once {
    state: AtomicU8,
}

impl Once {
    fn call_once<F: FnOnce()>(&self, f: F) {
        // Fast path: already complete
        // ✅ ACQUIRE: If complete, we need to see the initialized data
        if self.state.load(Ordering::Acquire) == COMPLETE {
            return;
        }

        // Try to become the initializer
        match self.state.compare_exchange(
            UNINIT,
            RUNNING,
            Ordering::Acquire,   // ✅ SUCCESS: We'll be writing, acquire for consistency
            Ordering::Acquire,   // ✅ FAILURE: Someone else is/was initializing
        ) {
            Ok(_) => {
                // We're the initializer
                f();

                // ✅ RELEASE: Publish the initialized data
                self.state.store(COMPLETE, Ordering::Release);
            }
            Err(COMPLETE) => {
                // Already done, nothing to do
            }
            Err(RUNNING) => {
                // Wait for the initializer
                while self.state.load(Ordering::Acquire) == RUNNING {
                    core::hint::spin_loop();
                }
            }
            _ => unreachable!(),
        }
    }
}
```

---

### Scenario 6: Double-Checked Locking

**Situation**: Fast path check, then lock, then check again.

```rust
use core::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;
use core::ptr;

struct Lazy<T> {
    ptr: AtomicPtr<T>,
    init_mutex: Mutex<()>,
}

impl<T> Lazy<T> {
    fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        // Fast path: check if already initialized
        // ✅ ACQUIRE: If set, we need to see the pointed-to data
        let p = self.ptr.load(Ordering::Acquire);
        if !p.is_null() {
            return unsafe { &*p };
        }

        // Slow path: lock and initialize
        let _guard = self.init_mutex.lock().unwrap();

        // Double-check after acquiring mutex
        // ✅ ACQUIRE: Another thread might have initialized while we waited
        let p = self.ptr.load(Ordering::Acquire);
        if !p.is_null() {
            return unsafe { &*p };
        }

        // Actually initialize
        let ptr = Box::into_raw(Box::new(f()));

        // ✅ RELEASE: Publish the pointer so other threads can see the data
        self.ptr.store(ptr, Ordering::Release);

        unsafe { &*ptr }
    }
}
```

---

## Common Mistakes and Fixes

### Mistake 1: Relaxed for Synchronization

```rust
// ❌ WRONG: Using Relaxed for a ready flag
static DATA: AtomicU64 = AtomicU64::new(0);
static READY: AtomicBool = AtomicBool::new(false);

fn producer() {
    DATA.store(42, Ordering::Relaxed);
    READY.store(true, Ordering::Relaxed);  // ❌ Should be Release!
}

fn consumer() {
    while !READY.load(Ordering::Relaxed) {}  // ❌ Should be Acquire!
    let value = DATA.load(Ordering::Relaxed);
    // value might be 0! Stores could be reordered!
}
```

```rust
// ✅ CORRECT
fn producer() {
    DATA.store(42, Ordering::Relaxed);
    READY.store(true, Ordering::Release);  // ✅ Release
}

fn consumer() {
    while !READY.load(Ordering::Acquire) {}  // ✅ Acquire
    let value = DATA.load(Ordering::Relaxed);
    // value is guaranteed to be 42
}
```

### Mistake 2: Acquire Without Matching Release

```rust
// ❌ WRONG: Acquire without Release is pointless
fn producer() {
    DATA.store(42, Ordering::Relaxed);
    READY.store(true, Ordering::Relaxed);  // ❌ No Release!
}

fn consumer() {
    while !READY.load(Ordering::Acquire) {}  // Acquire without matching Release
    // This Acquire doesn't synchronize with anything!
}
```

**Rule**: Acquire only synchronizes with a Release on the **same atomic variable**.

### Mistake 3: Release Without Matching Acquire

```rust
// ❌ WRONG: Release without Acquire is pointless
fn producer() {
    DATA.store(42, Ordering::Relaxed);
    READY.store(true, Ordering::Release);  // Release is here...
}

fn consumer() {
    while !READY.load(Ordering::Relaxed) {}  // ❌ No Acquire!
    // The producer's writes might not be visible!
}
```

### Mistake 4: SeqCst Everywhere "Just to Be Safe"

```rust
// ❌ INEFFICIENT: SeqCst for a simple counter
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn increment() {
    COUNTER.fetch_add(1, Ordering::SeqCst);  // ❌ Overkill!
}
```

```rust
// ✅ CORRECT: Relaxed for independent counter
fn increment() {
    COUNTER.fetch_add(1, Ordering::Relaxed);  // ✅ Faster, same correctness
}
```

**SeqCst is rarely needed**. Use it only when you need a total order visible to all threads.

### Mistake 5: Wrong Ordering for CAS Failure

```rust
// ❌ WRONG: Acquire on failure wastes cycles
self.state.compare_exchange(
    expected,
    desired,
    Ordering::Acquire,
    Ordering::Acquire,  // ❌ Failure path doesn't need Acquire
)
```

```rust
// ✅ CORRECT: Relaxed on failure (we're just going to retry)
self.state.compare_exchange(
    expected,
    desired,
    Ordering::Acquire,
    Ordering::Relaxed,  // ✅ Just retry, no sync needed
)
```

### Mistake 6: Forgetting the Acquire Fence Before Deallocation

```rust
// ❌ WRONG: Missing Acquire fence
if ref_count.fetch_sub(1, Ordering::Release) == 1 {
    // ❌ Might not see all writes from other threads!
    drop(Box::from_raw(ptr));
}
```

```rust
// ✅ CORRECT: Acquire fence ensures visibility
if ref_count.fetch_sub(1, Ordering::Release) == 1 {
    fence(Ordering::Acquire);  // ✅ See all writes before dealloc
    drop(Box::from_raw(ptr));
}
```

---

## Decision Flowchart

```
START: I need an atomic operation
         │
         ▼
┌─────────────────────────────────────┐
│ Does this atomic protect access     │
│ to other (non-atomic) data?         │
└─────────────────────────────────────┘
         │
    ┌────┴────┐
    │         │
   NO        YES
    │         │
    ▼         ▼
 RELAXED   ┌──────────────────────────┐
           │ What operation is this?  │
           └──────────────────────────┘
                      │
         ┌────────────┼────────────┐
         │            │            │
       LOAD        STORE     READ-MODIFY-WRITE
         │            │            │
         ▼            ▼            ▼
    ┌─────────┐  ┌─────────┐  ┌─────────────────┐
    │ Will I  │  │ Did I   │  │ Am I both       │
    │ READ    │  │ WRITE   │  │ reading AND     │
    │ protected│  │ protected│  │ writing         │
    │ data    │  │ data    │  │ protected data? │
    │ AFTER?  │  │ BEFORE? │  │                 │
    └─────────┘  └─────────┘  └─────────────────┘
         │            │            │
        YES          YES          YES
         │            │            │
         ▼            ▼            ▼
      ACQUIRE     RELEASE       AcqRel
```

---

## Ordering Pairs: What Synchronizes With What

For synchronization to work, you need **matching pairs**:

| Writer | Reader | Synchronizes? |
|--------|--------|---------------|
| `Release` store | `Acquire` load | ✅ Yes |
| `Release` store | `Relaxed` load | ❌ No |
| `Relaxed` store | `Acquire` load | ❌ No |
| `SeqCst` store | `SeqCst` load | ✅ Yes |
| `AcqRel` RMW | `Acquire` load | ✅ Yes |
| `Release` store | `AcqRel` RMW | ✅ Yes |

**Key insight**: The synchronization happens between a Release and an Acquire on the **same variable**. The Acquire "sees" everything that happened before the Release.

---

## Testing Your Understanding

### Quiz 1: What's wrong here?

```rust
static FLAG: AtomicBool = AtomicBool::new(false);
static VALUE: AtomicU32 = AtomicU32::new(0);

fn writer() {
    VALUE.store(42, Ordering::Release);
    FLAG.store(true, Ordering::Release);
}

fn reader() {
    while !FLAG.load(Ordering::Acquire) {}
    println!("{}", VALUE.load(Ordering::Acquire));
}
```

<details>
<summary>Answer</summary>

The VALUE.store should be `Relaxed`, not `Release`. The `Release` on FLAG is sufficient to publish all prior writes. Similarly, VALUE.load should be `Relaxed` after the `Acquire` on FLAG.

```rust
fn writer() {
    VALUE.store(42, Ordering::Relaxed);  // ✅
    FLAG.store(true, Ordering::Release);
}

fn reader() {
    while !FLAG.load(Ordering::Acquire) {}
    println!("{}", VALUE.load(Ordering::Relaxed));  // ✅
}
```

Not wrong per se (stronger ordering is always safe), but inefficient.
</details>

### Quiz 2: What ordering for this lock?

```rust
fn try_lock(&self) -> bool {
    self.locked.compare_exchange(
        false,
        true,
        ???,  // What ordering?
        ???,  // What ordering?
    ).is_ok()
}
```

<details>
<summary>Answer</summary>

```rust
self.locked.compare_exchange(
    false,
    true,
    Ordering::Acquire,  // Success: acquiring the lock
    Ordering::Relaxed,  // Failure: just retry
)
```
</details>

### Quiz 3: Is this counter correct?

```rust
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn increment() -> u64 {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn get() -> u64 {
    COUNTER.load(Ordering::Relaxed)
}
```

<details>
<summary>Answer</summary>

Yes! This is correct. The counter doesn't protect any other data, so `Relaxed` is appropriate. Each individual operation is atomic, which is all we need.
</details>

---

## Summary Table

| Situation | Load | Store | RMW |
|-----------|------|-------|-----|
| Independent counter/flag | `Relaxed` | `Relaxed` | `Relaxed` |
| Reader of protected data | `Acquire` | - | - |
| Writer of protected data | - | `Release` | - |
| Lock acquisition | - | - | `Acquire` (success) |
| Lock release | - | `Release` | - |
| CAS failure | - | - | `Relaxed` |
| Before deallocation | `Acquire` fence | - | - |
| Ref count increment | - | - | `Relaxed` |
| Ref count decrement | - | - | `Release` |
| Need total order | `SeqCst` | `SeqCst` | `SeqCst` |

---

## Final Checklist

Before committing atomic code, ask yourself:

- [ ] Does this atomic protect other data? If no → `Relaxed`
- [ ] Am I reading protected data after this? → `Acquire`
- [ ] Did I write protected data before this? → `Release`
- [ ] Is this a read-modify-write that does both? → `AcqRel`
- [ ] Does my `Acquire` have a matching `Release`?
- [ ] Does my `Release` have a matching `Acquire`?
- [ ] Am I using `SeqCst` unnecessarily? Consider weaker orderings
- [ ] For CAS failure, am I using `Relaxed`?
- [ ] Before deallocation, do I have an `Acquire` fence?

---

*This document supplements [04-memory-ordering.md](./04-memory-ordering.md) with practical guidance.*
