# Fundamentals 02 — Sync primitives for wasm and native

Zero-to-expert on the synchronization layer. If you're sharing state between
tasks, building channels, or implementing concurrent data structures, you need
these.

---

## 1. The wasm constraint

On `wasm32-unknown-unknown` there are **no OS threads**. The `std::sync` types
(Mutex, Condvar, etc.) either panic or silently do nothing. The platform's sync
primitives must work identically on:
- **Native** (Linux/macOS/Windows) — real OS threads, atomic operations
- **Emscripten** — emulated threads via pthreads
- **Wasm** — single-threaded, event-loop driven

The solution: two implementations behind the same API, selected by cfg.

## 2. Atomic primitives

### AtomicCell<T>
A lock-free cell for any `Copy` type up to word size:

```rust
let cell = AtomicCell::new(42u64);
cell.store(100);
assert_eq!(cell.load(), 100);
```

Uses `AtomicU64`/`AtomicU32` on native, `UnsafeCell` on wasm (single-threaded
→ no races possible).

### AtomicFlag
A single-bit atomic boolean. Used for:
- "Is generation in progress?" flags
- Shutdown signals
- One-time initialization guards

```rust
let flag = AtomicFlag::new();
assert!(!flag.test_and_set());  // returns false (was clear)
assert!(flag.test());           // now set
flag.clear();
```

## 3. Locking primitives

### SpinMutex<T>
Busy-wait mutex optimized for short critical sections:

```rust
let mutex = SpinMutex::new(vec![1, 2, 3]);
let mut guard = mutex.lock();
guard.push(4);
drop(guard); // releases the lock
```

**Not for long operations.** A spin mutex consumes 100% CPU while waiting. Use
for: protecting small data structures, fast counter updates, flag setting.

### SpinRwLock<T>
Reader-writer lock with spin-wait. Multiple readers OR one writer:

```rust
let lock = SpinRwLock::new(data);
let read_guard = lock.read();   // multiple readers allowed
let write_guard = lock.write(); // exclusive access
```

**Poisoning**: Like `std::sync::RwLock`, if a writer panics while holding the
lock, the lock becomes poisoned. Subsequent `write()` calls return `Err`.
`read()` on a poisoned lock still works (returns the data).

## 4. Condition variables

### Condvar
Blocks a thread until notified. Used with a Mutex:

```rust
let mutex = Mutex::new(false);
let condvar = Condvar::new();

// Writer thread
{
    let mut guard = mutex.lock().unwrap();
    *guard = true;
    condvar.notify_one();
}

// Reader thread
{
    let mut guard = mutex.lock().unwrap();
    while !*guard {
        guard = condvar.wait(guard).unwrap();
    }
    // guard is true
}
```

On wasm, `wait()` yields to the event loop instead of blocking (cooperative
waiting).

## 5. Barriers and WaitGroups

### Barrier
Blocks N threads until all have arrived:

```rust
let barrier = Barrier::new(3);
// Thread 1: barrier.wait();
// Thread 2: barrier.wait();
// Thread 3: barrier.wait(); // all three unblock simultaneously
```

### WaitGroup
Wait for N asynchronous tasks to complete:

```rust
let wg = WaitGroup::new();
for i in 0..5 {
    let wg = wg.clone();
    spawn(async move {
        do_work(i).await;
        wg.done();
    });
}
wg.wait().await; // blocks until all 5 tasks call done()
```

## 6. MPMC Channel

Multi-producer, multi-consumer channel with seekable cursors:

```rust
let (tx, rx) = mpp::channel::<String>(100);
tx.send("hello".into());
let msg = rx.recv().await;
```

Features:
- **AtomicU64 cursors** — readers can seek to any position in the stream
- **Broadcaster** — one producer, many consumers (each gets a copy)
- **FairGate** — serializes access to prevent starvation

## 7. Idleman

Detects when all tasks have quiesced (no progress being made):

```rust
let idleman = Idleman::new();
let ticket = idleman.register();
// ... do work ...
ticket.done();
idleman.wait(); // blocks until all tickets are done
```

Used by the agentic loop to detect when memory generation should fire (all
generators have finished, no more tokens being consumed).

## 8. Target-specific behavior

| Primitive | Native (multi) | Wasm (single) |
|---|---|---|
| AtomicCell | `AtomicU64` | `UnsafeCell` (no races) |
| SpinMutex | CAS spin loop | `RefCell` borrow check |
| SpinRwLock | RwLock spin | `RefCell` read/write borrow |
| Condvar | `park_thread` | event-loop yield |
| Barrier | thread park/unpark | counter + yield |
| WaitGroup | atomic counter | atomic counter |

The `multi` feature enables the native implementations. Without it, wasm-safe
fallbacks are used everywhere.

## 9. SendWrapper in practice

`SendWrapper<T>` lets `!Send` values cross `Send` boundaries on single-threaded
wasm:

```rust
// This compiles on wasm but NOT on native:
fn takes_send<T: Send>(_: T) {}
takes_send(SendWrapper::new(js_value)); // OK on wasm, compile error on native
```

**Use cases**:
- Wrapping `JsFuture` for valtron execution
- Wrapping JS objects that need to be stored in `Arc`
- Bridging `!Send` WASM APIs to `Send`-requiring traits

**Never use on native** — if `SendWrapper` compiles on native, the wrapped type
was already `Send`, making the wrapper pointless.
