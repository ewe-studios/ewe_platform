//! Worker-panic accounting and panic-payload delivery.
//!
//! Two defects live here:
//!
//! - Issue #241: a worker that panicked sent BOTH `ThreadActivity::Panicked`
//!   and `ThreadActivity::Stopped`. The listener decrements `live_threads` for
//!   each, so the counter drifted down by one per panic.
//! - Issue #242: `block_on_future` (behind `#[valtron_test]` async bodies) used
//!   to report a generic "produced no result" panic when the body panicked,
//!   hiding the real assertion failure.
//! - Issue #243: nothing read the worker-activity queue during normal pool
//!   operation, so the `Panicked` arm never ran and a worker-level panic lost a
//!   worker for good.
//!
//! Each test is gated on `multi`: the worker registry and the pool-polled
//! `block_on_future` only exist under that feature. `cranelift` cannot unwind,
//! so the panic tests are ignored there (see `docs/panic_unwinding.md`).

use std::any::Any;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

// Name the polyfilled instant (root rule: always use
// `foundation_compact::Instant`, never a stand-in alias). On native targets it
// is `std::time::Instant`; on wasm32 it is the host-clock instant.
use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::synca::Entry;
use foundation_core::valtron::multi::{is_pool_initialized, PoolConfig, ThreadRegistry};
use foundation_core::valtron::{
    block_on_future, valtron_test, BoxedExecutionEngine, ExecutionIterator, State,
};

/// An [`ExecutionIterator`] that panics inside executor-owned code.
///
/// WHY: valtron wraps *tasks* in `catch_unwind`, so a panicking `TaskIterator`
/// does not kill its worker. Only a panic the executor does not catch reaches
/// the worker-level `catch_unwind` that sends `ThreadActivity::Panicked`. This
/// iterator goes straight into the shared task queue, so its panic takes that
/// exact path — the worker-panic path issue #241 is about.
///
/// The sender fires just before the panic so the test knows the worker reached
/// the task. The panic event is sent right after the unwind.
struct PanickingExecutionIterator {
    fired: mpsc::Sender<()>,
}

impl ExecutionIterator for PanickingExecutionIterator {
    fn next(
        &mut self,
        _parent_id: Entry,
        _engine: BoxedExecutionEngine,
        _ctx: Ctx,
    ) -> Option<State> {
        let _ = self.fired.send(());
        panic!("intentional worker panic: issue #241 test");
    }
}

/// Issue #241: a worker panic must count as ONE exit.
///
/// WHAT: builds a private registry with two workers, makes one worker panic
/// outside the per-task `catch_unwind`, drains the activity queue, and asserts
/// the live-thread counter still reports two workers.
///
/// HOW: drives the queue with the `#[doc(hidden)]` `pump_activity` hook. This
/// registry has no activity monitor, so the test stays the only reader; the
/// hook and a running monitor compete for the same events. Before the fix the
/// panic sent `Panicked` and then `Stopped`, so the second decrement left the
/// counter at one while two workers were alive.
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding: the worker panic aborts the process"
)]
fn panicking_worker_does_not_drift_live_threads() {
    const WORKERS: usize = 2;

    let config = PoolConfig::default().with_op_read_time(Duration::from_millis(5));
    let registry = Arc::new(ThreadRegistry::with_config(7, WORKERS, &config));

    for _ in 0..WORKERS {
        registry.spawn_worker().expect("should spawn worker");
    }
    assert_eq!(
        registry.live_threads(),
        WORKERS,
        "the registry must start with its configured worker count"
    );

    let (fired_tx, fired_rx) = mpsc::channel();
    // `PushError` carries the boxed iterator and so has no `Debug` impl here.
    let queued = registry
        .shared_tasks()
        .push(Box::new(PanickingExecutionIterator { fired: fired_tx }));
    assert!(queued.is_ok(), "should queue the panicking iterator");
    registry.latch().signal_all();

    fired_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a worker must poll the panicking iterator");

    // The worker sends `Panicked` immediately after the panic unwinds. Let that
    // send land, then drain every activity event it produced.
    std::thread::sleep(Duration::from_millis(100));
    let deadline = Instant::now() + Duration::from_secs(5);
    while registry.pump_activity() {
        assert!(
            Instant::now() < deadline,
            "timed out draining worker activity after the panic"
        );
    }

    assert_eq!(
        registry.live_threads(),
        WORKERS,
        "a panicking worker must decrement live_threads once, not twice: \
         sending both Panicked and Stopped drifted the counter down"
    );

    registry.shutdown();
}

/// An [`ExecutionIterator`] that reports that it ran, then finishes.
struct NotifyingExecutionIterator {
    done: mpsc::Sender<()>,
}

impl ExecutionIterator for NotifyingExecutionIterator {
    fn next(
        &mut self,
        _parent_id: Entry,
        _engine: BoxedExecutionEngine,
        _ctx: Ctx,
    ) -> Option<State> {
        let _ = self.done.send(());
        None
    }
}

/// Issue #243: a panicking worker must be replaced while the pool runs.
///
/// WHAT: a registry with two workers panics both outside the per-task
/// `catch_unwind`, starts the activity monitor, then queues a normal task. The
/// task can only run if the pool replaced at least one dead worker.
///
/// HOW: the `Panicked` arm of the activity handler is the only code that
/// respawns a worker, and it runs only when an event is read from the activity
/// queue. Before the fix nothing read the queue during normal operation, so the
/// pool lost one worker for good on every worker-level panic and the queued
/// task never ran. The test drains through
/// [`ThreadRegistry::start_activity_monitor`], the same path
/// [`foundation_core::valtron::multi::initialize_pool_with_config_and_ctx`] now
/// starts.
///
/// The registry requires at least two workers, so the test panics two. Each
/// `Panicked` marker comes from a thread that died moments later, so two
/// markers mean both original workers are dead. No thread is left to read the
/// follow-up task unless a replacement started.
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding: the worker panic aborts the process"
)]
fn panicking_worker_is_respawned_while_the_pool_runs() {
    const WORKERS: usize = 2;

    // A short idle yield keeps `shutdown` from waiting the 4 s default for a
    // respawned worker that parks just after the yielder interrupt.
    let config = PoolConfig::default()
        .with_op_read_time(Duration::from_millis(5))
        .with_yield_wait_time(Duration::from_millis(20));
    let registry = Arc::new(ThreadRegistry::with_config(11, WORKERS, &config));
    for _ in 0..WORKERS {
        registry.spawn_worker().expect("should spawn worker");
    }
    registry.start_activity_monitor();

    let (fired_tx, fired_rx) = mpsc::channel();
    for _ in 0..WORKERS {
        let queued = registry
            .shared_tasks()
            .push(Box::new(PanickingExecutionIterator {
                fired: fired_tx.clone(),
            }));
        assert!(queued.is_ok(), "should queue the panicking iterator");
    }
    drop(fired_tx);
    registry.latch().signal_all();

    for worker in 0..WORKERS {
        fired_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| {
                panic!("worker {worker} must poll a panicking iterator");
            });
    }

    // Every worker is now dead or dying. Queue work that only a replacement can
    // pick up.
    let (done_tx, done_rx) = mpsc::channel();
    let queued = registry
        .shared_tasks()
        .push(Box::new(NotifyingExecutionIterator { done: done_tx }));
    assert!(queued.is_ok(), "should queue the follow-up iterator");
    registry.latch().signal_all();

    done_rx.recv_timeout(Duration::from_secs(5)).expect(
        "the activity monitor must respawn a panicked worker, or no thread is \
         left to run the queued task (issue #243)",
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while registry.live_threads() != WORKERS {
        assert!(
            Instant::now() < deadline,
            "the respawns must restore the worker count; live_threads = {}",
            registry.live_threads()
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    registry.shutdown();
}

/// Read a panic payload as a string, whether the panic used a `&str` or a
/// `String` message.
fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        String::from("<panic payload is not a string>")
    }
}

/// Issue #242: `block_on_future` must re-raise the task's panic payload.
///
/// WHAT: runs a future that panics with a unique marker on a pool worker, from
/// a second thread that blocks in `block_on_future`, and checks the panic the
/// blocking thread catches.
///
/// HOW: before the fix the executor dropped the payload and closed the result
/// channel, so `block_on_future` panicked with the generic
/// "future produced no result" message and the marker was lost.
#[valtron_test(threads = 3)]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding: the task panic aborts the process"
)]
fn block_on_future_reraises_the_task_panic_payload() {
    const MARKER: &str = "unique valtron body panic marker 7c1f";

    assert!(
        is_pool_initialized(),
        "the #[valtron_test] harness must have installed the global pool"
    );

    let body = std::thread::spawn(|| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            block_on_future(async move {
                panic!("{MARKER}");
            })
        }))
    });

    let outcome = body
        .join()
        .expect("the blocking thread must not abort the test process");
    let payload = outcome.expect_err("block_on_future must re-raise the task panic");
    let message = panic_payload_message(payload.as_ref());

    assert!(
        message.contains(MARKER),
        "block_on_future must surface the original panic payload; got: {message}"
    );
    assert!(
        !message.contains("produced no result"),
        "the generic no-result panic masks the real failure; got: {message}"
    );
}

/// `block_on_future` returns the future's own output, including `None`.
///
/// WHAT: a future whose output is `Option::None` is a normal completion, not a
/// missing result. The panic slot watches the *task* for a panic; it does not
/// inspect the value. This test pins that a `None` output comes back as `None`
/// and does not trip the "produced no result" branch.
#[valtron_test(threads = 3)]
fn block_on_future_returns_a_none_output_without_panicking() {
    let none: Option<u32> = block_on_future(async { None });
    assert_eq!(
        none, None,
        "a `None` output is a value, not a missing result"
    );

    let some: Option<u32> = block_on_future(async { Some(7) });
    assert_eq!(some, Some(7));
}
