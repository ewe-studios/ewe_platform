# Sync Primitives Overview

## What This Library Provides

This library provides **no_std-compatible** synchronization primitives that work across:
- Native platforms (Linux, macOS, Windows)
- WebAssembly (both single-threaded and multi-threaded)
- Embedded systems (bare-metal, no OS)

All primitives are built on `core::sync::atomic` with no external dependencies.

### Complete Primitive Set (16 types)

| Category | Primitives | Count |
|----------|------------|-------|
| **Mutexes** | `SpinMutex`, `RawSpinMutex`, `NoopMutex` | 3 |
| **RwLocks** | `SpinRwLock`, `ReaderSpinRwLock`, `RawSpinRwLock`, `NoopRwLock` | 4 |
| **Once** | `Once`, `OnceLock`, `RawOnce` | 3 |
| **Atomics** | `AtomicCell`, `AtomicOption`, `AtomicLazy`, `AtomicFlag` | 4 |
| **Sync Helpers** | `SpinBarrier`, `SpinWait` | 2 |
| **Total** | | **16** |

---

## Quick Start

### Choosing the Right Primitive

| I need to... | Use this | Notes |
|--------------|----------|-------|
| Protect shared mutable data | `SpinMutex<T>` or `RawSpinMutex<T>` | Spin-based, no OS needed |
| Allow multiple readers OR one writer | `SpinRwLock<T>` or `RawSpinRwLock<T>` | **Writer-preferring** (default, balanced) |
| Multiple readers, **maximize read throughput** | `ReaderSpinRwLock<T>` | **Reader-preferring** (reads >95%, writes rare) |
| Initialize something once | `Once` or `OnceLock<T>` | Lazy static alternative |
| Store small atomic values | `AtomicCell<T>` | For `Copy` types ≤ pointer size |
| Atomically swap Option values | `AtomicOption<T>` | For pointer-sized types |
| Lazy value with custom init | `AtomicLazy<T, F>` | Once + value storage |
| Simple flag | `AtomicFlag` | Simpler than `AtomicBool` |
| Barrier synchronization | `SpinBarrier` | Wait for N threads |

### Poisoning vs Raw Variants

| Variant | When to Use | API |
|---------|-------------|-----|
| `SpinMutex<T>` | Production code, need panic safety | Returns `LockResult<Guard>` |
| `RawSpinMutex<T>` | Embedded, `panic = abort`, simpler API | Returns `Guard` directly |

**Rule of thumb**: Use poisoning variants unless you're in an embedded context where panics abort.

---

## Basic Examples

### SpinMutex (with poisoning)

```rust
use foundation_nostd::primitives::SpinMutex;

// Create a mutex protecting a counter
let counter = SpinMutex::new(0u32);

// Lock and modify
{
    let mut guard = counter.lock().unwrap();
    *guard += 1;
} // Lock released here when guard is dropped

// Try lock with spin limit (avoid infinite spin)
match counter.try_lock_with_spin_limit(1000) {
    Ok(guard) => println!("Got lock: {}", *guard),
    Err(_) => println!("Couldn't acquire lock after 1000 spins"),
}
```

### RawSpinMutex (without poisoning)

```rust
use foundation_nostd::primitives::RawSpinMutex;

let counter = RawSpinMutex::new(0u32);

// Simpler API - no Result to unwrap
let mut guard = counter.lock();
*guard += 1;
```

### SpinRwLock (Writer-Preferring)

```rust
use foundation_nostd::primitives::SpinRwLock;

let data = SpinRwLock::new(vec![1, 2, 3]);

// Multiple readers allowed
{
    let r1 = data.read().unwrap();
    let r2 = data.read().unwrap(); // Both can read simultaneously
    println!("Sum: {}", r1.iter().sum::<i32>());
}

// Writers get exclusive access
{
    let mut w = data.write().unwrap();
    w.push(4);
}
```

### ReaderSpinRwLock (Reader-Preferring)

```rust
use foundation_nostd::primitives::reader_spin_rwlock::ReaderSpinRwLock;

// Use when reads vastly outnumber writes (>95%)
let cache = ReaderSpinRwLock::new(vec![1, 2, 3]);

// Readers can acquire even if writer is waiting (maximizes read throughput)
{
    let r1 = cache.read().unwrap();
    let r2 = cache.read().unwrap();
    // ... many more readers
}

// Writers wait for all readers (may wait longer)
{
    let mut w = cache.write().unwrap();
    w.push(4);
}
```

**See [10-rwlock-policies.md](./10-rwlock-policies.md) for detailed policy comparison and when to use each.**

### Once and OnceLock

```rust
use foundation_nostd::primitives::{Once, OnceLock};

// One-time initialization (like lazy_static)
static INIT: Once = Once::new();
static mut CONFIG: Option<String> = None;

INIT.call_once(|| {
    unsafe { CONFIG = Some("initialized".to_string()); }
});

// Better: OnceLock for type-safe lazy init
static CONFIG2: OnceLock<String> = OnceLock::new();

let config = CONFIG2.get_or_init(|| "default".to_string());
```

### AtomicCell

```rust
use foundation_nostd::primitives::AtomicCell;

let cell = AtomicCell::new(42u32);

// Load/store
let value = cell.load();
cell.store(100);

// Atomic swap
let old = cell.swap(200);

// Compare and exchange
let result = cell.compare_exchange(200, 300);
```

---

## WASM Considerations

### Single-Threaded WASM (default)

Standard WASM builds are **single-threaded**. In this mode:
- `Mutex<T>` becomes `NoopMutex<T>` (no actual locking)
- No atomic operations needed
- Zero overhead

```rust
// Compiles to no-ops in single-threaded WASM
let m = Mutex::new(42);
let guard = m.lock().unwrap(); // No actual spinning
```

### Multi-Threaded WASM (SharedArrayBuffer)

When compiled with `target-feature = +atomics`:
- Real spin locks used
- Requires `--target-feature=+atomics,+bulk-memory,+mutable-globals`
- Browser must support SharedArrayBuffer

```bash
# Building for multi-threaded WASM
RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals' \
  cargo build --target wasm32-unknown-unknown
```

See [05-wasm-considerations.md](./05-wasm-considerations.md) for details.

---

## Performance Notes

### When to Use Spin Locks

✅ **Good for:**
- Very short critical sections (< 1μs)
- no_std / embedded environments
- WASM (no OS primitives available)
- When you can't block

❌ **Bad for:**
- Long critical sections (use OS locks instead)
- High contention scenarios
- When power consumption matters (spinning wastes CPU)

### Spin Count Limits

Always use `try_lock_with_spin_limit()` in production to avoid infinite spinning:

```rust
// Good: bounded spinning
match mutex.try_lock_with_spin_limit(10_000) {
    Ok(guard) => { /* use guard */ },
    Err(_) => { /* handle contention */ },
}

// Risky: unbounded spinning (only if you're sure contention is low)
let guard = mutex.lock().unwrap();
```

---

## Further Reading

| Document | Topic |
|----------|-------|
| [01-spin-locks.md](./01-spin-locks.md) | How spin locks work internally |
| [02-poisoning.md](./02-poisoning.md) | Lock poisoning explained |
| [03-atomics.md](./03-atomics.md) | Atomic operations deep dive |
| [04-memory-ordering.md](./04-memory-ordering.md) | Memory ordering semantics |
| [05-wasm-considerations.md](./05-wasm-considerations.md) | WASM-specific behavior |
| [06-usage-patterns.md](./06-usage-patterns.md) | Common patterns |
| [07-implementation-guide.md](./07-implementation-guide.md) | Library internals |
| [08-ordering-practical-guide.md](./08-ordering-practical-guide.md) | **Practical guide to Ordering** ⭐ |
| [09-unsafecell-guide.md](./09-unsafecell-guide.md) | **UnsafeCell deep dive** ⭐ |
| [10-rwlock-policies.md](./10-rwlock-policies.md) | **RwLock preference policies explained** ⭐ |

---

## API Reference Summary

### Mutex Types

| Type | Poisoning | API |
|------|-----------|-----|
| `SpinMutex<T>` | Yes | `lock() -> LockResult<Guard>` |
| `RawSpinMutex<T>` | No | `lock() -> Guard` |
| `NoopMutex<T>` | No | Same as Raw, but no-op |

### RwLock Types

| Type | Poisoning | Policy | Use When |
|------|-----------|--------|----------|
| `SpinRwLock<T>` | Yes | **Writer-preferring** | Default choice, balanced fairness |
| `ReaderSpinRwLock<T>` | Yes | **Reader-preferring** | Reads >95%, writes rare and can wait |
| `RawSpinRwLock<T>` | No | Writer-preferring | Embedded, panic=abort |
| `NoopRwLock<T>` | No | No-op | Single-threaded WASM |

### Once Types

| Type | Poisoning | Use Case |
|------|-----------|----------|
| `Once` | Yes | One-time side effects |
| `OnceLock<T>` | Yes | Lazy value initialization |
| `RawOnce` | No | Simple embedded use |

### Atomic Types

| Type | Size Limit | Operations |
|------|------------|------------|
| `AtomicCell<T>` | ≤ pointer size | load, store, swap, CAS |
| `AtomicOption<T>` | pointer size | take, swap, is_some |
| `AtomicLazy<T, F>` | any | get, force |
| `AtomicFlag` | 1 byte | set, clear, is_set |

---

*Next: [01-spin-locks.md](./01-spin-locks.md) - How spin locks work*
