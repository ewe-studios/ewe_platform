//! `CancelSignal` — the one cross-clone mutable object in a call's context
//! (Decision 04 Q4).
//!
//! WHY: A `Ctx` is owned + cheap-`Clone` with otherwise-immutable Arc'd
//! internals; the single shared *mutable* surface is cancellation. A clone
//! **shares** the call's signal (a clone IS the same call). Propagation is never
//! implicit: derived work that must not be cancelled with the call detaches with
//! a fresh signal, and linking to a parent is opt-in and **one-way down** —
//! `linked(parent)` fires when the parent fires OR when cancelled itself, and a
//! local cancel never propagates upward.
//!
//! WHAT: [`CancelSignal`] with `new`/`linked`/`cancel`/`is_canceled`/`cancelled`.
//!
//! HOW: an `Arc` over `{ ResumeSignal, optional parent }`. `ResumeSignal` is valtron's
//! flag-plus-notifier port: `set(true)` wakes the tasks parked on it, and the waking stays
//! inside valtron. A pending [`cancelled`](CancelSignal::cancelled) future registers its
//! waker through [`EventReadiness::register`] on the signal **and every ancestor**, so a
//! parent's `cancel()` wakes linked children.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use foundation_core::valtron::{EventReadiness, ReadinessWaker, ResumeSignal};

struct Inner {
    /// The fired flag and its notifier. Valtron owns the wake.
    fired: ResumeSignal,
    /// Opt-in one-way-down link; `is_canceled` also consults the parent.
    parent: Option<CancelSignal>,
}

/// A shared, awaitable cancellation flag for one call.
#[derive(Clone)]
pub struct CancelSignal(Arc<Inner>);

impl CancelSignal {
    /// An independent signal (no parent).
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            fired: ResumeSignal::new(false),
            parent: None,
        }))
    }

    /// A child signal that also fires when `parent` fires (one-way down). Firing
    /// this child never propagates upward to the parent.
    #[must_use]
    pub fn linked(parent: &CancelSignal) -> Self {
        Self(Arc::new(Inner {
            fired: ResumeSignal::new(false),
            parent: Some(parent.clone()),
        }))
    }

    /// Fire the signal, waking everything awaiting it (and any linked children).
    /// Idempotent.
    pub fn cancel(&self) {
        // `set(true)` wakes through the signal's notifier. A second concurrent
        // caller can wake the same tasks again; a repeated wake is harmless,
        // because every waiter re-checks `is_canceled`.
        if !self.0.fired.get() {
            self.0.fired.set(true);
        }
    }

    /// Whether this signal (or any ancestor it is linked to) has fired.
    #[must_use]
    pub fn is_canceled(&self) -> bool {
        self.0.fired.get()
            || self
                .0
                .parent
                .as_ref()
                .is_some_and(CancelSignal::is_canceled)
    }

    /// Resolve once this signal fires (race it against other work in `select!`).
    pub async fn cancelled(&self) {
        Cancelled { signal: self }.await;
    }

    /// Register `waker` on this signal and every ancestor, so a parent's
    /// `cancel()` wakes a child's pending `cancelled()` future.
    fn register(&self, waker: &ReadinessWaker) {
        // The return value says "this source wakes on every change"; the caller
        // re-checks `is_canceled` instead, so it is not needed here.
        let _ = EventReadiness::register(&self.0.fired, waker);
        if let Some(parent) = &self.0.parent {
            parent.register(waker);
        }
    }
}

impl Default for CancelSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for CancelSignal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CancelSignal")
            .field("canceled", &self.is_canceled())
            .field("linked", &self.0.parent.is_some())
            .finish()
    }
}

struct Cancelled<'a> {
    signal: &'a CancelSignal,
}

impl Future for Cancelled<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.signal.is_canceled() {
            return Poll::Ready(());
        }
        // the bridge for a source behind a `Future`: waking this waker wakes
        // `cx.waker()`, and valtron owns what that means.
        let waker = ReadinessWaker::from_waker(cx.waker().clone());
        self.signal.register(&waker);
        // Re-check to close the register/fire race.
        if self.signal.is_canceled() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
