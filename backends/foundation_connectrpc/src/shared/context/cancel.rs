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
//! HOW: an `Arc` over `{ AtomicBool, parked wakers, optional parent }`. A pending
//! [`cancelled`](CancelSignal::cancelled) future registers its waker in the
//! signal **and every ancestor**, so a parent's `cancel()` wakes linked children.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

struct Inner {
    canceled: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
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
            canceled: AtomicBool::new(false),
            wakers: Mutex::new(Vec::new()),
            parent: None,
        }))
    }

    /// A child signal that also fires when `parent` fires (one-way down). Firing
    /// this child never propagates upward to the parent.
    #[must_use]
    pub fn linked(parent: &CancelSignal) -> Self {
        Self(Arc::new(Inner {
            canceled: AtomicBool::new(false),
            wakers: Mutex::new(Vec::new()),
            parent: Some(parent.clone()),
        }))
    }

    /// Fire the signal, waking everything awaiting it (and any linked children).
    /// Idempotent.
    pub fn cancel(&self) {
        if !self.0.canceled.swap(true, Ordering::AcqRel) {
            let wakers = {
                let mut guard = self.0.wakers.lock().unwrap_or_else(|e| e.into_inner());
                std::mem::take(&mut *guard)
            };
            for waker in wakers {
                waker.wake();
            }
        }
    }

    /// Whether this signal (or any ancestor it is linked to) has fired.
    #[must_use]
    pub fn is_canceled(&self) -> bool {
        self.0.canceled.load(Ordering::Acquire)
            || self.0.parent.as_ref().is_some_and(CancelSignal::is_canceled)
    }

    /// Resolve once this signal fires (race it against other work in `select!`).
    pub async fn cancelled(&self) {
        Cancelled { signal: self }.await;
    }

    /// Register `waker` on this signal and every ancestor, so a parent's
    /// `cancel()` wakes a child's pending `cancelled()` future.
    fn register(&self, waker: &Waker) {
        self.0
            .wakers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(waker.clone());
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
        self.signal.register(cx.waker());
        // Re-check to close the register/fire race.
        if self.signal.is_canceled() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
