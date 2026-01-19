# Implementation Guide

This document explains how the library is built, covering design decisions, code structure, and guidance for extending the library.

---

## Architecture Overview

### Module Structure

```
foundation_nostd/src/primitives/
├── mod.rs                 # Re-exports, type aliases, WASM detection
│
├── poison.rs              # Error types: PoisonError, TryLockError
│
├── spin_mutex.rs          # SpinMutex with poisoning
├── raw_spin_mutex.rs      # RawSpinMutex without poisoning
│
├── spin_rwlock.rs         # SpinRwLock with poisoning
├── raw_spin_rwlock.rs     # RawSpinRwLock without poisoning
│
├── once.rs                # Once with poisoning
├── once_lock.rs           # OnceLock container
├── raw_once.rs            # RawOnce without poisoning
│
├── atomic_cell.rs         # AtomicCell<T>
├── atomic_option.rs       # AtomicOption<T>
├── atomic_lazy.rs         # AtomicLazy<T, F>
├── atomic_flag.rs         # AtomicFlag
│
├── barrier.rs             # SpinBarrier
├── spin_wait.rs           # SpinWait backoff helper
│
└── noop.rs                # No-op variants for single-threaded WASM
```

### Design Principles

1. **no_std first** - Everything works without std
2. **Zero dependencies** - Only `core::sync::atomic`
3. **API compatibility** - Match `std::sync` where possible
4. **WASM optimization** - Automatic no-op for single-threaded
5. **Poisoning optional** - Raw variants for embedded

---

## Core Building Blocks

### 1. UnsafeCell for Interior Mutability

All our types use `UnsafeCell` to enable mutation through shared references:

```rust
use core::cell::UnsafeCell;

pub struct SpinMutex<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,  // Interior mutability
}
```

`UnsafeCell` tells the compiler:
- This data may be mutated through `&self`
- Don't apply read-only optimizations

### 2. Atomic State Variables

State is stored in atomic integers for lock-free access:

```rust
use core::sync::atomic::AtomicU8;

// SpinMutex state bits
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;
const LOCKED_POISONED: u8 = 0b11;

pub struct SpinMutex<T> {
    state: AtomicU8,  // Atomic for thread-safe state changes
    // ...
}
```

### 3. Guard Pattern (RAII)

Guards ensure locks are always released:

```rust
pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Always release, even on panic
        self.mutex.unlock();
    }
}
```

---

## Implementing SpinMutex

### State Encoding

We use 2 bits for state:

| Bit 0 (LOCKED) | Bit 1 (POISONED) | Meaning |
|----------------|------------------|---------|
| 0 | 0 | Unlocked, clean |
| 1 | 0 | Locked, clean |
| 0 | 1 | Unlocked, poisoned |
| 1 | 1 | Locked, poisoned |

```rust
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;
```

### The lock() Method

```rust
impl<T> SpinMutex<T> {
    pub fn lock(&self) -> LockResult<SpinMutexGuard<'_, T>> {
        loop {
            // Read current state
            let state = self.state.load(Ordering::Relaxed);

            // Check if unlocked (bit 0 clear)
            if state & LOCKED == 0 {
                // Try to set locked bit
                let new_state = state | LOCKED;

                match self.state.compare_exchange_weak(
                    state,
                    new_state,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Successfully acquired
                        let guard = SpinMutexGuard { mutex: self };

                        // Check if poisoned
                        if state & POISONED != 0 {
                            return Err(PoisonError::new(guard));
                        } else {
                            return Ok(guard);
                        }
                    }
                    Err(_) => continue, // CAS failed, retry
                }
            }

            // Locked by someone else, spin
            core::hint::spin_loop();
        }
    }
}
```

### The Guard and Drop

```rust
pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<T> Deref for SpinMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: We hold the lock
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for SpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We hold the lock exclusively
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Check if panicking
        #[cfg(feature = "std")]
        let panicking = std::thread::panicking();
        #[cfg(not(feature = "std"))]
        let panicking = false;

        if panicking {
            // Set poison flag
            self.mutex.state.fetch_or(POISONED, Ordering::Release);
        }

        // Clear locked flag
        self.mutex.state.fetch_and(!LOCKED, Ordering::Release);
    }
}
```

### Send and Sync Bounds

```rust
// SAFETY: SpinMutex synchronizes access to T
unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

// Guard requires the mutex to be Sync
unsafe impl<T: Send + Sync> Sync for SpinMutexGuard<'_, T> {}
```

---

## Implementing RwLock

### State Encoding

RwLock needs to track:
- Number of readers (0 to many)
- Writer waiting (to block new readers)
- Writer active (exclusive access)

```rust
// Bits 0-29: Reader count (up to 1 billion)
// Bit 30: Writer waiting
// Bit 31: Writer active

const READER_MASK: u32 = (1 << 30) - 1;
const WRITER_WAITING: u32 = 1 << 30;
const WRITER_ACTIVE: u32 = 1 << 31;
```

### Writer-Preferring Policy

When a writer is waiting, new readers block:

```rust
impl<T> SpinRwLock<T> {
    pub fn read(&self) -> LockResult<SpinReadGuard<'_, T>> {
        loop {
            let state = self.state.load(Ordering::Relaxed);

            // Block if writer active or waiting
            if state & (WRITER_ACTIVE | WRITER_WAITING) != 0 {
                core::hint::spin_loop();
                continue;
            }

            // Try to increment reader count
            if self.state
                .compare_exchange_weak(
                    state,
                    state + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return Ok(SpinReadGuard { lock: self });
            }
        }
    }

    pub fn write(&self) -> LockResult<SpinWriteGuard<'_, T>> {
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
            } else {
                core::hint::spin_loop();
            }
        }

        // Wait for readers to drain and no active writer
        loop {
            let state = self.state.load(Ordering::Relaxed);
            let readers = state & READER_MASK;

            if readers == 0 && state & WRITER_ACTIVE == 0 {
                // Try to become active writer (clear waiting, set active)
                let new_state = (state & !WRITER_WAITING) | WRITER_ACTIVE;
                if self.state
                    .compare_exchange_weak(
                        state,
                        new_state,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return Ok(SpinWriteGuard { lock: self });
                }
            }

            core::hint::spin_loop();
        }
    }
}
```

---

## Implementing Once

### State Machine

```rust
const INCOMPLETE: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;
const POISONED: u8 = 3;
```

```
INCOMPLETE ─── call_once() ───► RUNNING ───► COMPLETE
                                   │
                                   └──(panic)──► POISONED
```

### call_once Implementation

```rust
impl Once {
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        // Fast path: already complete
        if self.state.load(Ordering::Acquire) == COMPLETE {
            return;
        }

        self.call_once_slow(f);
    }

    #[cold]
    fn call_once_slow<F: FnOnce()>(&self, f: F) {
        loop {
            match self.state.compare_exchange(
                INCOMPLETE,
                RUNNING,
                Ordering::Acquire,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // We're the initializer
                    #[cfg(feature = "std")]
                    {
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
                            Ok(()) => {
                                self.state.store(COMPLETE, Ordering::Release);
                            }
                            Err(e) => {
                                self.state.store(POISONED, Ordering::Release);
                                std::panic::resume_unwind(e);
                            }
                        }
                    }

                    #[cfg(not(feature = "std"))]
                    {
                        f();
                        self.state.store(COMPLETE, Ordering::Release);
                    }
                    return;
                }
                Err(COMPLETE) => return,
                Err(POISONED) => panic!("Once instance poisoned"),
                Err(RUNNING) => {
                    // Wait for another thread
                    while self.state.load(Ordering::Acquire) == RUNNING {
                        core::hint::spin_loop();
                    }
                }
                Err(_) => unreachable!(),
            }
        }
    }
}
```

---

## Implementing AtomicCell

### Type Constraint

`AtomicCell<T>` works for types that fit in a `usize`:

```rust
use core::mem;

pub struct AtomicCell<T: Copy> {
    value: AtomicUsize,
    _marker: PhantomData<T>,
}

impl<T: Copy> AtomicCell<T> {
    pub fn new(value: T) -> Self {
        assert!(mem::size_of::<T>() <= mem::size_of::<usize>());
        assert!(mem::align_of::<T>() <= mem::align_of::<usize>());

        Self {
            value: AtomicUsize::new(Self::encode(value)),
            _marker: PhantomData,
        }
    }
}
```

### Encoding and Decoding

```rust
impl<T: Copy> AtomicCell<T> {
    fn encode(value: T) -> usize {
        let mut result = 0usize;
        unsafe {
            core::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                &mut result as *mut usize as *mut u8,
                mem::size_of::<T>(),
            );
        }
        result
    }

    fn decode(value: usize) -> T {
        let mut result = mem::MaybeUninit::<T>::uninit();
        unsafe {
            core::ptr::copy_nonoverlapping(
                &value as *const usize as *const u8,
                result.as_mut_ptr() as *mut u8,
                mem::size_of::<T>(),
            );
            result.assume_init()
        }
    }
}
```

---

## WASM Optimization

### Conditional Compilation

```rust
// mod.rs

// Single-threaded WASM: use no-op locks
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type Mutex<T> = NoopMutex<T>;

// Multi-threaded WASM or native: use real locks
#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type Mutex<T> = SpinMutex<T>;
```

### NoopMutex Implementation

```rust
// noop.rs

use core::cell::UnsafeCell;

pub struct NoopMutex<T> {
    data: UnsafeCell<T>,
}

// SAFETY: Single-threaded WASM has no concurrent access
unsafe impl<T: Send> Send for NoopMutex<T> {}
unsafe impl<T: Send> Sync for NoopMutex<T> {}

impl<T> NoopMutex<T> {
    pub const fn new(value: T) -> Self {
        Self { data: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> NoopMutexGuard<'_, T> {
        NoopMutexGuard { mutex: self }
    }

    pub fn try_lock(&self) -> Option<NoopMutexGuard<'_, T>> {
        Some(self.lock())
    }

    pub fn is_poisoned(&self) -> bool {
        false // Never poisoned in no-op
    }
}

pub struct NoopMutexGuard<'a, T> {
    mutex: &'a NoopMutex<T>,
}

impl<T> Deref for NoopMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for NoopMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}
```

---

## SpinWait Backoff

### Exponential Backoff Pattern

```rust
pub struct SpinWait {
    count: u32,
}

impl SpinWait {
    pub const fn new() -> Self {
        Self { count: 0 }
    }

    /// Returns false when caller should yield/block
    pub fn spin(&mut self) -> bool {
        if self.count < 10 {
            // Exponential: 1, 2, 4, 8, 16, 32, 64, 128, 256, 512
            for _ in 0..(1 << self.count) {
                core::hint::spin_loop();
            }
            self.count += 1;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}
```

### Usage in Lock Acquisition

```rust
impl<T> SpinMutex<T> {
    pub fn lock(&self) -> LockResult<SpinMutexGuard<'_, T>> {
        let mut wait = SpinWait::new();

        loop {
            if let Some(result) = self.try_lock_inner() {
                return result;
            }

            if !wait.spin() {
                // Optional: Could yield to OS here if available
                wait.reset();
            }
        }
    }
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_basic() {
        let mutex = SpinMutex::new(42);
        assert_eq!(*mutex.lock().unwrap(), 42);

        *mutex.lock().unwrap() = 100;
        assert_eq!(*mutex.lock().unwrap(), 100);
    }

    #[test]
    fn test_mutex_try_lock() {
        let mutex = SpinMutex::new(42);

        let guard = mutex.try_lock().unwrap();
        assert!(mutex.try_lock().is_err()); // Already locked
        drop(guard);
        assert!(mutex.try_lock().is_ok());
    }

    #[test]
    fn test_poisoning() {
        let mutex = SpinMutex::new(42);

        let _ = std::panic::catch_unwind(|| {
            let _guard = mutex.lock().unwrap();
            panic!("test");
        });

        assert!(mutex.is_poisoned());
        assert!(mutex.lock().is_err());
    }
}
```

### Concurrency Tests

```rust
#[test]
fn test_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let mutex = Arc::new(SpinMutex::new(0u32));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let mutex = Arc::clone(&mutex);
            thread::spawn(move || {
                for _ in 0..1000 {
                    *mutex.lock().unwrap() += 1;
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(*mutex.lock().unwrap(), 4000);
}
```

---

## Extending the Library

### Adding a New Primitive

1. **Define the struct** with atomic state and UnsafeCell

```rust
pub struct MyPrimitive<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}
```

2. **Implement Send/Sync** with safety documentation

```rust
// SAFETY: Explain why this is safe
unsafe impl<T: Send> Send for MyPrimitive<T> {}
unsafe impl<T: Send> Sync for MyPrimitive<T> {}
```

3. **Create a guard type** if needed

```rust
pub struct MyGuard<'a, T> {
    primitive: &'a MyPrimitive<T>,
}

impl<T> Drop for MyGuard<'_, T> {
    fn drop(&mut self) {
        // Release logic
    }
}
```

4. **Add WASM no-op variant** if applicable

```rust
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub struct NoopMyPrimitive<T> { /* ... */ }
```

5. **Export from mod.rs** with type aliases

```rust
pub use my_primitive::{MyPrimitive, MyGuard};

#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type AutoMyPrimitive<T> = NoopMyPrimitive<T>;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type AutoMyPrimitive<T> = MyPrimitive<T>;
```

6. **Add tests** for correctness and concurrency

---

## Common Implementation Pitfalls

### 1. Forgetting Ordering

```rust
// BAD: Relaxed on lock acquisition
self.state.compare_exchange(
    UNLOCKED,
    LOCKED,
    Ordering::Relaxed,  // Wrong!
    Ordering::Relaxed,
);

// GOOD: Acquire on success
self.state.compare_exchange(
    UNLOCKED,
    LOCKED,
    Ordering::Acquire,  // Correct
    Ordering::Relaxed,
);
```

### 2. Incorrect Sync Bounds

```rust
// BAD: Only requires T: Send
unsafe impl<T: Send> Sync for SpinMutex<T> {}

// GOOD: Document why this is safe
/// SAFETY: SpinMutex provides synchronization for T.
/// The atomic state ensures only one thread accesses T at a time.
unsafe impl<T: Send> Sync for SpinMutex<T> {}
```

### 3. Missing spin_loop Hint

```rust
// BAD: Hot loop without hint
while locked.load(Ordering::Relaxed) {}

// GOOD: CPU-friendly spinning
while locked.load(Ordering::Relaxed) {
    core::hint::spin_loop();
}
```

---

## Performance Optimization

### 1. Test-and-Test-and-Set

```rust
// Instead of immediate CAS:
loop {
    // Read-only check first (cheap)
    while self.state.load(Ordering::Relaxed) == LOCKED {
        core::hint::spin_loop();
    }

    // Then try CAS (expensive)
    if self.state.compare_exchange_weak(
        UNLOCKED,
        LOCKED,
        Ordering::Acquire,
        Ordering::Relaxed,
    ).is_ok() {
        break;
    }
}
```

### 2. Use compare_exchange_weak in Loops

```rust
// In loops, weak is faster (may spuriously fail)
while self.state
    .compare_exchange_weak(...)  // Use weak
    .is_err()
{
    // ...
}
```

### 3. Inline Hot Paths

```rust
#[inline]
pub fn lock(&self) -> LockResult<SpinMutexGuard<'_, T>> {
    // Fast path inline
    if let Ok(guard) = self.try_lock_fast() {
        return Ok(guard);
    }
    self.lock_slow()  // Slow path outlined
}

#[cold]
fn lock_slow(&self) -> LockResult<SpinMutexGuard<'_, T>> {
    // Spinning logic
}
```

---

## Summary

| Component | Key Points |
|-----------|------------|
| **State** | Atomic integers with bit encoding |
| **Data** | UnsafeCell for interior mutability |
| **Guards** | RAII pattern for automatic release |
| **Poisoning** | Panic detection in Drop |
| **WASM** | Conditional no-op implementations |
| **Ordering** | Acquire on lock, Release on unlock |
| **Testing** | Unit + concurrency tests |

**Implementation Checklist:**
- [ ] Use atomic state variables
- [ ] Wrap data in UnsafeCell
- [ ] Implement guard with Drop
- [ ] Add Send/Sync with safety docs
- [ ] Use correct memory ordering
- [ ] Add WASM no-op variant
- [ ] Export from mod.rs
- [ ] Write comprehensive tests

---

*This concludes the fundamentals documentation. See individual source files for complete implementation details.*
