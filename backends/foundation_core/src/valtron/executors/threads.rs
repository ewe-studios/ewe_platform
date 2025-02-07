use std::{
    any::Any,
    collections::HashMap,
    env,
    marker::PhantomData,
    ops::Index,
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
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    DoNext, ExecutionAction, ExecutionIterator, ExecutorError, FnMutReady, FnReady, OnNext,
    PriorityOrder, ProcessController, TaskIterator, TaskReadyResolver, TaskStatus,
    TaskStatusMapper,
};

#[cfg(not(feature = "web_spin_lock"))]
use std::sync::{Mutex, RwLock};

#[cfg(feature = "web_spin_lock")]
use wasm_sync::{Mutex, RwLock};

/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Max value for the thread counters.
const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;

pub(crate) fn get_num_threads() -> usize {
    match env::var("VALTRON_NUM_THREADS")
        .ok()
        .and_then(|s| usize::from_str(&s).ok())
    {
        Some(x @ 1..) => return x,
        _ => {
            match thread::available_parallelism()
                .ok()
                .and_then(|s| Some(s.get()))
            {
                Some(system_value) => system_value,
                None => 1,
            }
        }
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

    /// signal used for communicating death to the thread.
    pub kill_signal: sync::Arc<OnSignal>,

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
            kill_signal: sync::Arc::new(OnSignal::new()),
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

// --- Constants

const MAX_ROUNDS_IDLE_COUNT: u32 = 64;
const MAX_ROUNDS_WHEN_SLEEPING_ENDS: u32 = 32;

const BACK_OFF_JITER: f32 = 0.65;
const BACK_OFF_THREAD_FACTOR: u32 = 6;
const BACK_OFF_MIN_DURATION: time::Duration = time::Duration::from_millis(1); // 2ms
const BACK_OFF_MAX_DURATION: time::Duration = time::Duration::from_millis(300); // 300ms

// --- ThreadPool

pub struct ThreadPool {
    rng: Mutex<ChaCha8Rng>,
    num_threads: usize,
    latch: Arc<LockSignal>,
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

    // atomics
    live_threads: Arc<AtomicUsize>,
    idle_threads: Arc<AtomicUsize>,

    // thread mapings.
    thread_map: RwLock<HashMap<ThreadId, ThreadRef>>,
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,
}

// -- constructor

impl ThreadPool {
    pub fn new(
        seed_for_rng: u64,
        num_threads: usize,
        priority: PriorityOrder,
        op_read_time: time::Duration,
        thread_stack_size: Option<usize>,
    ) -> Self {
        let thread_latch = Arc::new(LockSignal::new());
        let tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::unbounded());

        let (sender, receiver) = flume::unbounded::<ThreadActivity>();
        Self {
            tasks,
            priority,
            num_threads,
            op_read_time,
            thread_stack_size,
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
        }
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
    /// will kill the thread pool and await all threads
    /// handle to compeletely finish.
    pub fn kill(&mut self) {
        self.global_kill_signal.turn_on();
        self.await_threads();
    }

    /// pulls all relevant threads `JoinHandle`
    /// joining them all till they all have finished and exited.
    pub(crate) fn await_threads(&mut self) {
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
                kill_signal: sync::Arc::new(OnSignal::new()),
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

        let sender_id = thread_id.clone();
        let sender = self.activity_sender.clone();
        match b.spawn(move || {
            tracing::info!("Starting LocalExecutionEngine for {}", &thread_name);

            match panic::catch_unwind(|| {
                // create LocalExecutionEngine here and
                // let it handle everything going forward.
                let thread_executor = LocalThreadExecutor::from_seed(
                    seed_clone,
                    task_clone,
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
                    priority,
                    process_clone,
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
        match self.tasks.push(Box::new(task)) {
            Ok(_) => Ok(()),
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `task` provides a builder which specifically allows you to build out
    /// the underlying tasks to be scheduled into the global queue.
    pub fn task<
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
    >(
        &self,
    ) -> ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, Resolver, Task> {
        ThreadPoolTaskBuilder::new(self.tasks.clone(), self.latch.clone())
    }
}

// -- core methods

impl ThreadPool {
    pub fn run_until(&self) {
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
