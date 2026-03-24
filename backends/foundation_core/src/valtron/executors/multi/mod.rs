#![allow(clippy::type_complexity)]

use std::sync::{Arc, Mutex};

use crate::valtron::{
    get_allocatable_thread_count, task::TaskIterator, ExecutionAction, TaskReadyResolver,
    TaskStatusMapper,
};

use crate::synca::{LockSignal, OnSignal};
use crate::valtron::{SharedTaskQueue, ThreadPoolTaskBuilder, ThreadRegistry};

use super::PoolGuard;

/// Global registry - stores the ThreadRegistry Arc.
/// Uses Mutex<Option> to allow resetting between tests.
static REGISTRY: Mutex<Option<Arc<ThreadRegistry>>> = Mutex::new(None);

/// Handle for spawning tasks into the shared queue.
///
/// This is the new return type of `get_pool()`. It provides the same
/// spawn API surface as the old `&ThreadPool` by constructing
/// `ThreadPoolTaskBuilder` directly with the shared queue and latch.
#[derive(Clone)]
pub struct LocalPoolHandle {
    shared_tasks: SharedTaskQueue,
    latch: Arc<LockSignal>,
    kill_signal: Arc<OnSignal>,
}

impl LocalPoolHandle {
    fn new(registry: &Arc<ThreadRegistry>) -> Self {
        Self {
            shared_tasks: registry.shared_tasks(),
            latch: registry.latch(),
            kill_signal: registry.kill_signal(),
        }
    }

    /// Create a task builder for scheduling work.
    pub fn spawn<Task, Action>(
        &self,
    ) -> ThreadPoolTaskBuilder<
        Task::Ready,
        Task::Pending,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
        Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
        Task,
    >
    where
        Task::Ready: Send + 'static,
        Task::Pending: Send + 'static,
        Task: TaskIterator<Spawner = Action> + Send + 'static,
        Action: ExecutionAction + Send + 'static,
    {
        ThreadPoolTaskBuilder::new(self.shared_tasks.clone(), self.latch.clone())
    }

    /// Create a task builder with explicit Mapper and Resolver types.
    pub fn spawn2<Task, Action, Mapper, Resolver>(
        &self,
    ) -> ThreadPoolTaskBuilder<Task::Ready, Task::Pending, Action, Mapper, Resolver, Task>
    where
        Task::Ready: Send + 'static,
        Task::Pending: Send + 'static,
        Task: TaskIterator<Spawner = Action> + Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Task::Ready, Task::Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Task::Ready, Task::Pending> + Send + 'static,
    {
        ThreadPoolTaskBuilder::new(self.shared_tasks.clone(), self.latch.clone())
    }

    /// Signal all threads to die.
    pub fn kill(&self) {
        self.kill_signal.turn_on();
        self.latch.signal_all();
    }
}

/// [`get_pool`] returns a handle for spawning tasks.
pub fn get_pool() -> LocalPoolHandle {
    let registry =
        REGISTRY.lock().unwrap().clone().expect(
            "Thread pool not initialized, ensure to call block_on or initialize_pool first",
        );
    LocalPoolHandle::new(&registry)
}

/// [`block_on`] instantiates the thread registry, spawns worker threads,
/// runs the setup function, then blocks until all tasks complete.
///
/// Returns a `PoolGuard` - when dropped, kills all threads and blocks
/// until they're cleaned up. No Ctrl-C handler is registered automatically.
pub fn block_on<F>(seed_from_rng: u64, thread_num: Option<usize>, setup: F) -> PoolGuard
where
    F: FnOnce(LocalPoolHandle),
{
    let guard = initialize_pool(seed_from_rng, thread_num);
    let handle = get_pool();
    setup(handle);
    tracing::debug!("Initialize and call WaitGroup::wait");
    guard.waitgroup().wait();
    guard
}

/// `initialize_pool` creates a ThreadRegistry and spawns worker threads.
///
/// Returns a `PoolGuard` — when dropped, signals all threads to die,
/// waits for them to complete via WaitGroup, and joins all handles.
///
/// The caller is responsible for signal handling via the PoolGuard.
pub fn initialize_pool(seed_for_rng: u64, user_thread_num: Option<usize>) -> PoolGuard {
    let thread_num = match user_thread_num {
        None => get_allocatable_thread_count(),
        Some(num) => num,
    };

    let registry = Arc::new(ThreadRegistry::with_seed_and_threads(
        seed_for_rng,
        thread_num,
    ));

    // Spawn worker threads
    for _ in 0..thread_num {
        registry
            .spawn_worker()
            .expect("Failed to spawn worker thread");
    }

    *REGISTRY.lock().unwrap() = Some(registry.clone());

    PoolGuard::new(registry.clone())
}

/// [`spawn`] provides a builder which allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// Uses the global registry's shared queue and latch.
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
{
    get_pool().spawn::<Task, Action>()
}

/// [`spawn2`] provides a builder which allows you to build out
/// the underlying tasks with explicit Mapper and Resolver types.
pub fn spawn2<Task, Action, Mapper, Resolver>(
) -> ThreadPoolTaskBuilder<Task::Ready, Task::Pending, Action, Mapper, Resolver, Task>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Task::Ready, Task::Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Task::Ready, Task::Pending> + Send + 'static,
{
    get_pool().spawn2::<Task, Action, Mapper, Resolver>()
}

#[cfg(test)]
mod multi_threaded_tests {
    use core::time;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use rand::RngCore;
    use serial_test::serial;
    use tracing_test::traced_test;

    use super::{block_on, get_pool, initialize_pool};
    use crate::{
        synca::mpp,
        valtron::{FnReady, NoSpawner, TaskIterator, TaskStatus},
    };

    struct DCounter(usize, Arc<Mutex<Vec<usize>>>);

    impl DCounter {
        pub fn new(val: usize, list: Arc<Mutex<Vec<usize>>>) -> Self {
            Self(val, list)
        }
    }

    impl TaskIterator for DCounter {
        type Pending = ();

        type Ready = usize;

        type Spawner = NoSpawner;

        fn next_status(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            let mut items = self.1.lock().unwrap();
            let item_size = items.len();

            if item_size == self.0 {
                return None;
            }

            items.push(item_size);
            let new_len = items.len();

            Some(crate::valtron::TaskStatus::Ready(new_len))
        }
    }

    struct Counter(usize, Arc<Mutex<Vec<usize>>>, mpp::Sender<()>);

    impl Counter {
        pub fn new(val: usize, list: Arc<Mutex<Vec<usize>>>) -> (Self, mpp::Receiver<()>) {
            let (sender, receiver) = mpp::bounded(10);
            (Counter(val, list, sender), receiver)
        }
    }

    impl TaskIterator for Counter {
        type Pending = ();

        type Ready = usize;

        type Spawner = NoSpawner;

        fn next_status(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            tracing::debug!("Counter Task is running");

            let result = {
                let mut items = self.1.lock().unwrap();
                let item_size = items.len();

                if item_size == self.0 {
                    None // signal "done" — will send after lock release
                } else {
                    items.push(item_size);
                    Some(crate::valtron::TaskStatus::Ready(items.len()))
                }
            }; // lock released here

            match result {
                None => {
                    tracing::debug!("Sending signal with sender");
                    self.2.send(()).expect("send signal");
                    None
                }
                some => some,
            }
        }
    }

    #[derive(Default)]
    struct PanicCounter;

    impl TaskIterator for PanicCounter {
        type Pending = ();

        type Ready = usize;

        type Spawner = NoSpawner;

        fn next_status(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            tracing::debug!("PanicCounter Task is running");
            panic!("Bad stuff");
        }
    }

    mod blocked_on_execution {
        use super::*;

        #[test]
        #[serial]
        #[traced_test]
        fn can_finish_even_when_task_panics() {
            let seed = rand::rng().next_u64();

            let handler_kill = thread::spawn(move || {
                tracing::debug!("Waiting for kill signal");
                thread::sleep(time::Duration::from_secs(5));
                tracing::debug!("Got kill signal");
                get_pool().kill();
                tracing::debug!("Closing thread");
            });

            let (task_sent_sender, task_sent_receiver) = mpp::bounded(1);
            let _guard = block_on(seed, None, |pool| {
                pool.spawn()
                    .with_task(PanicCounter)
                    .with_resolver(Box::new(FnReady::new(|item, _| {
                        tracing::info!("Received next: {item:?}");
                    })))
                    .schedule()
                    .expect("should deliver task");
                task_sent_sender.send(()).expect("deliver message");
            });

            task_sent_receiver
                .recv_timeout(time::Duration::from_secs(2))
                .expect("should have spawned task");

            tracing::info!("Wait for thread to die");
            handler_kill.join().expect("should finish");
            tracing::info!("Wait for thread to die");
        }

        #[test]
        #[serial]
        #[traced_test]
        fn can_queue_and_complete_task() {
            let seed = rand::rng().next_u64();

            let shared_list = Arc::new(Mutex::new(vec![]));
            let (counter, receiver) = Counter::new(5, shared_list.clone());

            let handler_kill = thread::spawn(move || {
                tracing::debug!("Waiting for kill signal");
                receiver
                    .recv_timeout(time::Duration::from_secs(2))
                    .expect("receive signal");
                tracing::debug!("Got kill signal");
                get_pool().kill();
                tracing::debug!("Closing thread");
            });

            let (task_sent_sender, task_sent_receiver) = mpp::bounded(1);
            let _guard = block_on(seed, None, |pool| {
                tracing::debug!("Spawning new task into pool");
                pool.spawn()
                    .with_task(counter)
                    .with_resolver(Box::new(FnReady::new(|item, _| {
                        tracing::info!("Received next: {item:?}");
                    })))
                    .schedule()
                    .expect("should deliver task");
                tracing::debug!("Task spawned");
                task_sent_sender.send(()).expect("deliver message");
            });

            task_sent_receiver
                .recv_timeout(time::Duration::from_secs(2))
                .expect("should have spawned task");

            handler_kill.join().expect("should finish");

            assert_eq!(shared_list.lock().unwrap().clone(), vec![0, 1, 2, 3, 4]);
        }
    }

    mod inline_execution {
        use crate::valtron::Stream;

        use super::*;

        #[test]
        #[serial]
        #[traced_test]
        fn can_queue_and_complete_task_with_iterator() {
            let seed = rand::rng().next_u64();

            let shared_list = Arc::new(Mutex::new(Vec::new()));
            let counter = DCounter::new(5, shared_list.clone());

            let _guard = initialize_pool(seed, None);

            let iter = get_pool()
                .spawn()
                .with_task(counter)
                .schedule_iter(std::time::Duration::from_nanos(50))
                .expect("should deliver task");

            let complete: Vec<usize> = iter
                .map(|item| match item {
                    TaskStatus::Ready(value) => Some(value),
                    _ => None,
                })
                .take_while(|t: &Option<usize>| t.is_some())
                .map(|t: Option<usize>| t.unwrap())
                .collect();

            assert_eq!(complete, vec![1, 2, 3, 4, 5]);
        }

        #[test]
        #[serial]
        #[traced_test]
        fn can_queue_use_stream_iterator_from_task_iterator() {
            let seed = rand::rng().next_u64();

            let shared_list = Arc::new(Mutex::new(Vec::new()));
            let counter = DCounter::new(5, shared_list.clone());

            let _guard = initialize_pool(seed, None);

            let iter = get_pool()
                .spawn()
                .with_task(counter)
                .stream_iter(std::time::Duration::from_nanos(50))
                .expect("should deliver task");

            let complete: Vec<usize> = iter
                .map(|item| match item {
                    Stream::Next(value) => Some(value),
                    _ => None,
                })
                .take_while(|t| t.is_some())
                .map(|t| t.unwrap())
                .collect();

            assert_eq!(complete, vec![1, 2, 3, 4, 5]);
        }
    }
}
