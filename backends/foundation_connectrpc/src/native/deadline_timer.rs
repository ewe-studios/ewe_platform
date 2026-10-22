//! A process-wide deadline timer for the HTTP/2 and HTTP/3 servers — native
//! only.
//!
//! WHY: h2/h3 dispatch futures run on the valtron pool, which has no timer a
//! future can wait on. A valtron task that reports `Delayed` until the deadline
//! does not work either: a worker whose local tasks are all asleep sleeps until
//! the nearest sleeper is due, and the busy-polled QUIC stream tasks queued on
//! that worker stall with it (a 15s deadline stalled an h3 bidi call for 15s).
//!
//! WHAT: [`ThreadTimer`], a [`DeadlineTimer`] backed by one timer thread.
//!
//! HOW: deadlines live in an ordered map. The thread blocks on a condition
//! variable until the earliest deadline (or until a new one is added), then
//! wakes every future that is due. It never polls: with no deadlines it waits
//! without a timeout. A future that is dropped first removes its entry.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::Instant;

use crate::shared::router::DeadlineTimer;
use crate::shared::transport::BoxFuture;

/// The deadlines waiting to fire, keyed by `(deadline, id)`.
struct Timers {
    pending: Mutex<BTreeMap<(Instant, u64), Waker>>,
    changed: Condvar,
    next_id: AtomicU64,
    /// Whether the timer thread is running; without it nothing fires.
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
            .name("connectrpc-deadlines".to_string())
            .spawn(|| run(timers()))
            .map_err(|e| {
                tracing::error!(error = %e, "could not start the deadline timer thread; call deadlines are not enforced");
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

/// A [`DeadlineTimer`] on the process-wide timer thread. See the module docs.
pub(crate) struct ThreadTimer;

impl DeadlineTimer for ThreadTimer {
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<'_, ()> {
        let timers = timers();
        if !timers.running {
            return Box::pin(std::future::pending());
        }
        Box::pin(Sleep {
            timers,
            key: (deadline, timers.next_id.fetch_add(1, Ordering::Relaxed)),
            registered: false,
        })
    }
}

struct Sleep {
    timers: &'static Timers,
    key: (Instant, u64),
    registered: bool,
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
