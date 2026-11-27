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
use std::sync::Arc;
use std::task::{Context, Poll};

use foundation_ai::agentic::{TurnEvent as ModelTurnEvent, TurnSummary};
use foundation_ai::types::UserModelContent;
use foundation_compact::{Duration, Instant};
use foundation_context::env::timer::TimerFuture;
use foundation_context::env::Env;
use foundation_context::Ctx;
use futures::Stream;

use crate::core::config::TurnLimits;
use crate::core::registry::TurnEventStream;
use crate::proto::v1 as pb;

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

/// Wrap a model event stream for one wire turn.
///
/// `guard` keeps the instance's one-turn claim until the stream ends or drops.
/// `abort` stops the session when the client goes away or a limit is reached.
pub fn guarded_turn<E, G>(
    events: E,
    guard: G,
    abort: Arc<dyn Fn() + Send + Sync>,
    ctx: Ctx,
    env: Env,
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
        ctx,
        env,
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
    ctx: Ctx,
    /// The effects bundle. The turn now derives its timer from the context
    /// ([`Ctx::timer`], spec 92), so this field is not read here; the registry
    /// still passes it in the `guarded_turn` contract.
    #[allow(dead_code)]
    env: Env,
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
        // Spec 92: the context derives the timer. The `Env` timer is only an
        // override, so the turn no longer reads `self.env` for it.
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
                        Poll::Pending => return Poll::Pending,
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
