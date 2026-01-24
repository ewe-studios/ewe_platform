# Memory Ordering Deep Dive

## Why Memory Ordering Matters

Modern CPUs and compilers reorder operations for performance. Without proper ordering, threads may see operations in different orders:

```
Thread A:                    Thread B sees:
─────────                    ──────────────
data = 42;                   flag = true;  ← Reordered!
flag = true;                 data = 42;
```

Memory ordering tells the CPU and compiler how operations can be reordered.

---

## The Five Orderings

Rust provides five memory orderings via `std::sync::atomic::Ordering`:

| Ordering | Strength | Use Case |
|----------|----------|----------|
| `Relaxed` | Weakest | Independent counters |
| `Acquire` | Medium | Reading shared data |
| `Release` | Medium | Publishing shared data |
| `AcqRel` | Strong | Read-modify-write |
| `SeqCst` | Strongest | Total ordering |

---

## Ordering Explained

### Relaxed

**No synchronization guarantees.** Only guarantees atomicity of the single operation.

```rust
use core::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

// Thread A
COUNTER.fetch_add(1, Ordering::Relaxed);

// Thread B
COUNTER.fetch_add(1, Ordering::Relaxed);

// Both increments are atomic, but order between threads is undefined
```

**When to use:**
- Statistics counters
- Progress indicators
- Any atomic that doesn't synchronize other data

**When NOT to use:**
- When the atomic guards access to other data
- When order between threads matters

### Acquire

**All reads/writes after this operation cannot be reordered before it.**

```
                    ┌─────────────────────────┐
  [operations]      │   Acquire Load          │      [operations]
                    │   ═══════════           │      can't move up
  can move here     │                         │      ↓↓↓↓↓↓↓↓↓↓↓
                    └─────────────────────────┘
```

```rust
// Thread B: Acquire the data
if flag.load(Ordering::Acquire) {
    // All operations below see everything Thread A did before Release
    println!("data = {}", data.load(Ordering::Relaxed));
}
```

### Release

**All reads/writes before this operation cannot be reordered after it.**

```
                    ┌─────────────────────────┐
  [operations]      │   Release Store         │      [operations]
  can't move down   │   ═══════════════       │
  ↓↓↓↓↓↓↓↓↓↓↓       │                         │      can move here
                    └─────────────────────────┘
```

```rust
// Thread A: Publish data
data.store(42, Ordering::Relaxed);
flag.store(true, Ordering::Release);
// Everything before this is visible to Acquire loads of flag
```

### Acquire-Release Together

The classic pattern: producer-consumer synchronization.

```rust
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static DATA: AtomicU32 = AtomicU32::new(0);
static READY: AtomicBool = AtomicBool::new(false);

// Producer (Thread A)
fn produce() {
    DATA.store(42, Ordering::Relaxed);      // Step 1: Write data
    READY.store(true, Ordering::Release);   // Step 2: Publish
}

// Consumer (Thread B)
fn consume() {
    while !READY.load(Ordering::Acquire) {  // Step 1: Wait for ready
        core::hint::spin_loop();
    }
    let value = DATA.load(Ordering::Relaxed); // Step 2: Read data
    assert_eq!(value, 42); // Guaranteed!
}
```

The Release-Acquire pair forms a **synchronizes-with** relationship:

```
Thread A:                    Thread B:
─────────                    ─────────
DATA = 42 ────────┐
                  │ synchronizes-with
READY = true ─────┼─────────► READY.load() = true
(Release)         │           (Acquire)
                  │
                  └─────────► DATA.load() = 42 ✓
```

### AcqRel (Acquire + Release)

For read-modify-write operations that both read and write:

```rust
// Spin lock: both reading (to check) and writing (to acquire)
while lock.compare_exchange(
    false,           // Expected
    true,            // Desired
    Ordering::AcqRel,    // Success: both acquire and release
    Ordering::Relaxed,   // Failure: just retry
).is_err() {
    core::hint::spin_loop();
}
```

### SeqCst (Sequentially Consistent)

**Total ordering across all SeqCst operations.** All threads see the same order.

```rust
use core::sync::atomic::{AtomicBool, Ordering};

static X: AtomicBool = AtomicBool::new(false);
static Y: AtomicBool = AtomicBool::new(false);

// Thread A
X.store(true, Ordering::SeqCst);

// Thread B
Y.store(true, Ordering::SeqCst);

// Thread C
let x = X.load(Ordering::SeqCst);
let y = Y.load(Ordering::SeqCst);

// Thread D
let y = Y.load(Ordering::SeqCst);
let x = X.load(Ordering::SeqCst);

// With SeqCst: If C sees (x=true, y=false), then D cannot see (y=true, x=false)
// Both threads agree on the global order of X and Y stores
```

**When to use:**
- When you need total ordering
- When debugging concurrency issues (stronger = easier to reason about)
- When other orderings are unclear

**When NOT to use:**
- Performance-critical code (SeqCst is slowest)
- When weaker ordering suffices

---

## Visual Guide: The Ordering Hierarchy

```
          ┌───────────────────────────────────────┐
          │           SeqCst                      │
          │   Total global ordering               │
          │   Slowest, safest                     │
          └───────────────────────────────────────┘
                          │
          ┌───────────────┴───────────────┐
          ▼                               ▼
┌─────────────────┐             ┌─────────────────┐
│    Acquire      │             │    Release      │
│ Prevents later  │             │ Prevents earlier│
│ ops moving up   │             │ ops moving down │
└─────────────────┘             └─────────────────┘
          │                               │
          └───────────┬───────────────────┘
                      ▼
          ┌───────────────────────────────────────┐
          │           AcqRel                      │
          │   Both Acquire and Release            │
          │   For read-modify-write ops           │
          └───────────────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────────────┐
          │           Relaxed                     │
          │   Only atomic, no ordering            │
          │   Fastest                             │
          └───────────────────────────────────────┘
```

---

## Common Patterns and Their Orderings

### Pattern 1: Spin Lock

```rust
use core::sync::atomic::{AtomicBool, Ordering};

struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    fn lock(&self) {
        while self.locked
            .compare_exchange_weak(
                false,              // Expected: unlocked
                true,               // Desired: locked
                Ordering::Acquire,  // Success: acquire semantics
                Ordering::Relaxed,  // Failure: just retry
            )
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        // After this: all protected data is visible
    }

    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
        // Before this: all writes to protected data are flushed
    }
}
```

**Why these orderings?**
- `Acquire` on lock: See all writes before the previous unlock
- `Release` on unlock: Make our writes visible to next lock

### Pattern 2: One-Time Initialization

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
        if self.state.load(Ordering::Acquire) == COMPLETE {
            return;
        }

        // Slow path: try to be the initializer
        if self.state
            .compare_exchange(UNINIT, RUNNING, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            f();
            self.state.store(COMPLETE, Ordering::Release);
        } else {
            // Wait for another thread
            while self.state.load(Ordering::Acquire) != COMPLETE {
                core::hint::spin_loop();
            }
        }
    }
}
```

**Why these orderings?**
- `Acquire` on load: See the initialized data
- `Release` on store: Publish the initialized data

### Pattern 3: Reference Counting

```rust
use core::sync::atomic::{AtomicUsize, Ordering, fence};

struct Arc<T> {
    ptr: *mut ArcInner<T>,
}

struct ArcInner<T> {
    ref_count: AtomicUsize,
    data: T,
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        unsafe {
            // Relaxed is fine: we already have access to the data
            (*self.ptr).ref_count.fetch_add(1, Ordering::Relaxed);
        }
        Arc { ptr: self.ptr }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        unsafe {
            // Release: our writes to data must be visible before dealloc
            if (*self.ptr).ref_count.fetch_sub(1, Ordering::Release) == 1 {
                // Acquire fence: see all writes from other threads
                fence(Ordering::Acquire);
                drop(Box::from_raw(self.ptr));
            }
        }
    }
}
```

**Why these orderings?**
- `Relaxed` increment: We already have a reference, no sync needed
- `Release` decrement: Our writes must be visible before potential dealloc
- `Acquire` fence before dealloc: See all writes from other threads

### Pattern 4: Double-Checked Locking

```rust
use core::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

struct Lazy<T> {
    ptr: AtomicPtr<T>,
    init: Mutex<()>,
}

impl<T> Lazy<T> {
    fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        // Fast path: already initialized
        let ptr = self.ptr.load(Ordering::Acquire);
        if !ptr.is_null() {
            return unsafe { &*ptr };
        }

        // Slow path: initialize
        let _guard = self.init.lock().unwrap();

        // Double-check after acquiring lock
        let ptr = self.ptr.load(Ordering::Acquire);
        if !ptr.is_null() {
            return unsafe { &*ptr };
        }

        // Actually initialize
        let value = Box::new(f());
        let ptr = Box::into_raw(value);
        self.ptr.store(ptr, Ordering::Release);

        unsafe { &*ptr }
    }
}
```

---

## Memory Fences

Sometimes you need ordering without an atomic operation:

```rust
use core::sync::atomic::{fence, Ordering};

// Ensure all previous writes are visible
fence(Ordering::Release);

// Ensure all subsequent reads see previous writes
fence(Ordering::Acquire);

// Full barrier
fence(Ordering::SeqCst);
```

**When to use fences:**
- When you need ordering between non-atomic and atomic operations
- When you're building complex lock-free structures
- When atomic ordering isn't enough

---

## Hardware Reality

### x86/x64

Strong memory model. Most orderings are "free":
- `Relaxed`, `Acquire`, `Release` compile to the same code
- Only `SeqCst` stores add a fence (`mfence` or `xchg`)

### ARM/RISC-V

Weaker memory model. Orderings matter more:
- `Acquire` → `ldar` (load-acquire)
- `Release` → `stlr` (store-release)
- Relaxed → plain load/store

### WASM

WASM atomics follow the `SeqCst` model for simplicity:
- All atomic operations are sequentially consistent
- `Relaxed` still works but may be slower than necessary

---

## Choosing the Right Ordering

### Decision Tree

```
Is this atomic protecting other data?
│
├── No → Relaxed
│
└── Yes → Is it a read, write, or read-modify-write?
          │
          ├── Read → Acquire
          │
          ├── Write → Release
          │
          └── RMW → AcqRel
                    │
                    └── Need total ordering? → SeqCst
```

### Quick Reference

| Scenario | Ordering |
|----------|----------|
| Statistics counter | `Relaxed` |
| Flag check (reader) | `Acquire` |
| Flag set (writer) | `Release` |
| Spin lock acquire | `Acquire` (success), `Relaxed` (fail) |
| Spin lock release | `Release` |
| Lock-free CAS loop | `AcqRel` (success), `Relaxed` (fail) |
| When unsure | `SeqCst` (then optimize) |

---

## Common Mistakes

### Mistake 1: Relaxed for Synchronization

```rust
// WRONG!
static DATA: AtomicU32 = AtomicU32::new(0);
static READY: AtomicBool = AtomicBool::new(false);

// Producer
DATA.store(42, Ordering::Relaxed);
READY.store(true, Ordering::Relaxed);  // No release!

// Consumer
while !READY.load(Ordering::Relaxed) {} // No acquire!
let value = DATA.load(Ordering::Relaxed);
// value might be 0! The stores could be reordered!
```

### Mistake 2: Acquire Without Release

```rust
// WRONG!
// Producer stores with Relaxed (should be Release)
DATA.store(42, Ordering::Relaxed);
READY.store(true, Ordering::Relaxed);

// Consumer loads with Acquire
while !READY.load(Ordering::Acquire) {}
// Acquire without matching Release is pointless!
```

### Mistake 3: SeqCst Everywhere

```rust
// INEFFICIENT
counter.fetch_add(1, Ordering::SeqCst);  // Just a counter!

// BETTER
counter.fetch_add(1, Ordering::Relaxed);  // Same result, faster
```

---

## Testing Memory Ordering

Memory ordering bugs are hard to reproduce. Tools to help:

1. **Miri** - Rust's undefined behavior detector
```bash
cargo +nightly miri test
```

2. **Loom** - Model checker for concurrency
```rust
use loom::sync::atomic::{AtomicUsize, Ordering};
use loom::thread;

#[test]
fn test_ordering() {
    loom::model(|| {
        let x = AtomicUsize::new(0);
        // Loom explores all possible interleavings
    });
}
```

3. **ThreadSanitizer** - Runtime race detector
```bash
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test
```

---

## Summary

| Ordering | Purpose | Barrier Type |
|----------|---------|--------------|
| `Relaxed` | Atomic only, no sync | None |
| `Acquire` | Read that syncs | Load barrier |
| `Release` | Write that syncs | Store barrier |
| `AcqRel` | RMW that syncs | Both |
| `SeqCst` | Total ordering | Full fence |

**Rules of thumb:**
1. Start with `Relaxed` if it doesn't sync other data
2. Use `Acquire`/`Release` pairs for producer-consumer
3. Use `AcqRel` for read-modify-write that syncs
4. Use `SeqCst` when unsure, then optimize

---

*Next: [05-wasm-considerations.md](./05-wasm-considerations.md) - WASM threading and optimization*
