//! Integration tests for the panic handler on the spawn builders.
//!
//! WHY: `TaskSpawnConfig::with_panic_handler` used to be stored and never
//! installed, so every terminal (`spawn`, `recv`, `stream`, `action`) dropped
//! the payload of a panicking task. An `InlineAction` had the same gap: it
//! built its `ConsumingIter` with no handler at all (issue #244).
//!
//! WHAT: One test per changed terminal proves the handler receives the panic
//! payload of the task it spawned.

#![cfg_attr(
    not(feature = "multi"),
    expect(
        clippy::arc_with_non_send_sync,
        reason = "one Arc queue serves both trees; the single-threaded task payload is not Send + Sync and never crosses a thread"
    )
)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_context::Ctx;
use foundation_core::valtron::{
    spawn_builder, ExecutionAction, PriorityOrder, ProgressIndicator, SharedTaskQueue,
    TaskIterator, TaskStatus,
};
use foundation_core::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
    valtron::{LocalThreadExecutor, ProcessController},
};

/// A task that panics on its first poll, with a fixed, recognisable message.
struct PanicsOnFirstPoll;

impl TaskIterator for PanicsOnFirstPoll {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        panic!("intentional panic from a spawned task");
    }
}

#[derive(Default, Clone)]
struct NoYielder;

impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _duration: Duration) {}
}

fn new_executor() -> LocalThreadExecutor<NoYielder> {
    let seed = fastrand::u64(..);
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));
    LocalThreadExecutor::from_seed(
        seed,
        "test".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        Duration::from_secs(10),
        None,
        None,
    )
}

/// A shared slot the panic handler writes the payload's message into.
type Slot = Arc<Mutex<Option<String>>>;

fn new_slot() -> Slot {
    Arc::new(Mutex::new(None))
}

/// A handler that stores the panic message for the test to read afterwards.
fn recording_handler(slot: Slot) -> impl Fn(Box<dyn std::any::Any + Send>) + Send + Sync + 'static {
    move |payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_string());
        *slot.lock().unwrap() = message;
    }
}

fn assert_payload_recorded(slot: &Slot) {
    let recorded = slot.lock().unwrap().clone();
    assert_eq!(
        recorded.as_deref(),
        Some("intentional panic from a spawned task")
    );
}

/// WHY: `recv` must install the builder's handler on the `ConsumingIter` it
/// drives (issue #244).
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding: the task's panic aborts the process"
)]
fn recv_delivers_the_panic_payload_to_the_handler() {
    let executor = new_executor();
    let slot = new_slot();
    let _receiver = spawn_builder(executor.boxed_engine())
        .with_task(PanicsOnFirstPoll)
        .with_panic_handler(recording_handler(Arc::clone(&slot)))
        .as_scheduled()
        .recv(Duration::from_millis(50))
        .expect("schedule via recv");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_payload_recorded(&slot);
}

/// WHY: `stream` must install the builder's handler on the
/// `StreamConsumingIter` it drives (issue #244).
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding: the task's panic aborts the process"
)]
fn stream_delivers_the_panic_payload_to_the_handler() {
    let executor = new_executor();
    let slot = new_slot();
    let _stream = spawn_builder(executor.boxed_engine())
        .with_task(PanicsOnFirstPoll)
        .with_panic_handler(recording_handler(Arc::clone(&slot)))
        .as_scheduled()
        .stream(Duration::from_millis(50))
        .expect("schedule via stream");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_payload_recorded(&slot);
}

/// WHY: an `InlineAction` must carry the builder's handler into the
/// `ConsumingIter` it builds in `apply`, so a panic in the spawned child reaches
/// the same handler as a directly delivered task (issue #244).
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding: the task's panic aborts the process"
)]
fn action_delivers_the_panic_payload_to_the_handler() {
    let executor = new_executor();
    let slot = new_slot();
    let (mut action, _receiver) = spawn_builder(executor.boxed_engine())
        .with_task(PanicsOnFirstPoll)
        .with_panic_handler(recording_handler(Arc::clone(&slot)))
        .as_scheduled()
        .action(Duration::from_millis(50));

    action
        .apply(None, executor.boxed_engine())
        .expect("schedule via action");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_payload_recorded(&slot);
}
