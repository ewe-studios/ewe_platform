//! Mapped Pipe adapters — zero-task, zero-alloc type transforms (F37/F39).
//!
//! `MappedSender` wraps a `PipeSender<Inner>` with a `&Outer → Inner` transform,
//! applied inline on every send. `FilterMapReceiver` wraps a `PipeReceiver<Inner>`
//! with a `Inner → Option<Outer>` transform. No extra tasks, no loops — one
//! push/pop per call.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use std::sync::Mutex;

use crate::valtron::pipe::{PipeReceiver, PipeSender, SendError};
use crate::valtron::{QueueClosedReadiness, QueueReadiness, QueueVacancyReadiness};
use concurrent_queue::{PopError, PushError};

// ═══════════════════════════════════════════════════════════════════════════════
// MappedSender — PipeSender<Inner> → PipeSender<Outer> via &Outer → Inner
// ═══════════════════════════════════════════════════════════════════════════════

/// Wraps a `PipeSender<Inner>` and applies `&Outer → Inner` inline on every push.
///
/// Created via `PipeSender::map_to()`.
pub struct MappedSender<Inner, Outer, F> {
    inner: PipeSender<Inner>,
    f: F,
    /// Single-slot stash: when `try_send` hits Full, the already-transformed
    /// inner item is saved here. Drained first on the next poll — no data loss.
    stashed: Mutex<Option<Inner>>,
    _p: core::marker::PhantomData<Outer>,
}

impl<Inner, Outer, F> MappedSender<Inner, Outer, F>
where
    F: Fn(&Outer) -> Inner,
{
    /// Non-blocking: borrows `outer`, applies `f(&outer)`, pushes the inner item.
    /// Drains the stash first if one was left from a previous Full.
    ///
    /// Returns `Err(())` on `Full` (caller retries next poll — `outer` was not
    /// consumed). Returns `Ok(())` on success. The stash absorbs transient Fulls.
    // A named error type would change this public signature; the feature that
    // does that also bumps the crate version. Until then, `()` has one meaning.
    ///
    /// # Errors
    /// Returns `Err(())` when the inner pipe is full. The item stays in the
    /// stash and the caller retries at the next poll.
    ///
    /// # Panics
    /// Panics if the internal stash mutex is poisoned.
    #[allow(clippy::result_unit_err)]
    pub fn try_send(&self, outer: &Outer) -> Result<(), ()> {
        // Drain stash first.
        if let Some(inner) = self.stashed.lock().unwrap().take() {
            self.inner.try_send(inner).map_err(|e| {
                if e.is_full() {
                    *self.stashed.lock().unwrap() = Some(e.into_inner());
                }
            })?;
        }

        let inner = (self.f)(outer);
        self.inner.try_send(inner).map_err(|e| {
            if e.is_full() {
                *self.stashed.lock().unwrap() = Some(e.into_inner());
            }
        })
    }

    /// Async send: takes ownership of `outer`, applies `f(&outer)`, pushes.
    /// Parks on Full via the inner pipe's waker. On Closed, returns
    /// `Err(SendError(outer))` — the caller recovers their value.
    pub fn send(&self, outer: Outer) -> MappedSendFuture<'_, Inner, Outer, F> {
        MappedSendFuture {
            sender: self,
            outer: Some(outer),
            stashed: None,
        }
    }

    pub fn close(&self) -> bool {
        self.inner.close()
    }
    pub fn is_full(&self) -> bool {
        self.inner.is_full()
    }
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
    // `is_empty` is a new public method; it needs a crate version bump, which a
    // lint-only feature must not make. Add both together in that change.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub fn vacancy(&self) -> QueueVacancyReadiness<Inner>
    where
        Inner: Send,
    {
        self.inner.vacancy()
    }

    /// Ready only once the inner pipe closes, so a task parked on it is not
    /// woken by every consumer pop. See [`PipeSender::closed_readiness`].
    pub fn closed_readiness(&self) -> QueueClosedReadiness<Inner>
    where
        Inner: Send,
    {
        self.inner.closed_readiness()
    }
}

impl<Inner, Outer, F: Clone> Clone for MappedSender<Inner, Outer, F> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            f: self.f.clone(),
            stashed: Mutex::new(None),
            _p: core::marker::PhantomData,
        }
    }
}

impl<T> PipeSender<T> {
    /// Wrap with a borrow-based transform: caller sends `&Outer`, pipe sees `Inner`.
    ///
    /// ```
    /// use foundation_core::valtron::Pipe;
    ///
    /// enum Message {
    ///     Binary(Vec<u8>),
    ///     Text(String),
    /// }
    ///
    /// let (ws_tx, ws_rx) = Pipe::<Message>::with_depth(4);
    /// // The caller sends `&Vec<u8>`; the pipe holds a `Message`.
    /// let byte_tx = ws_tx.map_to(|bytes: &Vec<u8>| Message::Binary(bytes.clone()));
    ///
    /// byte_tx.try_send(&b"hello".to_vec()).expect("send");
    /// match ws_rx.try_recv().expect("receive") {
    ///     Message::Binary(bytes) => assert_eq!(bytes, b"hello".to_vec()),
    ///     Message::Text(_) => panic!("the transform wraps bytes"),
    /// }
    ///
    /// // The same transform on the async path.
    /// futures_lite::future::block_on(byte_tx.send(b"world".to_vec())).expect("send");
    /// match ws_rx.try_recv().expect("receive") {
    ///     Message::Binary(bytes) => assert_eq!(bytes, b"world".to_vec()),
    ///     Message::Text(_) => panic!("the transform wraps bytes"),
    /// }
    /// ```
    pub fn map_to<Outer, F: Fn(&Outer) -> T>(self, f: F) -> MappedSender<T, Outer, F> {
        MappedSender {
            inner: self,
            f,
            stashed: Mutex::new(None),
            _p: core::marker::PhantomData,
        }
    }
}

/// Future for [`MappedSender::send`].
pub struct MappedSendFuture<'a, Inner, Outer, F> {
    sender: &'a MappedSender<Inner, Outer, F>,
    outer: Option<Outer>,
    /// Already-transformed inner item, ready for retry after Full.
    stashed: Option<Inner>,
}

impl<Inner, Outer, F> Unpin for MappedSendFuture<'_, Inner, Outer, F> {}

impl<Inner, Outer, F> Future for MappedSendFuture<'_, Inner, Outer, F>
where
    F: Fn(&Outer) -> Inner,
{
    type Output = Result<(), SendError<Outer>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item: Inner = if let Some(inner) = self.stashed.take() {
            inner
        } else {
            let outer = self.outer.as_ref().expect("polled after Ready");
            (self.sender.f)(outer)
        };

        match self.sender.inner.inner.queue.push(item) {
            Ok(()) => {
                self.sender.inner.inner.wake_consumer();
                Poll::Ready(Ok(()))
            }
            Err(PushError::Closed(_)) => {
                Poll::Ready(Err(SendError(self.outer.take().expect("outer consumed"))))
            }
            Err(PushError::Full(item)) => {
                *self.sender.inner.inner.producer_waker.lock().unwrap() = Some(cx.waker().clone());
                match self.sender.inner.inner.queue.push(item) {
                    Ok(()) => {
                        self.sender.inner.inner.wake_consumer();
                        Poll::Ready(Ok(()))
                    }
                    Err(PushError::Closed(_)) => {
                        Poll::Ready(Err(SendError(self.outer.take().expect("outer consumed"))))
                    }
                    Err(PushError::Full(item)) => {
                        self.stashed = Some(item);
                        Poll::Pending
                    }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FilterMapReceiver — PipeReceiver<Inner> → PipeReceiver<Outer> via &Inner → Option<Outer>
// ═══════════════════════════════════════════════════════════════════════════════

/// Wraps a `PipeReceiver<Inner>` and applies `&Inner → Option<Outer>` inline on pop.
///
/// Created via `PipeReceiver::filter_map_to()`.
pub struct FilterMapReceiver<Inner, Outer, F> {
    inner: PipeReceiver<Inner>,
    f: F,
    _p: core::marker::PhantomData<Outer>,
}

impl<Inner, Outer, F> FilterMapReceiver<Inner, Outer, F>
where
    F: Fn(&Inner) -> Option<Outer>,
{
    /// Non-blocking pop through the transform. Returns `Err(Empty)` if the inner
    /// pipe is empty, or if the transform returns `None` for the popped item.
    ///
    /// # Errors
    /// Returns `TryRecvError::Empty` when the inner pipe is empty or the
    /// transform returns `None`, and `TryRecvError::Closed` when the pipe is
    /// closed.
    pub fn try_recv(&self) -> Result<Outer, crate::valtron::pipe::TryRecvError> {
        let inner = self.inner.try_recv()?;
        (self.f)(&inner).ok_or(crate::valtron::pipe::TryRecvError::Empty)
    }

    /// Async receive through the transform. Parks on empty.
    pub fn receive(&self) -> FilterMapRecvFuture<'_, Inner, Outer, F> {
        FilterMapRecvFuture { receiver: self }
    }

    pub fn close(&self) -> bool {
        self.inner.close()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn readiness(&self) -> QueueReadiness<Inner>
    where
        Inner: Send,
    {
        self.inner.readiness()
    }
}

impl<Inner, Outer, F: Clone> Clone for FilterMapReceiver<Inner, Outer, F> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            f: self.f.clone(),
            _p: core::marker::PhantomData,
        }
    }
}

impl<T> PipeReceiver<T> {
    /// Wrap with a borrow-based filter: pipe yields `&Inner`, caller sees `Outer`.
    ///
    /// ```
    /// use foundation_core::valtron::{Pipe, TryRecvError};
    ///
    /// enum Message {
    ///     Binary(Vec<u8>),
    ///     Text(String),
    /// }
    ///
    /// let (ws_tx, ws_rx) = Pipe::<Message>::with_depth(4);
    /// // The pipe holds `Message`; the caller sees only binary bodies.
    /// let body_rx = ws_rx.filter_map_to(|msg: &Message| match msg {
    ///     Message::Binary(data) => Some(data.clone()),
    ///     _ => None,
    /// });
    ///
    /// // A filtered item is consumed and reported as empty.
    /// ws_tx.try_send(Message::Text("ignored".into())).expect("send");
    /// assert_eq!(body_rx.try_recv(), Err(TryRecvError::Empty));
    ///
    /// ws_tx.try_send(Message::Binary(b"data".to_vec())).expect("send");
    /// assert_eq!(body_rx.try_recv().expect("body"), b"data".to_vec());
    ///
    /// // The same filter on the async path: a non-matching item is skipped.
    /// ws_tx.try_send(Message::Text("ignored".into())).expect("send");
    /// ws_tx.try_send(Message::Binary(b"body".to_vec())).expect("send");
    /// let body = futures_lite::future::block_on(body_rx.receive());
    /// assert_eq!(body, Some(b"body".to_vec()));
    /// ```
    pub fn filter_map_to<Outer, F>(self, f: F) -> FilterMapReceiver<T, Outer, F> {
        FilterMapReceiver {
            inner: self,
            f,
            _p: core::marker::PhantomData,
        }
    }
}

/// Future for [`FilterMapReceiver::receive`].
pub struct FilterMapRecvFuture<'a, Inner, Outer, F> {
    receiver: &'a FilterMapReceiver<Inner, Outer, F>,
}

impl<Inner, Outer, F> Unpin for FilterMapRecvFuture<'_, Inner, Outer, F> {}

impl<Inner, Outer, F> Future for FilterMapRecvFuture<'_, Inner, Outer, F>
where
    F: Fn(&Inner) -> Option<Outer>,
{
    type Output = Option<Outer>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Try non-blocking first.
        match self.receiver.inner.inner.queue.pop() {
            Ok(item) => {
                self.receiver.inner.inner.wake_producer();
                if let Some(outer) = (self.receiver.f)(&item) {
                    return Poll::Ready(Some(outer));
                }
                // Filtered — item discarded, park and retry on wake.
            }
            Err(PopError::Closed) => return Poll::Ready(None),
            Err(PopError::Empty) => {}
        }

        // Park.
        *self.receiver.inner.inner.consumer_waker.lock().unwrap() = Some(cx.waker().clone());
        match self.receiver.inner.inner.queue.pop() {
            Ok(item) => {
                self.receiver.inner.inner.wake_producer();
                match (self.receiver.f)(&item) {
                    Some(outer) => Poll::Ready(Some(outer)),
                    None => Poll::Pending,
                }
            }
            Err(PopError::Closed) => Poll::Ready(None),
            Err(PopError::Empty) => Poll::Pending,
        }
    }
}
