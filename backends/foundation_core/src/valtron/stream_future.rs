//! Stream-to-Future async bridge for valtron.
//!
//! Wrapper types that convert any [`StreamIterator`] into standard Rust `Future`
//! and `Stream` types, bridging valtron's sync iterator model with async code.
//!
//! A stream that drives its own producer (`WakeableStream::drives_its_producer`)
//! is re-polled on `Wait`, `Init` and `Pending` while it can make progress, with
//! a bounded budget. On a single-threaded engine no other thread sends the wake
//! that a parked adapter waits for, so parking on those control states would
//! stall the stream. `Delayed` keeps its timer behaviour and still parks.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::Stream as FuturesStream;

use super::executors::DEFAULT_WAIT_REDRIVE_BUDGET;
use super::streams::{Stream, StreamIterator, StreamSpread};
use super::wake::{WakeResult, WakeableStream};

// ============================================================================
// StreamCollectFuture
// ============================================================================

/// Drives a [`StreamIterator`] to completion, collecting all `Next` values into a `Vec`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StreamCollectFuture<SI: StreamIterator> {
    inner: Option<SI>,
    collected: Vec<SI::D>,
    /// Whether the async consumer has announced itself to the stream (see
    /// [`prime`]).
    primed: bool,
    /// How many times a `Wait` from a driving stream is re-polled before the
    /// adapter parks. See [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    wait_redrive_budget: usize,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI: StreamIterator> StreamCollectFuture<SI> {
    pub fn new(iter: SI) -> Self {
        Self {
            inner: Some(iter),
            primed: false,
            collected: Vec::new(),
            wait_redrive_budget: DEFAULT_WAIT_REDRIVE_BUDGET,
        }
    }

    /// Set how many times a `Wait` from a driving stream is re-polled before
    /// the adapter parks. The default is [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    #[must_use]
    pub fn with_wait_redrive_budget(mut self, budget: usize) -> Self {
        self.wait_redrive_budget = budget;
        self
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI> Future for StreamCollectFuture<SI>
where
    SI: WakeableStream + Unpin,
    SI::D: Unpin,
{
    type Output = Vec<SI::D>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");
        if !this.primed {
            this.primed = true;
            prime(iter, cx);
        }

        let mut wait_redrive_budget = this.wait_redrive_budget;

        loop {
            match iter.next() {
                Some(Stream::Next(v)) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                    this.collected.push(v);
                }
                Some(Stream::Ignore) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                }
                Some(Stream::Spread(items)) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                    // A Spread carries multiple Done payloads; collect every one
                    // and skip Pending payloads (they are not values to collect).
                    for item in items {
                        if let StreamSpread::Done(v) = item {
                            this.collected.push(v);
                        }
                    }
                }
                Some(Stream::Pending(_) | Stream::Init | Stream::Wait)
                    if iter.drives_its_producer() && iter.can_make_progress() =>
                {
                    // A driving stream runs its producer inside `next()`. On a
                    // single-threaded engine no other thread sends a wake, so
                    // re-poll instead of parking. A `Pending` progress hint or
                    // an `Init` is a control state here, not an item to wait
                    // for. Bound the re-polls so a stream that never makes
                    // progress still yields to the executor. A parked engine
                    // reports `can_make_progress() == false`; it keeps the old
                    // register-then-park behaviour below.
                    if wait_redrive_budget == 0 {
                        let waker = cx.waker().clone();
                        let _ = iter.wake_send(Box::new(move || waker.wake()));
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    wait_redrive_budget -= 1;
                }
                Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    let waker = cx.waker().clone();
                    match iter.wake_send(Box::new(move || waker.wake())) {
                        WakeResult::Successful => {
                            // Register-then-recheck: a `push` between our last
                            // `next()` (empty) and `wake()` already fired the
                            // still-empty registry and will not re-fire. Catch it
                            // here; park only if still non-target.
                            match iter.next() {
                                Some(Stream::Next(v)) => this.collected.push(v),
                                Some(Stream::Ignore) => {}
                                Some(Stream::Spread(items)) => {
                                    for item in items {
                                        if let StreamSpread::Done(v) = item {
                                            this.collected.push(v);
                                        }
                                    }
                                }
                                Some(
                                    Stream::Pending(_)
                                    | Stream::Delayed(_)
                                    | Stream::Init
                                    | Stream::Wait,
                                ) => return Poll::Pending,
                                None => {
                                    return Poll::Ready(std::mem::take(&mut this.collected));
                                }
                            }
                        }
                        WakeResult::NotSupported => {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                None => {
                    return Poll::Ready(std::mem::take(&mut this.collected));
                }
            }
        }
    }
}

// ============================================================================
// StreamReadyFuture
// ============================================================================

/// Drives a [`StreamIterator`] until the first `Next(D)`, returning the value and remaining iterator.
pub struct StreamReadyFuture<SI: StreamIterator> {
    inner: Option<SI>,
    /// Whether the async consumer has announced itself to the stream (see
    /// [`prime`]).
    primed: bool,
    /// How many times a `Wait` from a driving stream is re-polled before the
    /// adapter parks. See [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    wait_redrive_budget: usize,
}

impl<SI: StreamIterator> StreamReadyFuture<SI> {
    pub fn new(iter: SI) -> Self {
        Self {
            inner: Some(iter),
            primed: false,
            wait_redrive_budget: DEFAULT_WAIT_REDRIVE_BUDGET,
        }
    }

    /// Set how many times a `Wait` from a driving stream is re-polled before
    /// the adapter parks. The default is [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    #[must_use]
    pub fn with_wait_redrive_budget(mut self, budget: usize) -> Self {
        self.wait_redrive_budget = budget;
        self
    }
}

impl<SI> Future for StreamReadyFuture<SI>
where
    SI: WakeableStream + Unpin,
{
    type Output = Option<(SI::D, SI)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");
        if !this.primed {
            this.primed = true;
            prime(iter, cx);
        }

        let mut wait_redrive_budget = this.wait_redrive_budget;

        loop {
            match iter.next() {
                Some(Stream::Next(v)) => {
                    let remaining = this.inner.take().expect("inner should exist");
                    return Poll::Ready(Some((v, remaining)));
                }
                Some(Stream::Ignore) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                }
                Some(Stream::Pending(_) | Stream::Init | Stream::Wait)
                    if iter.drives_its_producer() && iter.can_make_progress() =>
                {
                    // A driving stream runs its producer inside `next()`. On a
                    // single-threaded engine no other thread sends a wake, so
                    // re-poll instead of parking. A `Pending` progress hint or
                    // an `Init` is a control state here, not an item to wait
                    // for. Bound the re-polls so a stream that never makes
                    // progress still yields to the executor. A parked engine
                    // reports `can_make_progress() == false`; it keeps the old
                    // register-then-park behaviour below.
                    if wait_redrive_budget == 0 {
                        let waker = cx.waker().clone();
                        let _ = iter.wake_send(Box::new(move || waker.wake()));
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    wait_redrive_budget -= 1;
                }
                Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    let waker = cx.waker().clone();
                    match iter.wake_send(Box::new(move || waker.wake())) {
                        WakeResult::Successful => {
                            // Register-then-recheck: a `push` between our last
                            // `next()` (empty) and `wake()` already fired the
                            // still-empty registry and will not re-fire. Catch it
                            // here; park only if still non-target.
                            match iter.next() {
                                Some(Stream::Next(v)) => {
                                    let remaining = this.inner.take().expect("inner should exist");
                                    return Poll::Ready(Some((v, remaining)));
                                }
                                Some(Stream::Ignore) => {}
                                Some(Stream::Spread(items)) => {
                                    for item in items {
                                        if let StreamSpread::Done(v) = item {
                                            let remaining =
                                                this.inner.take().expect("inner should exist");
                                            return Poll::Ready(Some((v, remaining)));
                                        }
                                    }
                                }
                                Some(
                                    Stream::Pending(_)
                                    | Stream::Delayed(_)
                                    | Stream::Init
                                    | Stream::Wait,
                                ) => return Poll::Pending,
                                None => return Poll::Ready(None),
                            }
                        }
                        WakeResult::NotSupported => {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                Some(Stream::Spread(items)) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                    for item in items {
                        if let StreamSpread::Done(v) = item {
                            let remaining = this.inner.take().expect("inner should exist");
                            return Poll::Ready(Some((v, remaining)));
                        }
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

// ============================================================================
// StreamPendingFuture
// ============================================================================

/// Drives a [`StreamIterator`] until the first `Pending(P)`, returning the context and remaining iterator.
pub struct StreamPendingFuture<SI: StreamIterator> {
    inner: Option<SI>,
    /// Whether the async consumer has announced itself to the stream (see
    /// [`prime`]).
    primed: bool,
    /// How many times a `Wait` from a driving stream is re-polled before the
    /// adapter parks. See [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    wait_redrive_budget: usize,
}

impl<SI: StreamIterator> StreamPendingFuture<SI> {
    pub fn new(iter: SI) -> Self {
        Self {
            inner: Some(iter),
            primed: false,
            wait_redrive_budget: DEFAULT_WAIT_REDRIVE_BUDGET,
        }
    }

    /// Set how many times a `Wait` from a driving stream is re-polled before
    /// the adapter parks. The default is [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    #[must_use]
    pub fn with_wait_redrive_budget(mut self, budget: usize) -> Self {
        self.wait_redrive_budget = budget;
        self
    }
}

impl<SI> Future for StreamPendingFuture<SI>
where
    SI: WakeableStream + Unpin,
{
    type Output = Option<(SI::P, SI)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");
        if !this.primed {
            this.primed = true;
            prime(iter, cx);
        }

        let mut wait_redrive_budget = this.wait_redrive_budget;

        loop {
            match iter.next() {
                Some(Stream::Pending(p)) => {
                    let remaining = this.inner.take().expect("inner should exist");
                    return Poll::Ready(Some((p, remaining)));
                }
                Some(Stream::Ignore) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                }
                Some(Stream::Init | Stream::Wait)
                    if iter.drives_its_producer() && iter.can_make_progress() =>
                {
                    // A driving stream runs its producer inside `next()`. On a
                    // single-threaded engine no other thread sends a wake, so
                    // re-poll instead of parking. `Pending` is this adapter's
                    // result and is returned above, so only an `Init` or a
                    // `Wait` is a control state to drive past here. Bound the
                    // re-polls so a stream that never makes progress still
                    // yields to the executor. A parked engine reports
                    // `can_make_progress() == false`; it keeps the old
                    // register-then-park behaviour below.
                    if wait_redrive_budget == 0 {
                        let waker = cx.waker().clone();
                        let _ = iter.wake_send(Box::new(move || waker.wake()));
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    wait_redrive_budget -= 1;
                }
                Some(Stream::Next(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    let waker = cx.waker().clone();
                    match iter.wake_send(Box::new(move || waker.wake())) {
                        WakeResult::Successful => {
                            // Register-then-recheck: a `push` between our last
                            // `next()` (empty) and `wake()` already fired the
                            // still-empty registry and will not re-fire. Catch it
                            // here; park only if still non-target.
                            match iter.next() {
                                Some(Stream::Pending(p)) => {
                                    let remaining = this.inner.take().expect("inner should exist");
                                    return Poll::Ready(Some((p, remaining)));
                                }
                                Some(Stream::Ignore) => {}
                                Some(Stream::Spread(items)) => {
                                    for item in items {
                                        if let StreamSpread::Pending(p) = item {
                                            let remaining =
                                                this.inner.take().expect("inner should exist");
                                            return Poll::Ready(Some((p, remaining)));
                                        }
                                    }
                                }
                                Some(
                                    Stream::Next(_)
                                    | Stream::Delayed(_)
                                    | Stream::Init
                                    | Stream::Wait,
                                ) => return Poll::Pending,
                                None => return Poll::Ready(None),
                            }
                        }
                        WakeResult::NotSupported => {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                Some(Stream::Spread(items)) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                    for item in items {
                        if let StreamSpread::Pending(p) = item {
                            let remaining = this.inner.take().expect("inner should exist");
                            return Poll::Ready(Some((p, remaining)));
                        }
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

// ============================================================================
// StreamAsFutureStream
// ============================================================================

/// Wraps a [`StreamIterator`] as a `futures_core::Stream`, yielding each `Stream<D, P>` item as-is.
///
/// This is the **pure/raw** lens (returned by `into_pure_future_stream()`): it never
/// parks and forwards every item — including the in-band control states
/// `Wait`/`Pending`/`Delayed`/`Init` — unchanged. An `await` consumer that wants the
/// iterator to park on those states should use [`StreamParkingFutureStream`]
/// (returned by `into_future_stream()`) instead.
pub struct StreamAsFutureStream<SI: StreamIterator> {
    inner: Option<SI>,
}

impl<SI: StreamIterator> StreamAsFutureStream<SI> {
    pub fn new(iter: SI) -> Self {
        Self { inner: Some(iter) }
    }
}

impl<SI> FuturesStream for StreamAsFutureStream<SI>
where
    SI: StreamIterator + Unpin,
{
    type Item = Stream<SI::D, SI::P>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");
        Poll::Ready(iter.next())
    }
}

// ============================================================================
// StreamParkingFutureStream
// ============================================================================

/// WHY: [`StreamAsFutureStream`] is a raw lens whose `poll_next` is always
/// `Poll::Ready(iter.next())` — it forwards the in-band control states
/// (`Wait`/`Pending`/`Delayed`/`Init`) as items and never parks, so an `await`
/// consumer busy-polls while the iterator is not ready. This adapter fills that gap.
///
/// WHAT: A [`futures_core::Stream`] whose `Item` is the full `Stream<D, P>` enum —
/// it yields `Next(v)` and `Spread(items)` as-is, drives past `Ignore`, and ends on
/// iterator exhaustion, enabling `while let Some(item) = body.next().await` over
/// the same item type as the raw lens.
///
/// HOW: On `Next(v)`/`Spread(items)` returns `Ready(Some(..))`; on
/// `Wait`/`Init`/`Pending` from a driving stream re-polls with the bounded
/// budget; on any other `Wait`/`Pending`/`Delayed`/`Init` registers a wake
/// through [`WakeableStream::wake`] and parks (falling back to a
/// `wake_by_ref()` self-wake when the iterator reports `NotSupported`); on
/// `Ignore` loops; on `None` returns `Ready(None)` permanently.
pub struct StreamParkingFutureStream<SI: StreamIterator> {
    inner: Option<SI>,
    /// Whether the async consumer has announced itself to the stream (see
    /// [`prime`]).
    primed: bool,
    /// How many times a `Wait` from a driving stream is re-polled before the
    /// adapter parks. See [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    wait_redrive_budget: usize,
}

impl<SI: StreamIterator> StreamParkingFutureStream<SI> {
    pub fn new(iter: SI) -> Self {
        Self {
            inner: Some(iter),
            primed: false,
            wait_redrive_budget: DEFAULT_WAIT_REDRIVE_BUDGET,
        }
    }

    /// Set how many times a `Wait` from a driving stream is re-polled before
    /// the adapter parks. The default is [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    #[must_use]
    pub fn with_wait_redrive_budget(mut self, budget: usize) -> Self {
        self.wait_redrive_budget = budget;
        self
    }
}

impl<SI> FuturesStream for StreamParkingFutureStream<SI>
where
    SI: WakeableStream + Unpin,
{
    type Item = Stream<SI::D, SI::P>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        let Some(iter) = this.inner.as_mut() else {
            return Poll::Ready(None);
        };
        if !this.primed {
            this.primed = true;
            prime(iter, cx);
        }

        let mut wait_redrive_budget = this.wait_redrive_budget;

        loop {
            match iter.next() {
                Some(Stream::Next(v)) => return Poll::Ready(Some(Stream::Next(v))),
                Some(Stream::Spread(items)) => return Poll::Ready(Some(Stream::Spread(items))),
                Some(Stream::Ignore) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                }
                Some(Stream::Pending(_) | Stream::Init | Stream::Wait)
                    if iter.drives_its_producer() && iter.can_make_progress() =>
                {
                    // A driving stream runs its producer inside `next()`. On a
                    // single-threaded engine no other thread sends a wake, so
                    // re-poll instead of parking. A `Pending` progress hint or
                    // an `Init` is a control state here, not an item to wait
                    // for. Bound the re-polls so a stream that never makes
                    // progress still yields to the executor. A parked engine
                    // reports `can_make_progress() == false`; it keeps the old
                    // register-then-park behaviour below.
                    if wait_redrive_budget == 0 {
                        let waker = cx.waker().clone();
                        let _ = iter.wake_send(Box::new(move || waker.wake()));
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    wait_redrive_budget -= 1;
                }
                Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    let waker = cx.waker().clone();
                    match iter.wake_send(Box::new(move || waker.wake())) {
                        WakeResult::Successful => {
                            // Register-then-recheck: a `push` between our last
                            // `next()` (empty) and `wake()` already fired the
                            // still-empty registry and will not re-fire. Catch it
                            // here; park only if still non-target.
                            match iter.next() {
                                Some(Stream::Next(v)) => return Poll::Ready(Some(Stream::Next(v))),
                                Some(Stream::Spread(items)) => {
                                    return Poll::Ready(Some(Stream::Spread(items)));
                                }
                                Some(Stream::Ignore) => {}
                                Some(
                                    Stream::Pending(_)
                                    | Stream::Delayed(_)
                                    | Stream::Init
                                    | Stream::Wait,
                                ) => return Poll::Pending,
                                None => {
                                    this.inner = None;
                                    return Poll::Ready(None);
                                }
                            }
                        }
                        WakeResult::NotSupported => {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                None => {
                    this.inner = None;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

// ============================================================================
// StreamNextStream
// ============================================================================

/// WHY: `into_ready_future()` yields one value plus the remaining iterator, which
/// is ceremonial for consuming a body chunk-by-chunk (re-wrap loop). `into_pure_future_stream()`
/// is a raw lens that yields `Stream<D, P>` items as-is (including `Wait`) and never
/// returns `Poll::Pending`, so it cannot serve as an awaitable value stream. This
/// adapter fills the gap (F45 Resolution 5).
///
/// WHAT: A [`futures_core::Stream`] whose `Item` is `SI::D` — it surfaces `Next(v)`
/// (and `Spread` `Done(v)`) payloads, drives past in-band control states, and ends
/// on iterator exhaustion, enabling `while let Some(chunk) = body.next().await`.
///
/// HOW: On `Next(v)` returns `Ready(Some(v))`; on `Spread` buffers every `Done`
/// payload and drains them one per poll; on `Wait`/`Init`/`Pending` from a
/// driving stream re-polls with the bounded budget; on any other
/// `Wait`/`Pending`/`Delayed`/`Init` registers a wake through
/// [`WakeableStream::wake`] and parks (falling back to a `wake_by_ref()`
/// self-wake when the iterator reports `NotSupported`); on `Ignore` loops; on
/// `None` returns `Ready(None)` permanently.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StreamNextStream<SI: StreamIterator> {
    inner: Option<SI>,
    /// `Done` payloads peeled from a single `Spread` item, surfaced one per poll
    /// before pulling more work from the inner iterator.
    spread: Vec<SI::D>,
    /// Whether the async consumer has announced itself to the stream (see
    /// [`prime`]).
    primed: bool,
    /// How many times a `Wait` from a driving stream is re-polled before the
    /// adapter parks. See [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    wait_redrive_budget: usize,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI: StreamIterator> StreamNextStream<SI> {
    pub fn new(iter: SI) -> Self {
        Self {
            inner: Some(iter),
            primed: false,
            spread: Vec::new(),
            wait_redrive_budget: DEFAULT_WAIT_REDRIVE_BUDGET,
        }
    }

    /// Set how many times a `Wait` from a driving stream is re-polled before
    /// the adapter parks. The default is [`DEFAULT_WAIT_REDRIVE_BUDGET`].
    #[must_use]
    pub fn with_wait_redrive_budget(mut self, budget: usize) -> Self {
        self.wait_redrive_budget = budget;
        self
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI> FuturesStream for StreamNextStream<SI>
where
    SI: WakeableStream + Unpin,
    SI::D: Unpin,
{
    type Item = SI::D;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Drain any buffered Spread(Done) payloads first (lossless fan-out).
        if !this.spread.is_empty() {
            return Poll::Ready(Some(this.spread.remove(0)));
        }

        let Some(iter) = this.inner.as_mut() else {
            return Poll::Ready(None);
        };
        if !this.primed {
            this.primed = true;
            prime(iter, cx);
        }

        let mut wait_redrive_budget = this.wait_redrive_budget;

        loop {
            match iter.next() {
                Some(Stream::Next(v)) => return Poll::Ready(Some(v)),
                Some(Stream::Ignore) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                }
                Some(Stream::Pending(_) | Stream::Init | Stream::Wait)
                    if iter.drives_its_producer() && iter.can_make_progress() =>
                {
                    // A driving stream runs its producer inside `next()`. On a
                    // single-threaded engine no other thread sends a wake, so
                    // re-poll instead of parking. A `Pending` progress hint or
                    // an `Init` is a control state here, not an item to wait
                    // for. Bound the re-polls so a stream that never makes
                    // progress still yields to the executor. A parked engine
                    // reports `can_make_progress() == false`; it keeps the old
                    // register-then-park behaviour below.
                    if wait_redrive_budget == 0 {
                        let waker = cx.waker().clone();
                        let _ = iter.wake_send(Box::new(move || waker.wake()));
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    wait_redrive_budget -= 1;
                }
                Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    let waker = cx.waker().clone();
                    match iter.wake_send(Box::new(move || waker.wake())) {
                        WakeResult::Successful => {
                            // Register-then-recheck: a `push` between our last
                            // `next()` (empty) and `wake()` already fired the
                            // still-empty registry and will not re-fire. Catch it
                            // here; park only if still non-target.
                            match iter.next() {
                                Some(Stream::Next(v)) => return Poll::Ready(Some(v)),
                                Some(Stream::Ignore) => {}
                                Some(Stream::Spread(items)) => {
                                    for item in items {
                                        if let StreamSpread::Done(v) = item {
                                            this.spread.push(v);
                                        }
                                    }
                                    if !this.spread.is_empty() {
                                        return Poll::Ready(Some(this.spread.remove(0)));
                                    }
                                }
                                Some(
                                    Stream::Pending(_)
                                    | Stream::Delayed(_)
                                    | Stream::Init
                                    | Stream::Wait,
                                ) => return Poll::Pending,
                                None => {
                                    this.inner = None;
                                    return Poll::Ready(None);
                                }
                            }
                        }
                        WakeResult::NotSupported => {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                Some(Stream::Spread(items)) => {
                    wait_redrive_budget = this.wait_redrive_budget;
                    for item in items {
                        if let StreamSpread::Done(v) = item {
                            this.spread.push(v);
                        }
                    }
                    if !this.spread.is_empty() {
                        return Poll::Ready(Some(this.spread.remove(0)));
                    }
                    // Spread carried no Done payloads; keep polling.
                }
                None => {
                    this.inner = None;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

/// Tell `iter` that an async consumer is pulling it, before the first pull:
/// announce it down any combinators. A queue-backed stream then answers an
/// empty pull with `Wait` at once, instead of blocking the polling thread for a
/// timed wait first (it only learned of the async consumer from the first
/// `wake`, after that blocking pull). The real wake is registered when a pull
/// comes back empty, as before. `announce` registers no callback, so calling it
/// here cannot leak on repeated polls.
fn prime<SI: WakeableStream>(iter: &mut SI, _cx: &Context<'_>) {
    iter.announce();
}
