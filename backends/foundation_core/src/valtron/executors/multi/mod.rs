#![allow(clippy::type_complexity)]

#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
use foundation_nostd::primitives::OnceLock;

use crate::valtron::get_allocatable_thread_count;

use super::{
    ExecutionAction, TaskIterator, TaskReadyResolver, TaskStatusMapper, ThreadPool,
    ThreadPoolTaskBuilder,
};

static CANCELATION_REGISTRATION: OnceLock<Option<()>> = OnceLock::new();
static GLOBAL_THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

/// [`get_pool`] returns the initialized `thread_pool` for your use.
pub fn get_pool() -> &'static ThreadPool {
    match GLOBAL_THREAD_POOL.get() {
        Some(pool) => pool,
        None => panic!("Thread pool not initialized, ensure to call block_on first"),
    }
}

/// [`block_on`] instantiates the thread pool if it has not already
/// then starts blocking the current thread executing the pool until all
/// tasks are ready, but before it does so it calls the provided function
/// once to allow you perform whatever instantiation of tasks or operations
/// you require.
pub fn block_on<F>(seed_from_rng: u64, thread_num: Option<usize>, setup: F)
where
    F: FnOnce(&ThreadPool),
{
    let pool = thread_pool(seed_from_rng, thread_num);
    tracing::debug!("Executing ThreadPool with block_on function");
    setup(pool);
    tracing::debug!("Initialize and call ThreadPool::run_until");
    pool.run_until();
}

/// [`thread_pool`] is the core function for initializing a thread pool which is
/// then returned for scheduling tasks on the pool.
pub fn thread_pool(pool_seed: u64, user_thread_num: Option<usize>) -> &'static ThreadPool {
    // register thread pool
    let thread_pool = GLOBAL_THREAD_POOL.get_or_init(|| {
        let thread_num = match user_thread_num {
            None => get_allocatable_thread_count(),
            Some(num) => num,
        };

        ThreadPool::with_seed_and_threads(pool_seed, thread_num)
    });

    // register cancellation handler
    let _ = CANCELATION_REGISTRATION.get_or_init(|| {
        use ctrlc;
        ctrlc::set_handler(move || {
            if let Some(pool) = GLOBAL_THREAD_POOL.get() {
                tracing::info!("Killing thread pool due to signal");
                pool.kill();
            }
        })
        .expect("Error setting Ctrl-C handler");

        Some(())
    });

    thread_pool
}

/// [`spawn`] provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
///
/// ## Panics
/// The following function will panic if the thread pool has not being initialized
/// via call to [`thread_pool`] to setup global thread pool instance.
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
    match GLOBAL_THREAD_POOL.get() {
        Some(pool) => pool.spawn(),
        None => panic!("Thread pool not initialized, ensure to call block_on first"),
    }
}

/// [`spawn2`] provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you to provide types for both Mapper and Resolver.
///
/// ## Panics
/// The following function will panic if the thread pool has not being initialized
/// via call to [`thread_pool`] to setup global thread pool instance.
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
    match GLOBAL_THREAD_POOL.get() {
        Some(pool) => pool.spawn2(),
        None => panic!("Thread pool not initialized, ensure to call block_on first"),
    }
}

#[cfg(test)]
mod multi_threaded_tests {
    use core::time;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use rand::RngCore;
    use tracing_test::traced_test;

    use super::{block_on, get_pool};
    use crate::{
        synca::mpp,
        valtron::{multi::thread_pool, FnReady, NoSpawner, TaskIterator, TaskStatus},
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

        fn next(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            let item_size = self.1.lock().unwrap().len();

            if item_size == self.0 {
                return None;
            }

            self.1.lock().unwrap().push(item_size);

            Some(crate::valtron::TaskStatus::Ready(
                self.1.lock().unwrap().len(),
            ))
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

        fn next(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            tracing::debug!("Counter Task is running");
            let mut items = self.1.lock().unwrap();
            let item_size = items.len();

            if item_size == self.0 {
                tracing::debug!("Sending signal with sender");
                self.2.send(()).expect("send signal");
                return None;
            }
            items.push(item_size);

            Some(crate::valtron::TaskStatus::Ready(items.len()))
        }
    }

    #[derive(Default)]
    struct PanicCounter;

    impl TaskIterator for PanicCounter {
        type Pending = ();

        type Ready = usize;

        type Spawner = NoSpawner;

        fn next(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            tracing::debug!("PanicCounter Task is running");
            panic!("Bad stuff");
        }
    }

    // Test show casing use of block_on as a central means of accessing
    // the execution pool.
    mod blocked_on_execution {
        use super::*;

        #[test]
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
            block_on(seed, None, |pool| {
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
            block_on(seed, None, |pool| {
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

    // Test showcasing direct inline execution as we execute a task.
    mod inline_execution {
        use crate::valtron::Stream;

        use super::*;

        #[test]
        #[traced_test]
        fn can_queue_and_complete_task_with_iterator() {
            let seed = rand::rng().next_u64();

            let shared_list = Arc::new(Mutex::new(Vec::new()));
            let counter = DCounter::new(5, shared_list.clone());

            let pool = thread_pool(seed, None);

            let iter = pool
                .spawn()
                .with_task(counter)
                .schedule_iter(std::time::Duration::from_nanos(50))
                .expect("should deliver task");

            let complete: Vec<usize> = iter
                .map(|item| match item {
                    TaskStatus::Ready(value) => Some(value),
                    _ => None,
                })
                .take_while(|t| t.is_some())
                .map(|t| t.unwrap())
                .collect();

            assert_eq!(complete, vec![1, 2, 3, 4, 5]);
        }

        #[test]
        #[traced_test]
        fn can_queue_use_stream_iterator_from_task_iterator() {
            let seed = rand::rng().next_u64();

            let shared_list = Arc::new(Mutex::new(Vec::new()));
            let counter = DCounter::new(5, shared_list.clone());

            let pool = thread_pool(seed, None);

            let iter = pool
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
