//! `LocalThreadExecutor` tests, moved out of `src/valtron/executors/local.rs`
//! (tests live in `tests/`, never inline).
//!
//! These run with and without `multi`: the executor is meant to work in both.
//! Task builders and spawn actions differ by config (`Send` under `multi`, not
//! otherwise), so the tests go through the gated helpers below
//! (`any_task_for_mode`, `typed_task_for_mode`, `boxed_spawn_action`), which
//! pick the `Send` or non-`Send` variant. Only tests that move work to
//! another OS thread stay `#[cfg(feature = "multi")]`.
#![cfg(not(target_family = "wasm"))]

use std::time;

use concurrent_queue::ConcurrentQueue;
use foundation_core::compati::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{thread, time::Duration};

#[cfg(not(feature = "multi"))]
use foundation_core::valtron::{
    any_task as any_task_for_mode, typed_task as typed_task_for_mode, IntoBoxedExecutionAction,
};
#[cfg(feature = "multi")]
use foundation_core::valtron::{
    send_any_task as any_task_for_mode, send_typed_task as typed_task_for_mode,
    IntoBoxedSendExecutionAction,
};

/// Boxes an action for `TaskStatus::Spawn`, which holds a `Send` action
/// under `multi` and a non-`Send` one otherwise.
#[cfg(feature = "multi")]
fn boxed_spawn_action<A>(action: A) -> foundation_core::valtron::BoxedSendExecutionAction
where
    A: IntoBoxedSendExecutionAction,
{
    action.into_box_send_execution_action()
}

#[cfg(not(feature = "multi"))]
fn boxed_spawn_action<A>(action: A) -> foundation_core::valtron::BoxedExecutionAction
where
    A: IntoBoxedExecutionAction,
{
    action.into_box_execution_action()
}
use foundation_core::{
    panic_if_failed,
    retries::ExponentialBackoffDecider,
    synca::SleepyMan,
    valtron::{
        BoolSignal, EventReadiness, ExecutionAction, InlineAction, InlineActionBehaviour, OnNext,
        ProcessController, TaskIterator, TaskStatus, WrapTask,
    },
};

use foundation_compact::rng::Rng;
use foundation_core::synca::*;
use foundation_core::valtron::*;
use std::sync::{self as sync, Arc};

#[derive(Default, Clone)]
struct NoYielder;

impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _: time::Duration) {}
}

struct Counter(&'static str, usize, usize, usize);

impl TaskIterator for Counter {
    type Ready = usize;
    type Pending = time::Duration;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let old_count = self.1;
        let new_count = old_count + 1;
        self.1 = new_count;

        // Field 0 is the task's label; surfacing it here is what makes an
        // interleaving failure readable when several counters run at once.
        tracing::trace!(task = self.0, count = new_count, "Counter advanced");

        if new_count == self.2 {
            return None;
        }

        if new_count == self.3 {
            return Some(TaskStatus::Delayed(time::Duration::from_millis(5)));
        }

        Some(TaskStatus::Ready(new_count))
    }
}

enum ListItemInner {
    List(Option<Vec<usize>>),
    Response(NotifyRecvIterator<TaskStatus<usize, ()>>),
    Done,
}

struct ListItems(InlineActionBehaviour, Option<ListItemInner>);

impl TaskIterator for ListItems {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.1.take() {
            Some(val) => match val {
                ListItemInner::List(mut items) => {
                    if let Some(inner) = items.take() {
                        let task = WrapTask::new(inner.into_iter());

                        let (inline_action, receiver) =
                            InlineAction::new(self.0, task, std::time::Duration::from_millis(100));

                        self.1 = Some(ListItemInner::Response(receiver));

                        return Some(TaskStatus::Spawn(boxed_spawn_action(inline_action)));
                    }

                    self.1 = Some(ListItemInner::Done);
                    None
                }
                ListItemInner::Response(mut iter) => {
                    let val_next = iter.next();

                    self.1 = Some(ListItemInner::Response(iter));

                    match val_next {
                        Some(NotificationItem::Ready(status)) => match status {
                            TaskStatus::Init => Some(TaskStatus::Init),
                            TaskStatus::Spread(items) => Some(TaskStatus::Spread(items)),
                            TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                            TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                            TaskStatus::Ready(item) => Some(TaskStatus::Ready(item)),
                            TaskStatus::Spawn(action) => Some(TaskStatus::Spawn(action)),
                            TaskStatus::Ignore => Some(TaskStatus::Ignore),
                            TaskStatus::Wait => Some(TaskStatus::Wait),
                            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
                        },
                        Some(NotificationItem::None) => Some(TaskStatus::Wait),
                        None => None,
                    }
                }
                ListItemInner::Done => None,
            },
            None => None,
        }
    }
}

// ============================================================================
// PHASE 0: InlineAction and InlineAction tests
// ============================================================================

#[test]
fn test_task_line_action_directly() {
    let seed = foundation_compact::rng::rng().next_u64();
    let kill_signal = Arc::new(OnSignal::new());

    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        Some(kill_signal.clone()),
        None,
    );

    let item_list = vec![1, 2, 3];
    let task = WrapTask::new(item_list.into_iter());

    let (mut inline_action, receiver) = InlineAction::new(
        InlineActionBehaviour::Lift,
        task,
        std::time::Duration::from_millis(10),
    );

    inline_action
        .apply(None, executor.boxed_engine())
        .expect("should deliver event");

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    let all_response: Vec<_> = receiver
        .filter_map(|item| match item {
            NotificationItem::Ready(v) => Some(v),
            NotificationItem::None => None,
        })
        .collect();
    assert_eq!(
        all_response,
        vec![
            TaskStatus::Ready(1),
            TaskStatus::Ready(2),
            TaskStatus::Ready(3),
        ]
    );
}

#[test]
fn test_task_line_send_action() {
    let seed = foundation_compact::rng::rng().next_u64();
    let kill_signal = Arc::new(OnSignal::new());

    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        Some(kill_signal.clone()),
        None,
    );

    let receiver = panic_if_failed!(any_task_for_mode(executor.boxed_engine())
        .with_task(ListItems(
            InlineActionBehaviour::Lift,
            Some(ListItemInner::List(Some(vec![1, 2, 3])))
        ))
        .as_scheduled()
        .recv(std::time::Duration::from_millis(100)));

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    let all_response: Vec<TaskStatus<usize, ()>> = receiver
        .filter_map(|item| match item {
            NotificationItem::Ready(v) => Some(v),
            NotificationItem::None => None,
        })
        .collect();

    assert_eq!(
        all_response,
        vec![
            TaskStatus::Ready(1),
            TaskStatus::Ready(2),
            TaskStatus::Ready(3),
        ]
    );
}

#[test]
fn test_task_line_send_action_for_parent_lifted() {
    let seed = foundation_compact::rng::rng().next_u64();
    let kill_signal = Arc::new(OnSignal::new());

    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        Some(kill_signal.clone()),
        None,
    );

    let receiver = panic_if_failed!(any_task_for_mode(executor.boxed_engine())
        .with_task(ListItems(
            InlineActionBehaviour::LiftWithParent,
            Some(ListItemInner::List(Some(vec![1, 2, 3])))
        ))
        .as_scheduled()
        .recv(std::time::Duration::from_millis(100)));

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    let all_response: Vec<TaskStatus<usize, ()>> = receiver
        .filter_map(|item| match item {
            NotificationItem::Ready(v) => Some(v),
            NotificationItem::None => None,
        })
        .collect();

    assert_eq!(
        all_response,
        vec![
            TaskStatus::Ready(1),
            TaskStatus::Ready(2),
            TaskStatus::Ready(3),
        ]
    );
}

#[test]
fn test_task_line_send_action_for_sequenced() {
    let seed = foundation_compact::rng::rng().next_u64();
    let kill_signal = Arc::new(OnSignal::new());

    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        Some(kill_signal.clone()),
        None,
    );

    let receiver = panic_if_failed!(any_task_for_mode(executor.boxed_engine())
        .with_task(ListItems(
            InlineActionBehaviour::Sequenced,
            Some(ListItemInner::List(Some(vec![1, 2, 3])))
        ))
        .as_scheduled()
        .recv(std::time::Duration::from_millis(100)));

    executor.run_until(|state| ProgressIndicator::NoWork == state);

    let all_response: Vec<TaskStatus<usize, ()>> = receiver
        .filter_map(|item| match item {
            NotificationItem::Ready(v) => Some(v),
            NotificationItem::None => None,
        })
        .collect();

    assert_eq!(
        all_response,
        vec![
            TaskStatus::Ready(1),
            TaskStatus::Ready(2),
            TaskStatus::Ready(3),
        ]
    );
}

// ============================================================================
// PHASE 1: Local Executor tests
// ============================================================================

#[test]
fn scenario_0_can_kill_local_executor_via_kill_signal() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();
    let kill_signal = Arc::new(OnSignal::new());

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        Some(kill_signal.clone()),
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(typed_task_for_mode(executor.boxed_engine())
        .with_task(Counter("Counter1", 0, 3, 3))
        .on_next(move |next, _| count_clone.lock().unwrap().push(next))
        .as_broadcast()
        .spawn());

    assert_eq!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(State::Progressed))
    );

    let thread_handle = thread::spawn(move || {
        tracing::debug!("Starting sleep timer for 100ms to kill executor");
        thread::sleep(Duration::from_millis(100));
        tracing::debug!("Trigger kill signal");
        kill_signal.turn_on();
        tracing::debug!("Executor kill signal sent");
    });

    tracing::debug!("Blocking main thread with block_on()");
    executor.block_on();
    tracing::debug!("Thread released from block_on()");

    thread_handle.join().expect("should end correctly");
}

#[test]
fn scenario_one_task_a_runs_to_completion() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    let on_next = OnNext::on_next(Counter("Counter1", 0, 3, 3), move |next, _engine| {
        count_clone.lock().unwrap().push(next)
    });

    panic_if_failed!(global.push(on_next.into()));

    assert_eq!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(State::Progressed))
    );
    assert_eq!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(State::Progressed))
    );
    assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
    assert_eq!(executor.number_of_sleepers(), 0);

    let count_list = counts.lock().unwrap();
    assert_eq!(
        &*count_list,
        &vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
    );
}

#[test]
fn scenario_one_can_use_local_executor_builder_to_queue_task() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(typed_task_for_mode(executor.boxed_engine())
        .with_task(Counter("Counter1", 0, 3, 3))
        .on_next(move |next, _| count_clone.lock().unwrap().push(next))
        .as_broadcast()
        .spawn());

    assert_eq!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(State::Progressed))
    );
    assert_eq!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(State::Progressed))
    );
    assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
    assert_eq!(executor.number_of_sleepers(), 0);

    let count_list = counts.lock().unwrap();
    assert_eq!(
        &*count_list,
        &vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
    );
}

#[test]
fn scenario_2_task_a_goes_to_sleep_as_only_task_in_queue() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        Counter("Counter1", 10, 20, 12),
        move |next, _engine| { count_clone.lock().unwrap().push(next) }
    ))));

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(&*counts.lock().unwrap(), &vec![TaskStatus::Ready(11),]);

    assert_eq!(
        executor.run_once(),
        ProgressIndicator::SpinWait(time::Duration::from_millis(5))
    );
    assert_eq!(&*counts.lock().unwrap(), &vec![TaskStatus::Ready(11),]);

    // wait for 5ms and validate we made progress
    thread::sleep(time::Duration::from_millis(5));

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![TaskStatus::Ready(11), TaskStatus::Ready(13),]
    );
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            TaskStatus::Ready(11),
            TaskStatus::Ready(13),
            TaskStatus::Ready(14),
        ]
    );
}

#[test]
fn scenario_3_task_goes_to_sleep_as_highest_priority_on_wakeup_with_other_tasks() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Top,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        Counter("Counter1", 0, 4, 2),
        move |next, _| count_clone.lock().unwrap().push(("Counter1", next))
    ))));

    let count_clone2 = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter2", 0, 20, 10), move |next, _| count_clone2
            .lock()
            .unwrap()
            .push(("Counter2", next)))
        .into()
    ));

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("Counter1", TaskStatus::Ready(1)),]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("Counter1", TaskStatus::Ready(1)),]
    );

    assert_eq!(executor.number_of_sleepers(), 1);

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
        ]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
        ]
    );

    // wait for 5ms and validate we made progress
    tracing::debug!("Sleeping thread for 5ms");
    thread::sleep(time::Duration::from_millis(5));
    tracing::debug!("Finished sleeping thread for 5ms");

    // Counter1 is brought back in as priority
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter1", TaskStatus::Ready(3)),
        ]
    );

    // Counter1 finishes and removed from queue, so count is same.
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter1", TaskStatus::Ready(3)),
        ]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter1", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(3)),
        ]
    );
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter1", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
        ]
    );
}

#[test]
fn scenario_4_task_goes_to_sleep_as_lowest_priority_on_wakeup_with_other_tasks() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter1", 0, 4, 2), move |next, _| count_clone
            .lock()
            .unwrap()
            .push(("Counter1", next)))
        .into()
    ));

    let count_clone2 = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter2", 0, 5, 10), move |next, _| count_clone2
            .lock()
            .unwrap()
            .push(("Counter2", next)))
        .into()
    ));

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("Counter1", TaskStatus::Ready(1)),]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("Counter1", TaskStatus::Ready(1)),]
    );

    assert_eq!(executor.number_of_sleepers(), 1);

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
        ]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
        ]
    );

    // wait for 5ms and validate we made progress
    tracing::debug!("Sleeping thread for 5ms");
    thread::sleep(time::Duration::from_millis(5));
    tracing::debug!("Finished sleeping thread for 5ms");

    // Counter1 is brought back in as priority
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
        ]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
        ]
    );

    // Counter2 triggers Done signal
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
        ]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(_)
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
            ("Counter1", TaskStatus::Ready(3)),
        ]
    );

    assert!(matches!(executor.run_once(), ProgressIndicator::NoWork));

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
            ("Counter1", TaskStatus::Ready(3)),
        ]
    );
}

#[derive(Clone, Default)]
enum DaemonSpawner {
    #[default]
    NoSpawning,
    InThread,
    TopOfThread,
    OutofThread,
}

impl ExecutionAction for DaemonSpawner {
    fn apply(
        &mut self,
        key: Option<Entry>,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        match self {
            DaemonSpawner::NoSpawning => Ok(SpawnInfo::new(SpawnType::None, None, key)),
            DaemonSpawner::TopOfThread => {
                tracing::debug!("Spawning task as TopOfThread");
                match any_task(executor)
                    .maybe_parent(key)
                    .with_task(SimpleCounter("SubTask1", 0, 5))
                    .as_lifted()
                    .spawn()
                {
                    Ok(info) => Ok(info),
                    Err(err) => Err(Box::new(err)),
                }
            }
            DaemonSpawner::InThread => {
                tracing::debug!("Spawning task as InThread");
                match any_task(executor)
                    .with_task(SimpleCounter("SubTask2", 0, 5))
                    .as_scheduled()
                    .spawn()
                {
                    Ok(info) => Ok(info),
                    Err(err) => Err(Box::new(err)),
                }
            }
            DaemonSpawner::OutofThread => {
                tracing::debug!("Spawning task as OutOfThread");
                match any_task_for_mode(executor)
                    .with_task(SimpleCounter("SubTask3", 0, 5))
                    .as_broadcast()
                    .spawn()
                {
                    Ok(info) => Ok(info),
                    Err(err) => Err(Box::new(err)),
                }
            }
        }
    }
}

struct DaemonCounter(Arc<AtomicUsize>);
impl TaskIterator for DaemonCounter {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let current = self.0.load(Ordering::SeqCst);
        println!("Current state: {:?}", current);
        if current == 0 {
            return Some(TaskStatus::Pending(()));
        }

        if current == 1 {
            return Some(TaskStatus::Ready(()));
        }

        if current == 2 {
            return Some(TaskStatus::Spawn(Box::new(DaemonSpawner::InThread)));
        }

        if current == 3 {
            return Some(TaskStatus::Spawn(Box::new(DaemonSpawner::TopOfThread)));
        }

        if current == 5 {
            return None;
        }

        println!("Spawn out of thread task");
        Some(TaskStatus::Spawn(Box::new(DaemonSpawner::OutofThread)))
    }
}

struct SimpleCounter(&'static str, usize, usize);

impl TaskIterator for SimpleCounter {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let old_count = self.1;
        let new_count = old_count + 1;
        self.1 = new_count;

        tracing::debug!(
            "Counter({}) has current count {} from old count {}",
            self.0,
            new_count,
            old_count,
        );

        if new_count == self.2 {
            tracing::debug!("Counter({}) ending task at {}", self.0, new_count,);
            return None;
        }

        if new_count == 3 {
            tracing::debug!("Counter({}) going to sleep at {}", self.0, new_count,);
            return Some(TaskStatus::Delayed(time::Duration::from_millis(10)));
        }

        Some(TaskStatus::Ready(new_count))
    }
}

#[test]
fn scenario_5_task_can_spawn_task_via_actions() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<(), ()>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let gen_state = Arc::new(AtomicUsize::new(0));

    let count_clone = counts.clone();
    panic_if_failed!(typed_task_for_mode(executor.boxed_engine())
        .with_task(DaemonCounter(gen_state.clone()))
        .on_next(move |next, _| count_clone.lock().unwrap().push(("DaemonCounter", next)))
        .as_broadcast()
        .spawn());

    assert_eq!(executor.run_once(), ProgressIndicator::CanProgress(None));

    assert_eq!(&*counts.lock().unwrap(), &vec![]);

    assert_eq!(executor.number_of_sleepers(), 0);
    assert_eq!(executor.number_of_local_tasks(), 1);

    gen_state.store(1, Ordering::SeqCst);

    assert_eq!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(State::Progressed))
    );
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("DaemonCounter", TaskStatus::Ready(())),]
    );

    assert_eq!(executor.number_of_sleepers(), 0);
    assert_eq!(executor.number_of_local_tasks(), 1);

    gen_state.store(2, Ordering::SeqCst);

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(_))
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("DaemonCounter", TaskStatus::Ready(())),]
    );

    assert_eq!(executor.number_of_sleepers(), 0);
    assert_eq!(executor.number_of_local_tasks(), 2);

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(_))
    ));
    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![("DaemonCounter", TaskStatus::Ready(())),]
    );

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(_))
    ));
    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(Some(_))
    ));

    assert_eq!(executor.number_of_sleepers(), 0);
    assert_eq!(executor.number_of_local_tasks(), 5);

    gen_state.store(0, Ordering::SeqCst);

    assert!(matches!(
        executor.run_once(),
        ProgressIndicator::CanProgress(None)
    ));
    assert_eq!(executor.number_of_sleepers(), 0);
    assert_eq!(executor.number_of_local_tasks(), 5);
}

#[test]
fn scenario_5_task_a_spawns_task_b_that_goes_to_sleep_but_also_ties_task_a_to_its_readiness() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<(), ()>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let gen_state = Arc::new(AtomicUsize::new(0));

    let count_clone = counts.clone();
    panic_if_failed!(typed_task_for_mode(executor.boxed_engine())
        .with_task(DaemonCounter(gen_state.clone()))
        .on_next(move |next, _| count_clone.lock().unwrap().push(("DaemonCounter", next)))
        .as_broadcast()
        .spawn());

    assert_eq!(executor.run_once(), ProgressIndicator::CanProgress(None));

    assert_eq!(&*counts.lock().unwrap(), &vec![]);

    assert_eq!(executor.number_of_sleepers(), 0);
    assert_eq!(executor.number_of_local_tasks(), 1);

    gen_state.store(1, Ordering::SeqCst);

    let mut count = 5;

    while count > 0 {
        executor.run_once();
        count -= 1;
    }

    assert_eq!(executor.number_of_local_tasks(), 1);
    assert_eq!(executor.number_of_sleepers(), 0);

    gen_state.store(2, Ordering::SeqCst);

    count = 5;
    while count > 0 {
        executor.run_once();
        count -= 1;
    }

    gen_state.store(3, Ordering::SeqCst);

    count = 5;
    while count > 0 {
        executor.run_once();
        count -= 1;
    }

    gen_state.store(4, Ordering::SeqCst);

    count = 2;
    while count > 0 {
        executor.run_once();
        count -= 1;
    }

    gen_state.store(5, Ordering::SeqCst);

    std::thread::sleep(std::time::Duration::from_millis(100 * 6));

    executor.run_until(|state| state == ProgressIndicator::NoWork);

    assert_eq!(executor.number_of_local_tasks(), 0);
    assert_eq!(executor.number_of_sleepers(), 0);

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("DaemonCounter", TaskStatus::Ready(())),
            ("DaemonCounter", TaskStatus::Ready(())),
            ("DaemonCounter", TaskStatus::Ready(())),
            ("DaemonCounter", TaskStatus::Ready(())),
            ("DaemonCounter", TaskStatus::Ready(())),
        ]
    );
}

#[test]
fn scenario_6_can_run_until_ready_signal_is_seen_and_no_work_remains_when_condition_hits() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter1", 0, 4, 2), move |next, _| count_clone
            .lock()
            .unwrap()
            .push(("Counter1", next)))
        .into()
    ));

    let count_clone2 = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter2", 0, 5, 10), move |next, _| count_clone2
            .lock()
            .unwrap()
            .push(("Counter2", next)))
        .into()
    ));

    let count_clone3 = Arc::clone(&counts);
    executor.run_until(move |_| {
        let items = count_clone3.lock().unwrap();

        if items.len() >= 6 {
            return true;
        }
        false
    });

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
            ("Counter1", TaskStatus::Ready(3)),
        ]
    );

    assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
}

#[test]
fn scenario_6_can_run_until_ready_signal_is_seen_and_no_work_remains_when_condition_doesnt_hits() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter1", 0, 4, 2), move |next, _| count_clone
            .lock()
            .unwrap()
            .push(("Counter1", next)))
        .into()
    ));

    let count_clone2 = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter2", 0, 5, 10), move |next, _| count_clone2
            .lock()
            .unwrap()
            .push(("Counter2", next)))
        .into()
    ));

    let count_clone3 = Arc::clone(&counts);
    executor.run_until(move |_| {
        let items = count_clone3.lock().unwrap();

        // should be 6 but intentionally setting to 7 to validate it
        // never loops forever
        if items.len() >= 7 {
            return true;
        }
        false
    });

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
            ("Counter2", TaskStatus::Ready(4)),
            ("Counter1", TaskStatus::Ready(3)),
        ]
    );

    assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
}

#[test]
fn scenario_6_can_run_until_ready_signal_is_seen_with_pending_work() {
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration>)>>> =
        Arc::new(Mutex::new(Vec::new()));

    let seed = foundation_compact::rng::rng().next_u64();

    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let count_clone = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter1", 0, 4, 2), move |next, _| count_clone
            .lock()
            .unwrap()
            .push(("Counter1", next)))
        .into()
    ));

    let count_clone2 = Arc::clone(&counts);
    panic_if_failed!(global.push(
        OnNext::on_next(Counter("Counter2", 0, 5, 10), move |next, _| count_clone2
            .lock()
            .unwrap()
            .push(("Counter2", next)))
        .into()
    ));

    let count_clone3 = Arc::clone(&counts);
    executor.run_until(move |_| {
        let items = count_clone3.lock().unwrap();
        if items.len() >= 4 {
            return true;
        }
        false
    });

    assert_eq!(
        &*counts.lock().unwrap(),
        &vec![
            ("Counter1", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(1)),
            ("Counter2", TaskStatus::Ready(2)),
            ("Counter2", TaskStatus::Ready(3)),
        ]
    );
}

// ============================================================================
// Feature 02: Linked Task State Propagation
// ============================================================================
// Tests for this feature require complex setup with actual TaskIterators
// that return Pending/Sleep/Spawn states. The implementation in
// dependent_lift.rs is verified through the scenario_5 tests which test
// the combined task behavior end-to-end.
//
// The key change is that DualSequenceChildAndParentLinkedTask now:
// 1. Has a pending_parent_state field to store parent states
// 2. Returns parent states requiring executor action (Pending with duration,
//    SpawnFinished, Panicked, Reschedule) on the next poll
// 3. Only ignores "pass-through" states (ReadyValue, Progressed, etc.)

// ============================================================================
// Feature 01: TaskStatus::Depends Signal Tests
// ============================================================================

/// A counter that returns Ready values until it hits `trigger_at`,
/// then returns Depends(signal) until `resume_at`.
struct DependsCounter {
    count: usize,
    max: usize,
    trigger_at: usize,
    depends_returned: bool,
    signal: BoolSignal,
}

impl DependsCounter {
    fn new(trigger_at: usize, signal: BoolSignal) -> Self {
        Self {
            count: 0,
            max: 10,
            trigger_at,
            depends_returned: false,
            signal,
        }
    }
}

impl TaskIterator for DependsCounter {
    type Ready = usize;
    type Pending = time::Duration;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.count += 1;

        if self.count >= self.max {
            return None;
        }

        // Return Depends exactly once when we hit trigger_at
        if self.count == self.trigger_at && !self.depends_returned {
            self.depends_returned = true;
            return Some(TaskStatus::Depends(sync::Arc::new(self.signal.clone())));
        }

        Some(TaskStatus::Ready(self.count))
    }
}

/// A custom EventReadiness that flips after N polls.
struct CounterSignal {
    polls: AtomicUsize,
    threshold: usize,
}

impl CounterSignal {
    fn new(threshold: usize) -> Self {
        Self {
            polls: AtomicUsize::new(0),
            threshold,
        }
    }
}

impl EventReadiness for CounterSignal {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.polls.fetch_add(1, Ordering::SeqCst) >= self.threshold
    }
}

#[test]
fn depends_false_registers_as_sleeper() {
    let global: SharedTaskQueue = sync::Arc::new(ConcurrentQueue::bounded(10));
    let signal = BoolSignal::new(false);

    let seed = foundation_compact::rng::rng().next_u64();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let signal_clone = signal.clone();
    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        DependsCounter::new(1, signal),
        move |_next: TaskStatus<usize, time::Duration>, _engine| {
            // callback - we track via run_once return values
        }
    ))));

    // First run_once: task returns Depends(false), should register as sleeper
    let result = executor.run_once();
    assert!(matches!(result, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 1);

    // Flip the signal so the sleeper is ready
    signal_clone.clone_inner().store(true, Ordering::SeqCst);

    // Second run_once: wakeup_ready_sleepers wakes the task (signal=true)
    // Task returns Ready(2) since Depends was already consumed once
    let result2 = executor.run_once();
    assert!(matches!(result2, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 0);
}

#[test]
fn depends_signal_wakes_task_and_continues() {
    let global: SharedTaskQueue = sync::Arc::new(ConcurrentQueue::bounded(10));
    let signal = BoolSignal::new(false);

    let seed = foundation_compact::rng::rng().next_u64();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    let signal_clone = signal.clone_inner();

    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        DependsCounter::new(1, signal),
        move |_next: TaskStatus<usize, time::Duration>, _engine| {}
    ))));

    // Run 1: Depends(false) → registered as sleeper
    let r1 = executor.run_once();
    assert!(matches!(r1, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 1);

    // Flip the signal AFTER the task is sleeping
    signal_clone.store(true, Ordering::SeqCst);

    // Run 2: wakeup_ready_sleepers sees signal=true, wakes the task
    // Task returns Ready(2) since Depends was consumed
    let r2 = executor.run_once();
    assert!(matches!(r2, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 0);
}

#[test]
fn depends_true_violation_pushed_to_back() {
    let global: SharedTaskQueue = sync::Arc::new(ConcurrentQueue::bounded(10));
    // Signal starts true - task will immediately get Depends(true)
    let signal = BoolSignal::new(true);

    let seed = foundation_compact::rng::rng().next_u64();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        DependsCounter::new(1, signal),
        move |_next: TaskStatus<usize, time::Duration>, _engine| {}
    ))));

    // Depends(true) → violation, treated as Pending, pushed to back
    let result = executor.run_once();
    assert!(matches!(result, ProgressIndicator::CanProgress(_)));
    // No sleepers since it was treated as Pending
    assert_eq!(executor.number_of_sleepers(), 0);
    // Violation tracked
    assert_eq!(executor.number_of_depends_true_violations(), 1);
}

#[test]
fn custom_event_readiness_flips_after_polls() {
    let global: SharedTaskQueue = sync::Arc::new(ConcurrentQueue::bounded(10));
    let signal = sync::Arc::new(CounterSignal::new(2));

    let seed = foundation_compact::rng::rng().next_u64();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    // Task returns Depends immediately with a signal that becomes ready after 2 polls
    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        DependsCounterCustom {
            count: 0,
            max: 3,
            signal: signal.clone(),
        },
        move |_next: TaskStatus<usize, time::Duration>, _engine| {}
    ))));

    // First run_once: Depends(false), registers as sleeper
    let r1 = executor.run_once();
    assert!(matches!(r1, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 1);

    // Second run_once: wakeup_ready_sleepers polls signal (poll count = 1, still false)
    let r2 = executor.run_once();
    // Sleeper still not ready, still sleeping
    assert!(matches!(r2, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 1);

    // Third run_once: wakeup_ready_sleepers polls signal (poll count = 2, now true)
    let r3 = executor.run_once();
    // Sleeper woken up, task continues
    assert!(matches!(r3, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 0);
}

struct DependsCounterCustom {
    count: usize,
    max: usize,
    signal: sync::Arc<CounterSignal>,
}

impl TaskIterator for DependsCounterCustom {
    type Ready = usize;
    type Pending = time::Duration;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.count += 1;
        if self.count >= self.max {
            return None;
        }
        Some(TaskStatus::Depends(self.signal.clone()))
    }
}

#[test]
fn bool_signal_clone_and_flip() {
    let global: SharedTaskQueue = sync::Arc::new(ConcurrentQueue::bounded(10));
    let signal = BoolSignal::new(false);
    let atomic = signal.clone_inner();

    let seed = foundation_compact::rng::rng().next_u64();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        time::Duration::from_secs(10),
        None,
        None,
    );

    panic_if_failed!(global.push(Box::new(OnNext::on_next(
        DependsCounter::new(1, signal.clone()),
        move |_next: TaskStatus<usize, time::Duration>, _engine| {}
    ))));

    // Verify signal is initially false
    assert!(!signal.is_ready(None));

    // Register as sleeper
    let r1 = executor.run_once();
    assert!(matches!(r1, ProgressIndicator::CanProgress(_)));
    assert_eq!(executor.number_of_sleepers(), 1);

    // Flip via the atomic handle
    atomic.store(true, Ordering::SeqCst);

    // Verify signal is now true
    assert!(signal.is_ready(None));

    // Wake up
    let r2 = executor.run_once();
    assert!(matches!(r2, ProgressIndicator::CanProgress(_)));
    // Task continues past Depends, no more sleepers
    assert_eq!(executor.number_of_sleepers(), 0);
}
