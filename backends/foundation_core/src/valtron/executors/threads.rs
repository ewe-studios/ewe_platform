use std::{
    any::Any,
    collections::HashMap,
    env,
    marker::PhantomData,
    panic,
    sync::{
        self,
        atomic::{self, AtomicUsize},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{self, Instant},
};

use concurrent_queue::{ConcurrentQueue, PushError};
use derive_more::derive::From;
use flume;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::str::FromStr;

use crate::{
    retries::ExponentialBackoffDecider,
    synca::{Entry, EntryList, IdleMan, LockSignal, OnSignal, RunOnDrop, SleepyMan},
    valtron::{AnyResult, BoxedError, LocalThreadExecutor},
};

use super::{
    constants::*, BoxedExecutionEngine, BoxedPanicHandler, BoxedSendExecutionIterator, DoNext,
    ExecutionAction, ExecutionIterator, ExecutorError, FnMutReady, FnReady, OnNext, PriorityOrder,
    ProcessController, TaskIterator, TaskReadyResolver, TaskStatus, TaskStatusMapper,
};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Mutex, RwLock};

#[cfg(target_arch = "wasm32")]
use wasm_sync::{Mutex, RwLock};

/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Max value for the thread counters.
const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;

/// [get_allocatable_thread_count] ensures we allocate enough of threads as requested
/// less 1 as the 1 thread will be used for waiting for kill signal and the remaining
/// for task execution in situations where on 2 threads can be created apart from the current
/// process.
pub(crate) fn get_allocatable_thread_count() -> usize {
    let max_threads = get_max_threads();
    let desired_threads = get_num_threads();

    if desired_threads > max_threads {
        panic!(
            "Desired thread count cant be greater than maximum allowed threads {}",
            max_threads
        );
    }

    if desired_threads == 0 {
        panic!("Desired thread count cant be zero");
    }

    if max_threads < 2 {
        panic!("Available thread cant be less than 2 we use 1 thread for the service kill signal, and 1 for tasks");
    }

    if desired_threads < 2 {
        panic!("Requested thread cant be less than 2 we use 1 thread for the service kill signal, and 1 for tasks");
    }

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

    return desired_threads;
}

#[cfg(test)]
mod test_allocatable_threads {
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    fn get_allocatable_thread_count_as_far_as_1_remains() {
        let max_threads = get_max_threads();
        assert!(max_threads > 3);

        env::remove_var("VALTRON_NUM_THREADS");
        env::set_var("VALTRON_NUM_THREADS", format!("{}", max_threads - 2));

        assert_eq!(get_allocatable_thread_count(), max_threads - 2);

        env::set_var("VALTRON_NUM_THREADS", format!("{}", max_threads - 3));
        assert_eq!(get_allocatable_thread_count(), max_threads - 3);

        env::set_var("VALTRON_NUM_THREADS", format!("{}", max_threads - 1));
        assert_eq!(get_allocatable_thread_count(), max_threads - 1);
    }
}

/// [get_max_threads] returns the max threads allowed
/// in the current system.
pub(crate) fn get_max_threads() -> usize {
    match std::thread::available_parallelism()
        .ok()
        .and_then(|s| Some(s.get()))
    {
        Some(system_value) => {
            tracing::debug!("thread::available_parallelism() reported: {}", system_value);
            system_value
        }
        None => 1,
    }
}

/// [get_num_threads] will attempt to fetch the desired thread we want
/// from the either the environment variable `VALTRON_NUM_THREADS`
/// or gets the maximum allowed thread from the platform
/// via [std::thread::available_parallelism];
pub(crate) fn get_num_threads() -> usize {
    let thread_num = match env::var("VALTRON_NUM_THREADS")
        .ok()
        .and_then(|s| usize::from_str(&s).ok())
    {
        Some(x @ 1..) => {
            tracing::debug!("Retreived thread_number from VALTRON_NUM_THREADS");
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
        env::set_var("VALTRON_NUM_THREADS", "2");
        assert_eq!(get_num_threads(), 2);
        env::remove_var("VALTRON_NUM_THREADS");
    }
}

pub struct ThreadYielder {
    thread_id: ThreadId,
    latch: Arc<LockSignal>,
    sender: flume::Sender<ThreadActivity>,
}

impl ThreadYielder {
    pub fn new(
        thread_id: ThreadId,
        latch: Arc<LockSignal>,
        sender: flume::Sender<ThreadActivity>,
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
            tracing::debug!("Thread succesfully locked thread");
        } else {
            tracing::debug!("Thread is alerady locked");
        }

        self.sender
            .send(ThreadActivity::Blocked(self.thread_id.clone()))
            .expect("should sent event");
        self.latch.wait();
        self.sender
            .send(ThreadActivity::Unblocked(self.thread_id.clone()))
            .expect("should sent event");
    }

    fn yield_for(&self, dur: std::time::Duration) {
        // will request that the current thread be
        // parked for the giving duration though parking
        // may not last that duration so you need to be aware
        // the thread could be woken up, so we
        // specifically loop till we've reached beyond duration
        let started = Instant::now();

        let mut remaining_timeout = dur.clone();
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
            remaining_timeout = remaining_timeout - elapsed;
        }

        self.sender
            .send(ThreadActivity::Unparked(self.thread_id.clone()))
            .expect("should sent event");
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ThreadId(Entry);

impl ThreadId {
    pub fn new(entry: Entry) -> Self {
        Self(entry)
    }

    pub fn get_mut(&mut self) -> &mut Entry {
        &mut self.0
    }

    pub fn get_ref(&self) -> &Entry {
        &self.0
    }

    pub fn get_cloned(&self) -> Entry {
        self.0.clone()
    }
}

pub enum ThreadActivity {
    /// Indicates when a Thread with an executor has started
    Started(ThreadId),

    /// Indicates when a Thread with an executor has stopped
    Stopped(ThreadId),

    /// Indicates when a Thread with an executor has
    /// blocked the thread with a CondVar waiting for
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
    Paniced(ThreadId, Box<dyn Any + Send>),
    BroadcastedTask,
}

impl core::fmt::Debug for ThreadActivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadActivity::Paniced(thread_id, _) => {
                write!(f, "ThreadActivity::Paniced({:?})", thread_id)
            }
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Clone)]
pub struct SharedThreadRegistry(sync::Arc<RwLock<ThreadPoolRegistryInner>>);

impl SharedThreadRegistry {
    pub fn new(inner: sync::Arc<RwLock<ThreadPoolRegistryInner>>) -> Self {
        Self(inner)
    }

    pub fn executor_count(&self) -> usize {
        let registry = self.0.read().unwrap();
        registry.threads.active_slots()
    }

    pub fn get_thread(&self, thread: ThreadId) -> ThreadRef {
        let registry = self.0.read().unwrap();

        match registry.threads.get(thread.get_ref()) {
            Some(thread_ref) => thread_ref.clone(),
            None => unreachable!("We should never get to a point where a ThreadId is invalid"),
        }
    }

    pub fn register_thread(
        &self,
        thread: ThreadRef,
        latch: Arc<LockSignal>,
        sender: flume::Sender<ThreadActivity>,
    ) -> ThreadId {
        let mut registry = self.0.write().unwrap();

        // insert thread and get thread key.
        let entry = registry.threads.insert(thread);

        // update the key for the thread in place within the
        // entry and in the same lock.
        match registry.threads.get_mut(&entry) {
            Some(mutable_thread_ref) => {
                let thread_id = ThreadId(entry.clone());
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
        queue: SharedTaskQueue,
        registry_id: Option<ThreadId>,
        register: SharedThreadRegistry,
        global_kill_signal: sync::Arc<OnSignal>,
    ) -> ThreadRef {
        Self {
            seed,
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
    activity_sender: flume::Sender<ThreadActivity>,
    activity_receiver: flume::Receiver<ThreadActivity>,

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

    // thread mapings.
    thread_map: RwLock<HashMap<ThreadId, ThreadRef>>,
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,
}

// -- Default

// -- constructor

impl ThreadPool {
    /// with_rng allows you to provide a custom Random number generator
    /// that can be used to generate as the initial seed the
    /// ThreadPool uses for it's local execution threads.
    ///
    /// We use the default to max threads allowed per process via `get_num_threads()`.
    pub fn with_rng<R: rand::Rng>(rng: &mut R) -> Self {
        let num_threads = get_num_threads();
        Self::with_seed_and_threads(rng.next_u64(), num_threads)
    }

    /// [`ThreadPool::with_seed`] generates a `ThreadPool` using
    /// the provided seed and the default MAX threads allowed
    /// per Process using `get_num_threads()`.
    pub fn with_seed(seed_from_rng: u64) -> Self {
        let num_threads = get_num_threads();
        Self::with_seed_and_threads(seed_from_rng, num_threads)
    }

    /// [`ThreadPool::with_seed_and_threads`] generates a
    /// threadPool which uses the default values (see Constants section)
    /// set out in this modules for all required configuration
    /// which provide what we considered sensible defaults
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

    /// [`new`] creates a new ThreadPool for you which you can use
    /// for concurrent execution of `LocalExecutorEngine` executors
    /// within the total number of threads you provided via `num_threads`
    /// the threads are spawned and ready to take on work.
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
        if num_threads < 2 {
            panic!("Unable to create ThreadPool with 1 thread only, please specify >= 2");
        }

        if num_threads > THREADS_MAX {
            panic!(
                "Unable to create ThreadPool with thread numbers of {}, must no go past {}",
                num_threads, THREADS_MAX
            );
        }
        let thread_latch = Arc::new(LockSignal::new());
        let kill_latch = Arc::new(LockSignal::new());
        let tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::unbounded());

        let (sender, receiver) = flume::unbounded::<ThreadActivity>();
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
                .expect(&format!("should successfully create thread for {}", index));
        }

        thread_pool
    }
}

// -- ThreadExecutionError

pub type ThreadExecutionResult<T> = AnyResult<T, ThreadExecutionError>;

#[derive(From, Debug)]
pub enum ThreadExecutionError {
    #[from(ignore)]
    FailedStart(BoxedError),

    #[from(ignore)]
    Paniced(Box<dyn Any + Send>),
}

impl core::error::Error for ThreadExecutionError {}

impl core::fmt::Display for ThreadExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Paniced(_) => write!(f, "ThreadExecutionError(_)"),
            _ => write!(f, "{:?}", self),
        }
    }
}

// -- surface entry methods

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
        self.kill_latch.lock_and_wait();
    }

    /// pulls all relevant threads `JoinHandle`
    /// joining them all till they all have finished and exited.
    pub(crate) fn await_threads(&self) {
        let span = tracing::trace_span!("ThreadPool::await_threads");
        let _enter = span.enter();

        let thread_keys: Vec<ThreadId> = self
            .thread_handles
            .read()
            .unwrap()
            .iter()
            .map(|(key, _)| key.clone())
            .collect();

        let mut handles = self.thread_handles.write().unwrap();
        for thread_id in thread_keys {
            match handles.remove(&thread_id) {
                None => continue,
                Some(thread_handle) => {
                    thread_handle
                        .join()
                        .expect("should return result")
                        .expect("finished");
                    continue;
                }
            };
        }
    }

    /// `create_thread_executor` creates a new thread into the thread pool spawning
    /// a LocalThreadExecutor into a owned thread that is managed by the executor.
    pub(crate) fn create_thread_executor(&self) -> ThreadExecutionResult<ThreadRef> {
        let span = tracing::trace_span!("ThreadPool::create_thread_executor");
        let _enter = span.enter();

        let thread_name = format!("valtron_thread_{}", self.registry.executor_count() + 1);
        let mut b = thread::Builder::new().name(thread_name.clone());
        if let Some(thread_stack_size) = self.thread_stack_size.clone() {
            b = b.stack_size(thread_stack_size);
        }

        let thread_seed = self.rng.lock().unwrap().next_u64();
        let thread_id = self.registry.register_thread(
            ThreadRef {
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
        let seed_clone = thread_ref.seed.clone();
        let task_clone = thread_ref.tasks.clone();
        let process_clone = thread_ref.process.clone().unwrap();
        let thread_kill_signal = thread_ref.global_kill_signal.clone();

        let sender_id = thread_id.clone();
        let sender = self.activity_sender.clone();

        let thread_max_idle_count = self.thread_max_idle_count.clone();
        let thread_max_sleep_before_end = self.thread_max_sleep_before_end.clone();
        let thread_back_off_factor = self.thread_back_off_factor.clone();
        let thread_back_off_jitter = self.thread_back_off_jitter.clone();
        let thread_back_min_duration = self.thread_back_min_duration.clone();
        let thread_back_max_duration = self.thread_back_max_duration.clone();

        match b.spawn(move || {
            let span =
                tracing::trace_span!("ThreadPool::create_thread_executor.local_executor.thread");
            let _enter = span.enter();
            tracing::info!("Starting LocalExecutionEngine for {}", &thread_name);

            match panic::catch_unwind(|| {
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
            }) {
                Ok(_) => Ok(()),
                Err(err) => {
                    sender
                        .send(ThreadActivity::Paniced(sender_id, err))
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
        let _enter = span.enter();
        match self.tasks.push(Box::new(task)) {
            Ok(_) => {
                if self.tasks.len() == 1 {
                    self.latch.signal_one();
                } else if self.tasks.len() > 1 {
                    self.latch.signal_all();
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

// -- spawn methods

impl ThreadPool {
    /// `spawn2` provides a builder which specifically allows you to build out
    /// the underlying tasks to be scheduled into the global queue.
    ///
    /// It expects you to provide types for both Mapper and Resolver.
    pub fn spawn2<Task, Action, Mapper, Resolver>(
        &self,
    ) -> ThreadPoolTaskBuilder<Task::Done, Task::Pending, Action, Mapper, Resolver, Task>
    where
        Task::Done: Send + 'static,
        Task::Pending: Send + 'static,
        Task: TaskIterator<Spawner = Action> + Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Task::Done, Task::Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Task::Done, Task::Pending> + Send + 'static,
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
        ThreadPoolTaskBuilder::new(self.tasks.clone(), self.latch.clone())
    }
}

// -- core methods

impl ThreadPool {
    /// [`run_until`] will block the current thread
    /// and starts listening to both kill signal from the
    /// process via `Self::kill` and handle management operations
    /// from notifications sent by the execution threads when threads
    /// become idle, get killed, are parked, ..etc.
    ///
    /// CAUTION: You must be careful not to call `Self::run_until` and `Self::kill`
    /// in the same thread as a `CondVar` is used and will block the current thread
    /// till `run_until` signals the CondVar to wake up the blocked thread when `Self::kill`
    /// is ever called.
    pub fn run_until(&self) {
        let span = tracing::trace_span!("ThreadPool::run_until");
        let _enter = span.enter();
        self.block_until();
        self.await_threads();
        self.kill_latch.signal_all();
    }

    pub(crate) fn block_until(&self) {
        let span = tracing::trace_span!("ThreadPool::block_until");
        let _enter = span.enter();

        let _ = RunOnDrop::new(|| {
            tracing::debug!("ThreadPool::run has stopped running");
        });

        'mainloop: while !self.global_kill_signal.probe() {
            let thread_activity = match self.activity_receiver.recv_timeout(self.op_read_time) {
                Ok(activity) => activity,
                Err(err) => match err {
                    flume::RecvTimeoutError::Timeout => continue 'mainloop,
                    flume::RecvTimeoutError::Disconnected => break 'mainloop,
                },
            };

            match thread_activity {
                ThreadActivity::BroadcastedTask => {
                    tracing::debug!("A thread broadcasted a new message to the global queue",);

                    // wakeup any sleeping threads based on account.
                    if self.tasks.len() == 1 {
                        self.latch.signal_one();
                    } else if self.tasks.len() > 1 {
                        self.latch.signal_all();
                    }
                }
                ThreadActivity::Started(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} started", thread_id);
                    self.live_threads.fetch_add(1, atomic::Ordering::SeqCst);
                }
                ThreadActivity::Stopped(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} stopped", thread_id);
                    self.live_threads.fetch_sub(1, atomic::Ordering::SeqCst);
                }
                ThreadActivity::Parked(thread_id) => {
                    tracing::info!("Thread executor with id: {:?} has been parked", thread_id);
                    self.idle_threads.fetch_add(1, atomic::Ordering::SeqCst);
                    self.parked_threads.lock().unwrap().push(thread_id.clone());
                }
                ThreadActivity::Unparked(thread_id) => {
                    tracing::info!("Thread executor with id: {:?} has been unparked", thread_id);
                    self.idle_threads.fetch_sub(1, atomic::Ordering::SeqCst);
                    if let Some(index) = self
                        .parked_threads
                        .lock()
                        .unwrap()
                        .iter()
                        .position(|item| *item == thread_id)
                    {
                        self.parked_threads.lock().unwrap().remove(index);
                    }
                }
                ThreadActivity::Blocked(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} is blocked", thread_id);
                    self.blocked_threads.lock().unwrap().push(thread_id.clone());
                }
                ThreadActivity::Unblocked(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} is unblocked", thread_id);
                    if let Some(index) = self
                        .blocked_threads
                        .lock()
                        .unwrap()
                        .iter()
                        .position(|item| *item == thread_id)
                    {
                        self.blocked_threads.lock().unwrap().remove(index);
                    }
                }
                ThreadActivity::Paniced(thread_id, ctx) => {
                    tracing::debug!(
                        "Thread executor with id: {:?} panic'ed {:?}",
                        thread_id,
                        ctx
                    );

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

                    let prev_thread_num = self.live_threads.fetch_sub(1, atomic::Ordering::SeqCst);

                    // spawn new thread for
                    if prev_thread_num - 1 < self.num_threads {
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
        }
    }
}

pub struct ThreadPoolTaskBuilder<
    Done,
    Pending,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
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
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
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
            _marker: PhantomData::default(),
        }
    }

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

    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    pub fn with_panic_handler<T>(mut self, handler: T) -> Self
    where
        T: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(handler));
        self
    }

    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

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
            Ok(_) => {
                if self.tasks.len() == 1 {
                    self.latch.signal_one();
                } else if self.tasks.len() > 1 {
                    self.latch.signal_all();
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
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
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
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, FnMutReady<F, Action>, Task>
where
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next_mut(self, action: F) -> Self {
        self.with_resolver(FnMutReady::new(action))
    }
}
