# WASM Threading and Optimization

## WASM Threading Model

WebAssembly has two distinct threading modes:

| Mode | SharedArrayBuffer | Atomics | Use Case |
|------|-------------------|---------|----------|
| **Single-threaded** (default) | No | Not needed | Most web apps |
| **Multi-threaded** | Yes | Required | Workers, parallel compute |

---

## Single-Threaded WASM (Default)

### What It Means

Standard `wasm32-unknown-unknown` builds are **single-threaded**:
- Only one execution context
- No concurrent access to memory
- Atomics compile but are unnecessary overhead

```rust
// In single-threaded WASM, this is overkill:
let counter = AtomicU32::new(0);
counter.fetch_add(1, Ordering::SeqCst);  // Atomic overhead for nothing!

// A simple Cell would suffice:
let counter = Cell::new(0u32);
counter.set(counter.get() + 1);
```

### Detection at Compile Time

```rust
// Single-threaded WASM (no atomics target feature)
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
const SINGLE_THREADED_WASM: bool = true;

// Multi-threaded WASM (with atomics target feature)
#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
const MULTI_THREADED_WASM: bool = true;

// Native platforms
#[cfg(not(target_arch = "wasm32"))]
const NATIVE: bool = true;
```

---

## Optimizing for Single-Threaded WASM

### No-Op Locks

Since there's no concurrency, locks can be no-ops:

```rust
use core::cell::UnsafeCell;

/// Mutex that does nothing in single-threaded contexts
pub struct NoopMutex<T> {
    data: UnsafeCell<T>,
}

// SAFETY: Only one thread in single-threaded WASM
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
        Some(NoopMutexGuard { mutex: self })
    }
}

pub struct NoopMutexGuard<'a, T> {
    mutex: &'a NoopMutex<T>,
}

impl<T> core::ops::Deref for NoopMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: Single-threaded, no concurrent access
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> core::ops::DerefMut for NoopMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: Single-threaded, no concurrent access
        unsafe { &mut *self.mutex.data.get() }
    }
}
```

### No-Op RwLock

```rust
pub struct NoopRwLock<T> {
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for NoopRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for NoopRwLock<T> {}

impl<T> NoopRwLock<T> {
    pub const fn new(value: T) -> Self {
        Self { data: UnsafeCell::new(value) }
    }

    pub fn read(&self) -> NoopReadGuard<'_, T> {
        NoopReadGuard { lock: self }
    }

    pub fn write(&self) -> NoopWriteGuard<'_, T> {
        NoopWriteGuard { lock: self }
    }
}
```

### No-Op Once

```rust
use core::cell::Cell;

pub struct NoopOnce {
    done: Cell<bool>,
}

impl NoopOnce {
    pub const fn new() -> Self {
        Self { done: Cell::new(false) }
    }

    pub fn call_once<F: FnOnce()>(&self, f: F) {
        if !self.done.get() {
            f();
            self.done.set(true);
        }
    }

    pub fn is_completed(&self) -> bool {
        self.done.get()
    }
}
```

---

## Type Aliases for Automatic Selection

In `mod.rs`, provide type aliases that select the right implementation:

```rust
// Mutex type alias
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type Mutex<T> = NoopMutex<T>;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type Mutex<T> = SpinMutex<T>;

// RwLock type alias
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type RwLock<T> = NoopRwLock<T>;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type RwLock<T> = SpinRwLock<T>;

// Once type alias
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type Once = NoopOnce;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type Once = spin_once::Once;
```

### Usage

```rust
use foundation_nostd::primitives::Mutex;

// Works everywhere!
let counter = Mutex::new(0);
let guard = counter.lock();
```

- On **native**: Real spin lock
- On **single-threaded WASM**: No-op
- On **multi-threaded WASM**: Real spin lock

---

## Multi-Threaded WASM

### Enabling Multi-Threaded WASM

Requires specific compiler flags:

```bash
# Build with atomics support
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
  cargo build --target wasm32-unknown-unknown -Z build-std=std,panic_abort
```

### Browser Requirements

Multi-threaded WASM requires:

1. **SharedArrayBuffer** - Shared memory between workers
2. **Atomics** - Atomic operations on shared memory
3. **COOP/COEP headers** - Security isolation

```http
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

### Web Worker Pattern

```javascript
// main.js
const memory = new WebAssembly.Memory({
    initial: 256,
    maximum: 256,
    shared: true
});

const worker = new Worker('worker.js');
worker.postMessage({ memory });

// worker.js
self.onmessage = (event) => {
    const { memory } = event.data;
    // Use shared memory with main thread
};
```

---

## Spin Locks in WASM

### Why Spin Locks Work in WASM

WASM has no access to OS synchronization primitives. Spin locks are the only option:

```rust
// This is what browsers implement for Atomics.wait()
fn atomic_wait(addr: &AtomicU32, expected: u32) {
    while addr.load(Ordering::Acquire) == expected {
        // Spin!
    }
}
```

### Considerations for WASM Spin Locks

1. **No thread yielding** - Can't call `sched_yield()`
2. **Single core per worker** - Browser controls scheduling
3. **Use spin_loop()** - Helps browser know we're waiting

```rust
pub fn lock(&self) {
    while self.state
        .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        // This hint is important for WASM!
        // It maps to pause/yield instructions or nops
        core::hint::spin_loop();
    }
}
```

---

## WASM Memory Model

### SharedArrayBuffer Atomics

WASM atomics work on `SharedArrayBuffer`:

```javascript
// JavaScript side
const buffer = new SharedArrayBuffer(1024);
const view = new Int32Array(buffer);

// Atomic operations
Atomics.store(view, 0, 42);
Atomics.load(view, 0);
Atomics.compareExchange(view, 0, 42, 100);

// Wait and notify (like futex)
Atomics.wait(view, 0, 0);    // Block until view[0] != 0
Atomics.notify(view, 0, 1);  // Wake one waiter
```

### Rust Mapping

```rust
use core::sync::atomic::{AtomicU32, Ordering};

// These map to WASM atomic instructions
let atom = AtomicU32::new(0);

atom.store(42, Ordering::SeqCst);           // atomic.store
atom.load(Ordering::SeqCst);                // atomic.load
atom.compare_exchange(42, 100, ...);        // atomic.cmpxchg
```

---

## Feature Detection

### Compile-Time Detection

```rust
// Target architecture
#[cfg(target_arch = "wasm32")]
fn is_wasm() -> bool { true }

// Atomics feature (multi-threaded WASM)
#[cfg(target_feature = "atomics")]
fn has_atomics() -> bool { true }

// Combined checks
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
type Mutex<T> = NoopMutex<T>;

#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
type Mutex<T> = SpinMutex<T>;

#[cfg(not(target_arch = "wasm32"))]
type Mutex<T> = SpinMutex<T>;
```

### Runtime Detection (JavaScript)

```javascript
// Check for SharedArrayBuffer support
function hasSharedArrayBuffer() {
    return typeof SharedArrayBuffer !== 'undefined';
}

// Check for Atomics support
function hasAtomics() {
    return typeof Atomics !== 'undefined';
}

// Full threading support check
function hasThreadingSupport() {
    try {
        new SharedArrayBuffer(1);
        return true;
    } catch (e) {
        return false; // COOP/COEP not set or not supported
    }
}
```

---

## no_std Compatibility

All our primitives work in no_std, which is required for WASM:

```rust
#![no_std]

use core::sync::atomic::{AtomicU8, Ordering};
use core::cell::UnsafeCell;

// This compiles for wasm32-unknown-unknown
pub struct SpinMutex<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}
```

### What's Available in no_std

| Module | Available | Notes |
|--------|-----------|-------|
| `core::sync::atomic` | ✅ | All atomic types |
| `core::cell` | ✅ | UnsafeCell, Cell, RefCell |
| `core::hint::spin_loop` | ✅ | CPU hint for spinning |
| `std::thread` | ❌ | Not in no_std |
| `std::sync` | ❌ | Not in no_std |

---

## Performance Considerations

### WASM Atomic Performance

WASM atomics are slower than native:
- Each atomic operation is a function call to browser
- No direct CPU atomic instructions
- Additional overhead for SharedArrayBuffer

### Mitigation Strategies

1. **Minimize atomic operations**
```rust
// BAD: Multiple atomic ops
let a = counter.load(Ordering::Acquire);
let b = other.load(Ordering::Acquire);

// BETTER: Batch if possible
let (a, b) = load_both(&counter, &other);
```

2. **Use relaxed when possible**
```rust
// Fast path with relaxed read
if flag.load(Ordering::Relaxed) {
    // Slow path with acquire
    if flag.load(Ordering::Acquire) {
        // ...
    }
}
```

3. **Cache atomic values locally**
```rust
// BAD: Repeated atomic reads
for i in 0..1000 {
    if flag.load(Ordering::Acquire) {
        break;
    }
}

// BETTER: Local variable
let mut done = false;
for i in 0..1000 {
    if !done && flag.load(Ordering::Acquire) {
        done = true;
    }
    if done { break; }
}
```

---

## Common Patterns

### Pattern 1: Conditional Compilation

```rust
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
mod noop_impl {
    // Single-threaded implementations
}

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
mod atomic_impl {
    // Real atomic implementations
}

// Re-export based on platform
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub use noop_impl::*;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub use atomic_impl::*;
```

### Pattern 2: Feature-Gated Threading

```rust
// Cargo.toml
[features]
default = []
wasm-threads = []

// lib.rs
#[cfg(all(target_arch = "wasm32", feature = "wasm-threads"))]
compile_error!("Build with +atomics for wasm-threads feature");
```

### Pattern 3: Graceful Degradation

```rust
pub struct SharedCounter {
    #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
    value: AtomicU64,

    #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
    value: Cell<u64>,
}

impl SharedCounter {
    pub fn increment(&self) {
        #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
        self.value.fetch_add(1, Ordering::Relaxed);

        #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
        self.value.set(self.value.get() + 1);
    }
}
```

---

## Testing WASM Builds

### Testing Single-Threaded

```bash
# Build for WASM (single-threaded)
cargo build --target wasm32-unknown-unknown

# Run tests with wasm-pack
wasm-pack test --headless --firefox
```

### Testing Multi-Threaded

```bash
# Build with atomics
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
  cargo build --target wasm32-unknown-unknown

# Note: Multi-threaded WASM tests require special setup
```

---

## Summary

| Aspect | Single-Threaded WASM | Multi-Threaded WASM | Native |
|--------|---------------------|---------------------|--------|
| **Detection** | `wasm32` + no `atomics` | `wasm32` + `atomics` | not `wasm32` |
| **Lock type** | NoopMutex | SpinMutex | SpinMutex |
| **Atomics** | Optional | Required | Required |
| **Overhead** | Zero | Moderate | Low |
| **Browser** | All modern | Needs SAB headers | N/A |

**Key Takeaways:**

1. Default WASM is single-threaded - use no-op locks
2. Multi-threaded WASM requires special build flags
3. Use type aliases for automatic platform selection
4. All primitives must be no_std compatible
5. `core::hint::spin_loop()` is important for WASM

---

*Next: [06-usage-patterns.md](./06-usage-patterns.md) - Patterns and best practices*
