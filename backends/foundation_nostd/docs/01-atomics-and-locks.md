# Fundamentals 01 — Atomics and spin locks in no_std

Zero-to-expert on atomic operations and lock-free synchronization without `std`.

---

## 1. Why no_std atomics

In `no_std` environments (embedded, wasm, bare-metal), `std::sync` is not
available. But concurrent access to shared state still needs to be safe. The
primitives in `foundation_nostd` provide atomic operations using:
- **`core::sync::atomic`** on targets that support it (most)
- **`UnsafeCell`** on single-threaded targets (wasm32-unknown-unknown)

## 2. AtomicCell<T>

A lock-free cell for any `Copy` type:

```rust
#[repr(transparent)]
pub struct AtomicCell<T: Copy> {
    value: UnsafeCell<T>,
}
```

On targets with atomic support, this uses `AtomicU8`/`AtomicU16`/`AtomicU32`/
`AtomicU64` depending on the size of `T`. On single-threaded wasm, it's just
an `UnsafeCell` — no races possible on a single thread.

### Supported sizes
- 1 byte → `AtomicU8`
- 2 bytes → `AtomicU16`
- 4 bytes → `AtomicU32`
- 8 bytes → `AtomicU64`
- Larger → `spin_mutex` fallback (not lock-free)

### Operations
```rust
let cell = AtomicCell::new(42u64);
cell.store(100);
assert_eq!(cell.load(), 100);
cell.swap(200); // atomic swap, returns old value
cell.compare_and_swap(200, 300); // CAS
```

## 3. AtomicFlag

A single-bit atomic boolean:

```rust
pub struct AtomicFlag {
    state: AtomicU8,
}
```

States: `0` (clear), `1` (set). Operations:
- **`test()`** — check if set (atomic load)
- **`test_and_set()`** — atomically set, return previous state
- **`clear()`** — clear the flag

Uses: shutdown signals, "generation in progress" guards, one-time init markers.

## 4. SpinMutex<T>

A busy-wait mutex:

```rust
pub struct SpinMutex<T> {
    state: AtomicU8, // 0 = unlocked, 1 = locked
    data: UnsafeCell<T>,
}
```

**Locking**: Atomically CAS state from 0 to 1. If CAS fails, spin (yield on
wasm, pause instruction on x86) and retry.

**Not for long operations**: A spin mutex burns 100% CPU while waiting. Use only
for:
- Protecting small data structures (counters, flags, small arrays)
- Very short critical sections (a few instructions)
- Cases where you know contention will be rare

### Guard behavior
```rust
let mutex = SpinMutex::new(vec![1, 2, 3]);
{
    let mut guard = mutex.lock(); // blocks until acquired
    guard.push(4);
    // guard drops here → releases lock (atomic store 0)
}
```

The guard implements `Deref` and `DerefMut` for transparent access to the inner
value.

## 5. SpinRwLock<T>

A reader-writer spin lock:

```rust
pub struct SpinRwLock<T> {
    state: AtomicU32, // bits 0..30: reader count, bit 31: writer flag
    data: UnsafeCell<T>,
}
```

**State encoding**:
- `state == 0` → unlocked
- `state & WRITER_MASK != 0` → writer holds the lock
- `state & READER_MASK > 0` → N readers hold the lock

**Read lock**: Spin while writer holds. When free, increment reader count.
**Write lock**: Spin while any reader or writer holds. When free, set writer bit.

### Poisoning
Like `std::sync::RwLock`, if a writer panics while holding the lock, the lock
becomes poisoned. Subsequent `write()` calls return `Err(PoisonError)`.
`read()` on a poisoned lock still works (returns `Ok` with the data).

```rust
let lock = SpinRwLock::new(data);
match lock.write() {
    Ok(mut guard) => { /* exclusive access */ }
    Err(poisoned) => { lock was poisoned; poisoned.into_inner() to recover }
}
```

## 6. PoisonError and LockResult

All lock operations return `LockResult<T>`:

```rust
pub type LockResult<T> = Result<T, PoisonError<T>>;
```

`PoisonError<T>` lets you recover the inner value:

```rust
fn handle_poisoned<T>(result: LockResult<T>) -> T {
    match result {
        Ok(value) => value,
        Err(poisoned) => {
            log::warn!("lock was poisoned, recovering");
            poisoned.into_inner()
        }
    }
}
```

## 7. Memory ordering

Atomic operations use `Ordering` to specify visibility guarantees:

| Ordering | Guarantees | Use case |
|---|---|---|
| `Relaxed` | Atomicity only | Counters, flags |
| `Acquire` | No later op reordered before | Lock acquisition |
| `Release` | No earlier op reordered after | Lock release |
| `AcqRel` | Both acquire and release | Read-modify-write |
| `SeqCst` | Total ordering across all ops | Fences, consensus |

**Rule of thumb**: Use `Relaxed` for simple counters, `Acquire`/`Release` for
locks and synchronization points, `SeqCst` only when you need a global ordering
(most code doesn't).

## 8. Wasm considerations

On `wasm32-unknown-unknown`:
- **No threads** → `AtomicCell` is just `UnsafeCell`, `SpinMutex` is `RefCell`
- **No spinning** → `wait()` yields to the event loop
- **No poisoning** → no threads to panic while holding a lock

The `std` feature adds `std::sync` wrappers on targets where `std` is available.
