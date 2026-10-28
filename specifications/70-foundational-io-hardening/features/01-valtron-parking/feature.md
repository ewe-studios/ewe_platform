---
feature: "Valtron parking: ThreadedIterFuture and the first empty pull"
status: "done"
priority: "critical"
depends_on: ["PR #160"]
branch: "fix/io-01-valtron-parking"
crates: ["foundation_core"]
backlog: "docs/bug-backlog.md §4: ThreadedIterFuture busy-polls; A stream's first empty pull still blocks once"
starting_point: "fix/foundational-readiness commit fcbf523fa (work in progress; one test hangs)"
---

# Feature 01: Valtron parking

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); first in, first out (FIFO); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Two valtron paths waste a thread while they wait. After this feature both park and are woken.

1. **`ThreadedIterFuture` (feature `multi`)** runs a `!Send` future on a background thread. It
   polls the future with a no-op waker and calls `std::thread::yield_now()` on `Pending`, so the
   thread spins at 100 % of a core. When the result queue is full, it `thread::sleep`s (or
   `spin_loop`s). It ignores the result of `worker_queue.push(Err(e))`.
2. **The first empty pull blocks once.** `NotifyQueueStreamIterator::next`
   (`executors/local.rs`) only stops blocking after a consumer registered a wake. So the first
   pull on an empty queue by an async consumer (`.values()`, `TaskStatus::from_stream`,
   `into_future_stream`) still blocks the polling thread for `max_spins` timed waits.

## 2. Starting point (already written, not finished)

Commit `fcbf523fa` on branch `fix/foundational-readiness` has a first version. Cherry-pick only
its `foundation_core` parts:

| File | What the work in progress does |
|---|---|
| `src/valtron/executors/future_task.rs` | `ThreadUnpark` waker (unparks the worker thread); `Poll::Pending => thread::park()`; `push_waiting` parks a producer on a full queue until space or close; the `push(Err)` result is handled |
| `src/valtron/executors/local.rs` | `NotifyQueue` gets a `space: WakeRegistry`; `pop` fires it; `register_space_wake`; `close` fires it |
| `src/valtron/stream_future.rs` | Each async lens (`StreamAsFutureStream` and the others) has a `primed` flag; the first poll calls `prime(iter, cx)`, which registers a no-op wake so a queue-backed stream answers an empty pull with `Wait` at once |
| `src/valtron/task.rs` | small support change |
| `tests/valtron/threaded_future.rs`, `tests/valtron/wakeable_stream.rs` | new regression tests |

**Known problem:** with these changes, `tests/valtron/cross_runtime.rs`
`smol_drives_parked_future_task` (line ~299) hangs. `tokio_drives_parked_future_task` and
`valtron_drives_parked_future_task` must also be checked.

Leave out the non-`foundation_core` files of `fcbf523fa` (`foundation_netio` body reader,
`foundation_connectrpc` dispatch). They belong to feature 03.

## 3. Design

### 3.1 `ThreadedIterFuture`

- The worker thread's waker unparks the worker thread (`std::task::Wake` on an
  `Arc<ThreadUnpark(Thread)>`).
- On `Poll::Pending` the worker calls `std::thread::park()`. A spurious unpark is harmless: the
  loop polls again.
- A full result queue: register a space wake that unparks this thread, then `park()` (or
  `park_timeout(max_wait)` when a back-pressure wait is configured). On `PushError::Closed`
  the consumer is gone: stop and drop the value, with a `tracing::debug!`.
- No `yield_now`, no `spin_loop`, no `thread::sleep` in this file.

### 3.2 The first empty pull

The async lens must tell the stream "an async consumer is pulling" **before** the first pull,
so the stream answers an empty queue with `Wait` instead of blocking. The work in progress does
this with a no-op `wake` registration on the first poll (`prime`).

**Correctness rule the fix must keep:** after the async consumer receives `Wait`, a real wake
(one that wakes `cx.waker()`) must be registered **before** the consumer returns `Pending`, and
an item pushed between the empty pull and that registration must not be lost. Check this
against `WakeRegistry` semantics: is a registration one-shot? Does a later `register` replace an
earlier one, or are both kept? Does `fire` with no registration keep a "fired" latch for a later
`register`? The hang is most likely here: the no-op wake takes the one-shot slot or clears a
latch, so the real wake never fires.

### 3.3 Diagnose the hang deterministically

1. On the branch without the work in progress (base), run the three `*_drives_parked_future_task`
   tests 20 times each. They must pass.
2. Apply the work-in-progress hunks one file at a time (`future_task.rs`, then `local.rs`, then
   `stream_future.rs`). After each, run the three tests 20 times. The first file after which a
   test hangs holds the bug.
3. Add `tracing::trace!` at wake register, wake fire and park, run the hanging test with
   `RUST_LOG=trace`, and read the order of events. Remove the extra trace lines before the
   commit, or keep them at `trace` level only.
4. Fix the root cause. Do not work around it with a timeout or a periodic wake.

## 4. Tasks

- [x] T1. Create `fix/io-01-valtron-parking` from the base (see `requirements.md` §6). Cherry-pick
      the `foundation_core` changes of `fcbf523fa` (use `git checkout fcbf523fa -- <paths>` for the
      files in §2, then commit them as "wip: valtron parking from fcbf523fa").
- [x] T2. Diagnose the hang (§3.3). Write the cause and the evidence in the PR body and in
      `learnings.md`.
- [x] T3. Fix it. Keep the correctness rule of §3.2.
- [x] T4. Regression tests (all in `backends/foundation_core/tests/valtron/`, registered in the
      `mod.rs` there):
  - `threaded_future.rs`: `threaded_iter_future_does_not_spin_while_pending`. The future stays
    pending for 300 ms, then a second thread wakes it. Measure the worker thread's CPU time
    (`libc::clock_gettime(CLOCK_THREAD_CPUTIME_ID)` read inside the worker, or the process CPU
    time before and after) and assert it is under 50 ms. On the old code it is near 300 ms.
  - `threaded_future.rs`: `threaded_iter_future_backpressure_parks_until_space`. Queue size 1,
    the consumer takes one item every 100 ms, the producer yields 5 items. Assert the order and
    count, and that the producer used under 50 ms of CPU.
  - `threaded_future.rs`: `threaded_iter_future_error_reaches_consumer_when_queue_full`. The queue
    is full when the future returns `Err`; assert the consumer still receives the error.
  - `wakeable_stream.rs`: `first_empty_pull_does_not_block`. An async consumer polls a
    `NotifyQueue`-backed stream that is empty. Assert the first poll returns in under 5 ms (the
    old code waits `max_spins` timed waits) and that an item pushed later is received.
  - `wakeable_stream.rs`: `item_pushed_between_empty_pull_and_registration_is_not_lost`. Push the
    item from another thread at the moment between the empty pull and the wake registration (use
    a test hook or a barrier). Assert the consumer gets it.
  - The three `cross_runtime.rs` `*_drives_parked_future_task` tests must pass.
- [x] T5. Delete the two §4 backlog entries. Update `progress.md` and `learnings.md`.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_core --profile dev
cargo check --workspace --all-targets --profile dev
for t in threaded_iter_future_does_not_spin_while_pending threaded_iter_future_backpressure_parks_until_space \
         threaded_iter_future_error_reaches_consumer_when_queue_full first_empty_pull_does_not_block \
         item_pushed_between_empty_pull_and_registration_is_not_lost \
         smol_drives_parked_future_task tokio_drives_parked_future_task valtron_drives_parked_future_task; do
  for i in $(seq 1 20); do
    timeout 300 cargo test -p foundation_core --profile dev --features multi --test valtron "$t" \
      || { echo "FAIL $t run $i"; exit 1; }
  done
done
rg -n 'yield_now|spin_loop|thread::sleep' backends/foundation_core/src/valtron/executors/future_task.rs   # expect no output
```

Check the test binary name first (`ls backends/foundation_core/tests`); use the real one.

## 6. Done when

- [x] All §5 commands pass; each listed test passed 20 runs in a row. (Note: `cargo check
      --workspace --all-targets` still fails on macOS for an unrelated, pre-existing reason —
      `libc::pipe2` is Linux-only and `foundation_nativeapis/tests/reactor_shared_selector_tests.rs`
      uses it. `ci.py run foundation_core` is green.)
- [ ] Each new test failed (or hung to its timeout) on the base code. The PR body shows it.
- [x] `future_task.rs` has no `yield_now`, `spin_loop` or `thread::sleep`.
- [x] The hang's root cause is written down.
- [ ] CI is green on the PR.

## 7. Out of scope

Making the back-pressure wait and queue size configuration (feature 09). Other crates.

## 8. Review checklist (main session)

- [x] The waker really unparks the thread that polls (not the thread that created the future).
- [x] No lost wake: read the push/pop/register/fire order in `local.rs` and `stream_future.rs`.
- [x] `prime` does not consume or replace a real registration.
- [x] The CPU-time assertions measure the right thread and have margin, so they do not flake.
