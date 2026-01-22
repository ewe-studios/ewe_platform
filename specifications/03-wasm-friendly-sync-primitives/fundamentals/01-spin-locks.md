# Spin Locks Deep Dive

## What is a Spin Lock?

A **spin lock** is a mutual exclusion primitive where the thread trying to acquire the lock repeatedly checks ("spins") until it becomes available. Unlike OS-based locks that put threads to sleep, spin locks keep the CPU busy.

```
Thread A holds lock    Thread B wants lock
      │                      │
      │                      ├── Check: locked? YES
      │                      ├── Spin... check again
      │                      ├── Check: locked? YES
      │                      ├── Spin... check again
      │ releases lock ───────┤
                             ├── Check: locked? NO
                             └── Acquire lock ✓
```

---

## Why Spin Locks?

### Advantages

1. **No OS dependency** - Works in `no_std`, embedded, WASM
2. **No context switch** - For very short critical sections, spinning is faster than sleeping
3. **Predictable latency** - No scheduler involvement
4. **Simple implementation** - Just atomic operations

### Disadvantages

1. **Wastes CPU** - Spinning consumes power and cycles
2. **Priority inversion** - High-priority thread can spin waiting for low-priority thread
3. **No fairness** - Threads may acquire in arbitrary order
4. **Bad for long waits** - Should only protect very short critical sections

---

## How Spin Locks Work

### Basic Algorithm

```rust
use core::sync::atomic::{AtomicBool, Ordering};

pub struct BasicSpinLock {
    locked: AtomicBool,
}

impl BasicSpinLock {
    pub const fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    pub fn lock(&self) {
        // Spin until we successfully set locked = true
        while self.locked
            .compare_exchange_weak(
                false,              // Expected: unlocked
                true,               // Desired: locked
                Ordering::Acquire,  // On success: acquire semantics
                Ordering::Relaxed,  // On failure: just retry
            )
            .is_err()
        {
            // Hint to CPU that we're spinning
            core::hint::spin_loop();
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}
```

### Key Operations

| Operation | Purpose | Memory Ordering |
|-----------|---------|-----------------|
| `compare_exchange` | Atomically check-and-set | Acquire on success |
| `store(false)` | Release the lock | Release |
| `spin_loop()` | CPU hint for power efficiency | N/A |

---

## The Compare-Exchange Pattern

The core of any spin lock is the atomic compare-and-exchange (CAS):

```rust
// Pseudocode for compare_exchange
fn compare_exchange(current: &AtomicBool, expected: bool, new: bool) -> Result<bool, bool> {
    // Atomically:
    if *current == expected {
        *current = new;
        Ok(expected)  // Success: return old value
    } else {
        Err(*current) // Failure: return actual value
    }
}
```

### Why `compare_exchange_weak`?

Rust provides two variants:

| Variant | Spurious Failures | Use Case |
|---------|-------------------|----------|
| `compare_exchange` | No | Single attempt |
| `compare_exchange_weak` | Yes | Loop (spin lock) |

`_weak` may spuriously fail even when the expected value matches, but it's faster on some architectures (ARM, RISC-V). Since we're looping anyway, spurious failures are fine.

---

## Spin Lock with Data

The basic lock above doesn't protect any data. Here's how we add data protection:

```rust
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU8, Ordering};

const UNLOCKED: u8 = 0;
const LOCKED: u8 = 1;

pub struct RawSpinMutex<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

// SAFETY: We synchronize access via the atomic state
unsafe impl<T: Send> Send for RawSpinMutex<T> {}
unsafe impl<T: Send> Sync for RawSpinMutex<T> {}

impl<T> RawSpinMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU8::new(UNLOCKED),
            data: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> RawSpinMutexGuard<'_, T> {
        while self.state
            .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Spin-wait optimization: check without CAS first
            while self.state.load(Ordering::Relaxed) == LOCKED {
                core::hint::spin_loop();
            }
        }
        RawSpinMutexGuard { mutex: self }
    }
}

pub struct RawSpinMutexGuard<'a, T> {
    mutex: &'a RawSpinMutex<T>,
}

impl<T> Deref for RawSpinMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: We hold the lock
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for RawSpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We hold the lock exclusively
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for RawSpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.state.store(UNLOCKED, Ordering::Release);
    }
}
```

### The Guard Pattern

Why return a guard instead of just locking?

```rust
// Without guard (dangerous!):
mutex.lock();
do_something();
mutex.unlock(); // What if we forget? Or panic?

// With guard (safe):
{
    let guard = mutex.lock();
    do_something();
} // Automatically unlocked when guard is dropped
```

The guard ensures:
1. **Automatic unlock** on scope exit
2. **Unlock on panic** (Drop is called during unwinding)
3. **Lifetime tied to lock** - can't use data without holding lock

---

## Spin-Wait Optimization

Naive spinning hammers the CPU cache. Optimized spinning reads before attempting CAS:

```rust
pub fn lock(&self) {
    loop {
        // Fast path: try to acquire
        if self.state
            .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        // Slow path: spin-wait with read-only loads
        while self.state.load(Ordering::Relaxed) == LOCKED {
            core::hint::spin_loop();
        }
    }
}
```

Why this helps:
- `compare_exchange` invalidates cache lines on other cores
- `load` only reads, keeping cache line in shared state
- Reduces bus traffic under contention

---

## Bounded Spinning

Unbounded spinning can hang forever if something goes wrong. Always prefer bounded:

```rust
pub fn try_lock_with_spin_limit(&self, max_spins: u32) -> Option<RawSpinMutexGuard<'_, T>> {
    let mut spin_count = 0;

    loop {
        if self.state
            .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return Some(RawSpinMutexGuard { mutex: self });
        }

        spin_count += 1;
        if spin_count >= max_spins {
            return None; // Give up
        }

        core::hint::spin_loop();
    }
}
```

---

## Spin Lock vs OS Mutex

| Aspect | Spin Lock | OS Mutex |
|--------|-----------|----------|
| **Wait behavior** | Busy-wait (CPU spinning) | Sleep (yields CPU) |
| **Context switch** | No | Yes (expensive) |
| **Power usage** | High when contended | Low when waiting |
| **Best for** | Very short critical sections | Longer operations |
| **OS required** | No (`no_std` compatible) | Yes |
| **Fairness** | None (LIFO-ish) | Usually fair (FIFO) |

### When to Use Spin Locks

✅ **Good for:**
- Critical sections < 1μs
- `no_std` / embedded / WASM
- Lock rarely contended
- Real-time systems (predictable latency)

❌ **Bad for:**
- Critical sections > 10μs
- High contention
- Battery-powered devices
- Holding across I/O operations

---

## Read-Write Spin Lock

For read-heavy workloads, allow multiple concurrent readers:

```rust
use core::sync::atomic::{AtomicU32, Ordering};

// State encoding:
// Bits 0-29: Reader count (max ~1 billion readers)
// Bit 30: Writer waiting
// Bit 31: Writer active

const READER_MASK: u32 = (1 << 30) - 1;
const WRITER_WAITING: u32 = 1 << 30;
const WRITER_ACTIVE: u32 = 1 << 31;

pub struct RawSpinRwLock<T> {
    state: AtomicU32,
    data: UnsafeCell<T>,
}

impl<T> RawSpinRwLock<T> {
    pub fn read(&self) -> RawReadGuard<'_, T> {
        loop {
            let state = self.state.load(Ordering::Relaxed);

            // Can't read if writer active or waiting
            if state & (WRITER_ACTIVE | WRITER_WAITING) != 0 {
                core::hint::spin_loop();
                continue;
            }

            // Try to increment reader count
            if self.state
                .compare_exchange_weak(
                    state,
                    state + 1, // Add one reader
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return RawReadGuard { lock: self };
            }
        }
    }

    pub fn write(&self) -> RawWriteGuard<'_, T> {
        // First, set writer-waiting flag
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & WRITER_WAITING == 0 {
                if self.state
                    .compare_exchange_weak(
                        state,
                        state | WRITER_WAITING,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    break;
                }
            }
        }

        // Wait for all readers to finish
        loop {
            let state = self.state.load(Ordering::Relaxed);

            // Check if no readers and no active writer
            if state & (READER_MASK | WRITER_ACTIVE) == WRITER_WAITING {
                // Try to become active writer
                if self.state
                    .compare_exchange_weak(
                        state,
                        WRITER_ACTIVE,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return RawWriteGuard { lock: self };
                }
            }

            core::hint::spin_loop();
        }
    }
}
```

### Writer-Preferring Policy

The `WRITER_WAITING` flag implements writer preference:
- When a writer is waiting, new readers block
- Prevents writer starvation in read-heavy workloads
- Trade-off: readers may wait longer

**Note:** This library also provides `ReaderSpinRwLock` which uses a **reader-preferring** policy instead. In reader-preferring mode, there is no `WRITER_WAITING` flag, and readers can always acquire the lock even when writers are waiting. See [10-rwlock-policies.md](./10-rwlock-policies.md) for a detailed comparison and when to use each policy.

---

## WASM Considerations

### Single-Threaded WASM

Standard WASM is single-threaded. Spin locks become no-ops:

```rust
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub struct NoopMutex<T> {
    data: UnsafeCell<T>,
}

#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
impl<T> NoopMutex<T> {
    pub fn lock(&self) -> NoopMutexGuard<'_, T> {
        NoopMutexGuard { mutex: self }
    }
}
```

### Multi-Threaded WASM

With `target-feature=+atomics`, real spin locks are needed:

```rust
#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
pub type Mutex<T> = RawSpinMutex<T>;
```

---

## Common Pitfalls

### 1. Holding Lock Too Long

```rust
// BAD: I/O inside lock
let guard = mutex.lock();
file.write_all(&data)?;  // Slow I/O while holding lock!
drop(guard);

// GOOD: Minimize critical section
let data = {
    let guard = mutex.lock();
    guard.clone()
};
file.write_all(&data)?;  // I/O outside lock
```

### 2. Nested Locks (Deadlock)

```rust
// DEADLOCK!
let guard1 = mutex.lock();
let guard2 = mutex.lock(); // Same mutex - hangs forever!
```

### 3. Forgetting spin_loop() Hint

```rust
// BAD: Hammers CPU
while locked.load(Ordering::Relaxed) {}

// GOOD: CPU power optimization
while locked.load(Ordering::Relaxed) {
    core::hint::spin_loop();
}
```

---

## Performance Tuning

### Exponential Backoff

For high-contention scenarios, back off exponentially:

```rust
pub struct SpinWait {
    count: u32,
}

impl SpinWait {
    pub fn new() -> Self {
        Self { count: 0 }
    }

    pub fn spin(&mut self) -> bool {
        if self.count < 10 {
            // Exponential backoff: 1, 2, 4, 8, 16, ...
            for _ in 0..(1 << self.count) {
                core::hint::spin_loop();
            }
            self.count += 1;
            true // Keep spinning
        } else {
            false // Give up / yield
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}
```

### Cache Line Padding

Avoid false sharing by padding to cache line size:

```rust
#[repr(align(64))] // Typical cache line size
pub struct CacheAligned<T>(pub T);

pub struct PaddedSpinLock {
    state: CacheAligned<AtomicU8>,
}
```

---

## Summary

| Concept | Key Point |
|---------|-----------|
| **Spin lock** | Busy-wait mutex using atomic CAS |
| **Guard pattern** | RAII unlock on drop |
| **Bounded spinning** | Always limit spin iterations |
| **Writer-preferring** | `WRITER_WAITING` flag blocks new readers |
| **Spin-wait optimization** | Read-only loads between CAS attempts |
| **WASM** | No-op for single-threaded, real for multi-threaded |

---

*Next: [02-poisoning.md](./02-poisoning.md) - Lock poisoning explained*
