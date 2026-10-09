//! Integration tests for valtron verb builders — `action()` terminal.
//!
//! WHY: The `action()` terminal defers engine dispatch to
//! `ExecutionAction::apply()` time, returning an `InlineAction` + receiver
//! pair.  These tests verify every verb produces a working `InlineAction`.
//!
//! WHAT: schedule, lift, broadcast (`action()`), plus error paths.

use std::sync::Arc;
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{
    InlineAction, InlineActionBehaviour, NoAction, NotificationItem,
    NotifyRecvIterator, PriorityOrder, SharedTaskQueue, TaskStatus,
    WrapTask, spawn_builder, spawn_broadcaster,
};
use foundation_core::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
    valtron::{
        ExecutionAction, LocalThreadExecutor, ProcessController, ProgressIndicator,
    },
};
use tracing_test::traced_test;

// ── boilerplate ────────────────────────────────────────────────────────

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
        IdleMan::new(3, None, SleepyMan::new(3, ExponentialBackoffDecider::default())),
        PriorityOrder::Bottom,
        NoYielder,
        Duration::from_secs(10),
        None,
        None,
    )
}

fn drain_results<R, P, A>(
    receiver: NotifyRecvIterator<TaskStatus<R, P, A>>,
) -> Vec<TaskStatus<R, P, A>>
where
    A: ExecutionAction,
{
    receiver
        .filter_map(|item| match item {
            NotificationItem::Ready(v) => Some(v),
            _ => None,
        })
        .collect()
}

// ── action() terminal tests ────────────────────────────────────────────

/// WHY: `as_scheduled().action()` builds an `InlineAction` that, when
/// applied, schedules the task and sends outputs through the receiver.
#[test]
#[traced_test]
fn test_builder_action_schedule() {
    let executor = new_executor();
    let task = WrapTask::new(vec![10, 20].into_iter());

    let (mut action, receiver) = spawn_builder(executor.boxed_engine())
        .with_task(task)
        .as_scheduled()
        .action(Duration::from_millis(50));

    action
        .apply(None, executor.boxed_engine())
        .expect("schedule via action");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_eq!(
        drain_results(receiver),
        vec![TaskStatus::Ready(10), TaskStatus::Ready(20)]
    );
}

/// WHY: `as_lifted().action()` builds an `InlineAction` with `Lift`
/// behaviour — dispatched to the top of the local queue.
#[test]
#[traced_test]
fn test_builder_action_lift() {
    let executor = new_executor();
    let task = WrapTask::new(vec![1, 2].into_iter());

    let (mut action, receiver) = spawn_builder(executor.boxed_engine())
        .with_task(task)
        .as_lifted()
        .action(Duration::from_millis(50));

    action
        .apply(None, executor.boxed_engine())
        .expect("lift via action");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_eq!(
        drain_results(receiver),
        vec![TaskStatus::Ready(1), TaskStatus::Ready(2)]
    );
}

/// WHY: `broadcast().action()` must use `spawn_broadcaster` (the `Send`
/// resolver variant) so the `Send` bounds are satisfied.
#[test]
#[traced_test]
fn test_builder_action_broadcast() {
    let executor = new_executor();
    let task = WrapTask::new(vec![7, 8, 9].into_iter());

    let (mut action, receiver) = spawn_broadcaster(executor.boxed_engine())
        .with_task(task)
        .as_broadcast()
        .action(Duration::from_millis(50));

    action
        .apply(None, executor.boxed_engine())
        .expect("broadcast via action");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_eq!(
        drain_results(receiver),
        vec![TaskStatus::Ready(7), TaskStatus::Ready(8), TaskStatus::Ready(9)]
    );
}

// ── from_parts ─────────────────────────────────────────────────────────

/// WHY: `from_parts` constructs a valid `InlineAction` from raw components.
#[test]
#[traced_test]
fn test_from_parts_works() {
    use foundation_core::valtron::NotifyQueue;

    let executor = new_executor();
    let task = WrapTask::new(vec![42].into_iter());

    let channel: Arc<NotifyQueue<TaskStatus<usize, (), NoAction>>> =
        Arc::new(NotifyQueue::unbounded());
    let mut action = InlineAction::from_parts(
        InlineActionBehaviour::Schedule,
        task,
        channel.clone(),
    );
    let receiver = NotifyRecvIterator::from_notify_queue(channel, Duration::from_millis(50));

    action
        .apply(None, executor.boxed_engine())
        .expect("from_parts dispatch");
    executor.run_until(|state| ProgressIndicator::NoWork == state);

    assert_eq!(drain_results(receiver), vec![TaskStatus::Ready(42)]);
}

// ── error paths ────────────────────────────────────────────────────────

/// WHY: applying a consumed `InlineAction` must return an error.
#[test]
#[traced_test]
fn test_action_applied_twice_errors() {
    let executor = new_executor();
    let task = WrapTask::new(vec![1].into_iter());

    let (mut action, _receiver) = spawn_builder(executor.boxed_engine())
        .with_task(task)
        .as_scheduled()
        .action(Duration::from_millis(50));

    action
        .apply(None, executor.boxed_engine())
        .expect("first apply");

    let result = action.apply(None, executor.boxed_engine());
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Action has being used up"));
}

/// WHY: A `Sequenced` `InlineAction` applied without a parent `key` must
/// return `ParentMustBeSupplied`.
/// WHAT: Build the action directly via `from_parts` with `Sequenced`
/// behaviour, then call `apply(None, …)`.
#[test]
#[traced_test]
fn test_sequenced_without_parent_errors() {
    use foundation_core::valtron::NotifyQueue;

    let executor = new_executor();
    let task = WrapTask::new(vec![1].into_iter());
    let channel: Arc<NotifyQueue<TaskStatus<usize, (), NoAction>>> =
        Arc::new(NotifyQueue::unbounded());
    let mut action = InlineAction::from_parts(
        InlineActionBehaviour::Sequenced,
        task,
        channel,
    );

    let result = action.apply(None, executor.boxed_engine());
    assert!(result.is_err());

    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("Parent"),
        "expected ParentMustBeSupplied, got: {msg}"
    );
}
