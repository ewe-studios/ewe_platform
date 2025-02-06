use std::{
    any::Any,
    collections::HashMap,
    env,
    marker::PhantomData,
    panic,
    sync::{self, Arc},
    thread,
    time::{self, Instant},
};

use concurrent_queue::{ConcurrentQueue, PushError};
use flume;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::str::FromStr;

use crate::{
    retries::ExponentialBackoffDecider,
    synca::{
        ActivitySignal, DurationStore, Entry, EntryList, IdleMan, LockSignal, OnSignal, RunOnDrop,
        SleepyMan,
    },
    valtron::{AnyResult, BoxedError, LocalThreadExecutor},
};

use super::{
    BoxedSendExecutionIterator, DoNext, ExecutionAction, ExecutionIterator, ExecutorError, OnNext,
    PriorityOrder, ProcessController, TaskIterator, TaskReadyResolver, TaskStatusMapper,
};

#[allow(unused)]
#[cfg(not(feature = "web_spin_lock"))]
use std::sync::{Condvar, Mutex, RwLock};

#[cfg(feature = "web_spin_lock")]
use wasm_sync::{CondVar, Mutex, RwLock};

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
    latch: Arc<LockSignal>,
}

impl ThreadYielder {
    pub fn new(latch: Arc<LockSignal>) -> Self {
        Self { latch }
    }
}

impl Clone for ThreadYielder {
    fn clone(&self) -> Self {
        Self::new(self.latch.clone())
    }
}

impl ProcessController for ThreadYielder {
    fn yield_process(&self) {
        if self.latch.try_lock() {
            tracing::debug!("Thread succesfully locked thread");
        } else {
            tracing::debug!("Thread is alerady locked");
        }

        self.latch.wait();
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
            std::thread::park_timeout(remaining_timeout);

            // check the state and see if we've crossed that threshold.
            let elapsed = started.elapsed();
            if elapsed >= remaining_timeout {
                break;
            }
            remaining_timeout = remaining_timeout - elapsed;
        }
    }
}

#[derive(Clone, Debug)]
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
    Started(ThreadId),
    Stopped(ThreadId),
    Idle(ThreadId),
    Sleepy(ThreadId),
    Sleeping(ThreadId),
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

    pub fn register_thread(&self, thread: ThreadRef) -> ThreadId {
        let mut registry = self.0.write().unwrap();

        // insert thread and get thread key.
        let entry = registry.threads.insert(thread);

        // update the key for the thread in place within the
        // entry and in the same lock.
        match registry.threads.get_mut(&entry) {
            Some(mutable_thread_ref) => {
                mutable_thread_ref.registry_id = Some(ThreadId(entry.clone()));
                ThreadId::new(entry)
            }
            None => unreachable!("Thread must be registered at the entry key"),
        }
    }
}

pub type SharedTaskQueue = sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>;
pub type SharedActivityQueue = sync::Arc<ConcurrentQueue<ThreadActivity>>;
pub type SharedThreadYielder = sync::Arc<ThreadYielder>;

#[derive(Clone)]
pub(crate) struct ThreadRef {
    /// seed for the giving thread ref
    pub seed: u64,

    /// the queue for tasks
    pub tasks: SharedTaskQueue,

    /// the relevant key used by the thread in
    /// the core thread registry.
    pub registry_id: Option<ThreadId>,

    /// the register this thread is related to.
    pub register: SharedThreadRegistry,

    /// activity helps indicate the current
    /// state of the giving thread.
    pub activity: sync::Arc<ActivitySignal>,

    /// signal used for communicating death to the thread.
    pub kill_signal: sync::Arc<OnSignal>,

    /// global signal used for communicating death
    /// to all threads.
    pub global_kill_signal: sync::Arc<OnSignal>,

    /// process manager for controlling process yielding.
    pub process: ThreadYielder,
}

impl ThreadRef {
    pub fn new(
        seed: u64,
        queue: SharedTaskQueue,
        process: ThreadYielder,
        registry_id: Option<ThreadId>,
        register: SharedThreadRegistry,
        global_kill_signal: sync::Arc<OnSignal>,
    ) -> ThreadRef {
        Self {
            seed,
            tasks: queue,
            process,
            register,
            registry_id,
            global_kill_signal,
            kill_signal: sync::Arc::new(OnSignal::new()),
            activity: sync::Arc::new(ActivitySignal::new()),
        }
    }
}

pub(crate) struct ThreadPoolRegistryInner {
    threads: EntryList<ThreadRef>,
    sleepers: DurationStore<Entry>,
    latch: Arc<LockSignal>,
    process: SharedThreadYielder,
    notifications: flume::Sender<ThreadActivity>,
}

impl ThreadPoolRegistryInner {
    pub(crate) fn new(
        notifications: flume::Sender<ThreadActivity>,
        latch: Arc<LockSignal>,
    ) -> SharedThreadRegistry {
        SharedThreadRegistry::new(sync::Arc::new(RwLock::new(ThreadPoolRegistryInner {
            notifications,
            latch: latch.clone(),
            threads: EntryList::new(),
            sleepers: DurationStore::new(),
            process: Arc::new(ThreadYielder::new(latch)),
        })))
    }
}

// --- Constants

const MAX_ROUNDS_IDLE_COUNT: u32 = 64;
const MAX_ROUNDS_WHEN_SLEEPING_ENDS: u32 = 32;

const BACK_OFF_JITER: f32 = 0.65;
const BACK_OFF_THREAD_FACTOR: u32 = 6;
const BACK_OFF_MIN_DURATION: time::Duration = time::Duration::from_millis(120); // 120ms
const BACK_OFF_MAX_DURATION: time::Duration = time::Duration::from_secs(300); // 5mins

// --- ThreadPool

pub struct ThreadPool {
    rng: ChaCha8Rng,
    seed_for_rng: u64,
    num_threads: usize,
    latch: Arc<LockSignal>,
    tasks: SharedTaskQueue,
    op_read_time: time::Duration,
    registry: SharedThreadRegistry,
    thread_stack_size: Option<usize>,
    thread_map: HashMap<ThreadId, ThreadRef>,
    global_kill_signal: sync::Arc<OnSignal>,
    notifications: flume::Receiver<ThreadActivity>,
}

// -- constructor

impl ThreadPool {
    pub fn new(
        seed_for_rng: u64,
        num_threads: usize,
        op_read_time: time::Duration,
        thread_stack_size: Option<usize>,
    ) -> Self {
        let thread_latch = Arc::new(LockSignal::new());
        let tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::unbounded());

        let (sender, receiver) = flume::unbounded::<ThreadActivity>();
        Self {
            tasks,
            num_threads,
            seed_for_rng,
            op_read_time,
            thread_stack_size,
            notifications: receiver,
            thread_map: HashMap::new(),
            latch: thread_latch.clone(),
            rng: ChaCha8Rng::seed_from_u64(seed_for_rng),
            global_kill_signal: sync::Arc::new(OnSignal::new()),
            registry: ThreadPoolRegistryInner::new(sender, thread_latch.clone()),
        }
    }
}

// -- surface entry methods

impl ThreadPool {
    /// `create_thread_executor` creates a new thread into the thread pool spawning
    /// a LocalThreadExecutor into a owned thread that is managed by the executor.
    pub(crate) fn create_thread_executor(
        &mut self,
        priority: PriorityOrder,
    ) -> AnyResult<(), BoxedError> {
        let thread_name = format!("valtron_thread_{}", self.registry.executor_count() + 1);
        let mut b = thread::Builder::new().name(thread_name.clone());
        if let Some(thread_stack_size) = self.thread_stack_size.clone() {
            b = b.stack_size(thread_stack_size);
        }

        let thread_entry = self.registry.register_thread(ThreadRef {
            registry_id: None,
            tasks: self.tasks.clone(),
            seed: self.rng.next_u64(),
            register: self.registry.clone(),
            kill_signal: sync::Arc::new(OnSignal::new()),
            activity: sync::Arc::new(ActivitySignal::new()),
            process: ThreadYielder::new(self.latch.clone()),
            global_kill_signal: self.global_kill_signal.clone(),
        });

        let thread_ref = self.registry.get_thread(thread_entry.clone());

        match b.spawn(move || {
            tracing::info!("Starting LocalExecutionEngine for {}", &thread_name);

            panic::catch_unwind(|| {
                // create LocalExecutionEngine here and
                // let it handle everything going forward.
                let thread_executor = LocalThreadExecutor::from_seed(
                    thread_ref.seed.clone(),
                    thread_ref.tasks.clone(),
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
                    thread_ref.process.clone(),
                );

                thread_executor.block_on();
            })
            .ok();
            todo!()
        }) {
            Ok(handler) => {
                todo!()
            }
            Err(err) => Err(Box::new(err)),
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
        ThreadPoolTaskBuilder::new(self.tasks.clone())
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
    task: Option<Task>,
    resolver: Option<Resolver>,
    mappers: Option<Vec<Mapper>>,
    _marker: PhantomData<(Done, Pending, Action)>,
}

// impl<
//         F,
//         Done: Send + 'static,
//         Pending: Send + 'static,
//         Action: ExecutionAction<Executor = LocalExecutionEngine> + Send + 'static,
//         Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
//         Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
//     >
//     ThreadPoolTaskBuilder<
//         Done,
//         Pending,
//         Action,
//         Mapper,
//         FnReady<F, LocalExecution Action>,
//         Task,
//     >
// where
//     F: Fn(TaskStatus<Done, Pending, Action>, LocalExecutionEngine) + Send + 'static,
// {
//     pub fn on_next(self, action: F) -> Self {
//         self.with_resolver(FnReady::new(action))
//     }
// }

// impl<
//         F,
//         Done: Send + 'static,
//         Pending: Send + 'static,
//         Action: ExecutionAction<Executor = LocalExecutionEngine> + Send + 'static,
//         Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
//         Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
//     >
//     ThreadPoolTaskBuilder<
//         Done,
//         Pending,
//         Action,
//         Mapper,
//         FnMutReady<F, LocalExecution Action>,
//         Task,
//     >
// where
//     F: FnMut(TaskStatus<Done, Pending, Action>, LocalExecutionEngine) + Send + 'static,
// {
//     pub fn on_next_mut(self, action: F) -> Self {
//         self.with_resolver(FnMutReady::new(action))
//     }
// }

impl<
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, Resolver, Task>
{
    pub fn new(tasks: SharedTaskQueue) -> Self {
        Self {
            tasks,
            task: None,
            mappers: None,
            resolver: None,
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

    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    pub fn schedule(self) -> AnyResult<(), ExecutorError> {
        match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let task_iter = OnNext::new(task, resolver, mappers);
                    match self.tasks.push(task_iter.into()) {
                        Ok(_) => Ok(()),
                        Err(err) => match err {
                            PushError::Full(_) => Err(ExecutorError::QueueFull),
                            PushError::Closed(_) => Err(ExecutorError::QueueClosed),
                        },
                    }
                }
                (Some(resolver), None) => {
                    let task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    match self.tasks.push(task_iter.into()) {
                        Ok(_) => Ok(()),
                        Err(err) => match err {
                            PushError::Full(_) => Err(ExecutorError::QueueFull),
                            PushError::Closed(_) => Err(ExecutorError::QueueClosed),
                        },
                    }
                }
                (None, None) => {
                    let task_iter = DoNext::new(task);
                    match self.tasks.push(task_iter.into()) {
                        Ok(_) => Ok(()),
                        Err(err) => match err {
                            PushError::Full(_) => Err(ExecutorError::QueueFull),
                            PushError::Closed(_) => Err(ExecutorError::QueueClosed),
                        },
                    }
                }
                (None, Some(_)) => Err(ExecutorError::FailedToCreate),
            },
            None => Err(ExecutorError::TaskRequired),
        }
    }
}

// -- core methods

impl ThreadPool {
    pub fn run_until(&self) {
        let _ = RunOnDrop::new(|| {
            tracing::debug!("ThreadPool::run has stopped running");
        });

        'mainloop: while !self.global_kill_signal.probe() {
            let thread_activity = match self.notifications.recv_timeout(self.op_read_time) {
                Ok(activity) => activity,
                Err(err) => match err {
                    flume::RecvTimeoutError::Timeout => continue 'mainloop,
                    flume::RecvTimeoutError::Disconnected => break 'mainloop,
                },
            };

            match thread_activity {
                ThreadActivity::Started(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} started", thread_id);
                }
                ThreadActivity::Stopped(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} stopped", thread_id);
                }
                ThreadActivity::Idle(thread_id) => {
                    tracing::debug!(
                        "Thread executor with id: {:?} became idle with no task",
                        thread_id
                    );
                }
                ThreadActivity::Sleepy(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} is feeling sleepy", thread_id);
                }
                ThreadActivity::Sleeping(thread_id) => {
                    tracing::debug!("Thread executor with id: {:?} started sleeping", thread_id);
                }
                ThreadActivity::Paniced(thread_id, ctx) => {
                    tracing::debug!(
                        "Thread executor with id: {:?} panic'ed {:?}",
                        thread_id,
                        ctx
                    );
                }
            }
        }
    }
}
