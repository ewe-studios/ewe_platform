# Debugging Multi-Pool Test Hangs — A Complete Field Guide

This is a war-story walkthrough of a real bug hunt: the valtron `multi`
executor's test suite would intermittently hang with tests "running for over
60 seconds." It teaches you both **what was wrong** and **how to debug this
class of problem yourself** — especially how to get a thread dump out of a
hung Rust test when the obvious tools fight you.

Read this end-to-end the first time. Afterwards it doubles as a checklist.

---

## 1. The symptom

Running the test suite, some runs passed, some hung:

```
test valtron::multi_workers::can_queue_and_complete_task has been running for over 60 seconds
test valtron::sync_boundary_helpers::test_sync_one_single_value_task has been running for over 60 seconds
test valtron::thread_yielder_tests::multiple_delayed_tasks_shutdown_quickly has been running for over 60 seconds
... (8+ tests listed)
```

Two properties made this nasty:

1. **Flaky** — runs 1–3 pass in ~12s, runs 4–5 hang. Same binary, same input.
2. **The blocker rotated** — sometimes `can_queue_and_complete_task` was first
   to hang, sometimes `can_finish_even_when_task_panics`. The *other* listed
   tests were just queued behind whichever one was truly stuck.

An earlier incarnation of the same area produced a different surface error:

```
panicked at .../multi/mod.rs:72: called `Result::unwrap()` on an `Err` value: PoisonError { .. }
```

Both symptoms share one root: **a process-global singleton pool that more than
one test tries to own at once.**

---

## 2. The mental model you need first

The `multi` executor does **not** create a pool per call. It stores ONE pool in
process-global statics:

```rust
static REGISTRY: Mutex<Option<Arc<ThreadRegistry>>> = Mutex::new(None);
static BG_REGISTRY: Mutex<Option<Arc<BackgroundJobRegistry>>> = Mutex::new(None);
```

`initialize_pool()` fills them; `PoolGuard::drop` (or `block_on` returning)
clears them. Consequences:

- **Only one pool may be alive per process at a time.**
- `cargo test` runs a test binary with **N worker threads** (default = CPU
  count). Many of those threads each try to stand up the global pool.
- Therefore pool-using tests **cannot truly run in parallel** — they must be
  serialized somehow, and any worker thread that outlives its pool's teardown
  corrupts the next pool.

Hold this model. Every bug below is a different way that invariant got violated.

---

## 3. The bugs, in the order we found them

We fixed FOUR distinct problems. The hang only fully disappeared once all four
were addressed, which is exactly why it was confusing — fixing one made it
"better but still flaky," tempting you to think you were done.

### Bug A — `PoisonError` cascade across tests

`REGISTRY.lock().unwrap()` everywhere. When any thread panicked while holding
that mutex, the mutex became **poisoned**, and every later `.lock().unwrap()`
in every later test panicked too. One genuine failure → a wall of unrelated
"failures."

**Fix:** recover the guard instead of unwrapping:

```rust
let mut reg = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
```

Lesson: for process-global locks that must survive test panics, never
`.unwrap()` a `LockResult`. Poison recovery (`into_inner`) keeps the next test
alive.

### Bug B — duplicate test registration via stacked attributes

A test was written as:

```rust
#[test]                       // built-in
#[serial]                     // proc-macro
#[traced_test]                // proc-macro
#[valtron_test(threads = 2)]  // proc-macro — emits ITS OWN #[test]
fn my_test() { ... }
```

`#[valtron_test]` is like `#[tokio::test]`: it generates a `#[test]` wrapper.
With an explicit `#[test]` ALSO present, the function got registered **twice**.
The two registrations ran **concurrently**, both grabbing the one global pool —
instant race.

We tried to make the macro defend itself by stripping a caller-supplied
`#[test]`. It only half-works, and understanding why teaches you how attribute
macros expand:

> **Attribute proc-macros expand OUTERMOST-first.** `#[serial]` sits above
> `#[valtron_test]`, so it runs first and re-emits the function with the
> built-in `#[test]` hoisted into a wrapper layer — *before* `#[valtron_test]`
> ever sees the token stream. So `#[valtron_test]`'s filter (which removes a
> directly-adjacent `#[test]`) finds nothing to remove, then adds its own. Two
> `#[test]`s again.

**Fix (the real one):** a usage contract, documented in
`foundation_macros/src/valtron_entry.rs`:

- `#[valtron_test]` **replaces** `#[test]`. Never write both.
- Never add `#[serial]` either (see Bug C — the pool gate serializes for you).
- Non-test attributes like `#[traced_test]` are fine.

Lesson: when a macro "should have cleaned that up but didn't," check attribute
**ordering**. An outer macro can transform the tokens before your macro runs.

### Bug C — unfair serialization → starvation that looks like deadlock

To stop tests clobbering each other's global pool, `initialize_pool` acquires a
process-wide lock held for the PoolGuard's entire lifetime. First attempt:

```rust
static POOL_LIFECYCLE_LOCK: Mutex<()> = Mutex::new(());
```

This serialized correctly but **starved** waiters. `std::sync::Mutex` is
**not fair**: under heavy contention (8 cargo threads all hammering it) a
waiter can be skipped indefinitely while others keep re-acquiring. A test that
was "next" could wait past 60s even though every individual test is fast.

**Fix:** a FIFO-fair ticket gate — `FairGate` in `multi/mod.rs`. Each caller
takes a monotonically increasing ticket and parks on a condvar until
`now_serving == my_ticket`; the guard's `Drop` increments `now_serving` and
`notify_all()`s. Arrival order is preserved, no busy-waiting, no starvation:

```rust
struct FairGate { next_ticket: AtomicUsize, now_serving: AtomicUsize,
                  inner: Mutex<()>, cond: Condvar }
fn acquire(&'static self) -> FairGateGuard {
    let my = self.next_ticket.fetch_add(1, SeqCst);
    let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
    while self.now_serving.load(SeqCst) != my {
        g = self.cond.wait(g).unwrap_or_else(|p| p.into_inner());
    }
    FairGateGuard { gate: self }
}
// Drop: lock, now_serving += 1, cond.notify_all()
```

This gate ALSO removed the need for `#[serial]` entirely and made a previously
considered "generation counter" unnecessary: because only one pool is ever
alive (the gate guarantees it), a stale guard can never coexist with a newer
pool, so it can never clear a registry it no longer owns.

Lesson: "serialized but still hangs under load" with no lock CYCLE is the
signature of **starvation on an unfair lock**, not a classic deadlock. The fix
is fairness, not more locks.

A second, subtler consequence: timing-assertion tests (e.g.
`multiple_delayed_tasks_shutdown_quickly` asserts shutdown is `< 2s`) started
`Instant::now()` *before* `initialize_pool`. Now that `initialize_pool` blocks
on the gate, queue-wait time counted against the assertion. **Fix:** start the
timer AFTER `initialize_pool` returns.

### Bug D — the real deadlock: `block_on` waits on workers nothing ever kills

This is the one the thread dump nailed. Two tests used `block_on` with a
"kill handler" thread structured like this (BROKEN):

```rust
#[test]
#[serial]
fn can_queue_and_complete_task() {
    let handler_kill = thread::spawn(move || {
        receiver.recv_timeout(2s).unwrap();
        get_pool().kill();              // ← reads the GLOBAL pool
    });
    let _guard = block_on(seed, None, |pool| {
        pool.spawn()...schedule();
        task_sent_sender.send(());
        task_sent_receiver.recv_timeout(2s);   // setup returns here
    });                                 // block_on now calls waitgroup().wait()
    handler_kill.join();                // ← never reached
}
```

Why it deadlocks: `block_on(setup)` runs `setup`, then internally calls
`guard.waitgroup().wait()` — blocking until all worker threads finish. Workers
finish only after `kill()`. But `kill()` runs in `handler_kill`, which we only
`join()` AFTER `block_on` returns. So:

- `block_on` is parked in `waitgroup().wait()` waiting for workers to die,
- workers are parked in `yield_for` waiting for a kill that hasn't come,
- the killer thread may fire, but if it calls `get_pool()` after the guard has
  started clearing the registry it hits an empty/`PoisonError` registry.

**Fix:** spawn the kill handler and `join()` it INSIDE the `setup` closure,
using a **cloned pool handle** (never `get_pool()`):

```rust
#[traced_test]                          // NO #[test], NO #[serial]
fn can_queue_and_complete_task() {
    block_on(seed, None, |pool| {
        let pool_clone = pool.clone();
        let handler_kill = thread::spawn(move || {
            receiver.recv_timeout(2s).unwrap();
            pool_clone.kill();          // cloned handle, not the global
        });
        pool.spawn()...schedule();
        handler_kill.join().unwrap();   // kill happens BEFORE setup returns
    });                                 // → waitgroup().wait() returns fast
    assert_eq!(...);
}
```

Lesson: with `block_on`, anything that must influence shutdown has to happen
**before the setup closure returns**, because `block_on`'s `waitgroup().wait()`
runs immediately after. And inside a pool closure, use the `pool` you were
handed, not the global accessor.

### Bonus bug — `WaitGroup` lost-wakeup

While reading the dump we also found a latent race in `WaitGroup` (used by
`block_on`'s shutdown). Old design: atomic count + a separate `LockSignal`.

```rust
fn wait(&self) {
    loop {
        if self.count.load() == 0 { return; }
        self.signal.lock_and_wait();   // ← RACE WINDOW between check and park
    }
}
```

If the last `done()` fired its signal in the gap between the `count == 0` check
and `lock_and_wait()`, the waiter parked and **missed the wakeup** forever.
Classic lost-wakeup; only reproduced under tight timing (which the global-pool
serialization made tighter).

**Fix:** put the count INSIDE the mutex and check-then-park atomically — the
textbook condvar pattern:

```rust
struct WaitGroup { inner: Arc<(Mutex<usize>, Condvar)> }
fn wait(&self) {
    let (lock, cond) = &*self.inner;
    let mut count = lock.lock().unwrap_or_else(|p| p.into_inner());
    while *count != 0 { count = cond.wait(count).unwrap_or_else(|p| p.into_inner()); }
}
fn done(&self) {
    let (lock, cond) = &*self.inner;
    let mut count = lock.lock().unwrap_or_else(|p| p.into_inner());
    *count -= 1;
    if *count == 0 { cond.notify_all(); }   // notify UNDER the lock
}
```

Lesson: any "check a condition, then sleep" across threads must do both under
the same lock the notifier takes, or you can lose the wakeup. `Condvar::wait`
in a `while !predicate` loop is the only safe shape.

---

## 4. How to debug this yourself — the methodology

The order here matters. Each step is cheap and rules out a whole category.

### Step 1 — Reproduce deterministically enough

Flaky ≠ unreproducible. Loop the suite and count:

```bash
TESTBIN=$(ls -t target/uat/deps/mod-* | grep -v '\.d$' | head -1)
for i in $(seq 1 10); do
  timeout 60 "$TESTBIN" --test-threads=8 >/tmp/r.log 2>&1
  [ $? -eq 124 ] && echo "run $i: HANG" || echo "run $i: ok"
  pkill -9 -f "$(basename "$TESTBIN")" 2>/dev/null
done
```

If 2/10 hang, that's plenty. Note WHICH test is named first in the hang — but
remember it may just be queued behind the real culprit.

### Step 2 — Test the suspect in ISOLATION

```bash
for i in $(seq 1 15); do
  timeout 20 "$TESTBIN" --test-threads=1 --exact valtron::multi_workers::can_finish_even_when_task_panics \
    >/tmp/x.log 2>&1 || echo "iter $i HANG"
done
```

If it passes alone 15/15 but hangs in the suite, the bug is an **interaction
between consecutive pool lifecycles**, not the test itself. That single fact
eliminated "the test logic is wrong" and pointed us at shared global state.

### Step 3 — Resist print-debugging; it lies here

`libtest` captures each test's stderr and only flushes on completion. A hung
test prints **nothing**. Adding `--nocapture` flushes live — BUT the extra I/O
**perturbs timing** and made our Heisenbug vanish (8/8 passed under
`--nocapture`). If your bug is timing-sensitive, prints can hide it. Go
straight to a thread dump.

### Step 4 — Get a thread dump (the key skill)

You want "what is every thread blocked on, right now." On Linux that's gdb's
`thread apply all bt`. The catch:

> **`ptrace_scope = 1`** (`cat /proc/sys/kernel/yama/ptrace_scope`) forbids
> `gdb -p <pid>` from attaching to a process you didn't launch as a direct
> child — even your own background job, because its worker threads aren't gdb's
> children. You'll get an empty/`ptrace: Operation not permitted` dump.

Two ways around it:

**(a) Relax ptrace (needs sudo):**
```bash
echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
# then: gdb -p <pid> -batch -ex 'set pagination off' -ex 'thread apply all bt'
```

**(b) Launch the test UNDER gdb so it's a child (no sudo) — what we used:**
```bash
TESTBIN=target/uat/deps/mod-<hash>
gdb -batch \
  -ex 'set pagination off' \
  -ex 'set debuginfod enabled off' \
  -ex 'handle SIGINT stop print nopass' \
  -ex 'run --test-threads=8' \
  -ex 'thread apply all bt' \
  --args "$TESTBIN" --test-threads=8 >/tmp/gdb.log 2>&1 &
GP=$!
sleep 50                                   # let it reach the hang
INF=$(pgrep -f "$(basename "$TESTBIN")" | head -1)
kill -INT "$INF"                           # gdb stops on SIGINT, runs the queued bt
sleep 8
pkill -9 -f "$(basename "$TESTBIN")"
wait $GP
```

Key flags explained:
- `handle SIGINT stop print nopass` — make gdb HALT when the inferior gets
  SIGINT (instead of passing it through). This is what lets a `kill -INT` from
  outside trigger the queued `thread apply all bt`. If you use
  `nostop ... pass`, the signal sails through and you get no dump (we hit
  exactly that and got 0 frames the first time).
- `set debuginfod enabled off` — skips slow/interactive debuginfo downloads.
- `set pagination off` — don't block waiting for "press enter."

> Caveat: running under gdb slows execution and can itself mask a timing race
> (our first gdb run completed cleanly). RETRY in a loop until it hangs under
> gdb, then dump. Be suspicious if you pass `--test-threads=8` but gdb's
> overhead effectively serializes — confirm you actually see 8 worker threads
> in the dump.

### Step 5 — Read the dump

Grep for threads parked in YOUR code, not libc:

```bash
grep -nE "futex|Condvar|park|Mutex|RwLock|wait_timeout|<crate>::" /tmp/gdb.log
grep -nE "^Thread [0-9]+ " /tmp/gdb.log     # thread headers (names!)
```

What the dump told us, frame by frame, on the stuck worker thread:

```
#5  foundation_core::synca::waitgroup::WaitGroup::wait              ← parked here
#6  foundation_core::valtron::executors::multi::block_on (mod.rs:238)
#7  mod::valtron::multi_workers::can_queue_and_complete_task::{closure#1}
#9  serial_test::serial_code_lock::local_serial_core               ← STILL has #[serial]!
#10 mod::valtron::multi_workers::can_queue_and_complete_task
```

Two diagnoses from ONE backtrace:
- Frame #5–#6: blocked in `block_on`'s `waitgroup().wait()` → **the pool was
  never killed** (Bug D).
- Frame #9 `serial_test::serial_code_lock` → the test **still carried
  `#[serial]`** even though we thought we'd removed it (a revert had crept in).

Thread NAMES are gold: worker threads are named after the test fn (e.g.
`valtron::multi_`). Seeing old-pool worker threads still alive while new test
threads wait at `POOL_LIFECYCLE_GATE`'s condvar proved workers were outliving
their pool's teardown.

### Step 6 — Fix, then re-loop to prove it

Re-run Step 1's loop 10×. "Probably fixed" is not fixed for a Heisenbug; you
want 10/10 green across multiple full-suite runs before believing it.

---

## 5. The rules that fell out (keep these)

1. `#[valtron_test]` **replaces** `#[test]`. Never stack `#[test]` or
   `#[serial]` on it. Non-test attrs (`#[traced_test]`) are fine.
2. Don't use `#[serial]` for pool tests at all — `initialize_pool`'s FairGate
   serializes them process-wide and fairly.
3. In a `block_on` setup closure, use the `pool` handle you're given. Never
   call `get_pool()` from a thread that may outlive the guard.
4. Anything affecting pool shutdown must happen BEFORE the `block_on` setup
   closure returns (because `waitgroup().wait()` runs right after).
5. Time shutdown-speed assertions from AFTER `initialize_pool` returns.
6. Process-global locks: recover from poison (`unwrap_or_else(|p| p.into_inner())`),
   never `.unwrap()`.
7. Cross-thread "check then sleep": check and park under the same lock the
   notifier uses; `Condvar::wait` inside `while !predicate`.

## 6. Where the code lives

- Pool, statics, `FairGate`, `initialize_pool`, `block_on`, `PoolGuard`:
  `backends/foundation_core/src/valtron/executors/multi/mod.rs`
- `WaitGroup`: `backends/foundation_core/src/synca/waitgroup.rs`
- `#[valtron_test]` macro + the attribute contract:
  `backends/foundation_macros/src/valtron_entry.rs`
- Example tests (correct patterns):
  `backends/foundation_core/tests/valtron/multi_workers.rs`,
  `.../shutdown_mechanism_tests.rs`, `.../thread_yielder_tests.rs`
