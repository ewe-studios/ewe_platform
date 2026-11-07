//! A parked task wakes when its queue producer pushes (spec 70 feature 81).
//!
//! WHY: each producer in this crate used to push into a plain `ConcurrentQueue`
//! that could not wake the task parked on its readiness. The executor then
//! re-checked the readiness on a timer, at the readiness poll quantum (1 ms).
//! The queues are now `EventQueue`, whose push wakes the parked task by itself.
//!
//! WHAT: for each converted producer (watcher pump, builder change queue,
//! builder result queue, runner build queue), a task parks on the producer's
//! readiness, another thread pushes 200 ms later, and the task runs again. A
//! counting adapter (public trait only) proves the executor checked the
//! readiness at most [`MAX_CHECKS`] times, so the wake does not come from the
//! 1 ms re-check. The wake delay is reported and held under a loose limit: it
//! measures thread scheduling, not the wake path.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use concurrent_queue::ConcurrentQueue;
use foundation_core::synca::mpp;
use foundation_core::valtron::{
    execute, sleep_async, valtron_test, Ctx, EventQueue, EventReadiness, EventReadinessPtr,
    ReadinessWaker, StreamExt, StreamIteratorExt, TaskIterator, TaskStatus,
};
use foundation_nativeapis::shared::{WatchEvent, WatchEventKind};
use foundation_toolings::builder::{BuildOutput, BuildResult, ProjectBuilder, ProjectBuilderTask};
use foundation_toolings::runner::BinaryRunnerTask;
use foundation_toolings::watcher::{FileChange, FileChangeEventPump};
use foundation_toolings::{Http1, ProjectDefinition, ProxyRemoteConfig, ProxyType};

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

fn watch_event() -> WatchEvent {
    WatchEvent {
        kind: WatchEventKind::Modified,
        path: PathBuf::from("src/main.rs"),
    }
}

fn project_definition() -> ProjectDefinition {
    let source = ProxyRemoteConfig::new("127.0.0.1".into(), 3000);
    let dest = ProxyRemoteConfig::new("127.0.0.1".into(), 4000);
    ProjectDefinition {
        proxy: ProxyType::Http1(Http1::new(source, dest)),
        target_directory: "./target".into(),
        workspace_root: ".".into(),
        crate_name: "wake_test".into(),
        skip_rust_checks: true,
        stop_on_failure: false,
        reload_directories: vec!["./src".into()],
        build_directories: vec!["./src".into()],
        build_arguments: vec!["cargo".into(), "build".into()],
        run_arguments: vec!["cargo".into(), "run".into()],
        wait_before_reload: Duration::from_millis(100),
    }
}

/// A builder that returns at once, so the result queued by the background job
/// is the only thing the parked task waits for.
#[derive(Debug)]
struct InstantBuilder;

impl ProjectBuilder for InstantBuilder {
    fn name(&self) -> &'static str {
        "instant"
    }

    fn should_build(&self, _change: &FileChange) -> bool {
        true
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        Ok(BuildOutput::CheckPassed)
    }
}

#[valtron_test(threads = 2, timeout = 2_000)]
async fn watcher_pump_ready_wakes_on_push() {
    let pump = FileChangeEventPump::new();
    let ready: EventReadinessPtr = Arc::new(pump.ready.clone());
    let event = watch_event();
    let _ = park_and_trigger("watcher pump", ready, move || pump.push(&event)).await;
}

#[valtron_test(threads = 2, timeout = 2_000)]
async fn builder_change_readiness_wakes_on_a_watch_event() {
    let (tx, rx) = mpp::unbounded::<WatchEvent>();
    let task = ProjectBuilderTask::new(rx, EventQueue::unbounded());
    let ready: EventReadinessPtr = task.change_readiness();
    let event = watch_event();
    let _ = park_and_trigger("builder change source", ready, move || {
        tx.send(event).expect("send watch event");
    })
    .await;
}

#[valtron_test(threads = 2, timeout = 2_000)]
async fn builder_result_readiness_wakes_on_a_build_result() {
    let (tx, rx) = mpp::unbounded::<WatchEvent>();
    let mut task = ProjectBuilderTask::new(rx, EventQueue::unbounded()).builder(InstantBuilder);
    let ready: EventReadinessPtr = task.result_readiness();
    let event = watch_event();
    let _ = park_and_trigger("builder result queue", ready, move || {
        tx.send(event).expect("send watch event");
        let _ = task.next_status(Ctx::host());
    })
    .await;
}

#[valtron_test(threads = 2, timeout = 2_000)]
async fn runner_build_readiness_wakes_on_a_build_complete() {
    let build_queue = EventQueue::<()>::unbounded();
    let pusher = build_queue.clone();
    let running_tx = Arc::new(ConcurrentQueue::<()>::unbounded());
    let runner = BinaryRunnerTask::new(
        project_definition(),
        build_queue,
        running_tx,
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    );
    let ready: EventReadinessPtr = runner.readiness();
    let _ = park_and_trigger("runner build queue", ready, move || {
        pusher.push(()).expect("push");
    })
    .await;
}

// -- The whole builder task wakes on a watch event (issue #201)

/// A builder that counts its builds, so a test sees when one ran.
#[derive(Debug)]
struct CountingBuilder {
    builds: Arc<AtomicUsize>,
}

impl ProjectBuilder for CountingBuilder {
    fn name(&self) -> &'static str {
        "counting"
    }

    fn should_build(&self, _change: &FileChange) -> bool {
        true
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        self.builds.fetch_add(1, Ordering::SeqCst);
        Ok(BuildOutput::CheckPassed)
    }
}

/// Wait until the builder ran `want` builds. Fail after about five seconds.
async fn wait_for_builds(builds: &AtomicUsize, want: usize) {
    for _ in 0..500 {
        if builds.load(Ordering::SeqCst) >= want {
            return;
        }
        sleep_async(Duration::from_millis(10)).await;
    }
    panic!(
        "the builder ran {} builds; expected {want}",
        builds.load(Ordering::SeqCst)
    );
}

/// A watch event wakes the parked task once, without a poll loop. On the old
/// code the task parked on a queue that nothing pushed, so it never ran.
#[valtron_test(threads = 2, timeout = 2_000)]
async fn a_watch_event_wakes_the_builder_task() {
    let (tx, rx) = mpp::unbounded::<WatchEvent>();
    let task = ProjectBuilderTask::new(rx, EventQueue::unbounded());
    let ready: EventReadinessPtr = task.readiness();
    let event = watch_event();
    let _ = park_and_trigger("builder task", ready, move || {
        tx.send(event).expect("send watch event");
    })
    .await;
}

/// Two watch events make two builds. On the old code nothing woke the task, so
/// the first event was lost and zero builds ran.
#[valtron_test(threads = 2, timeout = 5_000)]
async fn two_watch_events_produce_two_builds() {
    let (tx, rx) = mpp::unbounded::<WatchEvent>();
    let builds = Arc::new(AtomicUsize::new(0));
    let task = ProjectBuilderTask::new(rx, EventQueue::unbounded()).builder(CountingBuilder {
        builds: Arc::clone(&builds),
    });
    let _stream = execute(task, None).expect("execute the builder task");

    tx.send(watch_event()).expect("first watch event");
    wait_for_builds(&builds, 1).await;

    tx.send(watch_event()).expect("second watch event");
    wait_for_builds(&builds, 2).await;

    assert_eq!(
        builds.load(Ordering::SeqCst),
        2,
        "two watch events produce exactly two builds"
    );
}

/// The composite readiness drops after the task drains the watch queue, so the
/// executor does not re-run it in a hot loop. On the old code the change queue
/// kept the event and stayed ready.
#[test]
fn the_task_reports_not_ready_after_it_drains_the_queue() {
    let (tx, rx) = mpp::unbounded::<WatchEvent>();
    let mut task = ProjectBuilderTask::new(rx, EventQueue::unbounded());
    tx.send(watch_event()).expect("send watch event");

    let _ = task.next_status(Ctx::host());

    let ready = task.readiness();
    assert!(
        !ready.is_ready(None),
        "the task is not ready after it drains the watch event"
    );
}
