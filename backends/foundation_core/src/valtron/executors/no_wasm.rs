#![cfg(all(not(target_arch = "wasm32"), not(feature = "nothread_runtime")))]

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

pub fn thread_pool(pool_seed: u64) -> &'static ThreadPool {
    // register thread pool
    let thread_pool = GLOBAL_THREAD_POOL.get_or_init(|| {
        let thread_num = get_allocatable_thread_count();
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
pub fn block_on<F>(seed_from_rng: u64, setup: F)
where
    F: FnOnce(&ThreadPool),
{
    let pool = thread_pool(seed_from_rng);
    setup(pool);
    pool.run_until();
}

/// `spawn` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<
    Task::Done,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner> + Send + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Done, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Done: Send + 'static,
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
) -> ThreadPoolTaskBuilder<Task::Done, Task::Pending, Action, Mapper, Resolver, Task>
where
    Task::Done: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Task::Done, Task::Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Task::Done, Task::Pending> + Send + 'static,
{
    match GLOBAL_THREAD_POOL.get() {
        Some(pool) => pool.spawn2(),
        None => panic!("Thread pool not initialized, ensure to call block_on first"),
    }
}

#[cfg(test)]
mod no_wasm_tests {
    use flume;
    use std::{sync::Mutex, thread};

    use rand::RngCore;
    use tracing_test::traced_test;

    use crate::valtron::{block_on, get_pool, FnReady, NoSpawner, TaskIterator};

    struct Counter(usize, Mutex<Vec<usize>>, flume::Sender<()>);

    impl Counter {
        pub fn new(val: usize) -> (Self, flume::Receiver<()>) {
            let (sender, receiver) = flume::bounded(10);
            (Counter(val, Mutex::new(vec![0]), sender), receiver)
        }
    }

    impl Counter {
        pub fn into_inner(self) -> Vec<usize> {
            self.1.into_inner().expect("get list")
        }
    }

    impl TaskIterator for Counter {
        type Pending = ();

        type Done = usize;

        type Spawner = NoSpawner;

        fn next(
            &mut self,
        ) -> Option<crate::valtron::TaskStatus<Self::Done, Self::Pending, Self::Spawner>> {
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

    #[test]
    #[traced_test]
    fn can_queue_and_complete_task() {
        let seed = rand::thread_rng().next_u64();

        let (counter, receiver) = Counter::new(5);

        let handler_kill = thread::spawn(move || {
            tracing::debug!("Waiting for kill signal");
            receiver.recv().expect("receive signal");
            tracing::debug!("Got kill signal");
            get_pool().kill();
            tracing::debug!("Closing thread");
        });

        block_on(seed, |pool| {
            pool.spawn()
                .with_task(counter)
                .with_resolver(Box::new(FnReady::new(|item, _| {
                    tracing::info!("Received next: {:?}", item)
                })))
                .schedule()
                .expect("should deliver task");
        });

        handler_kill.join().expect("should finish");
    }
}
