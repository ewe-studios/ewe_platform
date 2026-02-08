#![allow(clippy::type_complexity)]

use std::cell::OnceCell;
use std::sync::Arc;

use crate::valtron::{
    executors::constants::{
        BACK_OFF_JITER, BACK_OFF_MAX_DURATION, BACK_OFF_MIN_DURATION, BACK_OFF_THREAD_FACTOR,
        MAX_ROUNDS_IDLE_COUNT, MAX_ROUNDS_WHEN_SLEEPING_ENDS,
    },
    BoxedSendExecutionIterator, ExecutionAction, ExecutionTaskIteratorBuilder, LocalThreadExecutor,
    PriorityOrder, ProcessController, ProgressIndicator, TaskIterator, TaskReadyResolver,
    TaskStatusMapper,
};

use crate::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
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
    #[allow(clippy::missing_const_for_thread_local)]
    static GLOBAL_LOCAL_EXECUTOR_ENGINE: OnceCell<LocalThreadExecutor<NoThreadController>> = OnceCell::new();
}

/// [`initialize`] initializes the local single-threaded
/// execution engine, and is required to call this as your
/// first call when using this in WebAssembly or `SingleThreaded`
/// environment.
pub fn initialize_pool(seed_for_rng: u64) {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| {
        let _ = pool.get_or_init(|| {
            let tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
                Arc::new(ConcurrentQueue::unbounded());
            LocalThreadExecutor::from_seed(
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
                NoThreadController,
                None,
                None,
            )
        });
    });
}

/// `run_until` calls the [`LocalExecution`] queue and processes
/// the next pending message by moving it forward just until
/// the provided function returns true indicating it should stop.
///
/// This lets you review the state or some internal conditions
/// upon which you wish to stop processing.
///
/// It works like [`run_once`] but not just executed once but till
/// the condition is met.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// The progress indicator from the execution
pub fn run_until<S>(checker: S)
where
    S: Fn(&ProgressIndicator) -> bool,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.run_until(checker),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    });
}

/// `run_once` calls the `LocalExecution` queue and processes
/// the next pending message by moving it forward just once.
///
/// I rearly see you using this, but there might be situations
/// where more fine-grained control on how much work you wish
/// to do matters and you do not want a [`run_until_complete`]
/// where the underlying point of stopping is not controlled
/// by you.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// The progress indicator from the execution
#[must_use]
pub fn run_once() -> ProgressIndicator {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.run_once(),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// `run_until_complete` calls the `LocalExecution` queue and processes
/// all pending messages till they are completed.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
pub fn run_until_complete() {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.block_until_finished(),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    });
}

/// `spawn` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// A builder for creating a task iterator
#[must_use]
pub fn spawn<Task, Action>() -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + 'static>,
    Task,
>
where
    Task::Ready: 'static,
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
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// A builder for creating a task iterator
#[must_use]
pub fn spawn2<Task, Action, Mapper, Resolver>(
) -> ExecutionTaskIteratorBuilder<Task::Ready, Task::Pending, Action, Mapper, Resolver, Task>
where
    Task::Ready: 'static,
    Task::Pending: 'static,
    Task: TaskIterator<Spawner = Action> + 'static,
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Task::Ready, Task::Pending, Action> + 'static,
    Resolver: TaskReadyResolver<Action, Task::Ready, Task::Pending> + 'static,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => ExecutionTaskIteratorBuilder::new(pool.boxed_engine()),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

#[cfg(test)]
mod single_threaded_tests {
    use std::{cell::RefCell, rc::Rc};

    use rand::RngCore;
    use tracing_test::traced_test;

    use crate::valtron::{
        single::{initialize_pool, run_until, run_until_complete, spawn},
        FnReady, NoSpawner, Stream, TaskIterator, TaskStatus,
    };

    struct Counter(usize, Rc<RefCell<Vec<usize>>>);

    impl Counter {
        pub fn new(val: usize, list: Rc<RefCell<Vec<usize>>>) -> Self {
            Self(val, list)
        }
    }

    impl TaskIterator for Counter {
        type Pending = ();

        type Ready = usize;

        type Spawner = NoSpawner;

        fn next(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            let item_size = self.1.borrow().len();

            if item_size == self.0 {
                return None;
            }

            self.1.borrow_mut().push(item_size);

            Some(crate::valtron::TaskStatus::Ready(self.1.borrow().len()))
        }
    }

    #[test]
    #[traced_test]
    fn can_queue_task_only() {
        let seed = rand::rng().next_u64();

        let shared_list = Rc::new(RefCell::new(Vec::new()));
        let counter = Counter::new(5, shared_list.clone());

        initialize_pool(seed);

        spawn()
            .with_task(counter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {:?}", item)
            })))
            .schedule()
            .expect("should deliver task");

        assert_eq!(shared_list.borrow().len(), 0);
    }

    #[test]
    #[traced_test]
    fn can_queue_and_complete_task_with_run_until() {
        let seed = rand::rng().next_u64();

        let shared_list = Rc::new(RefCell::new(Vec::new()));
        let counter = Counter::new(5, shared_list.clone());

        initialize_pool(seed);

        spawn()
            .with_task(counter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {:?}", item);
            })))
            .schedule()
            .expect("should deliver task");

        let handle = shared_list.clone();
        run_until(|_| {
            if handle.borrow_mut().len() >= 5 {
                return true;
            }
            false
        });

        assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    #[traced_test]
    fn can_queue_and_complete_task() {
        let seed = rand::rng().next_u64();

        let shared_list = Rc::new(RefCell::new(Vec::new()));
        let counter = Counter::new(5, shared_list.clone());

        initialize_pool(seed);

        spawn()
            .with_task(counter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {:?}", item)
            })))
            .schedule()
            .expect("should deliver task");

        run_until_complete();

        assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    #[traced_test]
    fn can_queue_and_complete_task_with_iterator() {
        let seed = rand::rng().next_u64();

        let shared_list = Rc::new(RefCell::new(Vec::new()));
        let counter = Counter::new(5, shared_list.clone());

        initialize_pool(seed);

        let iter = spawn()
            .with_task(counter)
            .schedule_iter(std::time::Duration::from_nanos(50))
            .expect("should deliver task");

        run_until_complete();

        let complete: Vec<usize> = iter
            .map(|item| match item {
                TaskStatus::Ready(value) => Some(value),
                _ => None,
            })
            .take_while(|t| t.is_some())
            .map(|t| t.unwrap())
            .collect();

        assert_eq!(complete, vec![1, 2, 3, 4, 5]);
        assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    #[traced_test]
    fn can_queue_and_complete_stream_with_iterator() {
        let seed = rand::rng().next_u64();

        let shared_list = Rc::new(RefCell::new(Vec::new()));
        let counter = Counter::new(5, shared_list.clone());

        initialize_pool(seed);

        let iter = spawn()
            .with_task(counter)
            .stream_iter(std::time::Duration::from_nanos(50))
            .expect("should deliver task");

        run_until_complete();

        let complete: Vec<usize> = iter
            .map(|item| match item {
                Stream::Next(value) => Some(value),
                _ => None,
            })
            .take_while(|t| t.is_some())
            .map(|t| t.unwrap())
            .collect();

        assert_eq!(complete, vec![1, 2, 3, 4, 5]);
        assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
    }
}
