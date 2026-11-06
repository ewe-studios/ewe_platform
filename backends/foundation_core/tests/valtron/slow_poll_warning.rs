#![cfg(feature = "multi")]

//! Feature 139: the slow-poll watchdog names a task that blocks a pool worker.
//!
//! A blocking call inside a task body holds the worker that polls it, so a
//! small pool starves silently. With `PoolConfig::slow_poll_warning` set, the
//! executor emits `tracing::warn!(target: "valtron::executor", task = …,
//! micros = …, "a task poll blocked a worker")` at most once per task per
//! second.
//!
//! The capture installs a scoped subscriber before the pool starts: a worker
//! captures the ambient dispatcher at spawn time and pins it for its life, so
//! the subscriber must exist first.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::multi::{initialize_pool_with_config, spawn, PoolConfig};
use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};
use tracing::field::{Field, Visit};
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;

/// One captured slow-poll warning: the task name and the elapsed microseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SlowPollEvent {
    task: String,
    micros: u64,
}

/// Records every `valtron::executor` warning and its `task`/`micros` fields.
struct SlowPollCapture {
    events: Arc<Mutex<Vec<SlowPollEvent>>>,
}

impl<S> Layer<S> for SlowPollCapture
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().target() != "valtron::executor" {
            return;
        }
        if event.metadata().level() != &Level::WARN {
            return;
        }
        let mut visitor = SlowPollVisitor::default();
        event.record(&mut visitor);
        if let (Some(task), Some(micros)) = (visitor.task, visitor.micros) {
            self.events
                .lock()
                .unwrap()
                .push(SlowPollEvent { task, micros });
        }
    }
}

#[derive(Default)]
struct SlowPollVisitor {
    task: Option<String>,
    micros: Option<u64>,
}

impl Visit for SlowPollVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "task" {
            self.task = Some(value.to_string());
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "micros" {
            self.micros = Some(value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
        // `task = %name` records through `record_debug` (tracing's
        // `DisplayValue` uses it).
        if field.name() == "task" && self.task.is_none() {
            self.task = Some(format!("{value:?}"));
        }
    }
}

/// Run `body` with a scoped capture subscriber installed.
///
/// WHY: the worker threads pin the ambient dispatcher at spawn time. The pool
/// must therefore start inside the closure.
fn with_capture<F: FnOnce(Arc<Mutex<Vec<SlowPollEvent>>>)>(body: F) {
    let events = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(SlowPollCapture {
            events: events.clone(),
        }),
        || {
            // `tracing` caches each callsite's interest process-wide. Rebuild
            // it now that this subscriber is the default for this thread.
            tracing::callsite::rebuild_interest_cache();
            body(events.clone());
        },
    );
}

fn wait_for(condition: impl Fn() -> bool, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    condition()
}

/// Blocks 50 ms on its first poll, then completes. The worker that polls it is
/// held for those 50 ms, so the watchdog must report it.
struct SlowOnceTask {
    done: Arc<AtomicBool>,
}

impl TaskIterator for SlowOnceTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done.load(Ordering::SeqCst) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
        self.done.store(true, Ordering::SeqCst);
        Some(TaskStatus::Ready(()))
    }
}

#[test]
fn a_slow_poll_is_reported() {
    with_capture(|events| {
        let _guard = initialize_pool_with_config(
            139,
            Some(3),
            PoolConfig::default().with_slow_poll_warning(Some(Duration::from_millis(10))),
        );

        let done = Arc::new(AtomicBool::new(false));
        spawn()
            .with_task(SlowOnceTask { done: done.clone() })
            .schedule()
            .expect("schedule the slow task");

        assert!(
            wait_for(|| done.load(Ordering::SeqCst), Duration::from_secs(5)),
            "the slow task did not run"
        );
        assert!(
            wait_for(
                || !events.lock().unwrap().is_empty(),
                Duration::from_secs(5)
            ),
            "a 50 ms poll with a 10 ms threshold must emit a warning"
        );

        let events = events.lock().unwrap();
        let warning = events.first().expect("one warning");
        assert!(
            warning.task.contains("SlowOnceTask"),
            "the warning must name the task, got {:?}",
            warning.task
        );
        assert!(
            warning.micros >= 50_000,
            "the warning must report at least 50 ms, got {} micros",
            warning.micros
        );
    });
}

/// Polls 100 times without blocking; the watchdog must stay silent.
struct FastTask {
    polls: usize,
    done: Arc<AtomicBool>,
}

impl TaskIterator for FastTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls += 1;
        if self.polls >= 100 {
            self.done.store(true, Ordering::SeqCst);
            return None;
        }
        Some(TaskStatus::Ready(()))
    }
}

#[test]
fn a_fast_poll_is_silent() {
    with_capture(|events| {
        let _guard = initialize_pool_with_config(
            140,
            Some(3),
            PoolConfig::default().with_slow_poll_warning(Some(Duration::from_millis(50))),
        );

        let done = Arc::new(AtomicBool::new(false));
        spawn()
            .with_task(FastTask {
                polls: 0,
                done: done.clone(),
            })
            .schedule()
            .expect("schedule the fast task");

        assert!(
            wait_for(|| done.load(Ordering::SeqCst), Duration::from_secs(5)),
            "the fast task did not finish its 100 polls"
        );
        // Let a straggling poll land before the subscriber goes away.
        std::thread::sleep(Duration::from_millis(50));

        let events = events.lock().unwrap();
        assert!(
            events.is_empty(),
            "a fast poll must not warn, got {events:?}"
        );
    });
}

/// Blocks 50 ms on every poll for about 300 ms. The one-per-second rate limit
/// must keep the warning count at two or fewer.
struct RepeatingSlowTask {
    start: Instant,
    polls: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
}

impl TaskIterator for RepeatingSlowTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        if self.start.elapsed() >= Duration::from_millis(300) {
            self.done.store(true, Ordering::SeqCst);
            return None;
        }
        Some(TaskStatus::Wait)
    }
}

#[test]
fn the_warning_repeats_at_most_once_per_second() {
    with_capture(|events| {
        let _guard = initialize_pool_with_config(
            141,
            Some(3),
            PoolConfig::default().with_slow_poll_warning(Some(Duration::from_millis(10))),
        );

        let polls = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        spawn()
            .with_task(RepeatingSlowTask {
                start: Instant::now(),
                polls: polls.clone(),
                done: done.clone(),
            })
            .schedule()
            .expect("schedule the repeating slow task");

        assert!(
            wait_for(|| done.load(Ordering::SeqCst), Duration::from_secs(5)),
            "the repeating slow task must reach its stop condition"
        );
        assert!(
            polls.load(Ordering::SeqCst) >= 2,
            "the repeating task must run at least twice"
        );
        // Let the last measured poll's warning land before the subscriber goes
        // away (the watchdog emits after the task step returns).
        std::thread::sleep(Duration::from_millis(50));

        let events = events.lock().unwrap();
        assert!(
            !events.is_empty(),
            "the repeating 50 ms polls must warn at least once"
        );
        assert!(
            events.len() <= 2,
            "300 ms of slow polls must warn at most twice, got {}: {events:?}",
            events.len()
        );
        let poll_count = polls.load(Ordering::SeqCst);
        assert!(
            poll_count > events.len(),
            "the rate limit must suppress a warning for repeated slow polls: \
             {poll_count} polls produced {} warnings",
            events.len()
        );
    });
}
