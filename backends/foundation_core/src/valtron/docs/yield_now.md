# Yield now usage

Interesting ways to have a thread yield for other work to be performed.

- [sleep](https://doc.rust-lang.org/std/thread/fn.sleep.html)
- [yield_now](https://doc.rust-lang.org/std/thread/fn.yield_now.html)

### Custom Forever Threads that yields now for other work.

```rust

loop {
    std::thread::sleep(...);
    if ! everything_still_ok() {
        break;
    }

    std::thread::yield_now();
}

```

### Usage in a custom Heap Allocation

```rust
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

struct RawHeap {
    // stuff
}

impl RawHeap {
    fn alloc(&mut self) -> *mut u8 {
        unimplemented!()
    }
}

// Playground doesn't have static_assertions, but I use this in real code
// static_assertions::assert_impl_all!(RawHeap: Send);
unsafe impl Send for Heap {}
unsafe impl Sync for Heap {}

struct Heap {
    inner: UnsafeCell<RawHeap>,
    lock: AtomicBool
}

impl Heap {
    fn wait_for_lock(&self) {
        while self.lock.swap(true, Ordering::Acquire) {
            std::thread::yield_now()
        }
    }
    fn unlock(&self) {
        let was_locked = self.lock.swap(false, Ordering::Release);
        debug_assert!(was_locked);
    }
    fn alloc(&self) -> *mut u8 {
        self.wait_for_lock();
        // Safe: have exclusive access via lock
        let ret = unsafe { &mut* self.inner.get() }.alloc();
        self.unlock();
        ret
    }
}
```

## Sleeping threads with Spin Checks and CondVar

Taking from [rayon](https://github.com/rayon-rs/rayon/blob/d179ea9fa69cab08a3b2762ff9b15249a71b9b94/rayon-core/src/sleep/mod.rs#L118-L194)

You can do some spin checks until some counter is reached with the `thread::yield_now()`
at which point you can rest on a [CondVar](https://doc.rust-lang.org/std/sync/struct.Condvar.html) to awake a thread that is asleep via the same `CondVar`.

Since `CondVar` have a unique capacity:

```rust
A condition Variable:

Condition variables represent the ability to block a thread such that it consumes no CPU time while waiting for an event to occur. Condition variables are typically associated with a boolean predicate (a condition) and a mutex. The predicate is always verified inside of the mutex before determining that a thread must block.

Functions in this module will block the current thread of execution. Note that any attempt to use multiple mutexes on the same condition variable may result in a runtime panic.

---

use std::sync::{Arc, Mutex, Condvar};
use std::thread;

let pair = Arc::new((Mutex::new(false), Condvar::new()));
let pair2 = Arc::clone(&pair);

// Inside of our lock, spawn a new thread, and then wait for it to start.
thread::spawn(move || {
    let (lock, cvar) = &*pair2;
    let mut started = lock.lock().unwrap();
    *started = true;
    // We notify the condvar that the value has changed.
    cvar.notify_one();
});

// Wait for the thread to start up.
let (lock, cvar) = &*pair;
let mut started = lock.lock().unwrap();
while !*started {
    started = cvar.wait(started).unwrap();
}
```

Specifically the spin here:

```rust
while *is_blocked {
    is_blocked = sleep_state.condvar.wait(is_blocked).unwrap();
}
```

See the full code block:

```rust

/// The "sleep state" for an individual worker.
#[derive(Default)]
struct WorkerSleepState {
    /// Set to true when the worker goes to sleep; set to false when
    /// the worker is notified or when it wakes.
    is_blocked: Mutex<bool>,

    condvar: Condvar,
}

#[inline]
pub(super) fn no_work_found(
    &self,
    idle_state: &mut IdleState,
    latch: &CoreLatch,
    has_injected_jobs: impl FnOnce() -> bool,
) {
    if idle_state.rounds < ROUNDS_UNTIL_SLEEPY {
        thread::yield_now();
        idle_state.rounds += 1;
    } else if idle_state.rounds == ROUNDS_UNTIL_SLEEPY {
        idle_state.jobs_counter = self.announce_sleepy();
        idle_state.rounds += 1;
        thread::yield_now();
    } else if idle_state.rounds < ROUNDS_UNTIL_SLEEPING {
        idle_state.rounds += 1;
        thread::yield_now();
    } else {
        debug_assert_eq!(idle_state.rounds, ROUNDS_UNTIL_SLEEPING);
        self.sleep(idle_state, latch, has_injected_jobs);
    }
}

#[cold]
fn announce_sleepy(&self) -> JobsEventCounter {
    self.counters
        .increment_jobs_event_counter_if(JobsEventCounter::is_active)
        .jobs_counter()
}

#[cold]
fn sleep(
    &self,
    idle_state: &mut IdleState,
    latch: &CoreLatch,
    has_injected_jobs: impl FnOnce() -> bool,
) {
    let worker_index = idle_state.worker_index;

    if !latch.get_sleepy() {
        return;
    }

    let sleep_state = &self.worker_sleep_states[worker_index];
    let mut is_blocked = sleep_state.is_blocked.lock().unwrap();
    debug_assert!(!*is_blocked);

    // Our latch was signalled. We should wake back up fully as we
    // will have some stuff to do.
    if !latch.fall_asleep() {
        idle_state.wake_fully();
        return;
    }

    loop {
        let counters = self.counters.load(Ordering::SeqCst);

        // Check if the JEC has changed since we got sleepy.
        debug_assert!(idle_state.jobs_counter.is_sleepy());
        if counters.jobs_counter() != idle_state.jobs_counter {
            // JEC has changed, so a new job was posted, but for some reason
            // we didn't see it. We should return to just before the SLEEPY
            // state so we can do another search and (if we fail to find
            // work) go back to sleep.
            idle_state.wake_partly();
            latch.wake_up();
            return;
        }

        // Otherwise, let's move from IDLE to SLEEPING.
        if self.counters.try_add_sleeping_thread(counters) {
            break;
        }
    }

    // Successfully registered as asleep.

    // We have one last check for injected jobs to do. This protects against
    // deadlock in the very unlikely event that
    //
    // - an external job is being injected while we are sleepy
    // - that job triggers the rollover over the JEC such that we don't see it
    // - we are the last active worker thread
    std::sync::atomic::fence(Ordering::SeqCst);
    if has_injected_jobs() {
        // If we see an externally injected job, then we have to 'wake
        // ourselves up'. (Ordinarily, `sub_sleeping_thread` is invoked by
        // the one that wakes us.)
        self.counters.sub_sleeping_thread();
    } else {
        // If we don't see an injected job (the normal case), then flag
        // ourselves as asleep and wait till we are notified.
        //
        // (Note that `is_blocked` is held under a mutex and the mutex was
        // acquired *before* we incremented the "sleepy counter". This means
        // that whomever is coming to wake us will have to wait until we
        // release the mutex in the call to `wait`, so they will see this
        // boolean as true.)
        *is_blocked = true;
        while *is_blocked {
            is_blocked = sleep_state.condvar.wait(is_blocked).unwrap();
        }
    }

    // Update other state:
    idle_state.wake_fully();
    latch.wake_up();
}

```
