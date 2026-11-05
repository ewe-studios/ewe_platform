//! The steering readinesses wake a parked task on a push (spec 70 feature 81).
//!
//! WHY: the steering queues were plain `ConcurrentQueue`s, so a task parked on
//! `priority_readiness` or `followup_readiness` learned about a pushed message
//! only from a timer re-check at the readiness poll quantum (1 ms). The queues
//! are now `EventQueue`s, whose push wakes the parked task by itself.
//!
//! WHAT: one test per readiness. A task parks on it, another thread pushes
//! 200 ms later, and the task runs again. A counting adapter (public trait
//! only) proves the executor checked the readiness at most [`MAX_CHECKS`]
//! times, so the wake does not come from the 1 ms re-check. The wake delay is
//! reported and held under a loose limit: it measures thread scheduling, not
//! the wake path.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_ai::agentic::internals::SteeringQueues;
use foundation_ai::types::{base_types::TextContent, MessageRole, Messages, UserModelContent};
use foundation_core::valtron::{
    execute, valtron_test, Ctx, EventReadiness, EventReadinessPtr, ReadinessWaker, StreamExt,
    StreamIteratorExt, TaskIterator, TaskStatus,
};

/// How long the producer waits before it pushes, so the consumer is parked.
const TRIGGER_DELAY: Duration = Duration::from_millis(200);

/// The most time between the push and the task running again. This bound only
/// catches a wake that never comes promptly: the delay measures thread
/// scheduling, so it is loose (an emulated amd64 CI run measured 19 ms once).
/// [`MAX_CHECKS`] is the proof that the wake is event-driven.
const WAKE_LATENCY_LIMIT: Duration = Duration::from_millis(100);

/// The most readiness checks a parked task may cost. The executor checks the
/// readiness twice before it parks (once on its own, once after it registers the
/// waker) and twice when the wake releases it, so an event-driven park makes 4.
/// A 1 ms re-check over the 200 ms park makes about 200, so the bound tells the
/// two apart.
const MAX_CHECKS: usize = 4;

/// Counts the `is_ready` calls of the readiness it wraps and forwards the rest.
struct Counting {
    inner: EventReadinessPtr,
    checks: Arc<AtomicUsize>,
}

impl EventReadiness for Counting {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.inner.is_ready(dur)
    }

    fn deadline(&self) -> Option<Instant> {
        self.inner.deadline()
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.inner.register(waker)
    }
}

/// Parks once on a readiness, then reports the parked time and the delay
/// between the push and the wake.
struct ParkThenReport {
    parked_at: Instant,
    signal: Option<EventReadinessPtr>,
    pushed_at: Arc<Mutex<Option<Instant>>>,
    reported: bool,
}

impl TaskIterator for ParkThenReport {
    type Ready = (Duration, Option<Duration>);
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if let Some(signal) = self.signal.take() {
            return Some(TaskStatus::Depends(signal));
        }
        if self.reported {
            return None;
        }
        self.reported = true;
        let after_push = self.pushed_at.lock().unwrap().map(|at| at.elapsed());
        Some(TaskStatus::Ready((self.parked_at.elapsed(), after_push)))
    }
}

/// Park a task on `label`'s readiness; run `trigger` on another thread after
/// [`TRIGGER_DELAY`]. Returns the milliseconds until the task ran again and the
/// readiness checks it cost.
async fn park_and_trigger<F>(label: &str, signal: EventReadinessPtr, trigger: F) -> (u128, usize)
where
    F: FnOnce() + Send + 'static,
{
    assert!(
        signal.register(&ReadinessWaker::noop()),
        "{label}: the readiness must take the task's waker"
    );
    let checks = Arc::new(AtomicUsize::new(0));
    let counted: EventReadinessPtr = Arc::new(Counting {
        inner: signal,
        checks: Arc::clone(&checks),
    });
    let pushed_at = Arc::new(Mutex::new(None));
    let parked_at = Instant::now();
    let mut stream = execute(
        ParkThenReport {
            parked_at,
            signal: Some(counted),
            pushed_at: Arc::clone(&pushed_at),
            reported: false,
        },
        None,
    )
    .expect("execute")
    .into_next_stream();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(TRIGGER_DELAY);
        *pushed_at.lock().unwrap() = Some(Instant::now());
        trigger();
    });
    let (parked, after_push) = stream.next().await.expect("the task yields a value");
    thread.join().expect("trigger thread");
    let after_push = after_push.expect("the push stamp is set before the wake");
    let checks = checks.load(Ordering::SeqCst);
    eprintln!(
        "{label}: woke {} us after the push, {checks} checks, parked {} ms",
        after_push.as_micros(),
        parked.as_millis()
    );
    assert!(
        parked >= TRIGGER_DELAY,
        "{label}: the task ran after {} ms; the push came at {} ms",
        parked.as_millis(),
        TRIGGER_DELAY.as_millis()
    );
    assert!(
        after_push < WAKE_LATENCY_LIMIT,
        "{label}: the task woke {} us after the push; the limit is {} us",
        after_push.as_micros(),
        WAKE_LATENCY_LIMIT.as_micros()
    );
    assert!(
        checks <= MAX_CHECKS,
        "{label}: the readiness was checked {checks} times; the bound is {MAX_CHECKS}"
    );
    (after_push.as_micros(), checks)
}

fn user_msg(text: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: text.into(),
            signature: None,
        }),
        signature: None,
    }
}

#[valtron_test(threads = 2, timeout = 2_000)]
async fn priority_readiness_wakes_on_push_priority() {
    let queues = Arc::new(SteeringQueues::new());
    let ready = queues.priority_readiness();
    let _ = park_and_trigger("priority_readiness", ready, move || {
        queues.push_priority(user_msg("stop"));
    })
    .await;
}

#[valtron_test(threads = 2, timeout = 2_000)]
async fn followup_readiness_wakes_on_push_follow_up() {
    let queues = Arc::new(SteeringQueues::new());
    let ready = queues.followup_readiness();
    let _ = park_and_trigger("followup_readiness", ready, move || {
        queues.push_follow_up(user_msg("next"));
    })
    .await;
}
