#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
use wasm_sync::OnceLock;

use crate::valtron::get_allocatable_thread_count;

use super::{
    ExecutionAction, TaskIterator, TaskReadyResolver, TaskStatusMapper, ThreadPool,
    ThreadPoolTaskBuilder,
};

static CANCELATION_REGISTRATION: OnceLock<Option<()>> = OnceLock::new();
static GLOBAL_THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

pub fn thread_pool(pool_seed: u64, user_thread_num: Option<usize>) -> &'static ThreadPool {
    // register thread pool
    let thread_pool = GLOBAL_THREAD_POOL.get_or_init(|| {
        let thread_num = match user_thread_num {
            None => get_allocatable_thread_count(),
            Some(num) => num,
        };

        let thread_pool = ThreadPool::with_seed_and_threads(pool_seed, thread_num);
        thread_pool
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

/// [get_pool] returns the initialized thread_pool for your use.
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
    setup(pool);
    pool.run_until();
}

/// `spawn` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
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

/// `spawn2` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you to provide types for both Mapper and Resolver.
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
        valtron::{FnReady, NoSpawner, TaskIterator},
    };

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
            let mut items = self.1.lock().unwrap();
            let item_size = items.len();

            if item_size == self.0 {
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
            panic!("Bad stuff");
        }
    }

    #[test]
    #[traced_test]
    fn can_finish_even_when_task_panics() {
        let seed = rand::thread_rng().next_u64();

        let handler_kill = thread::spawn(move || {
            tracing::debug!("Waiting for kill signal");
            thread::sleep(time::Duration::from_millis(800));
            tracing::debug!("Got kill signal");
            get_pool().kill();
            tracing::debug!("Closing thread");
        });

        block_on(seed, None, |pool| {
            pool.spawn()
                .with_task(PanicCounter::default())
                .with_resolver(Box::new(FnReady::new(|item, _| {
                    tracing::info!("Received next: {item:?}")
                })))
                .schedule()
                .expect("should deliver task");
        });

        tracing::info!("Wait for thread to die");
        handler_kill.join().expect("should finish");
        tracing::info!("Wait for thread to die");
    }

    #[test]
    #[traced_test]
    fn can_queue_and_complete_task() {
        let seed = rand::thread_rng().next_u64();

        let shared_list = Arc::new(Mutex::new(vec![]));
        let (counter, receiver) = Counter::new(5, shared_list.clone());

        let handler_kill = thread::spawn(move || {
            tracing::debug!("Waiting for kill signal");
            receiver
                .recv_timeout(time::Duration::from_millis(800))
                .expect("receive signal");
            tracing::debug!("Got kill signal");
            get_pool().kill();
            tracing::debug!("Closing thread");
        });

        block_on(seed, None, |pool| {
            pool.spawn()
                .with_task(counter)
                .with_resolver(Box::new(FnReady::new(|item, _| {
                    tracing::info!("Received next: {item:?}")
                })))
                .schedule()
                .expect("should deliver task");
        });

        handler_kill.join().expect("should finish");

        assert_eq!(shared_list.lock().unwrap().clone(), vec![0, 1, 2, 3, 4]);
    }
}
