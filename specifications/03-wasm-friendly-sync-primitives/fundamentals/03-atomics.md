# Atomic Operations Deep Dive

## What are Atomics?

**Atomic operations** are CPU-level operations that complete in a single, indivisible step. No other thread can observe an atomic operation "in progress" - it either hasn't happened or has completed.

```
Non-atomic increment:        Atomic increment:
   Thread A    Thread B         Thread A    Thread B
   ─────────   ─────────        ─────────   ─────────
   read x=5                     atomic_add(x, 1)
               read x=5                     (blocked)
   x=5+1=6                      x is now 6
               x=5+1=6                      atomic_add(x, 1)
   write x=6                    x is now 7
               write x=6        ✓ Correct!
   ─────────────────────
   x=6 (WRONG! Lost update)
```

---

## Rust's Atomic Types

The `core::sync::atomic` module provides atomic types:

| Type | Size | Notes |
|------|------|-------|
| `AtomicBool` | 1 byte | Atomic boolean |
| `AtomicI8`, `AtomicU8` | 1 byte | Atomic 8-bit integers |
| `AtomicI16`, `AtomicU16` | 2 bytes | Atomic 16-bit integers |
| `AtomicI32`, `AtomicU32` | 4 bytes | Atomic 32-bit integers |
| `AtomicI64`, `AtomicU64` | 8 bytes | Atomic 64-bit integers |
| `AtomicIsize`, `AtomicUsize` | Pointer size | Atomic pointer-sized integers |
| `AtomicPtr<T>` | Pointer size | Atomic raw pointer |

### Platform Support

Not all platforms support all atomic widths:

```rust
// Check at compile time
#[cfg(target_has_atomic = "64")]
use core::sync::atomic::AtomicU64;

// Or use conditional compilation
#[cfg(target_has_atomic = "64")]
type Counter = AtomicU64;

#[cfg(not(target_has_atomic = "64"))]
type Counter = AtomicU32;
```

### WASM Atomic Support

| WASM Mode | Atomics Available |
|-----------|-------------------|
| Single-threaded (default) | No real atomics needed |
| Multi-threaded (`+atomics`) | Full atomic support |

---

## Core Atomic Operations

### 1. Load and Store

The most basic operations:

```rust
use core::sync::atomic::{AtomicU32, Ordering};

let counter = AtomicU32::new(0);

// Read the current value
let value = counter.load(Ordering::Relaxed);

// Write a new value
counter.store(42, Ordering::Relaxed);
```

### 2. Swap

Atomically replace and return old value:

```rust
let counter = AtomicU32::new(10);

// Swap: set to 20, return old value (10)
let old = counter.swap(20, Ordering::AcqRel);
assert_eq!(old, 10);
assert_eq!(counter.load(Ordering::Relaxed), 20);
```

### 3. Compare and Exchange (CAS)

The most powerful atomic operation - conditional update:

```rust
let counter = AtomicU32::new(5);

// Only update if current value equals expected
let result = counter.compare_exchange(
    5,                    // Expected value
    10,                   // New value (if expected matches)
    Ordering::AcqRel,     // Success ordering
    Ordering::Relaxed,    // Failure ordering
);

match result {
    Ok(5) => println!("Updated 5 → 10"),
    Err(actual) => println!("Expected 5, found {}", actual),
}
```

### 4. Fetch-and-Modify Operations

Atomic read-modify-write:

```rust
let counter = AtomicU32::new(10);

// Atomic increment, returns old value
let old = counter.fetch_add(5, Ordering::Relaxed);
assert_eq!(old, 10);
assert_eq!(counter.load(Ordering::Relaxed), 15);

// Other operations
counter.fetch_sub(3, Ordering::Relaxed);  // 15 - 3 = 12
counter.fetch_or(0b1000, Ordering::Relaxed);  // Bitwise OR
counter.fetch_and(0b1111, Ordering::Relaxed); // Bitwise AND
counter.fetch_xor(0b0101, Ordering::Relaxed); // Bitwise XOR
counter.fetch_max(20, Ordering::Relaxed);     // Max of current and 20
counter.fetch_min(5, Ordering::Relaxed);      // Min of current and 5
```

---

## Building Higher-Level Primitives

### AtomicCell: Generic Atomic Wrapper

For types that fit in a pointer, we can provide atomic operations:

```rust
use core::sync::atomic::{AtomicUsize, Ordering};
use core::mem;

/// Atomic cell for small Copy types
pub struct AtomicCell<T: Copy> {
    value: AtomicUsize,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Copy> AtomicCell<T> {
    /// Create a new atomic cell
    pub fn new(value: T) -> Self {
        assert!(mem::size_of::<T>() <= mem::size_of::<usize>());
        assert!(mem::align_of::<T>() <= mem::align_of::<usize>());

        Self {
            value: AtomicUsize::new(Self::to_usize(value)),
            _marker: core::marker::PhantomData,
        }
    }

    /// Load the current value
    pub fn load(&self) -> T {
        Self::from_usize(self.value.load(Ordering::Acquire))
    }

    /// Store a new value
    pub fn store(&self, value: T) {
        self.value.store(Self::to_usize(value), Ordering::Release);
    }

    /// Swap values atomically
    pub fn swap(&self, value: T) -> T {
        Self::from_usize(self.value.swap(Self::to_usize(value), Ordering::AcqRel))
    }

    /// Compare and exchange
    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
        self.value
            .compare_exchange(
                Self::to_usize(current),
                Self::to_usize(new),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(Self::from_usize)
            .map_err(Self::from_usize)
    }

    fn to_usize(value: T) -> usize {
        let mut result = 0usize;
        // SAFETY: T fits in usize (checked in new())
        unsafe {
            core::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                &mut result as *mut usize as *mut u8,
                mem::size_of::<T>(),
            );
        }
        result
    }

    fn from_usize(value: usize) -> T {
        let mut result = mem::MaybeUninit::<T>::uninit();
        // SAFETY: T fits in usize
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

### AtomicOption: Atomic Option<T>

For pointer-sized types, we can implement atomic `Option`:

```rust
use core::sync::atomic::{AtomicPtr, Ordering};
use core::ptr;

/// Atomic Option for boxed values
pub struct AtomicOption<T> {
    ptr: AtomicPtr<T>,
}

impl<T> AtomicOption<T> {
    pub const fn none() -> Self {
        Self {
            ptr: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn some(value: Box<T>) -> Self {
        Self {
            ptr: AtomicPtr::new(Box::into_raw(value)),
        }
    }

    /// Check if Some
    pub fn is_some(&self) -> bool {
        !self.ptr.load(Ordering::Relaxed).is_null()
    }

    /// Take the value, leaving None
    pub fn take(&self) -> Option<Box<T>> {
        let ptr = self.ptr.swap(ptr::null_mut(), Ordering::AcqRel);
        if ptr.is_null() {
            None
        } else {
            // SAFETY: We own the only pointer, was created by Box::into_raw
            Some(unsafe { Box::from_raw(ptr) })
        }
    }

    /// Swap with a new value
    pub fn swap(&self, new: Option<Box<T>>) -> Option<Box<T>> {
        let new_ptr = match new {
            Some(boxed) => Box::into_raw(boxed),
            None => ptr::null_mut(),
        };
        let old_ptr = self.ptr.swap(new_ptr, Ordering::AcqRel);
        if old_ptr.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(old_ptr) })
        }
    }
}

impl<T> Drop for AtomicOption<T> {
    fn drop(&mut self) {
        // Clean up if Some
        let ptr = *self.ptr.get_mut();
        if !ptr.is_null() {
            unsafe { drop(Box::from_raw(ptr)); }
        }
    }
}
```

### AtomicFlag: Simpler than AtomicBool

When you only need set/clear semantics:

```rust
use core::sync::atomic::{AtomicBool, Ordering};

pub struct AtomicFlag {
    flag: AtomicBool,
}

impl AtomicFlag {
    pub const fn new() -> Self {
        Self { flag: AtomicBool::new(false) }
    }

    pub const fn set_true() -> Self {
        Self { flag: AtomicBool::new(true) }
    }

    /// Set the flag, returns previous value
    pub fn set(&self) -> bool {
        self.flag.swap(true, Ordering::AcqRel)
    }

    /// Clear the flag, returns previous value
    pub fn clear(&self) -> bool {
        self.flag.swap(false, Ordering::AcqRel)
    }

    /// Check if set
    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    /// Test and set (returns true if was already set)
    pub fn test_and_set(&self) -> bool {
        self.flag.swap(true, Ordering::AcqRel)
    }
}
```

---

## Common Patterns

### 1. Spin Lock (Test-and-Set)

```rust
use core::sync::atomic::{AtomicBool, Ordering};

let locked = AtomicBool::new(false);

// Acquire lock
while locked.swap(true, Ordering::Acquire) {
    // Was already locked, spin
    core::hint::spin_loop();
}

// Critical section...

// Release lock
locked.store(false, Ordering::Release);
```

### 2. Reference Counter

```rust
use core::sync::atomic::{AtomicUsize, Ordering};

let ref_count = AtomicUsize::new(1);

// Clone: increment
ref_count.fetch_add(1, Ordering::Relaxed);

// Drop: decrement and check
if ref_count.fetch_sub(1, Ordering::AcqRel) == 1 {
    // We were the last reference, deallocate
}
```

### 3. Sequence Lock (Seqlock)

For read-heavy data that's occasionally updated:

```rust
use core::sync::atomic::{AtomicUsize, Ordering};

struct SeqLock<T> {
    seq: AtomicUsize,
    data: std::cell::UnsafeCell<T>,
}

impl<T: Copy> SeqLock<T> {
    pub fn read(&self) -> T {
        loop {
            // Read sequence before
            let seq1 = self.seq.load(Ordering::Acquire);

            // Odd sequence = write in progress
            if seq1 & 1 != 0 {
                core::hint::spin_loop();
                continue;
            }

            // Read data
            let data = unsafe { *self.data.get() };

            // Read sequence after
            let seq2 = self.seq.load(Ordering::Acquire);

            // If unchanged, data is consistent
            if seq1 == seq2 {
                return data;
            }
        }
    }

    pub fn write(&self, value: T) {
        // Increment sequence (now odd = writing)
        self.seq.fetch_add(1, Ordering::Release);

        // Write data
        unsafe { *self.data.get() = value; }

        // Increment sequence (now even = done)
        self.seq.fetch_add(1, Ordering::Release);
    }
}
```

### 4. Lock-Free Stack

```rust
use core::sync::atomic::{AtomicPtr, Ordering};
use core::ptr;

struct Node<T> {
    value: T,
    next: *mut Node<T>,
}

pub struct AtomicStack<T> {
    head: AtomicPtr<Node<T>>,
}

impl<T> AtomicStack<T> {
    pub fn new() -> Self {
        Self { head: AtomicPtr::new(ptr::null_mut()) }
    }

    pub fn push(&self, value: T) {
        let node = Box::into_raw(Box::new(Node {
            value,
            next: ptr::null_mut(),
        }));

        loop {
            let head = self.head.load(Ordering::Relaxed);
            unsafe { (*node).next = head; }

            if self.head
                .compare_exchange_weak(head, node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            if head.is_null() {
                return None;
            }

            let next = unsafe { (*head).next };

            if self.head
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let node = unsafe { Box::from_raw(head) };
                return Some(node.value);
            }
        }
    }
}
```

---

## CAS Loop Pattern

Many atomic algorithms follow this pattern:

```rust
fn atomic_update<F>(atomic: &AtomicU32, f: F) -> u32
where
    F: Fn(u32) -> u32,
{
    loop {
        // 1. Read current value
        let current = atomic.load(Ordering::Relaxed);

        // 2. Compute new value
        let new = f(current);

        // 3. Try to swap atomically
        match atomic.compare_exchange_weak(
            current,
            new,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => return new,     // Success
            Err(_) => continue,      // Retry - value changed
        }
    }
}

// Usage
let counter = AtomicU32::new(10);
atomic_update(&counter, |x| x * 2);  // 10 → 20
```

---

## compare_exchange vs compare_exchange_weak

| Method | Spurious Failures | Use Case |
|--------|-------------------|----------|
| `compare_exchange` | Never | Single attempt, exact result needed |
| `compare_exchange_weak` | Possible | Loop (CAS loop) |

`_weak` is faster on some architectures (LL/SC on ARM) but may fail even when the value matches. In loops, this is fine - we retry anyway.

```rust
// Single attempt - use strong
if atomic.compare_exchange(expected, new, Ordering::AcqRel, Ordering::Relaxed).is_ok() {
    // Definitely succeeded
}

// Loop - use weak
loop {
    if atomic.compare_exchange_weak(expected, new, Ordering::AcqRel, Ordering::Relaxed).is_ok() {
        break;
    }
}
```

---

## The ABA Problem

A subtle issue with CAS-based algorithms:

```
Thread A                    Thread B
────────                    ────────
Read head = A
                            Pop A
                            Pop B
                            Push A (same address!)
CAS(head, A, new)
→ Succeeds! But list is corrupted
```

Thread A's CAS succeeds because head is still `A`, but the list structure changed.

### Solutions

1. **Tagged pointers** - Include a counter in the pointer
2. **Hazard pointers** - Track active references
3. **Epoch-based reclamation** - Delay freeing memory

```rust
// Tagged pointer solution
struct TaggedPtr {
    ptr: AtomicU64,  // High bits = tag, low bits = pointer
}

impl TaggedPtr {
    fn load(&self) -> (*mut Node, u32) {
        let val = self.ptr.load(Ordering::Acquire);
        let ptr = (val & 0xFFFF_FFFF_FFFF) as *mut Node;
        let tag = (val >> 48) as u32;
        (ptr, tag)
    }

    fn compare_exchange(&self, old_ptr: *mut Node, old_tag: u32, new_ptr: *mut Node) -> bool {
        let old = (old_ptr as u64) | ((old_tag as u64) << 48);
        let new = (new_ptr as u64) | (((old_tag + 1) as u64) << 48);
        self.ptr.compare_exchange(old, new, Ordering::AcqRel, Ordering::Relaxed).is_ok()
    }
}
```

---

## no_std Considerations

All atomic types work in `no_std`:

```rust
#![no_std]

use core::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn increment() -> u32 {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
```

### Platform Detection

```rust
// Only compile if 64-bit atomics available
#[cfg(target_has_atomic = "64")]
static COUNTER_64: AtomicU64 = AtomicU64::new(0);

// Fallback for 32-bit only
#[cfg(not(target_has_atomic = "64"))]
static COUNTER_32: AtomicU32 = AtomicU32::new(0);
```

---

## Performance Tips

### 1. Use Relaxed When Possible

```rust
// Counter that doesn't synchronize other data
counter.fetch_add(1, Ordering::Relaxed);

// Use stronger ordering only when needed
if counter.fetch_sub(1, Ordering::AcqRel) == 1 {
    // Last reference, need to see all writes
}
```

### 2. Avoid False Sharing

```rust
// BAD: Counters on same cache line
struct Counters {
    a: AtomicU64,  // Both on same 64-byte cache line
    b: AtomicU64,  // Updates to 'a' invalidate 'b'
}

// GOOD: Pad to separate cache lines
#[repr(align(64))]
struct PaddedCounter(AtomicU64);

struct Counters {
    a: PaddedCounter,
    b: PaddedCounter,
}
```

### 3. Read Before CAS

```rust
// BAD: Immediate CAS attempt
loop {
    if atomic.compare_exchange_weak(0, 1, ...).is_ok() {
        break;
    }
}

// GOOD: Read-only check first
loop {
    // Fast path: read-only check
    if atomic.load(Ordering::Relaxed) != 0 {
        core::hint::spin_loop();
        continue;
    }

    // Slow path: CAS attempt
    if atomic.compare_exchange_weak(0, 1, ...).is_ok() {
        break;
    }
}
```

---

## Summary

| Concept | Key Point |
|---------|-----------|
| **Atomic** | Indivisible operations |
| **CAS** | compare_exchange - conditional update |
| **Fetch-and-X** | Atomic read-modify-write |
| **_weak** | Use in loops, may spuriously fail |
| **ABA problem** | Same value, different state |
| **AtomicCell** | Generic wrapper for small types |
| **AtomicOption** | Atomic Option<Box<T>> |

---

*Next: [04-memory-ordering.md](./04-memory-ordering.md) - Memory ordering semantics*
