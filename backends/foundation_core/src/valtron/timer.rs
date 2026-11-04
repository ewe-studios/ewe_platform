//! A process-wide timer thread for futures — native only.
//!
//! WHY: a future that wants to wait (a `select` timeout, a back-off) used to
//! schedule a separate valtron `sleep()` task and await it. That makes the
//! timeout depend on the pool running one more task: when every worker is busy
//! or the pool is shutting down, the timer never resolves and the future raced
//! against it hangs (and scheduling it could fail outright).
//!
//! WHAT: [`sleep_until`] / [`sleep_for`], futures that complete at a deadline
//! without any valtron task.
//!
//! HOW: deadlines live in an ordered map. One thread blocks on a condition
//! variable until the earliest deadline (or a new, earlier one is added), then
//! wakes every future that is due. It never polls: with no deadlines it waits
//! without a timeout. A future that is dropped first removes its entry. If the
//! thread cannot be started the futures still complete, by checking the clock
//! each time they are polled.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

/// The deadlines waiting to fire, keyed by `(deadline, id)`.
struct Timers {
    pending: Mutex<BTreeMap<(Instant, u64), Waker>>,
    changed: Condvar,
    next_id: AtomicU64,
    /// Whether the timer thread is running.
    running: bool,
}

impl Timers {
    fn pending(&self) -> MutexGuard<'_, BTreeMap<(Instant, u64), Waker>> {
        // The map holds no invariant a panic could break.
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn timers() -> &'static Timers {
    static TIMERS: OnceLock<Timers> = OnceLock::new();
    TIMERS.get_or_init(|| {
        let running = std::thread::Builder::new()
            .name("valtron-timer".to_string())
            .spawn(|| run(timers()))
            .map_err(|e| {
                tracing::error!(error = %e, "could not start the valtron timer thread; timers fall back to re-polling");
            })
            .is_ok();
        Timers {
            pending: Mutex::new(BTreeMap::new()),
            changed: Condvar::new(),
            next_id: AtomicU64::new(0),
            running,
        }
    })
}

/// The timer thread: sleep until the earliest deadline, wake what is due.
fn run(timers: &'static Timers) {
    let mut pending = timers.pending();
    loop {
        let now = Instant::now();
        let mut due = Vec::new();
        while let Some(entry) = pending.first_entry() {
            if entry.key().0 > now {
                break;
            }
            due.push(entry.remove());
        }
        if !due.is_empty() {
            drop(pending);
            for waker in due {
                waker.wake();
            }
            pending = timers.pending();
            continue;
        }
        pending = match pending.keys().next().map(|(deadline, _)| *deadline) {
            Some(deadline) => {
                let wait = deadline.saturating_duration_since(now);
                timers
                    .changed
                    .wait_timeout(pending, wait)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
            None => timers
                .changed
                .wait(pending)
                .unwrap_or_else(PoisonError::into_inner),
        };
    }
}

/// A future that completes at `deadline`, woken by the timer thread.
#[must_use = "a timer does nothing unless awaited"]
pub fn sleep_until(deadline: Instant) -> Sleep {
    let timers = timers();
    Sleep {
        timers,
        key: (deadline, timers.next_id.fetch_add(1, Ordering::Relaxed)),
        registered: false,
    }
}

/// A future that completes `duration` from now. See [`sleep_until`].
#[must_use = "a timer does nothing unless awaited"]
pub fn sleep_for(duration: Duration) -> Sleep {
    sleep_until(Instant::now() + duration)
}

/// The future returned by [`sleep_until`] and [`sleep_for`].
pub struct Sleep {
    timers: &'static Timers,
    key: (Instant, u64),
    registered: bool,
}

impl Sleep {
    /// When this timer fires.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.key.0
    }
}

impl core::fmt::Debug for Sleep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sleep")
            .field("deadline", &self.key.0)
            .finish()
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if Instant::now() >= self.key.0 {
            if self.registered {
                self.timers.pending().remove(&self.key);
                self.registered = false;
            }
            return Poll::Ready(());
        }
        if !self.timers.running {
            // No timer thread: nothing will wake us, so ask to be polled again.
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        let key = self.key;
        let earliest = {
            let mut pending = self.timers.pending();
            pending.insert(key, cx.waker().clone());
            pending.keys().next() == Some(&key)
        };
        self.registered = true;
        // A new earliest deadline must shorten the thread's current wait.
        if earliest {
            self.timers.changed.notify_one();
        }
        Poll::Pending
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        if self.registered {
            self.timers.pending().remove(&self.key);
        }
    }
}

/// A waker that runs a one-shot callback instead of resuming a future.
struct CallbackWake(Mutex<Option<Box<dyn FnOnce() + Send>>>);

impl Wake for CallbackWake {
    fn wake(self: std::sync::Arc<Self>) {
        let cb = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(cb) = cb {
            cb();
        }
    }
}

/// A callback scheduled on the timer thread by [`call_at`]. Dropping it cancels
/// the callback if it has not run yet.
#[must_use = "dropping a TimerCall cancels the callback"]
pub struct TimerCall {
    scheduled: Option<(&'static Timers, (Instant, u64))>,
}

impl core::fmt::Debug for TimerCall {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TimerCall")
            .field("deadline", &self.scheduled.map(|(_, key)| key.0))
            .finish()
    }
}

impl Drop for TimerCall {
    fn drop(&mut self) {
        if let Some((timers, key)) = self.scheduled.take() {
            timers.pending().remove(&key);
        }
    }
}

/// Run `callback` once on the timer thread at `deadline`, without any valtron
/// task and without a thread of its own.
///
/// WHY: code that must be woken by the clock (a `DrivenStreamIterator` whose
/// parked task has a deadline) needs a one-shot wake. The shared timer thread
/// already sleeps until the earliest deadline, so one more entry costs a map
/// insert, not a thread.
///
/// The callback runs on the timer thread: keep it short and never block. If the
/// timer thread could not be started, `callback` runs at once on the caller's
/// thread (the caller then sees a spurious wake and re-checks, as
/// [`Sleep`] does by re-polling).
pub fn call_at(deadline: Instant, callback: impl FnOnce() + Send + 'static) -> TimerCall {
    let timers = timers();
    if !timers.running {
        callback();
        return TimerCall { scheduled: None };
    }
    let key = (deadline, timers.next_id.fetch_add(1, Ordering::Relaxed));
    let waker = Waker::from(std::sync::Arc::new(CallbackWake(Mutex::new(Some(
        Box::new(callback),
    )))));
    let earliest = {
        let mut pending = timers.pending();
        pending.insert(key, waker);
        pending.keys().next() == Some(&key)
    };
    if earliest {
        timers.changed.notify_one();
    }
    TimerCall {
        scheduled: Some((timers, key)),
    }
}
