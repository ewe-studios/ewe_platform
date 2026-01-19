---
completed: 10
uncompleted: 37
created: 2026-01-19
author: "Main Agent"
metadata:
  version: "2.2"
  last_updated: 2026-01-19
  total_tasks: 47
  completion_percentage: 21
tools:
  - Rust
  - cargo
skills: []
---

# WASM-Friendly Sync Primitives - Tasks

## Task List

### Module Setup
- [ ] Create `primitives/mod.rs` - Module entry with re-exports and WASM type aliases
- [ ] Create `primitives/poison.rs` - Poisoning error types
- [ ] Update `foundation_nostd/lib.rs` - Add `pub mod primitives`

### Poison Error Types (poison.rs)
- [ ] Define `PoisonError<T>` matching std API
- [ ] Define `TryLockError<T>` enum (Poisoned, WouldBlock)
- [ ] Define `LockResult<T>` and `TryLockResult<T>` type aliases
- [ ] Implement `Error` and `Display` traits

### SpinMutex - With Poisoning (spin_mutex.rs)
- [ ] Define `SpinMutex<T>` struct with `UnsafeCell<T>` and atomic state
- [ ] Define `SpinMutexGuard<'a, T>` with poisoning detection
- [ ] Implement `new()`, `lock()`, `try_lock()`, `try_lock_with_spin_limit()`
- [ ] Implement `is_poisoned()`, `into_inner()`, `get_mut()`
- [ ] Implement `Deref`, `DerefMut`, `Drop` for guard (with poison-on-panic)
- [ ] Implement `Send`, `Sync` bounds

### RawSpinMutex - Without Poisoning (raw_spin_mutex.rs)
- [ ] Define `RawSpinMutex<T>` struct - simpler, no poison tracking
- [ ] Define `RawSpinMutexGuard<'a, T>`
- [ ] Implement `new()`, `lock()`, `try_lock()`, `try_lock_with_spin_limit()`
- [ ] Implement `Deref`, `DerefMut`, `Drop` for guard
- [ ] Implement `Send`, `Sync` bounds

### SpinRwLock - With Poisoning (spin_rwlock.rs)
- [ ] Define `SpinRwLock<T>` with writer-preferring state encoding
- [ ] Define `SpinReadGuard<'a, T>` and `SpinWriteGuard<'a, T>`
- [ ] Implement `new()`, `read()`, `try_read()`, `write()`, `try_write()`
- [ ] Implement `try_read_with_spin_limit()`, `try_write_with_spin_limit()`
- [ ] Implement writer-preferring logic (pending writers block new readers)
- [ ] Implement `Deref` for both, `DerefMut` for write guard
- [ ] Implement `Drop` for guards with poison-on-panic

### RawSpinRwLock - Without Poisoning (raw_spin_rwlock.rs)
- [ ] Define `RawSpinRwLock<T>` - simpler, no poison tracking
- [ ] Define `RawReadGuard<'a, T>` and `RawWriteGuard<'a, T>`
- [ ] Implement same API as SpinRwLock but without poisoning
- [ ] Implement writer-preferring logic

### Once - With Poisoning (once.rs)
- [ ] Define `Once` struct with atomic state
- [ ] Define `OnceState` enum (Incomplete, Running, Complete, Poisoned)
- [ ] Implement `new()`, `call_once()`, `call_once_force()`, `is_completed()`

### OnceLock (once_lock.rs)
- [ ] Define `OnceLock<T>` container
- [ ] Implement `new()`, `get()`, `get_or_init()`, `get_or_try_init()`
- [ ] Implement `set()`, `into_inner()`

### RawOnce - Without Poisoning (raw_once.rs)
- [ ] Define `RawOnce` - simple once without poisoning
- [ ] Implement `new()`, `call_once()`, `is_completed()`

### Atomic Primitives (atomic_*.rs)
- [ ] Implement `AtomicCell<T>` with `load()`, `store()`, `swap()`, `compare_exchange()`
- [ ] Implement `AtomicOption<T>` with `take()`, `swap()`, `is_some()`, `is_none()`
- [ ] Implement `AtomicLazy<T, F>` with `get()`, `force()`
- [ ] Implement `AtomicFlag` with `set()`, `clear()`, `is_set()`

### Synchronization Helpers
- [ ] Implement `SpinBarrier` with `wait()` returning `BarrierWaitResult`
- [ ] Implement `SpinWait` with exponential backoff (`spin()`, `reset()`)

### WASM Optimization (noop.rs)
- [ ] Implement `NoopMutex<T>` for single-threaded WASM
- [ ] Implement `NoopRwLock<T>` for single-threaded WASM
- [ ] Implement `NoopOnce` for single-threaded WASM
- [ ] Add `#[cfg]` gates for WASM detection
- [ ] Create type aliases in mod.rs for automatic selection

### Testing
- [ ] Unit tests for SpinMutex (lock, try_lock, poisoning)
- [ ] Unit tests for RawSpinMutex (no poisoning)
- [ ] Unit tests for SpinRwLock (read, write, writer-preferring)
- [ ] Unit tests for Once (call_once, poisoning)
- [ ] Unit tests for AtomicCell, AtomicOption
- [ ] Unit tests for try_lock_with_spin_limit

### Fundamentals Documentation
- [x] Write `00-overview.md` - Introduction and primitive selection guide
- [x] Write `01-spin-locks.md` - Spin lock implementation deep dive
- [x] Write `02-poisoning.md` - Poisoning mechanism explained
- [x] Write `03-atomics.md` - Atomic operations and types
- [x] Write `04-memory-ordering.md` - Memory ordering deep dive
- [x] Write `05-wasm-considerations.md` - WASM threading and optimization
- [x] Write `06-usage-patterns.md` - Patterns and best practices
- [x] Write `07-implementation-guide.md` - Library internals and design
- [x] Write `08-ordering-practical-guide.md` - Practical guide to using Ordering correctly
- [x] Write `09-unsafecell-guide.md` - UnsafeCell purpose, patterns, and pitfalls

## Implementation Order

1. **poison.rs** - Error types first (dependency for poisoning locks)
2. **raw_spin_mutex.rs** - Simplest lock, foundation for others
3. **spin_mutex.rs** - Add poisoning on top of raw
4. **raw_spin_rwlock.rs** - RwLock without poisoning
5. **spin_rwlock.rs** - Add poisoning
6. **raw_once.rs** - Simple once
7. **once.rs** - Once with poisoning
8. **once_lock.rs** - Container using Once
9. **atomic_cell.rs** - Generic atomic wrapper
10. **atomic_option.rs** - Atomic Option
11. **atomic_lazy.rs** - Lazy initialization
12. **atomic_flag.rs** - Simple flag
13. **barrier.rs** - Barrier synchronization
14. **spin_wait.rs** - Backoff helper
15. **noop.rs** - WASM no-op variants
16. **mod.rs** - Re-exports and type aliases
17. **Tests** - All unit tests
18. **Fundamentals** - All documentation

## Notes

### Atomic State Encoding (SpinMutex)
```rust
// Simple state: 0 = unlocked, 1 = locked
// With poisoning: Bit 0 = locked, Bit 1 = poisoned
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;
const LOCKED_POISONED: u8 = 0b11;
```

### Writer-Preferring RwLock State
```rust
// Bits 0-29: Reader count
// Bit 30: Writer waiting
// Bit 31: Writer active
const WRITER_WAITING: u32 = 1 << 30;
const WRITER_ACTIVE: u32 = 1 << 31;
const READER_MASK: u32 = (1 << 30) - 1;
```

### WASM Type Alias Pattern
```rust
// In mod.rs:
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
pub type Mutex<T> = NoopMutex<T>;

#[cfg(any(not(target_arch = "wasm32"), target_feature = "atomics"))]
pub type Mutex<T> = SpinMutex<T>;
```

### SpinWait Backoff Pattern
```rust
pub struct SpinWait {
    counter: u32,
}

impl SpinWait {
    pub fn spin(&mut self) -> bool {
        if self.counter < 10 {
            // Fast path: just spin_loop hint
            for _ in 0..(1 << self.counter) {
                core::hint::spin_loop();
            }
            self.counter += 1;
            true
        } else {
            false // Caller should yield or block
        }
    }
}
```

---
*Last Updated: 2026-01-19*
