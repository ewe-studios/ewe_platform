use std::{cell, collections::VecDeque, rc, sync, thread, time};

use crate::{
    synca::{Entry, EntryList, Sleepers, SleepyMan, Waker},
    valtron::AnyResult,
};
use derive_more::derive::From;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::State;
use concurrent_queue::ConcurrentQueue;

/// Executor is the backbone of the valtron execution model
/// they can be spawned within threads or be the singular owner
/// of a thread which the user/caller create to manage execution within the
/// thread.
pub trait Executor {
    /// `block` starts the Executor, blocking the thread where it
    /// it started until it receives the binary gets killed
    /// a kill signal.
    ///
    /// block never sleeps, it may cause a yielding of the thread via
    /// `thread::yield` to allow the processor handle other tasks when
    /// it has no pending work but it only ever really sleeps with
    /// an ever expanding exponential duration with a max cap
    /// unless some underlying work flows in.
    ///
    /// And when we say sleep
    fn block_on(&self);
}

type LocalExecutorState<T> = rc::Rc<cell::RefCell<ExecutorState<T>>>;

pub(crate) struct SingleWaker<T: Iterator<Item = State>> {
    task: Entry,
    executor: LocalExecutorState<T>,
}

impl<T: Iterator<Item = State>> SingleWaker<T> {
    pub fn new(task: Entry, executor: LocalExecutorState<T>) -> Self {
        Self { task, executor }
    }
}

impl<T: Iterator<Item = State>> Waker for SingleWaker<T> {
    fn wake(&self) {
        let mut executor = self.executor.borrow_mut();
        executor.wake_up(self.task.clone());
    }
}

pub type BoxedStateIterator = Box<dyn Iterator<Item = State>>;

/// ProgressIndicator communicates the potential status of
/// an ExecutorState and whether it can make progress in it's
/// work, this allows the manager of the executor to smartly
/// manage the overall management of the executor.
pub enum ProgressIndicator {
    /// Indicates no work is available for progression.
    NoWork,

    /// Indicates work is available and can be progressed
    CanProgress,

    /// Indicates it needs to wait for some period of time
    /// before progress can be made.
    SpinWait(time::Duration),
}

/// Underlying stats shared by all executors.
pub struct ExecutorState<T: Iterator<Item = State>> {
    /// global_tasks are the shared tasks coming from the main thread
    /// they generally will always come in fifo order and will be processed
    /// in the order received.
    pub(crate) global_tasks: sync::Arc<ConcurrentQueue<T>>,

    /// tasks owned by the executor which should be processed first
    /// before taking on the next task from the global queue,
    /// task generally come in here from the current task taking
    /// from the global queue.
    /// This allows us keep the overall execution of any async
    /// iterator within the same thread and keep a one thread
    /// execution.
    pub(crate) local_tasks: EntryList<T>,

    /// the queue used for performing perioritization, usually
    /// the global task taking will first be stored in the local
    /// tasks list and have it's related Entry pointer added
    /// into the queue for prioritization, and we will never
    /// take any new tasks from the global queue until all
    /// the entries in this queue is empty, this ensures
    /// we can consistently complete every received tasks
    /// till it finishes.
    pub(crate) processing: VecDeque<Entry>,

    /// this provives consistent and repeatable random number generation
    /// this is exposed by the executor to it's callers via scopes
    /// or direct use that lets you borrow a random number that
    /// always produces the same sets of values when given the
    /// same seed for repeatability.
    pub(crate) rng: rc::Rc<cell::RefCell<ChaCha8Rng>>,

    /// sleepers contain a means to register tasks currently being executed
    /// as sleeping, forcing the executing to either in a single threaded environment
    /// like WebAssembly spin endlessly until one of them is ready for
    /// progress once the sleeper indicate the task is ready for waking up.
    ///
    /// Each executor may treat these differently in that they may process
    /// one task at a time until completion or concurrently process
    /// multiple tasks at a time with alotted time slot between them.
    pub(crate) sleepers: Sleepers<SingleWaker<T>>,

    /// sleepy provides a managed indicator of how many times we've been idle
    /// and recommends how much sleep should the executor take next.
    pub(crate) sleepy: SleepyMan,

    /// wakers contains the list of enteries that should be woking up immediately
    /// by the executor, we store it in this list to make it self
    /// evidence what is now considered ready.
    pub(crate) wakers: Vec<Entry>,
}

// --- constructors

static DEQUEU_CAPACITY: usize = 10;

impl<T: Iterator<Item = State>> ExecutorState<T> {
    pub fn new(
        global_tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: ChaCha8Rng,
        sleepy: SleepyMan,
    ) -> Self {
        Self {
            sleepy,
            global_tasks,
            wakers: Vec::new(),
            sleepers: Sleepers::new(),
            local_tasks: EntryList::new(),
            rng: rc::Rc::new(cell::RefCell::new(rng)),
            processing: VecDeque::with_capacity(DEQUEU_CAPACITY),
        }
    }
}

// --- implementations

#[derive(Clone, Debug, From)]
pub enum ExecutorError {
    FailedToLift,
    FailedToSchedule,
}

impl std::error::Error for ExecutorError {}

impl core::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// ScheduleOutcome communicates the outcome of an attempt to
/// schedule a task from the global queue.
pub enum ScheduleOutcome {
    /// Indicates we successfully acquired a global task
    /// from the global queue.
    GlobalTaskAcquired,

    /// Indicate no global task was acquired and global task
    /// is empty.
    NoGlobalTaskAcquired,

    /// LocalTaskRunning indicates local task are still
    /// running or pending hence no attempt will be made
    /// to go to the global queue yet.
    LocalTaskRunning,
}

impl<T: Iterator<Item = State>> ExecutorState<T> {
    /// lift defers from the `ExecutorState::schedule` method
    /// where instead when adding the task into the local queue
    /// but also lifts the task as the highest priorty task
    /// to be processed.
    ///
    /// This means the currently processed task that might have
    /// called `ExecutorState::lift` since the executor will only
    /// ever execute a singular task each time (even if it concurrently)
    /// processes them (based on their operating semantics).
    ///
    /// But even if its from outside a task, understand the new task
    /// will take priorty till it's done.
    pub fn lift(&mut self, task: T) -> AnyResult<Entry, ExecutorError> {
        let task_entry = self.local_tasks.insert(task);
        self.processing.push_front(task_entry.clone());
        Ok(task_entry)
    }

    /// schedule appends the provided task into the
    /// end of the local task queue which then gets
    /// processed accordingly when its turns comes up
    /// once all previous tasks and it's sub-tasks have
    /// been resolved.
    pub fn schedule(&mut self, task: T) -> AnyResult<Entry, ExecutorError> {
        let task_entry = self.local_tasks.insert(task);
        self.processing.push_back(task_entry.clone());
        Ok(task_entry)
    }

    pub fn can_progress(&mut self) -> ScheduleOutcome {
        if self.local_tasks.active_slots() > 0 || self.processing.len() > 0 {
            return ScheduleOutcome::LocalTaskRunning;
        }

        match self.global_tasks.pop() {
            Ok(task) => {
                let task_entry = self.local_tasks.insert(task);
                self.processing.push_front(task_entry.clone());
                ScheduleOutcome::GlobalTaskAcquired
            }
            Err(_) => ScheduleOutcome::NoGlobalTaskAcquired,
        }
    }

    /// schedule_next will attempt to pull a new task from the
    /// global queue if no task is pending on the local queue
    /// and if so returns true to indicate success else a false
    /// to indicate no task was taking from the global queue
    /// as the local queue had a task or no task was found.
    pub fn schedule_next(&mut self) -> ScheduleOutcome {
        if self.local_tasks.active_slots() > 0 || self.processing.len() > 0 {
            return ScheduleOutcome::LocalTaskRunning;
        }

        match self.global_tasks.pop() {
            Ok(task) => {
                let task_entry = self.local_tasks.insert(task);
                self.processing.push_front(task_entry.clone());
                ScheduleOutcome::GlobalTaskAcquired
            }
            Err(_) => ScheduleOutcome::NoGlobalTaskAcquired,
        }
    }

    /// wake_up adds the entry into the list of wakers
    /// that should be woken up by the executor.
    pub fn wake_up(&mut self, entry: Entry) {
        self.wakers.push(entry);
    }
}

/// `SameThreadExecutor` is an implementation of an executor
/// which is only ever exists in the thread it was created in.
///
/// It is most suitable to be used in instances where multi-threading
/// is not possible such as WebAssembly.
///
/// `SameThreadExecutor` are not Send (!Send) so you cant
/// send them over to another thread but can potentially be
/// shared through an Arc.
pub struct SameThreadExecutor<T: Iterator<Item = State>> {
    state: LocalExecutorState<T>,
}

// --- constructors

impl<T: Iterator<Item = State>> SameThreadExecutor<T> {
    pub fn new(tasks: sync::Arc<ConcurrentQueue<T>>, rng: ChaCha8Rng, sleepy: SleepyMan) -> Self {
        Self {
            state: rc::Rc::new(cell::RefCell::new(ExecutorState::new(tasks, rng, sleepy))),
        }
    }

    pub fn from_rng<R: rand::Rng>(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: &mut R,
        sleepy: SleepyMan,
    ) -> Self {
        Self::new(tasks, ChaCha8Rng::seed_from_u64(rng.next_u64()), sleepy)
    }
}

// --- implementations

impl<T: Iterator<Item = State>> SameThreadExecutor<T> {
    /// `block` starts the thread execution, blocking the thread where it
    /// it spawned until it finishes executing and receives the
    /// a kill signal.
    ///
    /// block never sleeps, it may cause a yielding of the thread via
    /// `thread::yield` to allow the processor handle other tasks when
    /// it has no pending work but it only ever really sleeps with
    /// an ever expanding exponential duration with a max cap
    /// unless some underlying work flows in.
    ///
    /// And when we say sleep
    pub fn do_work(&self) {}

    pub fn yield_thread(&self) {
        thread::yield_now();
    }

    /// register a task to sleep for a giving duration.
    pub fn sleep_task_for(&self, task: Entry, how_long: time::Duration) {
        let waker = SingleWaker::new(task, self.state.clone());
        let mut state = self.state.borrow_mut();
        state.sleepers.insert(crate::synca::Wakeable::new(
            waker,
            time::Instant::now(),
            how_long,
        ));
    }
}

impl<T: Iterator<Item = State>> SameThreadExecutor<T> {
    fn block_on(&self) {
        loop {
            for _ in 0..200 {
                self.do_work();
            }

            self.yield_thread();
        }
    }
}
