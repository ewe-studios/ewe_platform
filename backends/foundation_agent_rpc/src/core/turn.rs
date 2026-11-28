//! The guarded turn: wire events, the one-turn claim and the idle timeout.
//!
//! WHY: a client sees one stream of wire events. The stream must hold the
//! instance's one-turn claim, must stop a turn that makes no progress, and must
//! abort the session when the client goes away.
//!
//! WHAT: [`wire_event`] (one model event to one wire event) and
//! [`guarded_turn`] (the stream that a registry returns).
//!
//! HOW: the stream polls the model events and a timer at the same time. The
//! timer comes from [`Ctx::timer`], so a simulation can test it with no real
//! wait.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use foundation_ai::agentic::{TurnEvent as ModelTurnEvent, TurnSummary};
use foundation_ai::types::UserModelContent;
use foundation_compact::{Duration, Instant};
use foundation_context::env::timer::TimerFuture;
use foundation_context::Ctx;
use futures::Stream;

use crate::core::config::TurnLimits;
use crate::core::registry::TurnEventStream;
use crate::proto::v1 as pb;

/// The shutdown signal of the running turns.
///
/// WHY: a graceful shutdown must end each running turn with one terminal
/// event, not with a dropped connection. The signal wakes every turn stream so
/// the stream can emit `TurnFailed("server shutting down")`.
///
/// WHAT: `fire` (set the flag and wake every registered turn), `is_fired` and
/// `register` (the stream registers its waker while it waits).
#[derive(Clone, Default)]
pub struct TurnShutdown {
    inner: Arc<TurnShutdownInner>,
}

#[derive(Default)]
struct TurnShutdownInner {
    fired: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl std::fmt::Debug for TurnShutdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnShutdown")
            .field("fired", &self.is_fired())
            .finish()
    }
}

impl TurnShutdown {
    /// Fire the signal once and wake every waiting turn.
    pub fn fire(&self) {
        if self.inner.fired.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut wakers = self
            .inner
            .wakers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    /// True after [`fire`](Self::fire).
    #[must_use]
    pub fn is_fired(&self) -> bool {
        self.inner.fired.load(Ordering::SeqCst)
    }

    /// Register a waiting turn. A signal that fires before the registration
    /// wakes the waker at once.
    pub fn register(&self, waker: &Waker) {
        if self.is_fired() {
            waker.wake_by_ref();
            return;
        }
        let mut wakers = self
            .inner
            .wakers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !wakers.iter().any(|existing| existing.will_wake(waker)) {
            wakers.push(waker.clone());
        }
    }
}

/// A turn event on the wire. Progress hints are not part of the contract.
#[must_use]
pub fn wire_event(event: ModelTurnEvent) -> Option<pb::TurnEvent> {
    use pb::__buffa::oneof::turn_event::Event;
    let event = match event {
        ModelTurnEvent::Text(text) => Event::Text(text),
        ModelTurnEvent::Thinking(text) => Event::Thinking(text),
        ModelTurnEvent::ToolCall {
            id,
            name,
            arguments,
        } => Event::from(pb::ToolCall {
            id,
            name,
            arguments_json: arguments.map_or_else(String::new, |arguments| {
                serde_json::Value::Object(arguments).to_string()
            }),
            ..Default::default()
        }),
        ModelTurnEvent::ToolResult {
            tool_call_id,
            name,
            content,
        } => Event::from(pb::ToolResult {
            tool_call_id,
            name,
            content: match content {
                UserModelContent::Text(text) => text.content,
                UserModelContent::Image(_) => "[image]".to_string(),
            },
            ..Default::default()
        }),
        ModelTurnEvent::Retract { reason } => Event::from(pb::Retract {
            reason,
            ..Default::default()
        }),
        ModelTurnEvent::Failed(error) => Event::from(pb::TurnFailed {
            message: error.to_string(),
            ..Default::default()
        }),
        ModelTurnEvent::Done(summary) => Event::from(turn_done(&summary)),
        _ => return None,
    };
    Some(pb::TurnEvent {
        event: Some(event),
        ..Default::default()
    })
}

/// The wire `TurnDone` of one summary.
#[must_use]
pub fn turn_done(summary: &TurnSummary) -> pb::TurnDone {
    pb::TurnDone {
        message_count: summary.message_count,
        usage: usage_of(&summary.usage).into(),
        ..Default::default()
    }
}

/// The wire usage of one token snapshot.
#[must_use]
pub fn usage_of(snapshot: &foundation_ai::agentic::TokenSnapshot) -> pb::Usage {
    pb::Usage {
        input_tokens: snapshot.input,
        output_tokens: snapshot.output,
        total_tokens: snapshot.total,
        ..Default::default()
    }
}

/// The turn events of the deterministic simulation test (DST) build.
///
/// WHY: the generated `AgentService` trait needs a `Send` event stream. With
/// `multi` off, the single-threaded valtron executor keeps its wake callbacks
/// as `Box<dyn Fn()>`, which is not `Send`, so the event stream of a session
/// turn is not `Send` either. The DST build runs on one thread only:
/// [`SimRuntime`](foundation_core::valtron::sim::SimRuntime) binds the engine,
/// the clock and the effect bundle to the thread that starts it.
///
/// The type exists only in the `dst` build that also lacks `multi` and
/// `worker`. In every other build the stream is already `Send` and the type is
/// not compiled.
///
/// # Safety
///
/// The assertion is sound only while the stream stays on the thread that made
/// it. The engine that answers its wake callbacks is thread-local, so a move
/// to another thread would run the callbacks on the wrong engine. The DST
/// harness keeps the stream on its `SimRuntime` thread and never moves it.
#[cfg(all(feature = "dst", not(any(feature = "multi", feature = "worker"))))]
pub struct DstSendStream<S>(S);

#[cfg(all(feature = "dst", not(any(feature = "multi", feature = "worker"))))]
impl<S> DstSendStream<S> {
    /// Wrap `inner` for the DST build.
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self(inner)
    }
}

// SAFETY: see the type-level documentation. The DST harness keeps the stream
// on the thread of its single-threaded engine.
#[cfg(all(feature = "dst", not(any(feature = "multi", feature = "worker"))))]
unsafe impl<S> Send for DstSendStream<S> {}

#[cfg(all(feature = "dst", not(any(feature = "multi", feature = "worker"))))]
impl<S: Stream + Unpin> Stream for DstSendStream<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().0).poll_next(cx)
    }
}

/// Wrap a model event stream for one wire turn.
///
/// `guard` keeps the instance's one-turn claim until the stream ends or drops.
/// `abort` stops the session when the client goes away or a limit is reached.
pub fn guarded_turn<E, G>(
    events: E,
    guard: G,
    abort: Arc<dyn Fn() + Send + Sync>,
    shutdown: TurnShutdown,
    ctx: Ctx,
    limits: &TurnLimits,
) -> TurnEventStream
where
    E: Stream<Item = ModelTurnEvent> + Send + 'static,
    G: Send + Unpin + 'static,
{
    let start = ctx.clock().now();
    let total = limits.max_duration.map(|max| start + max);
    Box::pin(GuardedTurn {
        events: Box::pin(events),
        guard: Some(guard),
        abort,
        shutdown,
        ctx,
        idle_timeout: limits.idle_timeout,
        total_deadline: total,
        timer: None,
        finished: false,
    })
}

struct GuardedTurn<E, G> {
    events: Pin<Box<E>>,
    guard: Option<G>,
    abort: Arc<dyn Fn() + Send + Sync>,
    shutdown: TurnShutdown,
    ctx: Ctx,
    idle_timeout: Duration,
    total_deadline: Option<Instant>,
    timer: Option<TimerFuture>,
    finished: bool,
}

impl<E, G> GuardedTurn<E, G>
where
    E: Stream<Item = ModelTurnEvent> + Send + 'static,
    G: Send + Unpin + 'static,
{
    fn arm_timer(&mut self) {
        let now = self.ctx.clock().now();
        let idle = now + self.idle_timeout;
        let deadline = self.total_deadline.map_or(idle, |total| total.min(idle));
        // Spec 92: the context derives the timer. An `Env` timer is only an
        // override, so the turn reads the timer from the context.
        self.timer = Some(self.ctx.timer().sleep_until(self.ctx.clone(), deadline));
    }

    fn finish(&mut self) {
        self.finished = true;
        self.timer = None;
        self.guard = None;
    }

    fn fail(&mut self, message: &str) -> pb::TurnEvent {
        (self.abort)();
        self.finish();
        pb::TurnEvent {
            event: Some(pb::__buffa::oneof::turn_event::Event::from(
                pb::TurnFailed {
                    message: message.to_string(),
                    ..Default::default()
                },
            )),
            ..Default::default()
        }
    }
}

impl<E, G> Stream for GuardedTurn<E, G>
where
    E: Stream<Item = ModelTurnEvent> + Send + 'static,
    G: Send + Unpin + 'static,
{
    type Item = crate::error::AgentRpcResult<pb::TurnEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.finished {
                return Poll::Ready(None);
            }
            if this.shutdown.is_fired() {
                return Poll::Ready(Some(Ok(this.fail("server shutting down"))));
            }
            if this.timer.is_none() {
                this.arm_timer();
            }
            match this.events.as_mut().poll_next(cx) {
                Poll::Ready(Some(event)) => {
                    let terminal = event.is_terminal();
                    let wire = wire_event(event);
                    if terminal {
                        this.finish();
                    } else {
                        this.arm_timer();
                    }
                    if let Some(wire) = wire {
                        return Poll::Ready(Some(Ok(wire)));
                    }
                    // A progress hint: keep the claim and poll again.
                }
                Poll::Ready(None) => {
                    this.finish();
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    let timer = this
                        .timer
                        .as_mut()
                        .expect("the timer is armed before a poll");
                    match timer.as_mut().poll(cx) {
                        Poll::Pending => {
                            this.shutdown.register(cx.waker());
                            return Poll::Pending;
                        }
                        Poll::Ready(()) => {
                            this.timer = None;
                            let now = this.ctx.clock().now();
                            let total_done =
                                this.total_deadline.is_some_and(|deadline| now >= deadline);
                            if total_done {
                                return Poll::Ready(Some(Ok(this.fail("turn time limit"))));
                            }
                            return Poll::Ready(Some(Ok(this.fail("idle timeout"))));
                        }
                    }
                }
            }
        }
    }
}

impl<E, G> Drop for GuardedTurn<E, G> {
    fn drop(&mut self) {
        if !self.finished {
            (self.abort)();
        }
    }
}
