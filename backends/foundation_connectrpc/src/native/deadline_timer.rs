//! The deadline timer for the HTTP/2 and HTTP/3 servers — native only.
//!
//! WHY: h2/h3 dispatch futures run on the valtron pool. A valtron task that
//! reports `Delayed` until the deadline does not work: a worker whose local
//! tasks are all asleep sleeps until the nearest sleeper is due, and the
//! busy-polled QUIC stream tasks queued on that worker stall with it (a 15s
//! deadline stalled an h3 bidi call for 15s).
//!
//! WHAT: [`ThreadTimer`], a [`DeadlineTimer`] on valtron's process-wide timer
//! thread ([`foundation_core::valtron::timer`]), which wakes each deadline's
//! future at its time and schedules no task.

use std::time::Instant;

use crate::shared::router::DeadlineTimer;
use crate::shared::transport::BoxFuture;

/// A [`DeadlineTimer`] on valtron's timer thread. See the module docs.
pub(crate) struct ThreadTimer;

impl DeadlineTimer for ThreadTimer {
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<'_, ()> {
        Box::pin(foundation_core::valtron::timer::sleep_until(deadline))
    }
}
