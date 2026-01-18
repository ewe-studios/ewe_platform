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
        atomic::{self, AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{self, Instant},
};

use crate::{synca::mpp::StreamRecvIterator, valtron::iterators::Stream};
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
        Entry, EntryList, IdleMan, LockSignal, OnSignal, RunOnDrop, SleepyMan,
    },
    valtron::{AnyResult, LocalThreadExecutor},
};

use super::{
    constants::{DEFAULT_OP_READ_TIME, MAX_ROUNDS_IDLE_COUNT, MAX_ROUNDS_WHEN_SLEEPING_ENDS, BACK_OFF_THREAD_FACTOR, BACK_OFF_JITER, BACK_OFF_MIN_DURATION, BACK_OFF_MAX_DURATION}, BoxedExecutionEngine, BoxedPanicHandler, BoxedSendExecutionIterator,
    ConsumingIter, StreamConsumingIter, DoNext, ExecutionAction, ExecutionIterator, ExecutorError, FnMutReady, FnReady,
    OnNext, PriorityOrder, ProcessController, ReadyConsumingIter, TaskIterator, TaskReadyResolver,
    TaskStatus, TaskStatusMapper,
};

use crate::compati::{Mutex, RwLock};

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
pub(crate) fn get_allocatable_thread_count() -> usize {
    let max_threads = get_max_threads();
    tracing::debug!("Max available threads: {max_threads:}");
    let desired_threads = get_num_threads();
    tracing::debug!("Desired thread count: {desired_threads:}");

    assert!((desired_threads <= max_threads), "Desired thread count cant be greater than maximum allowed threads {max_threads}");

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
    use tracing_test::traced_test;
    use serial_test::serial;

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
pub(crate) fn get_max_threads() -> usize {
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
pub(crate) fn get_num_threads() -> usize {
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
    fn yield_process(&self) {
        if self.latch.try_lock() {
            tracing::debug!("Thread successfully locked thread");
        } else {
            tracing::debug!("Thread is already locked");
        }

        self.sender
            .send(ThreadActivity::Blocked(self.thread_id.clone()))
            .expect("should sent event");
        tracing::debug!("Sent blocked signal");
        self.latch.wait();
        self.sender
            .send(ThreadActivity::Unblocked(self.thread_id.clone()))
            .expect("should sent event");
        tracing::debug!("Sent unblocked signal");
    }

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

pub type SharedTaskQueue = sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>;
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
    pub(crate) fn new() -> SharedThreadRegistry {
        SharedThreadRegistry::new(sync::Arc::new(RwLock::new(ThreadPoolRegistryInner {
            threads: EntryList::new(),
        })))
    }
}

// --- ThreadPool

pub struct ThreadPool {
    rng: Mutex<ChaCha8Rng>,
    num_threads: usize,
    latch: Arc<LockSignal>,
    kill_latch: Arc<LockSignal>,
    tasks: SharedTaskQueue,
    priority: PriorityOrder,
    op_read_time: time::Duration,
    parked_threads: Mutex<Vec<ThreadId>>,
    blocked_threads: Mutex<Vec<ThreadId>>,
    registry: SharedThreadRegistry,
    thread_stack_size: Option<usize>,
    global_kill_signal: sync::Arc<OnSignal>,
    activity_sender: mpp::Sender<ThreadActivity>,
    activity_receiver: mpp::Receiver<ThreadActivity>,

    // thread config values
    thread_max_idle_count: u32,
    thread_max_sleep_before_end: u32,
    thread_back_off_factor: u32,
    thread_back_off_jitter: f32,
    thread_back_min_duration: time::Duration,
    thread_back_max_duration: time::Duration,

    // atomics
    live_threads: Arc<AtomicUsize>,
    idle_threads: Arc<AtomicUsize>,

    // thread mappings.
    thread_map: RwLock<HashMap<ThreadId, ThreadRef>>,
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,
}

// -- Default

// -- constructor

impl ThreadPool {
    /// `with_rng` allows you to provide a custom Random number generator
    /// that can be used to generate as the initial seed the
    /// `ThreadPool` uses for it's local execution threads.
    ///
    /// We use the default to max threads allowed per process via `get_num_threads()`.
    pub fn with_rng<R: rand::Rng>(rng: &mut R) -> Self {
        let num_threads = get_num_threads();
        Self::with_seed_and_threads(rng.next_u64(), num_threads)
    }

    /// [`ThreadPool::with_seed`] generates a `ThreadPool` using
    /// the provided seed and the default MAX threads allowed
    /// per Process using `get_num_threads()`.
    #[must_use] 
    pub fn with_seed(seed_from_rng: u64) -> Self {
        let num_threads = get_num_threads();
        Self::with_seed_and_threads(seed_from_rng, num_threads)
    }

    /// [`ThreadPool::with_seed_and_threads`] generates a
    /// threadPool which uses the default values (see Constants section)
    /// set out in this modules for all required configuration
    /// which provide what we considered sensible defaults
    #[must_use] 
    pub fn with_seed_and_threads(seed_from_rng: u64, num_threads: usize) -> Self {
        Self::new(
            seed_from_rng,
            num_threads,
            PriorityOrder::Top,
            DEFAULT_OP_READ_TIME,
            None,
            MAX_ROUNDS_IDLE_COUNT,
            MAX_ROUNDS_WHEN_SLEEPING_ENDS,
            BACK_OFF_THREAD_FACTOR,
            BACK_OFF_JITER,
            BACK_OFF_MIN_DURATION,
            BACK_OFF_MAX_DURATION,
        )
    }

    /// [`new`] creates a new `ThreadPool` for you which you can use
    /// for concurrent execution of `LocalExecutorEngine` executors
    /// within the total number of threads you provided via `num_threads`
    /// the threads are spawned and ready to take on work.
    #[allow(clippy::too_many_arguments)]
    #[must_use] 
    pub fn new(
        seed_for_rng: u64,
        num_threads: usize,
        priority: PriorityOrder,
        op_read_time: time::Duration,
        thread_stack_size: Option<usize>,
        thread_max_idle_count: u32,
        thread_max_sleep_before_end: u32,
        thread_back_off_factor: u32,
        thread_back_off_jitter: f32,
        thread_back_min_duration: time::Duration,
        thread_back_max_duration: time::Duration,
    ) -> Self {
        assert!((num_threads >= 2), "Unable to create ThreadPool with 1 thread only, please specify >= 2");

        assert!((num_threads <= THREADS_MAX), 
                "Unable to create ThreadPool with thread numbers of {num_threads}, must no go past {THREADS_MAX}"
            );
        let thread_latch = Arc::new(LockSignal::new());
        let kill_latch = Arc::new(LockSignal::new());
        let tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::unbounded());

        let (sender, receiver) = mpp::unbounded::<ThreadActivity>();
        let thread_pool = Self {
            tasks,
            priority,
            kill_latch,
            num_threads,
            op_read_time,
            thread_stack_size,
            thread_max_idle_count,
            thread_max_sleep_before_end,
            thread_back_off_factor,
            thread_back_off_jitter,
            thread_back_min_duration,
            thread_back_max_duration,
            activity_receiver: receiver,
            latch: thread_latch.clone(),
            activity_sender: sender.clone(),
            parked_threads: Mutex::new(Vec::new()),
            blocked_threads: Mutex::new(Vec::new()),
            thread_map: RwLock::new(HashMap::new()),
            registry: ThreadPoolRegistryInner::new(),
            thread_handles: RwLock::new(HashMap::new()),
            live_threads: Arc::new(AtomicUsize::new(0)),
            idle_threads: Arc::new(AtomicUsize::new(0)),
            global_kill_signal: sync::Arc::new(OnSignal::new()),
            rng: Mutex::new(ChaCha8Rng::seed_from_u64(seed_for_rng)),
        };

        // spawn the number of threads requested.
        for index in 1..num_threads {
            let _ = thread_pool
                .create_thread_executor()
                .unwrap_or_else(|_| panic!("should successfully create thread for {index}"));
        }

        thread_pool
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

// -- surface entry methods

impl ThreadPool {
    #[allow(clippy::too_many_lines)]
    /// [`create_thread_executor`] creates a new thread into the thread pool spawning
    /// a [`LocalThreadExecutor`] into a owned thread that is managed by the executor.
    pub(crate) fn create_thread_executor(&self) -> ThreadExecutionResult<ThreadRef> {
        let span = tracing::trace_span!("ThreadPool::create_thread_executor");
        let _enter = span.enter();

        let thread_name = format!("valtron_thread_{}", self.registry.executor_count() + 1);
        let mut b = thread::Builder::new().name(thread_name.clone());
        if let Some(thread_stack_size) = self.thread_stack_size {
            b = b.stack_size(thread_stack_size);
        }

        let thread_seed = self.rng.lock().unwrap().next_u64();
        let thread_id = self.registry.register_thread(
            thread_name.clone(),
            ThreadRef {
                name: thread_name.clone(),
                process: None,
                registry_id: None,
                seed: thread_seed,
                tasks: self.tasks.clone(),
                register: self.registry.clone(),
                global_kill_signal: self.global_kill_signal.clone(),
            },
            self.latch.clone(),
            self.activity_sender.clone(),
        );

        let thread_ref = self.registry.get_thread(thread_id.clone());

        let priority = self.priority.clone();
        let seed_clone = thread_ref.seed;
        let task_clone = thread_ref.tasks.clone();
        let process_clone = thread_ref.process.clone().unwrap();
        let thread_kill_signal = thread_ref.global_kill_signal.clone();

        let sender_id = thread_id.clone();
        let sender = self.activity_sender.clone();

        let thread_max_idle_count = self.thread_max_idle_count;
        let thread_max_sleep_before_end = self.thread_max_sleep_before_end;
        let thread_back_off_factor = self.thread_back_off_factor;
        let thread_back_off_jitter = self.thread_back_off_jitter;
        let thread_back_min_duration = self.thread_back_min_duration;
        let thread_back_max_duration = self.thread_back_max_duration;

        match b.spawn(move || {
            let span =
                tracing::trace_span!("ThreadPool::create_thread_executor.local_executor.thread");
            let _enter = span.enter();

            match panic::catch_unwind(|| {
                sender
                    .send(ThreadActivity::Started(sender_id.clone()))
                    .expect("should sent event");

                // create LocalExecutionEngine here and
                // let it handle everything going forward.
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
                    Some(thread_kill_signal),
                    Some(sender.clone()),
                );

                thread_executor.block_on();

                tracing::debug!(
                    "Thread({:?}) has stopped executing, sending stop signal",
                    sender_id
                );
                sender
                    .send(ThreadActivity::Stopped(sender_id.clone()))
                    .expect("should sent event");

                tracing::debug!(
                    "Thread executor has died: {:?} and sent stopped activity",
                    sender_id
                );
            }) {
                Ok(()) => Ok(()),
                Err(err) => {
                    tracing::debug!("Thread executor has panic: {:?}: {:?}", sender_id, err);
                    sender
                        .send(ThreadActivity::Panicked(sender_id.clone(), err))
                        .expect("should sent event");
                    sender
                        .send(ThreadActivity::Stopped(sender_id))
                        .expect("should sent event");
                    Ok(())
                }
            }
        }) {
            Ok(handler) => {
                self.thread_handles
                    .write()
                    .unwrap()
                    .insert(thread_id.clone(), handler);

                self.thread_map
                    .write()
                    .unwrap()
                    .insert(thread_id.clone(), thread_ref.clone());

                self.live_threads.fetch_add(1, atomic::Ordering::AcqRel);

                Ok(thread_ref.clone())
            }
            Err(err) => Err(ThreadExecutionError::FailedStart(Box::new(err))),
        }
    }

    /// schedule adds the provided task into the global queue after boxing.
    /// This ensures we can sent the giving task into the global queue to be
    /// engaged by one of the many thread specific `LocalExecutorEngine`.
    pub fn schedule<T: ExecutionIterator + Send + 'static>(
        &self,
        task: T,
    ) -> AnyResult<(), ExecutorError> {
        let span = tracing::trace_span!("ThreadPool::schedule");
        span.in_scope(|| match self.tasks.push(Box::new(task)) {
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
        })
    }
}

// -- spawn methods

impl ThreadPool {
    /// `spawn2` provides a builder which specifically allows you to build out
    /// the underlying tasks to be scheduled into the global queue.
    ///
    /// It expects you to provide types for both Mapper and Resolver.
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
        ThreadPoolTaskBuilder::new(self.tasks.clone(), self.latch.clone())
    }

    /// `spawn` provides a builder which specifically allows you to build out
    /// the underlying tasks to be scheduled into the global queue.
    ///
    /// It expects you infer the type of `Task` and `Action` from the
    /// type implementing `TaskIterator`.
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
        ThreadPoolTaskBuilder::new(self.tasks.clone(), self.latch.clone())
    }
}

// -- core methods

impl ThreadPool {
    /// [`kill`] will deliver the kill signal to
    /// all running `LocalExecutorEngine` running in
    /// all the threads, then block the current thread with
    /// a `CondVar` awaiting the signal from `Self::run_until()` that
    /// both the execution process and all threads have fully been
    /// stopped and cleaned up at which point a signal is sent to awake
    /// the thread where [`Self::kill`].
    ///
    /// Its very important to not call both [`Self::kill`] and [`Self::run_until`]
    /// in the same thread as that will lead to a deadlock.
    ///
    /// Usually I would see you calling this method in a secondary thread listening
    /// for the kill (KILL, SIGUP, ...etc) signal from a CLI process.
    pub fn kill(&self) {
        let span = tracing::trace_span!("ThreadPool::kill");
        let _enter = span.enter();
        self.global_kill_signal.turn_on();
        self.latch.signal_all();
        tracing::debug!("Wait for kill latch to awake");

        let remaining_threads = self.live_threads.load(Ordering::Acquire);
        let blocked_threads = self.blocked_threads.lock().unwrap().len();
        if remaining_threads == 0 && blocked_threads == 0 {
            tracing::debug!("No more live threads or blocked threads, so can die");
            return;
        }
        self.kill_latch.lock_and_wait();
    }

    /// pulls all relevant threads `JoinHandle`
    /// joining them all till they all have finished and exited.
    pub(crate) fn await_threads(&self) {
        let span = tracing::trace_span!("ThreadPool::await_threads");
        let _enter = span.enter();

        let thread_keys: Vec<ThreadId> = {
            tracing::debug!("ThreadPool::await_threads: get handles");
            let handle = self.thread_handles.read().unwrap();

            tracing::debug!("ThreadPool::await_threads: clone handles");
            handle.keys().cloned().collect()
        };

        tracing::debug!("ThreadPool::await_threads: get writer");
        let mut handles = self.thread_handles.write().unwrap();
        tracing::debug!("Thread handles: {:?}", &handles);

        for thread_id in thread_keys {
            match handles.remove(&thread_id) {
                None => continue,
                Some(thread_handle) => {
                    tracing::debug!("Waiting for thread with id: {:?} to die", thread_id);
                    thread_handle
                        .join()
                        .expect("should return result")
                        .expect("finished");
                    continue;
                }
            };
        }

        tracing::debug!("ThreadPool::await_threads: Finished");
    }

    /// [`run_until`] will block the current thread
    /// and starts listening to both kill signal from the
    /// process via `Self::kill` and handle management operations
    /// from notifications sent by the execution threads when threads
    /// become idle, get killed, are parked, ..etc.
    ///
    /// CAUTION: You must be careful not to call `Self::run_until` and `Self::kill`
    /// in the same thread as a `CondVar` is used and will block the current thread
    /// till `run_until` signals the `CondVar` to wake up the blocked thread when `Self::kill`
    /// is ever called.
    pub fn run_until(&self) {
        let span = tracing::trace_span!("ThreadPool::run_until");
        let _enter = span.enter();
        self.block_until();
        self.await_threads();
        tracing::debug!("Signal death to kill latch");
        self.kill_latch.signal_all();
    }

    fn block_until(&self) {
        let span = tracing::trace_span!("ThreadPool::block_until");
        let _enter = span.enter();

        let _dropper = RunOnDrop::new(|| {
            tracing::debug!("ThreadPool::block_until has reached dropper");
            self.latch.signal_all();
            tracing::debug!("ThreadPool::block_until sent signal_all via latch");
        });

        loop {
            tracing::debug!("ThreadPool::block_until - Checking thread activity");
            if self.listen_for_normal_activity().is_none() {
                break;
            }
        }

        tracing::debug!("ThreadPool::block_until - finished");
    }

    fn add_thread_id_from_packed_list(&self, thread_id: ThreadId) {
        let mut thread_list = self.parked_threads.lock().unwrap();
        thread_list.push(thread_id);
    }

    fn remove_thread_id_from_packed_list(&self, thread_id: ThreadId) {
        let mut thread_list = self.parked_threads.lock().unwrap();
        if let Some(index) = thread_list.iter().position(|item| *item == thread_id) {
            thread_list.remove(index);
        }
    }

    fn add_thread_id_from_blocked_list(&self, thread_id: ThreadId) {
        let mut thread_list = self.blocked_threads.lock().unwrap();
        thread_list.push(thread_id);
    }

    fn remove_thread_id_from_blocked_list(&self, thread_id: ThreadId) {
        let mut thread_list = self.blocked_threads.lock().unwrap();
        if let Some(index) = thread_list.iter().position(|item| *item == thread_id) {
            thread_list.remove(index);
        }
    }

    fn listen_for_normal_activity(&self) -> Option<()> {
        let remaining_live = self.live_threads.load(atomic::Ordering::Acquire);
        if remaining_live == 0 {
            tracing::debug!("No more live threads, so returning");
            self.latch.signal_all();
            return None;
        }

        if self.global_kill_signal.probe() {
            tracing::debug!("signal wakeup via latch");
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
            ThreadActivity::BroadcastedTask if self.tasks.len() == 1 => {
                self.latch.signal_one();
            }
            ThreadActivity::BroadcastedTask if self.tasks.len() > 1 => {
                self.latch.signal_all();
            }
            ThreadActivity::BroadcastedTask => {
                tracing::debug!("Broadcast task to global queue");
            }
            ThreadActivity::Started(thread_id) => {
                tracing::debug!("Thread executor with id: {:?} started", thread_id);
                // self.live_threads.fetch_add(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Stopped(thread_id) => {
                let _ = self.live_threads.fetch_sub(1, atomic::Ordering::AcqRel);
                let remaining_live = self.live_threads.load(atomic::Ordering::Acquire);
                tracing::debug!(
                    "Thread executor with id: {:?} stopped with remaining: {}",
                    thread_id,
                    remaining_live
                );

                if self.global_kill_signal.probe() {
                    tracing::debug!("Stopping: should kill set, send latch wakeup");
                    self.latch.signal_all();
                } else {
                    tracing::debug!("Should kill signal not yet set");
                }
            }
            ThreadActivity::Parked(thread_id) => {
                tracing::debug!("Thread executor with id: {:?} has been parked", thread_id);
                self.idle_threads.fetch_add(1, atomic::Ordering::SeqCst);
                self.add_thread_id_from_packed_list(thread_id);
            }
            ThreadActivity::Unparked(thread_id) => {
                tracing::debug!("Thread executor with id: {:?} has been unparked", thread_id);
                self.idle_threads.fetch_sub(1, atomic::Ordering::SeqCst);
                self.remove_thread_id_from_packed_list(thread_id);
            }
            ThreadActivity::Blocked(thread_id) => {
                tracing::debug!("Thread executor with id: {:?} is blocked", thread_id);
                self.add_thread_id_from_blocked_list(thread_id.clone());

                if self.global_kill_signal.probe() {
                    tracing::debug!("Blocked: should kill set, send latch wakeup");
                    self.latch.signal_all();
                } else {
                    tracing::debug!("Should kill signal not yet set");
                }
            }
            ThreadActivity::Unblocked(thread_id) => {
                tracing::debug!("Thread executor with id: {:?} is unblocked", thread_id);
                self.remove_thread_id_from_blocked_list(thread_id);
            }
            ThreadActivity::Panicked(thread_id, ctx) => {
                tracing::debug!(
                    "Thread executor with id: {:?} panic'ed {:?}",
                    thread_id,
                    ctx
                );

                self.remove_thread_id_from_packed_list(thread_id.clone());
                self.remove_thread_id_from_blocked_list(thread_id.clone());

                // remove thread's registered handler and ThreadRef.
                if let Some(thread_ref) = self.thread_map.write().unwrap().remove(&thread_id) {
                    std::mem::drop(thread_ref);
                }

                if let Some(thread_handler) =
                    self.thread_handles.write().unwrap().remove(&thread_id)
                {
                    let thread_result = thread_handler.join();
                    tracing::debug!("Panic'ed thread returned result: {:?}", &thread_result);
                    std::mem::drop(thread_result);
                }

                let prev_thread_num = self.live_threads.fetch_sub(1, atomic::Ordering::AcqRel);

                // spawn new thread for
                if prev_thread_num - 1 < self.num_threads && !self.global_kill_signal.probe() {
                    self.create_thread_executor()
                        .expect("should create new executor");

                    tracing::debug!(
                        "Thread {:?} died but generated new thread to replace it due to {:?}",
                        thread_id,
                        ctx
                    );
                }
            }
        }

        Some(())
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
    pub(crate) fn new(tasks: SharedTaskQueue, latch: Arc<LockSignal>) -> Self {
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

    /// [`ready_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapping iterator that allows you synchronously receive those
    /// values from a [`RecvIterator`] which are only the [`TaskStatus::Ready`].
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

    /// [`stream_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the state streams instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a [`StreamRecvIterator`] that implements the [`Iterator`] trait.
    ///
    /// But unlike [`schedule_iter`] returns [`Stream`] values that hide way the underling 
    /// value types of [`TaskStatus`] which simplifies the trait types your usage 
    /// requires.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn stream_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
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

                Ok(StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// [`schedule_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a [`RecvIterator`] that implements the [`Iterator`] trait.
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

    /// [`schedule`] adds a task into the global execution queue which should
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
