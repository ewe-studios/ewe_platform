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
//! HOW: deadlines live in ordered maps, one map per clock. Each deadline brings
//! the clock of the `Ctx` that made it, so the thread owns no clock and reads
//! no real clock by itself (a simulated clock in a test controls the wait).
//! One thread blocks on a condition variable until the earliest deadline (or a
//! new, earlier one is added), then wakes every future that is due. It never polls: with no deadlines it waits
//! without a timeout. A future that is dropped first removes its entry. If the
//! thread cannot be started the futures still complete, by checking the clock
//! each time they are polled.

use foundation_compact::Instant;
use foundation_context::{Clock, Ctx};
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

/// The deadlines of one clock, keyed by `(deadline, id)`.
struct ClockQueue {
    clock: Arc<dyn Clock>,
    deadlines: BTreeMap<(Instant, u64), Waker>,
}

/// The identity of a clock: the address of its shared allocation.
type ClockId = usize;

fn clock_id(clock: &Arc<dyn Clock>) -> ClockId {
    Arc::as_ptr(clock).cast::<()>() as usize
}

/// A timer entry key: which clock measures the deadline, and the deadline.
type EntryKey = (ClockId, (Instant, u64));

/// The deadlines waiting to fire, grouped by the clock that measures them.
struct Timers {
    pending: Mutex<HashMap<ClockId, ClockQueue>>,
    changed: Condvar,
    next_id: AtomicU64,
    /// Whether the timer thread is running.
    running: bool,
}

impl Timers {
    fn pending(&self) -> MutexGuard<'_, HashMap<ClockId, ClockQueue>> {
        // The map holds no invariant a panic could break.
        self.pending
            .lock()
            .unwrap_or_else(crate::compati::recover_poison)
    }

    /// Add a deadline measured on `clock`. Returns the key to remove it with,
    /// and whether it is now the earliest on its clock.
    fn insert(&self, clock: &Arc<dyn Clock>, deadline: Instant, waker: Waker) -> (EntryKey, bool) {
        let id = clock_id(clock);
        let key = (deadline, self.next_id.fetch_add(1, Ordering::Relaxed));
        let mut pending = self.pending();
        let queue = pending.entry(id).or_insert_with(|| ClockQueue {
            clock: Arc::clone(clock),
            deadlines: BTreeMap::new(),
        });
        queue.deadlines.insert(key, waker);
        let earliest = queue.deadlines.keys().next() == Some(&key);
        ((id, key), earliest)
    }

    /// Replace the waker of an existing entry, or add it again.
    fn register(
        &self,
        clock: &Arc<dyn Clock>,
        key: (Instant, u64),
        waker: Waker,
    ) -> (EntryKey, bool) {
        let id = clock_id(clock);
        let mut pending = self.pending();
        let queue = pending.entry(id).or_insert_with(|| ClockQueue {
            clock: Arc::clone(clock),
            deadlines: BTreeMap::new(),
        });
        queue.deadlines.insert(key, waker);
        let earliest = queue.deadlines.keys().next() == Some(&key);
        ((id, key), earliest)
    }

    fn remove(&self, entry: &EntryKey) {
        let mut pending = self.pending();
        if let Some(queue) = pending.get_mut(&entry.0) {
            queue.deadlines.remove(&entry.1);
            if queue.deadlines.is_empty() {
                pending.remove(&entry.0);
            }
        }
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
            pending: Mutex::new(HashMap::new()),
            changed: Condvar::new(),
            next_id: AtomicU64::new(0),
            running,
        }
    })
}

/// Wake the timer thread so it re-reads every clock.
///
/// A [`SimulatedClock`](crate::valtron::SimulatedClock) calls this from
/// `advance`: its time moves without any operating-system event, so the thread
/// must look again. A real clock needs no nudge.
#[cfg(feature = "test-clock")]
pub(crate) fn nudge() {
    // Take the lock so the nudge cannot fall between the thread's check and its wait.
    if let Some(timers) = TIMERS_IF_STARTED.get() {
        drop(timers.pending());
        timers.changed.notify_all();
    }
}

#[cfg(feature = "test-clock")]
static TIMERS_IF_STARTED: std::sync::OnceLock<&'static Timers> = std::sync::OnceLock::new();

/// The timer thread: sleep until the earliest deadline, wake what is due.
///
/// The thread reads no clock of its own. Each deadline carries the clock of the
/// `Ctx` that made it, and the thread asks that clock for the time. The real
/// operating-system wait is only the park until the nearest deadline.
fn run(timers: &'static Timers) {
    #[cfg(feature = "test-clock")]
    let _ = TIMERS_IF_STARTED.set(timers);
    let mut pending = timers.pending();
    loop {
        let mut due = Vec::new();
        let mut next_wait: Option<Duration> = None;
        for queue in pending.values_mut() {
            let now = queue.clock.now();
            while let Some(entry) = queue.deadlines.first_entry() {
                if entry.key().0 > now {
                    let wait = entry.key().0.saturating_duration_since(now);
                    next_wait = Some(next_wait.map_or(wait, |w| w.min(wait)));
                    break;
                }
                due.push(entry.remove());
            }
        }
        pending.retain(|_, queue| !queue.deadlines.is_empty());
        if !due.is_empty() {
            drop(pending);
            for waker in due {
                waker.wake();
            }
            pending = timers.pending();
            continue;
        }
        pending = match next_wait {
            Some(wait) => {
                timers
                    .changed
                    .wait_timeout(pending, wait)
                    .unwrap_or_else(crate::compati::recover_poison)
                    .0
            }
            None => timers
                .changed
                .wait(pending)
                .unwrap_or_else(crate::compati::recover_poison),
        };
    }
}

/// A future that completes at `deadline`, woken by the timer thread.
///
/// `deadline` is an instant on the clock of `ctx`. The future and the timer
/// thread both read that clock, so a simulated clock controls the wait.
#[must_use = "a timer does nothing unless awaited"]
pub fn sleep_until(deadline: Instant, ctx: &Ctx) -> Sleep {
    let timers = timers();
    Sleep {
        timers,
        clock: ctx.clock(),
        key: (deadline, timers.next_id.fetch_add(1, Ordering::Relaxed)),
        registered: false,
    }
}

/// A future that completes `duration` from now on the clock of `ctx`.
/// See [`sleep_until`].
///
/// WHY a `Ctx` argument: the timer is a process-wide thread, but it owns no
/// clock. Each deadline brings the clock of its caller, so the thread never
/// reads the real clock on its own and a test can inject a simulated clock.
#[must_use = "a timer does nothing unless awaited"]
pub fn sleep_for(duration: Duration, ctx: &Ctx) -> Sleep {
    sleep_until(ctx.clock().now() + duration, ctx)
}

/// The future returned by [`sleep_until`] and [`sleep_for`].
pub struct Sleep {
    timers: &'static Timers,
    clock: Arc<dyn Clock>,
    key: (Instant, u64),
    registered: bool,
}

impl Sleep {
    /// When this timer fires.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.key.0
    }

    fn entry(&self) -> EntryKey {
        (clock_id(&self.clock), self.key)
    }
}

impl core::fmt::Debug for Sleep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sleep")
            .field("deadline", &self.key.0)
            .finish_non_exhaustive()
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        // The future checks the same clock that the timer thread uses.
        if self.clock.now() >= self.key.0 {
            if self.registered {
                self.timers.remove(&self.entry());
                self.registered = false;
            }
            return Poll::Ready(());
        }
        if !self.timers.running {
            // No timer thread: nothing will wake us, so ask to be polled again.
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        let (_, earliest) = self
            .timers
            .register(&self.clock, self.key, cx.waker().clone());
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
            self.timers.remove(&self.entry());
        }
    }
}

/// A waker that runs a one-shot callback instead of resuming a future.
struct CallbackWake(Mutex<Option<Box<dyn FnOnce() + Send>>>);

impl Wake for CallbackWake {
    fn wake(self: std::sync::Arc<Self>) {
        let cb = self
            .0
            .lock()
            .unwrap_or_else(crate::compati::recover_poison)
            .take();
        if let Some(cb) = cb {
            cb();
        }
    }
}

/// A callback scheduled on the timer thread by [`call_at`]. Dropping it cancels
/// the callback if it has not run yet.
#[must_use = "dropping a TimerCall cancels the callback"]
pub struct TimerCall {
    scheduled: Option<(&'static Timers, EntryKey)>,
}

impl core::fmt::Debug for TimerCall {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TimerCall")
            .field(
                "deadline",
                &self.scheduled.as_ref().map(|(_, key)| (key.1).0),
            )
            .finish()
    }
}

impl Drop for TimerCall {
    fn drop(&mut self) {
        if let Some((timers, key)) = self.scheduled.take() {
            timers.remove(&key);
        }
    }
}

/// Run `callback` once on the timer thread at `deadline` (an instant on the
/// clock of `ctx`), without any valtron
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
pub fn call_at(
    deadline: Instant,
    ctx: &Ctx,
    callback: impl FnOnce() + Send + 'static,
) -> TimerCall {
    let timers = timers();
    if !timers.running {
        callback();
        return TimerCall { scheduled: None };
    }
    let waker = Waker::from(std::sync::Arc::new(CallbackWake(Mutex::new(Some(
        Box::new(callback),
    )))));
    let (entry, earliest) = timers.insert(&ctx.clock(), deadline, waker);
    if earliest {
        timers.changed.notify_one();
    }
    TimerCall {
        scheduled: Some((timers, entry)),
    }
}
