# UnsafeCell: The Foundation of Interior Mutability

`UnsafeCell<T>` is the **only** legal way to obtain mutable access to data through a shared reference in Rust. Every interior mutability type (`Cell`, `RefCell`, `Mutex`, `RwLock`, `AtomicXxx`) is built on top of it.

---

## Why UnsafeCell Exists

### The Problem: Rust's Aliasing Rules

Rust's safety guarantees depend on these rules:
- `&T` (shared reference): Data is immutable, can have many
- `&mut T` (exclusive reference): Data is mutable, can have only one

```rust
// This is forbidden - can't mutate through shared reference
fn bad(x: &i32) {
    *x = 42;  // ERROR: cannot assign to `*x`, which is behind a `&` reference
}
```

### The Solution: UnsafeCell

`UnsafeCell<T>` tells the compiler: "I'm handling mutation myself. Don't assume this data is immutable."

```rust
use core::cell::UnsafeCell;

fn works(x: &UnsafeCell<i32>) {
    // SAFETY: We ensure no other references exist
    unsafe { *x.get() = 42; }  // OK!
}
```

---

## How UnsafeCell Works

### The Type Definition

```rust
#[repr(transparent)]
pub struct UnsafeCell<T: ?Sized> {
    value: T,
}

impl<T> UnsafeCell<T> {
    pub const fn new(value: T) -> Self {
        UnsafeCell { value }
    }

    pub const fn get(&self) -> *mut T {
        // Returns a raw mutable pointer from a shared reference!
        &self.value as *const T as *mut T
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}
```

### The Magic: `#[repr(transparent)]`

`UnsafeCell<T>` has the same memory layout as `T`. It's zero-cost at runtime.

### The Key Method: `get(&self) -> *mut T`

This is the magic: from a shared `&self`, you get a mutable `*mut T`. This breaks Rust's normal rules, which is why everything using it requires `unsafe`.

---

## The Contract You Must Uphold

When using `UnsafeCell`, **you** are responsible for preventing:

### 1. Data Races

Multiple threads accessing the same data, with at least one writing, without synchronization.

```rust
// ❌ DATA RACE - Undefined Behavior!
use std::thread;
use core::cell::UnsafeCell;

let cell = UnsafeCell::new(0);
let ptr = cell.get();

thread::spawn(move || {
    unsafe { *ptr = 1; }  // Write
});

unsafe { println!("{}", *ptr); }  // Read - RACE!
```

**Fix**: Use atomics or locks to synchronize access.

### 2. Aliasing Violations

Creating `&mut T` while `&T` or another `&mut T` exists.

```rust
// ❌ ALIASING VIOLATION - Undefined Behavior!
let cell = UnsafeCell::new(42);

unsafe {
    let ref1: &mut i32 = &mut *cell.get();
    let ref2: &mut i32 = &mut *cell.get();  // Two &mut to same data!

    *ref1 = 1;
    *ref2 = 2;  // UB: ref1 is still alive
}
```

**Fix**: Never create overlapping mutable references.

### 3. Use-After-Free / Dangling Pointers

The raw pointer from `get()` doesn't track lifetimes.

```rust
// ❌ USE-AFTER-FREE - Undefined Behavior!
let ptr = {
    let cell = UnsafeCell::new(42);
    cell.get()  // Pointer escapes
};  // cell is dropped here

unsafe { *ptr = 100; }  // Dangling pointer!
```

**Fix**: Ensure the `UnsafeCell` outlives all uses of the pointer.

---

## Patterns for Safe Usage

### Pattern 1: Protecting with Atomics (SpinMutex)

Use atomics to ensure only one thread accesses the data at a time:

```rust
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: The atomic lock ensures exclusive access
unsafe impl<T: Send> Send for SpinMutex<T> {}
unsafe impl<T: Send> Sync for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        // Acquire lock
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinMutexGuard { mutex: self }
    }
}

pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<T> core::ops::Deref for SpinMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: We hold the lock, so we have exclusive access
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> core::ops::DerefMut for SpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We hold the lock, so we have exclusive access
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}
```

### Pattern 2: Single-Threaded Only (Cell, RefCell)

For single-threaded code, no synchronization needed:

```rust
use core::cell::UnsafeCell;

// NOT Sync - can only be used from one thread
pub struct SimpleCell<T> {
    value: UnsafeCell<T>,
}

// Send is OK if T is Send
unsafe impl<T: Send> Send for SimpleCell<T> {}
// NOT Sync - never implement Sync for unsynchronized UnsafeCell

impl<T: Copy> SimpleCell<T> {
    pub fn get(&self) -> T {
        // SAFETY: Single-threaded, T is Copy (no references escape)
        unsafe { *self.value.get() }
    }

    pub fn set(&self, value: T) {
        // SAFETY: Single-threaded, we're replacing the whole value
        unsafe { *self.value.get() = value; }
    }
}
```

### Pattern 3: Read-Write Lock (Multiple Readers OR One Writer)

```rust
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

pub struct RwLock<T> {
    // 0 = unlocked, >0 = reader count, u32::MAX = write locked
    state: AtomicU32,
    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    pub fn read(&self) -> ReadGuard<'_, T> {
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state < u32::MAX - 1 {  // Not write-locked
                if self.state.compare_exchange_weak(
                    state, state + 1,
                    Ordering::Acquire, Ordering::Relaxed
                ).is_ok() {
                    return ReadGuard { lock: self };
                }
            }
            core::hint::spin_loop();
        }
    }
}

pub struct ReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> core::ops::Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: Multiple readers allowed, no writers exist
        unsafe { &*self.lock.data.get() }
    }
}
// Note: No DerefMut - readers can't mutate
```

---

## UnsafeCell and Send/Sync

### The Rules

| Type | Send | Sync | Meaning |
|------|------|------|---------|
| `UnsafeCell<T>` | If T: Send | **Never** | Can't safely share between threads |
| Your wrapper | Depends | Depends | You decide based on your synchronization |

### Why UnsafeCell is Not Sync

`Sync` means "safe to share references between threads." But `UnsafeCell::get()` returns `*mut T` from `&self` - if multiple threads do this, you get data races.

```rust
// UnsafeCell is NOT Sync, so this is compile error:
use core::cell::UnsafeCell;
use std::thread;

let cell = UnsafeCell::new(42);
let cell_ref = &cell;

thread::spawn(move || {
    // ERROR: `UnsafeCell<i32>` cannot be shared between threads safely
    unsafe { *cell_ref.get() = 100; }
});
```

### Making Your Wrapper Sync

If your wrapper provides proper synchronization, you can implement Sync:

```rust
// SAFETY: AtomicBool ensures mutual exclusion
unsafe impl<T: Send> Sync for SpinMutex<T> {}
```

**The rule**: Only implement `Sync` if your synchronization prevents data races.

---

## Common Mistakes

### Mistake 1: Implementing Sync Without Synchronization

```rust
// ❌ WRONG - No synchronization!
struct BadWrapper<T> {
    data: UnsafeCell<T>,
}

// DON'T DO THIS - data races possible!
unsafe impl<T: Send> Sync for BadWrapper<T> {}
```

### Mistake 2: Creating References That Overlap

```rust
// ❌ WRONG - Overlapping mutable references
let cell = UnsafeCell::new(vec![1, 2, 3]);

unsafe {
    let vec_ref = &mut *cell.get();
    let first = &mut vec_ref[0];
    let second = &mut vec_ref[1];

    // This looks OK, but what about:
    let also_vec = &*cell.get();  // ❌ Shared ref while &mut exists!
}
```

### Mistake 3: Holding References Across Unlock

```rust
// ❌ WRONG - Reference escapes lock scope
let mutex = SpinMutex::new(vec![1, 2, 3]);

let reference = {
    let guard = mutex.lock();
    &guard[0]  // Reference into guarded data
};  // Guard dropped, lock released

// Reference is now dangling - another thread could modify!
println!("{}", reference);  // ❌ UB
```

**Fix**: The guard's lifetime prevents this in safe code. Only possible with manual `unsafe`.

### Mistake 4: Forgetting Volatile for Memory-Mapped I/O

```rust
// ❌ WRONG for MMIO - compiler may optimize away
let mmio = UnsafeCell::new(0u32);
unsafe { *mmio.get() = 1; }  // Might be optimized out!

// ✅ CORRECT for MMIO - use volatile
unsafe { core::ptr::write_volatile(mmio.get(), 1); }
```

---

## Advanced Tricks

### Trick 1: Zero-Cost Abstraction with Inline

```rust
#[inline(always)]
pub fn get(&self) -> &T {
    // SAFETY: documented invariants
    unsafe { &*self.data.get() }
}
```

With `#[inline(always)]`, the UnsafeCell access compiles to a simple memory load - zero overhead.

### Trick 2: Const Construction

`UnsafeCell::new()` is `const`, so you can use it in statics:

```rust
use core::cell::UnsafeCell;
use core::sync::atomic::AtomicBool;

struct SpinMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),  // const!
        }
    }
}

// Can use in static!
static COUNTER: SpinMutex<u32> = SpinMutex::new(0);
```

### Trick 3: get_mut() for Exclusive Access

When you have `&mut UnsafeCell<T>`, you can safely get `&mut T`:

```rust
let mut cell = UnsafeCell::new(42);

// No unsafe needed - we have exclusive access via &mut
let value: &mut i32 = cell.get_mut();
*value = 100;
```

This is useful in constructors or when you know you have exclusive access.

### Trick 4: into_inner() to Unwrap

```rust
let cell = UnsafeCell::new(vec![1, 2, 3]);
let vec: Vec<i32> = cell.into_inner();  // No unsafe!
```

### Trick 5: Raw Pointer Casts for FFI

```rust
use core::cell::UnsafeCell;

#[repr(C)]
struct FfiStruct {
    data: UnsafeCell<[u8; 256]>,
}

extern "C" {
    fn ffi_fill_buffer(ptr: *mut u8, len: usize);
}

impl FfiStruct {
    fn fill(&self) {
        // SAFETY: FFI function doesn't alias, we synchronize externally
        unsafe {
            ffi_fill_buffer(
                self.data.get() as *mut u8,
                256
            );
        }
    }
}
```

---

## UnsafeCell in no_std

`UnsafeCell` is in `core`, so it works everywhere:

```rust
#![no_std]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU8, Ordering};

// Works in no_std!
pub struct NoStdMutex<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}
```

---

## Debugging UnsafeCell Issues

### Tool 1: Miri

Miri detects undefined behavior in unsafe code:

```bash
cargo +nightly miri test
```

Miri will catch:
- Data races
- Use after free
- Invalid pointer dereferences
- Aliasing violations

### Tool 2: Thread Sanitizer

For runtime race detection:

```bash
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test
```

### Tool 3: Address Sanitizer

For memory errors:

```bash
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test
```

---

## Checklist Before Using UnsafeCell

- [ ] **Do I need interior mutability?** Maybe `Cell<T>` or `RefCell<T>` suffices.
- [ ] **Am I synchronizing access?** Atomics, locks, or single-threaded guarantee?
- [ ] **Am I preventing aliasing?** No `&mut T` while any other reference exists.
- [ ] **Is my SAFETY comment accurate?** Document why the unsafe is sound.
- [ ] **Did I implement Send/Sync correctly?** Only if synchronization is provided.
- [ ] **Did I test with Miri?** `cargo +nightly miri test`

---

## Quick Reference: When to Use What

| Need | Use This | Not UnsafeCell Directly |
|------|----------|------------------------|
| Simple single-threaded mutable value | `Cell<T>` | ✓ |
| Single-threaded with runtime borrow checking | `RefCell<T>` | ✓ |
| Thread-safe mutable value | `Mutex<T>`, `RwLock<T>` | ✓ |
| Lock-free atomic value | `AtomicXxx` | ✓ |
| Building your own sync primitive | `UnsafeCell<T>` | Use this |
| FFI with mutable shared state | `UnsafeCell<T>` | Use this |
| Embedded/no_std sync primitive | `UnsafeCell<T>` | Use this |

---

## Summary

| Concept | Key Point |
|---------|-----------|
| **Purpose** | Only legal way to mutate through `&T` |
| **get()** | Returns `*mut T` from `&self` - the "escape hatch" |
| **Not Sync** | UnsafeCell itself can't be shared between threads |
| **Your job** | Prevent data races and aliasing violations |
| **Send/Sync** | Implement on your wrapper only if you synchronize |
| **Testing** | Use Miri to catch UB: `cargo +nightly miri test` |
| **const** | `UnsafeCell::new()` is const - works in statics |

---

*This document is part of the fundamentals series. See [00-overview.md](./00-overview.md) for the full list.*
