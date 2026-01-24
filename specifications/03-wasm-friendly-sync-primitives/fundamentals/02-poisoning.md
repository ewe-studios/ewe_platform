# Lock Poisoning Explained

## What is Poisoning?

**Poisoning** is a safety mechanism that marks a lock as potentially corrupted after a thread panics while holding it. The data inside may be in an inconsistent state.

```
Thread A                          Lock State
   │                                  │
   ├── lock() ────────────────────►  LOCKED
   │                                  │
   ├── modify data...                 │
   │   data.field1 = new_value;       │
   │   // PANIC HERE!                 │
   │   data.field2 = ...  // Never runs
   │                                  │
   └── (unwinding) ───────────────►  POISONED
                                      │
Thread B                              │
   │                                  │
   ├── lock() ────────────────────►  Returns Err(PoisonError)
   │
   └── Must decide: recover or propagate?
```

---

## Why Poisoning Exists

### The Problem: Partial Modifications

Consider a struct with invariants:

```rust
struct BankAccount {
    balance: i64,
    transaction_count: u32,
}

impl BankAccount {
    fn transfer(&mut self, amount: i64) {
        self.balance -= amount;       // Step 1: Deduct
        // What if we panic here?
        self.transaction_count += 1;  // Step 2: Record
    }
}
```

If a panic occurs between step 1 and step 2:
- Balance is modified
- Transaction count is not updated
- **Data is inconsistent!**

### The Solution: Mark as Poisoned

Poisoning tells subsequent lock users: "Something went wrong. The data may be inconsistent."

```rust
use std::sync::Mutex;

let account = Mutex::new(BankAccount { balance: 100, transaction_count: 0 });

// Thread A panics while holding lock
std::panic::catch_unwind(|| {
    let mut guard = account.lock().unwrap();
    guard.balance -= 50;
    panic!("Oops!"); // Lock becomes poisoned
});

// Thread B tries to acquire
match account.lock() {
    Ok(guard) => { /* Normal path */ },
    Err(poisoned) => {
        // Lock is poisoned - decide what to do
        println!("Warning: lock was poisoned");
        let guard = poisoned.into_inner(); // Recover anyway
    }
}
```

---

## API Design

### Types

```rust
/// Error returned when a lock is poisoned
pub struct PoisonError<T> {
    guard: T,
}

impl<T> PoisonError<T> {
    /// Get the guard anyway (you're sure data is consistent)
    pub fn into_inner(self) -> T {
        self.guard
    }

    /// Get a reference to the guard
    pub fn get_ref(&self) -> &T {
        &self.guard
    }

    /// Get a mutable reference to the guard
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

/// Error returned by try_lock
pub enum TryLockError<T> {
    /// Lock is poisoned
    Poisoned(PoisonError<T>),
    /// Lock is held by another thread
    WouldBlock,
}

/// Result type for lock operations
pub type LockResult<T> = Result<T, PoisonError<T>>;
pub type TryLockResult<T> = Result<T, TryLockError<T>>;
```

### SpinMutex with Poisoning

```rust
use core::sync::atomic::{AtomicU8, Ordering};
use core::cell::UnsafeCell;

// State encoding
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;
const LOCKED_POISONED: u8 = 0b11;

pub struct SpinMutex<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU8::new(UNLOCKED),
            data: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> LockResult<SpinMutexGuard<'_, T>> {
        // Spin to acquire (accept both clean and poisoned states)
        loop {
            let state = self.state.load(Ordering::Relaxed);

            // Can acquire if unlocked (bit 0 clear)
            if state & LOCKED == 0 {
                let new_state = state | LOCKED;
                if self.state
                    .compare_exchange_weak(state, new_state, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    let guard = SpinMutexGuard { mutex: self };

                    // Check if poisoned
                    if state & POISONED != 0 {
                        return Err(PoisonError { guard });
                    } else {
                        return Ok(guard);
                    }
                }
            }

            core::hint::spin_loop();
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.state.load(Ordering::Relaxed) & POISONED != 0
    }
}
```

### Guard with Panic Detection

```rust
pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Check if we're panicking (unwinding)
        if std::thread::panicking() {
            // Set poisoned flag, clear locked flag
            self.mutex.state.fetch_or(POISONED, Ordering::Release);
            self.mutex.state.fetch_and(!LOCKED, Ordering::Release);
        } else {
            // Normal unlock: just clear locked flag
            self.mutex.state.fetch_and(!LOCKED, Ordering::Release);
        }
    }
}
```

### no_std Panic Detection

In `no_std`, there's no `std::thread::panicking()`. Solutions:

```rust
// Option 1: Use a thread-local flag (requires alloc or OS support)
#[cfg(feature = "std")]
fn is_panicking() -> bool {
    std::thread::panicking()
}

// Option 2: Always assume not panicking (loses poisoning in no_std)
#[cfg(not(feature = "std"))]
fn is_panicking() -> bool {
    false
}

// Option 3: Use panic hook to set a flag (complex)
#[cfg(not(feature = "std"))]
static PANICKING: AtomicBool = AtomicBool::new(false);
```

For this library, we use Option 1/2: poisoning works with `std`, degrades gracefully in `no_std`.

---

## When to Use Poisoning

### Use Poisoning When:

1. **Data has invariants** that could be violated by partial updates
2. **Recovery is possible** - you can fix or reset the data
3. **Debugging is important** - poisoning helps identify issues
4. **std::sync API compatibility** is needed

### Skip Poisoning When:

1. **Panics abort** (`panic = "abort"` in Cargo.toml) - no recovery needed
2. **Embedded systems** - simpler is better
3. **Performance critical** - poisoning adds overhead
4. **Data is always consistent** - single atomic field updates

---

## Handling Poisoned Locks

### Strategy 1: Propagate (Default)

```rust
// Just unwrap - panic if poisoned
let guard = mutex.lock().unwrap();

// Or with context
let guard = mutex.lock().expect("mutex was poisoned");
```

Best for: Most applications where poisoning indicates a bug.

### Strategy 2: Recover

```rust
let guard = match mutex.lock() {
    Ok(guard) => guard,
    Err(poisoned) => {
        log::warn!("Recovering from poisoned mutex");
        let mut guard = poisoned.into_inner();

        // Reset to consistent state
        *guard = Default::default();
        guard
    }
};
```

Best for: Services that must keep running, data can be reset.

### Strategy 3: Check and Decide

```rust
if mutex.is_poisoned() {
    // Handle before even trying to lock
    return Err(Error::DataCorrupted);
}

// Proceed knowing it's not poisoned (race is possible)
let guard = mutex.lock().unwrap();
```

Best for: Early bailout in critical paths.

### Strategy 4: Clear Poison

```rust
let guard = match mutex.lock() {
    Ok(guard) => guard,
    Err(poisoned) => {
        let guard = poisoned.into_inner();
        // Validate and fix data...
        guard
    }
};

// After successful operation, poison is effectively cleared
// (Next panic will re-poison)
```

---

## Poisoning vs Raw (Non-Poisoning)

| Aspect | SpinMutex (Poisoning) | RawSpinMutex (No Poisoning) |
|--------|----------------------|---------------------------|
| **API** | `lock() -> LockResult<Guard>` | `lock() -> Guard` |
| **Overhead** | State bit + panic check | None |
| **Safety** | Detects inconsistent data | No detection |
| **Use case** | Production, recovery needed | Embedded, panic=abort |

### When to Choose Raw

```rust
// Cargo.toml
[profile.release]
panic = "abort"  # No unwinding, poisoning useless
```

```rust
// Code
use foundation_nostd::primitives::RawSpinMutex;

let mutex = RawSpinMutex::new(42);
let guard = mutex.lock();  // Simple, no Result
```

---

## Implementation Details

### State Encoding

We use 2 bits:

| Bit 0 | Bit 1 | State |
|-------|-------|-------|
| 0 | 0 | Unlocked, not poisoned |
| 1 | 0 | Locked, not poisoned |
| 0 | 1 | Unlocked, poisoned |
| 1 | 1 | Locked, poisoned |

```rust
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;
const LOCKED_POISONED: u8 = 0b11;
```

### Atomic Operations

```rust
// Acquire: Set LOCKED bit, preserve POISONED bit
let state = self.state.load(Ordering::Relaxed);
let new_state = state | LOCKED;
self.state.compare_exchange(state, new_state, Ordering::Acquire, Ordering::Relaxed);

// Release (normal): Clear LOCKED bit, preserve POISONED bit
self.state.fetch_and(!LOCKED, Ordering::Release);

// Release (panicking): Set POISONED, clear LOCKED
self.state.fetch_or(POISONED, Ordering::Release);
self.state.fetch_and(!LOCKED, Ordering::Release);
```

---

## RwLock Poisoning

RwLocks poison only on **write guard** panic, not read guard:

```rust
impl<T> Drop for SpinWriteGuard<'_, T> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // Poison the lock
            self.lock.poison.store(true, Ordering::Release);
        }
        // Release write lock
        self.lock.state.store(0, Ordering::Release);
    }
}

impl<T> Drop for SpinReadGuard<'_, T> {
    fn drop(&mut self) {
        // No poisoning - read guards don't modify data
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}
```

Why? Read guards only have `&T` access. They can't modify data, so no invariants can be violated.

---

## Once Poisoning

`Once` also supports poisoning for one-time initialization:

```rust
pub struct Once {
    state: AtomicU8,
}

const INCOMPLETE: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;
const POISONED: u8 = 3;

impl Once {
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        if self.state.load(Ordering::Acquire) == COMPLETE {
            return;
        }

        self.call_once_slow(f);
    }

    fn call_once_slow<F: FnOnce()>(&self, f: F) {
        // Try to become the initializer
        match self.state.compare_exchange(
            INCOMPLETE,
            RUNNING,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                // We're the initializer
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

                match result {
                    Ok(()) => {
                        self.state.store(COMPLETE, Ordering::Release);
                    }
                    Err(payload) => {
                        self.state.store(POISONED, Ordering::Release);
                        std::panic::resume_unwind(payload);
                    }
                }
            }
            Err(COMPLETE) => {
                // Already initialized
            }
            Err(POISONED) => {
                panic!("Once instance has been poisoned");
            }
            Err(RUNNING) => {
                // Another thread is initializing, spin-wait
                while self.state.load(Ordering::Acquire) == RUNNING {
                    core::hint::spin_loop();
                }
                // Check final state
                if self.state.load(Ordering::Acquire) == POISONED {
                    panic!("Once instance has been poisoned");
                }
            }
            Err(_) => unreachable!(),
        }
    }
}
```

### Recovering from Poisoned Once

```rust
impl Once {
    /// Like call_once, but also runs on poisoned state
    pub fn call_once_force<F: FnOnce(&OnceState)>(&self, f: F) {
        // Allow running even if poisoned
    }
}
```

---

## Performance Considerations

### Overhead of Poisoning

1. **State bit** - One extra bit in atomic (negligible)
2. **Panic check** - `thread::panicking()` call on every drop
3. **Result unwrap** - Extra branch at call site

### Benchmarks (Approximate)

| Operation | RawSpinMutex | SpinMutex (Poisoning) |
|-----------|--------------|----------------------|
| Lock/unlock | ~20ns | ~25ns (+25%) |
| Contended | ~100ns | ~110ns (+10%) |

The overhead is small but measurable. For hot paths, consider `RawSpinMutex`.

---

## Common Patterns

### 1. Panic-Safe Updates

```rust
fn safe_update<T: Clone>(mutex: &SpinMutex<T>, f: impl FnOnce(&mut T)) {
    let mut guard = mutex.lock().unwrap();

    // Clone before modifying
    let backup = guard.clone();

    // Try the update
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut guard))) {
        Ok(()) => {}, // Success
        Err(e) => {
            // Restore backup
            *guard = backup;
            // Don't poison - we recovered
            std::panic::resume_unwind(e);
        }
    }
}
```

### 2. Transaction-Style Updates

```rust
impl BankAccount {
    fn transfer(&mut self, amount: i64) -> Result<(), Error> {
        // Validate first
        if self.balance < amount {
            return Err(Error::InsufficientFunds);
        }

        // All-or-nothing update
        self.balance -= amount;
        self.transaction_count += 1;

        Ok(())
    }
}
```

---

## Summary

| Concept | Key Point |
|---------|-----------|
| **Poisoning** | Marks lock as corrupted after panic |
| **PoisonError** | Returned by `lock()` when poisoned |
| **into_inner()** | Recover guard from PoisonError |
| **When to use** | Data has invariants, recovery needed |
| **When to skip** | panic=abort, embedded, simple data |
| **Raw variants** | `RawSpinMutex`, `RawSpinRwLock` - no poisoning |

---

*Next: [03-atomics.md](./03-atomics.md) - Atomic operations deep dive*
