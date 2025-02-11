// #![cfg(any(target_arch = "wasm32", feature = "nothread_runtime"))]

use std::cell::OnceCell;
use std::sync::Arc;

use crate::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
};

use super::{
    constants::*, BoxedSendExecutionIterator, ExecutionAction, ExecutionTaskIteratorBuilder,
    LocalThreadExecutor, PriorityOrder, ProcessController, TaskIterator, TaskReadyResolver,
    TaskStatusMapper,
};
use concurrent_queue::ConcurrentQueue;

#[derive(Clone, Default)]
pub struct NoThreadController;

impl ProcessController for NoThreadController {
    fn yield_process(&self) {
        tracing::info!("Called to yield process but NoThreadController does nothing");
    }

    fn yield_for(&self, dur: std::time::Duration) {
        tracing::info!(
            "Called to yield process for duration({:?}) but NoThreadController does nothing",
            dur
        );
    }
}

thread_local! {
    static GLOBAL_LOCAL_EXECUTOR_ENGINE: OnceCell<LocalThreadExecutor<NoThreadController>> = OnceCell::new();
}

/// [initialize] initializes the local single-threaded
/// execution engine, and is required to call this as your
/// first call when using this in WebAssembly or SingleThreaded
/// environment.
pub fn initialize(seed_for_rng: u64) {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| {
        let _ = pool.get_or_init(|| {
            let tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
                Arc::new(ConcurrentQueue::unbounded());
            let executor = LocalThreadExecutor::from_seed(
                seed_for_rng,
                tasks,
                IdleMan::new(
                    MAX_ROUNDS_IDLE_COUNT,
                    None,
                    SleepyMan::new(
                        MAX_ROUNDS_WHEN_SLEEPING_ENDS,
                        ExponentialBackoffDecider::new(
                            BACK_OFF_THREAD_FACTOR,
                            BACK_OFF_JITER,
                            BACK_OFF_MIN_DURATION,
                            Some(BACK_OFF_MAX_DURATION),
                        ),
                    ),
                ),
                PriorityOrder::Top,
                NoThreadController::default(),
                None,
                None,
            );
            executor
        });
    });
}

/// run_until_complete calls the LocalExecution queue and processes
/// all pending messages till they are completed.
pub fn run_until_complete() {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.block_until_finished(),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// `spawn` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
pub fn spawn<Task, Action>() -> ExecutionTaskIteratorBuilder<
    Task::Done,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner> + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Done, Task::Pending> + 'static>,
    Task,
>
where
    Task::Done: 'static,
    Task::Pending: 'static,
    Task: TaskIterator<Spawner = Action> + 'static,
    Action: ExecutionAction + 'static,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => ExecutionTaskIteratorBuilder::new(pool.boxed_engine()),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// `spawn2` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you to provide types for both Mapper and Resolver.
pub fn spawn2<Task, Action, Mapper, Resolver>(
) -> ExecutionTaskIteratorBuilder<Task::Done, Task::Pending, Action, Mapper, Resolver, Task>
where
    Task::Done: 'static,
    Task::Pending: 'static,
    Task: TaskIterator<Spawner = Action> + 'static,
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Task::Done, Task::Pending, Action> + 'static,
    Resolver: TaskReadyResolver<Action, Task::Done, Task::Pending> + 'static,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => ExecutionTaskIteratorBuilder::new(pool.boxed_engine()),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}
