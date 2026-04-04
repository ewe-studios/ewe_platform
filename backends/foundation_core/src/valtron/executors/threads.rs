#![allow(clippy::new_ret_no_self)]
#![allow(clippy::type_complexity)]

use std::{
    any::Any,
    collections::HashMap,
    env,
    marker::PhantomData,
    panic,
    sync::{
        self,
        atomic::{self, AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{self, Instant},
};

use crate::valtron::{ConcurrentQueueStreamIterator, Stream, DEFAULT_YIELD_WAIT_TIME};
use concurrent_queue::{ConcurrentQueue, PushError};
use derive_more::derive::From;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::str::FromStr;

use crate::{
    extensions::result_ext::SendableBoxedError,
    retries::ExponentialBackoffDecider,
    synca::{
        mpp::{self, RecvIterator},
        Entry, EntryList, IdleMan, LockSignal, OnSignal, SleepyMan, WaitGroup,
    },
    valtron::{AnyResult, LocalThreadExecutor},
};

use crate::valtron::{
    BoxedExecutionEngine, BoxedPanicHandler, BoxedSendExecutionIterator, ExecutionAction,
    ExecutorError, FnMutReady, FnReady, OnNext, PriorityOrder, ProcessController,
    ReadyConsumingIter, SharedTaskQueue, TaskIterator, TaskReadyResolver, TaskStatus,
    TaskStatusMapper,
};

use crate::valtron::{
    executors::constants::{
        BACK_OFF_JITER, BACK_OFF_MAX_DURATION, BACK_OFF_MIN_DURATION, BACK_OFF_THREAD_FACTOR,
        DEFAULT_OP_READ_TIME, MAX_ROUNDS_IDLE_COUNT, MAX_ROUNDS_WHEN_SLEEPING_ENDS,
    },
    ConsumingIter, DoNext, StreamConsumingIter,
};

use crate::compati::{Mutex, RwLock};

// ============================================================================
// PoolGuard - Drop-based Lifecycle Handle
// ============================================================================

/// Returned by `initialize_pool`. Provides deterministic cleanup.
///
/// On drop (or explicit `shutdown()` call):
/// 1. `OnSignal::turn_on()` — broadcast kill to all threads
/// 2. `LockSignal::signal_all()` — wake any blocked/parked threads
/// 3. `BackgroundJobRegistry::shutdown()` — wait for background workers
/// 4. `WaitGroup::wait()` — block until all task threads report death
/// 5. Join all `JoinHandle`s
///
/// This replaces the old pattern of spawning a thread to call `get_pool().kill()`.
pub struct PoolGuard {
    registry: Arc<ThreadRegistry>,
    bg_registry: Option<Arc<super::BackgroundJobRegistry>>,
    shut_down: AtomicBool,
}

impl PoolGuard {
    /// Create a new `PoolGuard` with the given `ThreadRegistry`.
    #[must_use]
    pub fn new(registry: Arc<ThreadRegistry>) -> Self {
        Self {
            registry,
            bg_registry: None,
            shut_down: AtomicBool::new(false),
        }
    }

    /// Create a new `PoolGuard` with both registries.
    #[must_use]
    pub fn with_bg_registry(
        registry: Arc<ThreadRegistry>,
        bg_registry: Arc<super::BackgroundJobRegistry>,
    ) -> Self {
        Self {
            registry,
            bg_registry: Some(bg_registry),
            shut_down: AtomicBool::new(false),
        }
    }

    /// Create a dummy `PoolGuard` that does nothing on drop.
    /// Used for single-threaded/wasm builds where no thread pool is created.
    #[must_use]
    pub fn dummy() -> Self {
        Self {
            registry: Arc::new(ThreadRegistry::with_seed_and_threads(0, 2)),
            bg_registry: None,
            shut_down: AtomicBool::new(true),
        }
    }

    /// Explicit shutdown. Idempotent — safe to call multiple times.
    pub fn shutdown(&self) {
        if self
            .shut_down
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            // Signal kill to all threads (shared signal stops both registries)
            self.registry.kill_signal().turn_on();
            // Wake all blocked/parked task threads
            self.registry.latch().signal_all();
            // Shut down background workers first (they share the kill signal)
            if let Some(ref bg) = self.bg_registry {
                bg.shutdown();
            }
            // Wait for all task threads to report done
            self.registry.waitgroup().wait();
            // Join all task thread handles
            self.registry.join_all_threads();
        }
    }

    /// Get the `WaitGroup` for this pool.
    #[must_use]
    pub fn waitgroup(&self) -> &WaitGroup {
        self.registry.waitgroup()
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Max value for the thread counters.
const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;

/// [`get_allocatable_thread_count`] ensures we allocate enough of threads as requested
/// less 1 as the 1 thread will be used for waiting for kill signal and the remaining
/// for task execution in situations where on 2 threads can be created apart from the current
/// process.
///
/// # Panics
///
/// Panics if the desired thread count is greater than the maximum allowed threads.
/// Panics if the desired thread count is zero.
pub fn get_allocatable_thread_count() -> usize {
    let max_threads = get_max_threads();
    tracing::debug!("Max available threads: {max_threads:}");
    let desired_threads = get_num_threads();
    tracing::debug!("Desired thread count: {desired_threads:}");

    assert!(
        (desired_threads <= max_threads),
        "Desired thread count cant be greater than maximum allowed threads {max_threads}"
    );

    assert!((desired_threads != 0), "Desired thread count cant be zero");

    assert!((max_threads >= 2), "Available thread cant be less than 2 we use 1 thread for the service kill signal, and 1 for tasks");

    assert!((desired_threads >= 2), "Requested thread cant be less than 2 we use 1 thread for the service kill signal, and 1 for tasks");

    // return size of threads with us keeping 2 for our purposes.
    if desired_threads == max_threads {
        return max_threads - 1;
    }

    let rem_threads = max_threads - desired_threads;
    tracing::debug!(
        "Remaining threads {} from desired: {} and max: {}",
        rem_threads,
        desired_threads,
        max_threads
    );

    desired_threads
}

#[cfg(test)]
mod test_allocatable_threads {
    use serial_test::serial;
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    #[serial]
    fn get_allocatable_thread_count_as_far_as_1_remains() {
        let max_threads = get_max_threads();
        assert!(max_threads > 3);

        env::remove_var("VALTRON_NUM_THREADS");

        let new_thread_count = max_threads - 2;
        tracing::debug!("Setting thread count to: {new_thread_count:}");

        let new_thread_count_str = format!("{new_thread_count:}");
        env::set_var("VALTRON_NUM_THREADS", new_thread_count_str.clone());
        assert_eq!(
            env::var("VALTRON_NUM_THREADS").unwrap(),
            new_thread_count_str
        );

        // Very flaky, for now asset non-zero
        // assert!(get_allocatable_thread_count() != 0);
        //
        assert_eq!(get_allocatable_thread_count(), max_threads - 2);
    }
}

/// [`get_max_threads`] returns the max threads allowed
/// in the current system.
pub fn get_max_threads() -> usize {
    let system_value = std::thread::available_parallelism()
        .ok()
        .map_or(1, std::num::NonZero::get);
    tracing::debug!("thread::available_parallelism() reported: {}", system_value);
    system_value
}

/// [`get_num_threads`] will attempt to fetch the desired thread we want
/// from the either the environment variable `VALTRON_NUM_THREADS`
/// or gets the maximum allowed thread from the platform
/// via [`std::thread::available_parallelism`];
pub fn get_num_threads() -> usize {
    let thread_num = match env::var("VALTRON_NUM_THREADS")
        .ok()
        .and_then(|s| usize::from_str(&s).ok())
    {
        Some(x) => {
            tracing::debug!("Retrieved thread_number: {x:} from VALTRON_NUM_THREADS");
            x
        }
        _ => get_max_threads(),
    };

    tracing::debug!("Reporting Thread available for use: {}", thread_num);

    thread_num
}

#[cfg(test)]
mod test_get_num_threads {
    use std::env;

    use tracing_test::traced_test;

    use crate::valtron::get_num_threads;

    #[test]
    #[traced_test]
    fn test_get_num_threads_when_env_is_not_set() {
        env::remove_var("VALTRON_NUM_THREADS");
        let thread_num = get_num_threads();
        dbg!(&thread_num);
        assert_ne!(thread_num, 0);
    }

    #[test]
    #[traced_test]
    fn test_get_num_threads_when_env_is_set() {
        env::remove_var("VALTRON_NUM_THREADS");
        env::set_var("VALTRON_NUM_THREADS", "2");
        assert_eq!(get_num_threads(), 2);
        env::remove_var("VALTRON_NUM_THREADS");
    }
}

pub struct ThreadYielder {
    thread_id: ThreadId,
    latch: Arc<LockSignal>,
    sender: mpp::Sender<ThreadActivity>,
}

impl ThreadYielder {
    pub fn new(
        thread_id: ThreadId,
        latch: Arc<LockSignal>,
        sender: mpp::Sender<ThreadActivity>,
    ) -> Self {
        Self {
            thread_id,
            latch,
            sender,
        }
    }
}

impl Clone for ThreadYielder {
    fn clone(&self) -> Self {
        Self::new(
            self.thread_id.clone(),
            self.latch.clone(),
            self.sender.clone(),
        )
    }
}

impl ProcessController for ThreadYielder {
    fn yield_for(&self, dur: std::time::Duration) {
        // will request that the current thread be
        // parked for the giving duration though parking
        // may not last that duration so you need to be aware
        // the thread could be woken up, so we
        // specifically loop till we've reached beyond duration
        let started = Instant::now();

        let mut remaining_timeout = dur;
        loop {
            self.sender
                .send(ThreadActivity::Parked(self.thread_id.clone()))
                .expect("should sent event");

            std::thread::park_timeout(remaining_timeout);

            // check the state and see if we've crossed that threshold.
            let elapsed = started.elapsed();
            if elapsed >= remaining_timeout {
                break;
            }
            remaining_timeout -= remaining_timeout;
        }

        self.sender
            .send(ThreadActivity::Unparked(self.thread_id.clone()))
            .expect("should sent event");
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ThreadId(Entry, String);

impl ThreadId {
    #[must_use]
    pub fn new(entry: Entry, name: String) -> Self {
        Self(entry, name)
    }

    pub fn get_mut(&mut self) -> &mut Entry {
        &mut self.0
    }

    #[must_use]
    pub fn get_ref(&self) -> &Entry {
        &self.0
    }

    #[must_use]
    pub fn get_cloned(&self) -> Entry {
        self.0
    }

    #[must_use]
    pub fn get_name(&self) -> &String {
        &self.1
    }
}

pub enum ThreadActivity {
    /// Indicates when a Thread with an executor has started
    Started(ThreadId),

    /// Indicates when a Thread with an executor has stopped
    Stopped(ThreadId),

    /// Indicates when a Thread with an executor has
    /// blocked the thread with a `CondVar` waiting for
    /// signal to become awake.
    Blocked(ThreadId),

    /// Indicates when a Thread with an executor has
    /// bcome unblocked and now is awake to process
    /// pending tasks.
    Unblocked(ThreadId),

    /// Parked indicates when a thread has parked
    /// it's self for some duration.
    Parked(ThreadId),

    /// Unparked indicates when a thread has awoken
    /// from it's parked state.
    Unparked(ThreadId),

    /// Indicates when a thread executor panics
    /// killing the thread.
    Panicked(ThreadId, Box<dyn Any + Send>),

    /// Indicates the delivery of a task to the global queue.
    BroadcastedTask,
}

impl core::fmt::Display for ThreadActivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadActivity::Panicked(id, _) => {
                write!(f, "ThreadActivity::Panicked({id:?})")
            }
            ThreadActivity::Started(id) => {
                write!(f, "ThreadActivity::Started({id:?})")
            }
            ThreadActivity::Stopped(id) => {
                write!(f, "ThreadActivity::Stopped({id:?})")
            }
            ThreadActivity::Blocked(id) => {
                write!(f, "ThreadActivity::Blocked({id:?})")
            }
            ThreadActivity::Unblocked(id) => {
                write!(f, "ThreadActivity::Unblocked({id:?})")
            }
            ThreadActivity::Parked(id) => {
                write!(f, "ThreadActivity::Parked({id:?})")
            }
            ThreadActivity::Unparked(id) => {
                write!(f, "ThreadActivity::Unparked({id:?})")
            }
            ThreadActivity::BroadcastedTask => {
                write!(f, "ThreadActivity::BroadcastedTask")
            }
        }
    }
}

impl core::fmt::Debug for ThreadActivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadActivity::Panicked(id, _) => {
                write!(f, "ThreadActivity::Panicked({id:?})")
            }
            ThreadActivity::Started(id) => {
                write!(f, "ThreadActivity::Started({id:?})")
            }
            ThreadActivity::Stopped(id) => {
                write!(f, "ThreadActivity::Stopped({id:?})")
            }
            ThreadActivity::Blocked(id) => {
                write!(f, "ThreadActivity::Blocked({id:?})")
            }
            ThreadActivity::Unblocked(id) => {
                write!(f, "ThreadActivity::Unblocked({id:?})")
            }
            ThreadActivity::Parked(id) => {
                write!(f, "ThreadActivity::Parked({id:?})")
            }
            ThreadActivity::Unparked(id) => {
                write!(f, "ThreadActivity::Unparked({id:?})")
            }
            ThreadActivity::BroadcastedTask => {
                write!(f, "ThreadActivity::BroadcastedTask")
            }
        }
    }
}

#[derive(Clone)]
pub struct SharedThreadRegistry(sync::Arc<RwLock<ThreadPoolRegistryInner>>);

impl SharedThreadRegistry {
    pub fn new(inner: sync::Arc<RwLock<ThreadPoolRegistryInner>>) -> Self {
        Self(inner)
    }

    #[must_use]
    pub fn executor_count(&self) -> usize {
        let registry = self.0.read().unwrap();
        registry.threads.active_slots()
    }

    #[must_use]
    pub fn get_thread(&self, thread: ThreadId) -> ThreadRef {
        let registry = self.0.read().unwrap();

        match registry.threads.get(thread.get_ref()) {
            Some(thread_ref) => thread_ref.clone(),
            None => unreachable!("We should never get to a point where a ThreadId is invalid"),
        }
    }

    pub fn register_thread(
        &self,
        name: String,
        thread: ThreadRef,
        latch: Arc<LockSignal>,
        sender: mpp::Sender<ThreadActivity>,
    ) -> ThreadId {
        let mut registry = self.0.write().unwrap();

        // insert thread and get thread key.
        let entry = registry.threads.insert(thread);

        // update the key for the thread in place within the
        // entry and in the same lock.
        match registry.threads.get_mut(&entry) {
            Some(mutable_thread_ref) => {
                let thread_id = ThreadId(entry, name);
                mutable_thread_ref.registry_id = Some(thread_id.clone());
                mutable_thread_ref.process =
                    Some(ThreadYielder::new(thread_id.clone(), latch, sender));
                thread_id
            }
            None => unreachable!("Thread must be registered at the entry key"),
        }
    }
}

pub type SharedActivityQueue = sync::Arc<ConcurrentQueue<ThreadActivity>>;
pub type SharedThreadYielder = sync::Arc<ThreadYielder>;

#[derive(Clone)]
pub struct ThreadRef {
    /// name of thread ref
    pub name: String,

    /// seed for the giving thread ref
    pub seed: u64,

    /// the queue for tasks
    pub tasks: SharedTaskQueue,

    /// the relevant key used by the thread in
    /// the core thread registry.
    pub registry_id: Option<ThreadId>,

    /// the register this thread is related to.
    pub register: SharedThreadRegistry,

    /// global signal used for communicating death
    /// to all threads.
    pub global_kill_signal: sync::Arc<OnSignal>,

    /// process manager for controlling process yielding.
    pub process: Option<ThreadYielder>,
}

impl ThreadRef {
    pub fn new(
        seed: u64,
        name: String,
        queue: SharedTaskQueue,
        registry_id: Option<ThreadId>,
        register: SharedThreadRegistry,
        global_kill_signal: sync::Arc<OnSignal>,
    ) -> ThreadRef {
        Self {
            seed,
            name,
            tasks: queue,
            register,
            registry_id,
            global_kill_signal,
            process: None,
        }
    }
}

pub struct ThreadPoolRegistryInner {
    threads: EntryList<ThreadRef>,
}

impl ThreadPoolRegistryInner {
    #[must_use]
    pub fn new() -> SharedThreadRegistry {
        SharedThreadRegistry::new(sync::Arc::new(RwLock::new(ThreadPoolRegistryInner {
            threads: EntryList::new(),
        })))
    }
}

// -- ThreadExecutionError

pub type ThreadExecutionResult<T> = AnyResult<T, ThreadExecutionError>;

#[derive(From, Debug)]
pub enum ThreadExecutionError {
    #[from(ignore)]
    FailedStart(SendableBoxedError),

    #[from(ignore)]
    Panicked(Box<dyn Any + Send>),
}

impl core::error::Error for ThreadExecutionError {}

impl core::fmt::Display for ThreadExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Panicked(_) => write!(f, "ThreadExecutionError(_)"),
            _ => write!(f, "{self:?}"),
        }
    }
}

pub struct ThreadPoolTaskBuilder<
    Done: Send + 'static,
    Pending: Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
> {
    tasks: SharedTaskQueue,
    latch: Arc<LockSignal>,
    task: Option<Task>,
    resolver: Option<Resolver>,
    mappers: Option<Vec<Mapper>>,
    panic_handler: Option<BoxedPanicHandler>,
    _marker: PhantomData<(Done, Pending, Action)>,
}

impl<
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, Resolver, Task>
{
    pub fn new(tasks: SharedTaskQueue, latch: Arc<LockSignal>) -> Self {
        Self {
            tasks,
            latch,
            task: None,
            mappers: None,
            resolver: None,
            panic_handler: None,
            _marker: PhantomData,
        }
    }

    #[allow(clippy::return_self_not_must_use)]
    pub fn with_mappers(mut self, mapper: Mapper) -> Self {
        let mut mappers = if self.mappers.is_some() {
            self.mappers.take().unwrap()
        } else {
            Vec::new()
        };

        mappers.push(mapper);
        self.mappers = Some(mappers);
        self
    }

    #[must_use]
    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    #[allow(clippy::return_self_not_must_use)]
    pub fn with_panic_handler<T>(mut self, handler: T) -> Self
    where
        T: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(handler));
        self
    }

    #[allow(clippy::return_self_not_must_use)]
    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// `ready_iter` adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapping iterator that allows you synchronously receive those
    /// values from a `RecvIterator` which are only the [`TaskStatus::Ready`].
    /// values of the task.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn ready_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<RecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ReadyConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ReadyConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(boxed_task.into()) {
            Ok(()) => {
                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }

                Ok(RecvIterator::from_chan(iter_chan, wait_cycle))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `stream_iter` adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the state streams instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a `ConcurrentQueueStreamIterator` that implements the [`Iterator`] trait.
    ///
    /// But unlike `schedule_iter` returns [`Stream`] values that hide the underlying
    /// value types of `TaskStatus` which simplifies the trait types your usage
    /// requires.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// Uses default configuration: `DEFAULT_PARK_DURATION` for park duration and `DEFAULT_MAX_TURNS` for max turns.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn stream_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<ConcurrentQueueStreamIterator<Done, Pending>, ExecutorError> {
        self.stream_iter_with_config(wait_cycle, crate::valtron::executors::DEFAULT_MAX_TURNS)
    }

    /// `stream_iter_with_config` adds a task into execution queue and returns
    /// a `ConcurrentQueueStreamIterator` with configurable polling behavior.
    ///
    /// But unlike `schedule_iter` returns [`Stream`] values that hide the underlying
    /// value types of `TaskStatus` which simplifies the trait types your usage
    /// requires.
    ///
    /// This makes it possible to build synchronous experiences in an async world,
    /// with fine-grained control over iterator polling strategy.
    ///
    /// # Arguments
    ///
    /// * `wait_cycle` - Thread park duration when queue is empty (passed to `ConcurrentQueueStreamIterator`)
    /// * `max_turns` - Max poll attempts before yielding `Stream::Ignore`
    ///
    /// # Returns
    ///
    /// Returns a `ConcurrentQueueStreamIterator` that yields `Stream<Done, Pending>` items.
    pub fn stream_iter_with_config(
        self,
        wait_cycle: time::Duration,
        max_turns: usize,
    ) -> AnyResult<ConcurrentQueueStreamIterator<Done, Pending>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<Stream<Done, Pending>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => StreamConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => StreamConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(boxed_task.into()) {
            Ok(()) => {
                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }

                Ok(ConcurrentQueueStreamIterator::new(
                    iter_chan, max_turns, wait_cycle,
                ))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `schedule_iter` adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a `RecvIterator` that implements the [`Iterator`] trait.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn schedule_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<RecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(boxed_task.into()) {
            Ok(()) => {
                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }

                Ok(RecvIterator::from_chan(iter_chan, wait_cycle))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `schedule` adds a task into the global execution queue which should
    /// be processed by the underlying thread pool.
    pub fn schedule(self) -> AnyResult<(), ExecutorError> {
        let task: BoxedSendExecutionIterator = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let mut task_iter = OnNext::new(task, resolver, mappers);
                    if let Some(panic_handler) = self.panic_handler {
                        task_iter = task_iter.with_panic_handler(panic_handler);
                    }
                    Box::new(task_iter)
                }
                (Some(resolver), None) => {
                    let mut task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    if let Some(panic_handler) = self.panic_handler {
                        task_iter = task_iter.with_panic_handler(panic_handler);
                    }
                    Box::new(task_iter)
                }
                (None, None) => {
                    let mut task_iter = DoNext::new(task);
                    if let Some(panic_handler) = self.panic_handler {
                        task_iter = task_iter.with_panic_handler(panic_handler);
                    }
                    Box::new(task_iter)
                }
                (None, Some(_)) => return Err(ExecutorError::FailedToCreate),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(task) {
            Ok(()) => {
                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }
                Ok(())
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }
}

impl<
        F,
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, FnReady<F, Action>, Task>
where
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next(self, action: F) -> Self {
        self.with_resolver(FnReady::new(action))
    }
}

impl<
        F,
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, FnMutReady<F, Action>, Task>
where
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next_mut(self, action: F) -> Self {
        self.with_resolver(FnMutReady::new(action))
    }
}

// ============================================================================
// Tests for WaitGroup and PoolGuard
// ============================================================================

#[cfg(test)]
mod waitgroup_tests {
    use std::{panic, sync::Arc, thread, time::Duration};

    use crate::synca::WaitGroup;

    use super::{PoolGuard, ThreadRegistry};

    /// Test 1: WaitGroup add/done/wait basic functionality
    #[test]
    fn test_waitgroup_add_done_unblocks_wait() {
        let wg = WaitGroup::new();

        wg.add(3);

        let wg_clone = wg.clone();
        let handle = thread::spawn(move || {
            wg_clone.wait();
        });

        // Give the thread time to start waiting
        thread::sleep(Duration::from_millis(50));

        // Decrement 3 times
        wg.done();
        wg.done();
        wg.done();

        // Wait should complete
        handle.join().expect("thread should complete");
    }

    /// Test 2: WaitGroup with zero count returns immediately
    #[test]
    fn test_waitgroup_zero_count_returns_immediately() {
        let wg = WaitGroup::new();
        // Don't add anything, count is 0
        wg.wait(); // Should return immediately
    }

    /// Test 3: WaitGroupGuard calls done() on drop
    #[test]
    fn test_waitgroup_guard_calls_done_on_drop() {
        let wg = WaitGroup::new();
        wg.add(1);

        {
            let _guard = wg.guard();
            // Guard is alive, count is still 1
        }
        // Guard dropped, count should be 0

        wg.wait(); // Should return immediately since count is 0
    }

    /// Test 4: WaitGroupGuard calls done() even during panic
    #[test]
    fn test_waitgroup_guard_calls_done_during_panic() {
        let wg = WaitGroup::new();
        wg.add(1);

        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let _guard = wg.guard();
            panic!("intentional panic");
        }));

        assert!(result.is_err(), "panic should have occurred");
        // Guard was dropped, so done() was called
        wg.wait(); // Should return immediately
    }

    /// Test 5: Multiple WaitGroupGuards
    #[test]
    fn test_waitgroup_multiple_guards() {
        let wg = WaitGroup::new();
        wg.add(3);

        let guard1 = wg.guard();
        let guard2 = wg.guard();
        let guard3 = wg.guard();

        // All guards alive, count is still 3

        drop(guard1); // count -> 2
        drop(guard2); // count -> 1
        drop(guard3); // count -> 0

        wg.wait(); // Should return immediately
    }

    /// Test 6: PoolGuard shutdown is idempotent
    #[test]
    fn test_poolguard_shutdown_idempotent() {
        let registry = Arc::new(ThreadRegistry::with_seed_and_threads(123, 2));
        let guard = PoolGuard::new(registry.clone());

        // First shutdown
        guard.shutdown();

        // Second shutdown - should not panic
        guard.shutdown();

        // Third shutdown - still no panic
        guard.shutdown();
    }

    /// Test 7: PoolGuard drop triggers shutdown
    #[test]
    fn test_poolguard_drop_triggers_shutdown() {
        let registry = Arc::new(ThreadRegistry::with_seed_and_threads(123, 2));
        let guard = PoolGuard::new(registry.clone());

        // Spawn a worker thread
        registry.spawn_worker().expect("should spawn worker");

        // Drop the guard - it will kill all threads
        drop(guard);

        // If we get here, shutdown completed
    }

    /// Test 8: WaitGroup across multiple threads
    #[test]
    fn test_waitgroup_multiple_threads() {
        let wg = WaitGroup::new();
        let num_threads = 5;

        wg.add(num_threads);

        let mut handles = vec![];

        for i in 0..num_threads {
            let wg_clone = wg.clone();
            let handle = thread::spawn(move || {
                // Simulate some work
                thread::sleep(Duration::from_millis(10 * (i as u64 + 1)));
                wg_clone.done();
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        wg.wait();

        // All threads should have called done()
        for handle in handles {
            handle.join().expect("all threads should complete");
        }
    }
}

// ============================================================================
// ThreadRegistry - Central coordination replacing ThreadPool
// ============================================================================

/// `ThreadRegistry` replaces `ThreadPool` as the coordination point.
/// It holds the shared task queue, signals, activity channel, and thread handles.
pub struct ThreadRegistry {
    // Shared work distribution
    pub(crate) shared_tasks: SharedTaskQueue,
    pub(crate) latch: Arc<LockSignal>,

    // Kill coordination
    pub(crate) kill_signal: Arc<OnSignal>,
    pub(crate) kill_latch: Arc<LockSignal>,

    // Activity monitoring
    activity_sender: mpp::Sender<ThreadActivity>,
    pub(crate) activity_receiver: mpp::Receiver<ThreadActivity>,

    // Shutdown coordination
    pub(crate) waitgroup: WaitGroup,

    // Thread management
    rng: Mutex<ChaCha8Rng>,
    pub(crate) max_threads: usize,
    pub(crate) live_threads: Arc<AtomicUsize>,
    idle_threads: Arc<AtomicUsize>,

    // Worker thread config
    priority: PriorityOrder,
    op_read_time: time::Duration,
    yield_wait_time: time::Duration,
    thread_stack_size: Option<usize>,
    thread_max_idle_count: u32,
    thread_max_sleep_before_end: u32,
    thread_back_off_factor: u32,
    thread_back_off_jitter: f32,
    thread_back_min_duration: time::Duration,
    thread_back_max_duration: time::Duration,

    // Thread tracking
    registry: SharedThreadRegistry,
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,
}

impl std::fmt::Debug for ThreadRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadRegistry")
            .field("shared_tasks", &"<SharedTaskQueue>")
            .field("latch", &"<LockSignal>")
            .field("kill_signal", &"<OnSignal>")
            .field("kill_latch", &"<LockSignal>")
            .field("activity_sender", &"<Sender<ThreadActivity>>")
            .field("activity_receiver", &"<Receiver<ThreadActivity>>")
            .field("waitgroup", &"<WaitGroup>")
            .field("rng", &"<Mutex<ChaCha8Rng>>")
            .field("max_threads", &self.max_threads)
            .field("live_threads", &self.live_threads)
            .field("idle_threads", &self.idle_threads)
            .field("priority", &self.priority)
            .field("op_read_time", &self.op_read_time)
            .field("yield_wait_time", &self.yield_wait_time)
            .field("thread_stack_size", &self.thread_stack_size)
            .field("thread_max_idle_count", &self.thread_max_idle_count)
            .field(
                "thread_max_sleep_before_end",
                &self.thread_max_sleep_before_end,
            )
            .field("thread_back_off_factor", &self.thread_back_off_factor)
            .field("thread_back_off_jitter", &self.thread_back_off_jitter)
            .field("thread_back_min_duration", &self.thread_back_min_duration)
            .field("thread_back_max_duration", &self.thread_back_max_duration)
            .field("registry", &"<SharedThreadRegistry>")
            .field("thread_handles", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl ThreadRegistry {
    /// Create a new `ThreadRegistry` with the given seed and thread count.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        seed_for_rng: u64,
        num_threads: usize,
        priority: PriorityOrder,
        op_read_time: time::Duration,
        yield_wait: time::Duration,
        thread_stack_size: Option<usize>,
        thread_max_idle_count: u32,
        thread_max_sleep_before_end: u32,
        thread_back_off_factor: u32,
        thread_back_off_jitter: f32,
        thread_back_min_duration: time::Duration,
        thread_back_max_duration: time::Duration,
    ) -> Self {
        assert!(
            num_threads >= 2,
            "Unable to create ThreadRegistry with < 2 threads"
        );

        assert!(
            num_threads <= THREADS_MAX,
            "Thread count must not exceed {THREADS_MAX}"
        );

        let latch = Arc::new(LockSignal::new());
        let kill_latch = Arc::new(LockSignal::new());
        let shared_tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::unbounded());
        let (activity_sender, activity_receiver) = mpp::unbounded::<ThreadActivity>();

        Self {
            shared_tasks: shared_tasks.clone(),
            latch: latch.clone(),
            kill_signal: Arc::new(OnSignal::new()),
            kill_latch,
            activity_sender: activity_sender.clone(),
            activity_receiver,
            waitgroup: WaitGroup::new(),
            rng: Mutex::new(ChaCha8Rng::seed_from_u64(seed_for_rng)),
            max_threads: num_threads,
            live_threads: Arc::new(AtomicUsize::new(0)),
            idle_threads: Arc::new(AtomicUsize::new(0)),
            priority,
            op_read_time,
            yield_wait_time: yield_wait,
            thread_stack_size,
            thread_max_idle_count,
            thread_max_sleep_before_end,
            thread_back_off_factor,
            thread_back_off_jitter,
            thread_back_min_duration,
            thread_back_max_duration,
            registry: SharedThreadRegistry::new(Arc::new(RwLock::new(ThreadPoolRegistryInner {
                threads: EntryList::new(),
            }))),
            thread_handles: RwLock::new(HashMap::new()),
        }
    }

    /// Create a `ThreadRegistry` with default configuration.
    #[must_use]
    pub fn with_seed_and_threads(seed_from_rng: u64, num_threads: usize) -> Self {
        Self::new(
            seed_from_rng,
            num_threads,
            PriorityOrder::Top,
            DEFAULT_OP_READ_TIME,
            DEFAULT_YIELD_WAIT_TIME,
            None,
            MAX_ROUNDS_IDLE_COUNT,
            MAX_ROUNDS_WHEN_SLEEPING_ENDS,
            BACK_OFF_THREAD_FACTOR,
            BACK_OFF_JITER,
            BACK_OFF_MIN_DURATION,
            BACK_OFF_MAX_DURATION,
        )
    }

    /// Accessor for `shared_tasks` queue.
    #[must_use]
    pub fn shared_tasks(&self) -> SharedTaskQueue {
        self.shared_tasks.clone()
    }

    /// Accessor for latch.
    #[must_use]
    pub fn latch(&self) -> Arc<LockSignal> {
        self.latch.clone()
    }

    /// Accessor for `kill_signal`.
    #[must_use]
    pub fn kill_signal(&self) -> Arc<OnSignal> {
        self.kill_signal.clone()
    }

    /// Accessor for waitgroup.
    #[must_use]
    pub fn waitgroup(&self) -> &WaitGroup {
        &self.waitgroup
    }

    /// Shutdown the registry - signals kill and waits for all threads.
    pub fn shutdown(&self) {
        self.kill_signal.turn_on();
        self.latch.signal_all();
        self.waitgroup.wait();
        self.join_all_threads();
        self.kill_latch.signal_all();
    }

    /// Join all thread handles.
    pub fn join_all_threads(&self) {
        let handles = {
            let mut map = self.thread_handles.write().unwrap();
            std::mem::take(&mut *map)
        };

        for (_id, handle) in handles {
            let _ = handle.join();
        }
    }

    /// Block until all threads are done.
    pub fn block_until_done(&self) {
        loop {
            if self.listen_for_activity().is_none() {
                break;
            }
        }
        self.waitgroup.wait();
        self.join_all_threads();
        self.kill_latch.signal_all();
    }

    /// Listen for activity events from worker threads.
    fn listen_for_activity(&self) -> Option<()> {
        let remaining_live = self.live_threads.load(atomic::Ordering::Acquire);
        if remaining_live == 0 {
            tracing::debug!("No more live threads, returning");
            self.latch.signal_all();
            return None;
        }

        if self.kill_signal.probe() {
            tracing::debug!("Kill signal detected, waking up");
            self.latch.signal_all();
        }

        let thread_activity = match self.activity_receiver.recv_timeout(self.op_read_time) {
            Ok(activity) => activity,
            Err(err) => match err {
                mpp::ReceiverError::Empty => return Some(()),
                mpp::ReceiverError::Timeout => return Some(()),
                mpp::ReceiverError::Closed(_) => return None,
            },
        };

        match thread_activity {
            ThreadActivity::BroadcastedTask => {
                if self.shared_tasks.len() == 1 {
                    self.latch.signal_one();
                }
            }
            ThreadActivity::Started(_id) => {
                tracing::debug!("Thread started: {:?}", _id);
            }
            ThreadActivity::Stopped(id) => {
                tracing::debug!("Thread stopped: {:?}", id);
                self.live_threads.fetch_sub(1, atomic::Ordering::AcqRel);
                self.remove_thread_id(id);
            }
            ThreadActivity::Panicked(id, err) => {
                tracing::debug!("Thread panicked: {:?}, err: {:?}", id, err);
                self.live_threads.fetch_sub(1, atomic::Ordering::AcqRel);
                self.remove_thread_id(id);

                // Respawn if under max threads and not killed
                if self.live_threads.load(atomic::Ordering::Acquire) < self.max_threads
                    && !self.kill_signal.probe()
                {
                    tracing::debug!("Respawning thread after panic");
                    let _ = self.spawn_worker();
                }
            }
            ThreadActivity::Blocked(id) => {
                tracing::debug!("Thread blocked: {:?}", id);
                self.idle_threads.fetch_add(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Unblocked(id) => {
                tracing::debug!("Thread unblocked: {:?}", id);
                self.idle_threads.fetch_sub(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Parked(id) => {
                tracing::debug!("Thread parked: {:?}", id);
                self.idle_threads.fetch_add(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Unparked(id) => {
                tracing::debug!("Thread unparked: {:?}", id);
                self.idle_threads.fetch_sub(1, atomic::Ordering::AcqRel);
            }
        }

        Some(())
    }

    fn remove_thread_id(&self, thread_id: ThreadId) {
        self.thread_handles.write().unwrap().remove(&thread_id);
    }

    /// Spawn a worker thread with `WaitGroup` tracking.
    pub fn spawn_worker(&self) -> ThreadExecutionResult<ThreadRef> {
        use std::thread::Builder;

        let mut rng = self.rng.lock().unwrap();
        let seed = rng.next_u64();
        drop(rng);

        let b = Builder::new();
        let b = if let Some(size) = self.thread_stack_size {
            b.stack_size(size)
        } else {
            b
        };

        // Register thread in registry first
        let thread_ref = ThreadRef::new(
            seed,
            format!("worker-{seed}"),
            self.shared_tasks.clone(),
            None,
            self.registry.clone(),
            self.kill_signal.clone(),
        );

        let thread_id = self.registry.register_thread(
            thread_ref.name.clone(),
            thread_ref.clone(),
            self.latch.clone(),
            self.activity_sender.clone(),
        );

        // Get updated thread_ref from registry (process field is now populated)
        let thread_ref = self.registry.get_thread(thread_id.clone());

        let priority = self.priority.clone();
        let seed_clone = thread_ref.seed;
        let task_clone = thread_ref.tasks.clone();
        let process_clone = thread_ref.process.clone().unwrap();
        let thread_kill_signal = thread_ref.global_kill_signal.clone();

        let sender_id = thread_id.clone();
        let sender = self.activity_sender.clone();

        let thread_yield_wait_duration = self.yield_wait_time;
        let thread_max_idle_count = self.thread_max_idle_count;
        let thread_max_sleep_before_end = self.thread_max_sleep_before_end;
        let thread_back_off_factor = self.thread_back_off_factor;
        let thread_back_off_jitter = self.thread_back_off_jitter;
        let thread_back_min_duration = self.thread_back_min_duration;
        let thread_back_max_duration = self.thread_back_max_duration;

        // Get WaitGroup guard BEFORE spawning
        let wg_guard = self.waitgroup.guard();
        self.waitgroup.add(1);

        match b.spawn(move || {
            let span = tracing::trace_span!("ThreadRegistry::spawn_worker.local_executor.thread");
            let _enter = span.enter();

            // Hold guard for lifetime of thread - dropped on exit (including panic)
            let _wg = wg_guard;

            match panic::catch_unwind(|| {
                sender
                    .send(ThreadActivity::Started(sender_id.clone()))
                    .expect("should send event");

                let thread_executor = LocalThreadExecutor::from_seed(
                    seed_clone,
                    task_clone,
                    IdleMan::new(
                        thread_max_idle_count,
                        None,
                        SleepyMan::new(
                            thread_max_sleep_before_end,
                            ExponentialBackoffDecider::new(
                                thread_back_off_factor,
                                thread_back_off_jitter,
                                thread_back_min_duration,
                                Some(thread_back_max_duration),
                            ),
                        ),
                    ),
                    priority,
                    process_clone,
                    thread_yield_wait_duration,
                    Some(thread_kill_signal),
                    Some(sender.clone()),
                );

                thread_executor.block_on();

                sender
                    .send(ThreadActivity::Stopped(sender_id.clone()))
                    .expect("should send event");
            }) {
                Ok(()) => Ok(()),
                Err(err) => {
                    tracing::debug!("Thread panicked: {:?}: {:?}", sender_id, err);
                    sender
                        .send(ThreadActivity::Panicked(sender_id.clone(), err))
                        .expect("should send event");
                    sender
                        .send(ThreadActivity::Stopped(sender_id))
                        .expect("should send event");
                    Ok(())
                }
            }
            // _wg dropped here → WaitGroup::done()
        }) {
            Ok(handle) => {
                self.thread_handles
                    .write()
                    .unwrap()
                    .insert(thread_id.clone(), handle);

                self.live_threads.fetch_add(1, atomic::Ordering::AcqRel);

                Ok(thread_ref.clone())
            }
            Err(err) => Err(ThreadExecutionError::FailedStart(Box::new(err))),
        }
    }
}
