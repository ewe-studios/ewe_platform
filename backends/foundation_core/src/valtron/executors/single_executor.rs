use std::{
    borrow::Borrow,
    cell::{self, RefCell},
    collections::{HashMap, VecDeque},
    rc, sync, thread, time,
};

use crate::{
    synca::{Entry, EntryList, IdleMan, Sleepers, Wakeable, Waker},
    valtron::AnyResult,
};
use derive_more::derive::From;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::State;
use concurrent_queue::{ConcurrentQueue, PushError};

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

pub(crate) struct SingleWaker<T: Iterator<Item = State>> {
    task: Entry,
    executor: ReferencedExecutorState<T>,
}

impl<T: Iterator<Item = State>> SingleWaker<T> {
    pub fn new(task: Entry, executor: ReferencedExecutorState<T>) -> Self {
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

/// PriorityOrder defines how wake up tasks should placed once woken up.
pub enum PriorityOrder {
    Top,
    Bottom,
}

/// Underlying stats shared by all executors.
pub struct ExecutorState<T: Iterator<Item = State>> {
    /// what priority should waking task be placed.
    pub(crate) priority: PriorityOrder,

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

    /// task_graph provides dependency mapping of a Task (Key) lifted
    /// by another Task (Value) allowing us to go from lifted task
    /// to it's tree of dependents.
    pub(crate) task_graph: HashMap<Entry, Entry>,

    /// map used to identify that a task was packed.
    pub(crate) packed_tasks: HashMap<Entry, bool>,

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
    pub(crate) idler: IdleMan,
}

// --- constructors

static DEQUEUE_CAPACITY: usize = 10;

impl<T: Iterator<Item = State>> ExecutorState<T> {
    pub fn new(
        global_tasks: sync::Arc<ConcurrentQueue<T>>,
        priority: PriorityOrder,
        rng: ChaCha8Rng,
        idler: IdleMan,
    ) -> Self {
        Self {
            idler,
            priority,
            global_tasks,
            sleepers: Sleepers::new(),
            task_graph: HashMap::new(),
            packed_tasks: HashMap::new(),
            local_tasks: EntryList::new(),
            rng: rc::Rc::new(cell::RefCell::new(rng)),
            processing: VecDeque::with_capacity(DEQUEUE_CAPACITY),
        }
    }
}

// --- implementations

#[derive(Clone, Debug, From)]
pub enum ExecutorError {
    /// Executor failed to lift a new task as priority.
    FailedToLift,

    /// Executor cant schedule new task as provided.
    FailedToSchedule,

    /// Indicates a task has already lifted another
    /// task as priority and cant lift another since by
    /// definition it should not at all be able to do so
    /// since its not being executed at such periods of time.
    AlreadyLiftedTask,

    /// Is issued when a task not currently being processed by the executor
    /// attempts to lift a task as priority, only currently executing tasks
    /// can do that.
    ParentMustBeExecutingToLift,

    /// Issued when queue is closed and we are unable to deliver tasks anymore.
    QueueClosed,

    /// Issued when the queue is a bounded queue and execution of new tasks is
    /// impossible.
    QueueFull,
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

    /// Indicate no global task was acquired and no task
    /// is being processed.
    NoTaskRunningOrAcquired,

    /// LocalTaskRunning indicates local task are still
    /// running or pending hence no attempt will be made
    /// to go to the global queue yet.
    LocalTaskRunning,
}

impl<T: Iterator<Item = State> + Send> ExecutorState<T> {
    /// Delivers a task (Iterator) type to the global execution queue
    /// and in such a case you do not have an handle to the task as we
    /// no more have control as to where it gets allocated.
    pub fn distribute(&mut self, task: T) -> AnyResult<(), ExecutorError> {
        match self.global_tasks.push(task) {
            Ok(_) => Ok(()),
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }
}

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

impl<T: Iterator<Item = State>> ExecutorState<T> {
    /// Returns the list of other task dependent on this giving tasks
    /// and their dependents in order.
    #[inline]
    pub fn get_task_dependents(&self, target: Entry) -> Vec<Entry> {
        let mut deps = Vec::new();

        let mut previous = Some(target);

        while previous.is_some() {
            let key = previous.clone().unwrap();
            match self.task_graph.get(&key) {
                Some(dep) => {
                    deps.push(dep.clone());
                    previous = Some(dep.clone());
                    continue;
                }
                None => break,
            }
        }

        deps
    }

    /// Returns true/false indicative if the provided task entry
    /// is considerd packed.
    #[inline]
    pub fn is_packed(&mut self, target: &Entry) -> bool {
        match self.packed_tasks.get(target) {
            Some(value) => value.clone(),
            None => false,
        }
    }

    /// De-registers this task and it's dependents from the packed hashmap.
    #[inline]
    pub fn unpack_task_and_dependents(&mut self, target: Entry) {
        self.packed_tasks.remove(&target);
        for dependent in self.get_task_dependents(target).into_iter() {
            self.packed_tasks.remove(&dependent);
        }
    }

    /// Register this task and its dependents in the packed hashmap.
    #[inline]
    pub fn pack_task_and_dependents(&mut self, target: Entry) {
        self.packed_tasks.insert(target.clone(), true);
        for dependent in self.get_task_dependents(target).into_iter() {
            self.packed_tasks.insert(dependent, true);
        }
    }

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
    #[inline]
    pub fn lift(&mut self, task: T, parent: Option<Entry>) -> AnyResult<Entry, ExecutorError> {
        // if there is a parent then you need to be
        // the top of the executing set.
        if let Some(parent_handle) = &parent {
            if let Some(current_handle) = self.processing.get(0) {
                if !current_handle.eq(parent_handle) {
                    return Err(ExecutorError::ParentMustBeExecutingToLift);
                }
            }
        }

        let task_entry = self.local_tasks.insert(task);
        self.processing.push_front(task_entry.clone());

        // create dependent graph map.
        if let Some(parent_handle) = &parent {
            self.task_graph
                .insert(task_entry.clone(), parent_handle.clone());
        }

        Ok(task_entry)
    }

    /// schedule appends the provided task into the
    /// end of the local task queue which then gets
    /// processed accordingly when its turns comes up
    /// once all previous tasks and it's sub-tasks have
    /// been resolved.
    ///
    /// Scheduled tasks do not create relationships at all
    /// any task can schedule new task to the executor
    /// and it never affects its execution or priority.
    #[inline]
    pub fn schedule(&mut self, task: T) -> AnyResult<Entry, ExecutorError> {
        let task_entry = self.local_tasks.insert(task);
        self.processing.push_back(task_entry.clone());
        Ok(task_entry)
    }
}

impl<T: Iterator<Item = State>> ExecutorState<T> {
    /// wake_up adds the entry into the list of wakers
    /// that should be woken up by the executor.
    #[inline]
    pub fn wake_up(&mut self, target: Entry) {
        // get all the list of dependents and add back into queue.
        let deps = self.get_task_dependents(target.clone());

        // remove packed registry
        self.packed_tasks.remove(&target);

        match self.priority {
            PriorityOrder::Top => {
                for dependent in deps.into_iter().rev() {
                    self.packed_tasks.remove(&dependent);
                    self.processing.push_front(dependent.clone());
                }
                self.processing.push_front(target.clone());
            }
            PriorityOrder::Bottom => {
                self.processing.push_back(target.clone());
                for dependent in deps.into_iter() {
                    self.packed_tasks.remove(&dependent);
                    self.processing.push_back(dependent.clone());
                }
            }
        }
    }

    /// next_wakeup checks all registered sleepers to see if they
    /// matured to the age of being woken up and placed back into
    /// the processing queue.
    #[inline]
    pub fn wakeup_ready_sleepers(&mut self) {
        self.sleepers.notify_ready();
    }

    /// Returns True/False indicative if the executor has any local
    /// task still processing
    #[inline]
    pub fn has_local_tasks(&mut self) -> bool {
        self.local_tasks.active_slots() > 0
    }

    /// schedule_next will attempt to pull a new task from the
    /// global queue if no task is pending on the local queue
    /// and if so returns true to indicate success else a false
    /// to indicate no task was taking from the global queue
    /// as the local queue had a task or no task was found.
    #[inline]
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
            Err(_) => ScheduleOutcome::NoTaskRunningOrAcquired,
        }
    }
}

pub struct ReferencedExecutorState<T: Iterator<Item = State>>(
    rc::Rc<cell::RefCell<ExecutorState<T>>>,
);

impl<T: Iterator<Item = State>> From<rc::Rc<cell::RefCell<ExecutorState<T>>>>
    for ReferencedExecutorState<T>
{
    fn from(value: rc::Rc<cell::RefCell<ExecutorState<T>>>) -> Self {
        ReferencedExecutorState(value)
    }
}

impl<T: Iterator<Item = State>> Clone for ReferencedExecutorState<T> {
    fn clone(&self) -> Self {
        ReferencedExecutorState(self.0.clone())
    }
}

impl<T: Iterator<Item = State>> ReferencedExecutorState<T> {
    fn borrow(&self) -> &rc::Rc<cell::RefCell<ExecutorState<T>>> {
        self.0.borrow()
    }

    fn borrow_mut(&self) -> cell::RefMut<'_, ExecutorState<T>> {
        self.0.borrow_mut()
    }
}

impl<T: Iterator<Item = State>> ReferencedExecutorState<T> {
    #[inline]
    pub fn schedule_and_do_work(&self) -> ProgressIndicator {
        let mut mutable_ref = self.0.borrow_mut();
        if !mutable_ref.has_local_tasks() {
            match mutable_ref.schedule_next() {
                ScheduleOutcome::GlobalTaskAcquired => {
                    tracing::debug!("Succesfully acquired new tasks for processing");
                }
                ScheduleOutcome::LocalTaskRunning => {
                    tracing::debug!(
                        "Invalid state reached, no local task should have been in queue"
                    );
                    unreachable!("No local task should be running at this point")
                }
                ScheduleOutcome::NoTaskRunningOrAcquired => {
                    tracing::debug!("No new tasks, no need to perform work");
                    return ProgressIndicator::NoWork;
                }
            }
        }

        match self.do_work() {
            ProgressIndicator::CanProgress => {
                todo!()
            }
            ProgressIndicator::NoWork => match mutable_ref.sleepers.min_duration() {
                None => match mutable_ref.idler.increment() {
                    Some(next_dur) => ProgressIndicator::SpinWait(next_dur),
                    None => ProgressIndicator::NoWork,
                },
                Some(duration) => ProgressIndicator::SpinWait(duration),
            },
            ProgressIndicator::SpinWait(duration) => ProgressIndicator::SpinWait(duration),
        }
    }

    /// do_work attempts to call the current iterator to progress
    /// executing the next operation internally till it's ready for
    /// work to begin.
    #[inline]
    pub fn do_work(&self) -> ProgressIndicator {
        let mut mutable_ref = self.0.borrow_mut();
        // check if any sleeper has ripened up to
        // readiness.
        mutable_ref.wakeup_ready_sleepers();

        // if after wake up, no task still enters
        // the processing queue then no work is available
        if mutable_ref.processing.is_empty() {
            return ProgressIndicator::NoWork;
        }

        let top_entry = mutable_ref.processing.pop_front().unwrap();
        if mutable_ref.is_packed(&top_entry) {
            return ProgressIndicator::CanProgress;
        }

        match mutable_ref.local_tasks.get_mut(&top_entry) {
            Some(iter) => match iter.next() {
                Some(state) => match state {
                    State::Done => {
                        // task is done
                        ProgressIndicator::CanProgress
                    }
                    State::Progressed => {
                        mutable_ref.processing.push_front(top_entry);
                        ProgressIndicator::CanProgress
                    }
                    State::Pending(duration) => {
                        // if we have a duration then we check if
                        // we have other tasks pending, and if so
                        // we indicate we can make progress else we
                        // tell the executor to pause us for that duration
                        // till the task is ready.
                        //
                        // The task that wants to sleep gets removed from
                        // the processing queue and gets registered with the
                        // sleepers (which monitors task that are sleeping).
                        match duration {
                            Some(inner) => {
                                // pack this entry and it's dependents into our packed registry.
                                mutable_ref.pack_task_and_dependents(top_entry.clone());

                                // I do not think I need to use the sleeper entry.
                                let _ = mutable_ref.sleepers.insert(Wakeable::from_now(
                                    SingleWaker::new(top_entry.clone(), self.clone()),
                                    inner,
                                ));

                                if mutable_ref.processing.len() > 0 {
                                    return ProgressIndicator::CanProgress;
                                }

                                ProgressIndicator::SpinWait(inner)
                            }
                            None => ProgressIndicator::CanProgress,
                        }
                    }
                    State::Reschedule => {
                        mutable_ref.processing.push_back(top_entry.clone());
                        ProgressIndicator::CanProgress
                    }
                },
                None => {
                    // Task Iterator is really done
                    ProgressIndicator::CanProgress
                }
            },
            None => unreachable!("An entry must always have a task attached"),
        }
    }
}

pub trait ProcessController {
    fn yield_process(&self);
    fn yield_for(&self, dur: time::Duration);
}

#[derive(Default, Clone)]
pub struct ThreadYield;

impl ProcessController for ThreadYield {
    fn yield_process(&self) {
        thread::yield_now();
    }

    fn yield_for(&self, dur: time::Duration) {
        thread::sleep(dur);
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
pub struct LocalThreadExecutor<T: Iterator<Item = State>> {
    state: ReferencedExecutorState<T>,
    yielder: Box<dyn ProcessController>,
}

// --- constructors

impl<T: Iterator<Item = State>> LocalThreadExecutor<T> {
    pub fn new(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: ChaCha8Rng,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: Box<dyn ProcessController>,
    ) -> Self {
        Self {
            yielder,
            state: rc::Rc::new(cell::RefCell::new(ExecutorState::new(
                tasks, priority, rng, idler,
            )))
            .into(),
        }
    }

    pub fn always_from_bottom<R: rand::Rng>(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: &mut R,
        idler: IdleMan,
    ) -> Self {
        Self::new(
            tasks,
            ChaCha8Rng::seed_from_u64(rng.next_u64()),
            idler,
            PriorityOrder::Bottom,
            Box::new(ThreadYield::default()),
        )
    }

    pub fn always_from_top<R: rand::Rng>(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: &mut R,
        idler: IdleMan,
    ) -> Self {
        Self::new(
            tasks,
            ChaCha8Rng::seed_from_u64(rng.next_u64()),
            idler,
            PriorityOrder::Top,
            Box::new(ThreadYield::default()),
        )
    }

    pub fn from_rng<R: rand::Rng>(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: &mut R,
        idler: IdleMan,
        priority: PriorityOrder,
    ) -> Self {
        Self::new(
            tasks,
            ChaCha8Rng::seed_from_u64(rng.next_u64()),
            idler,
            priority,
            Box::new(ThreadYield::default()),
        )
    }
}

// --- implementations

impl<T: Iterator<Item = State>> LocalThreadExecutor<T> {
    fn block_on(&self) {
        loop {
            for _ in 0..200 {
                match self.state.schedule_and_do_work() {
                    ProgressIndicator::CanProgress => continue,
                    ProgressIndicator::NoWork => {
                        self.yielder.yield_process();
                    }
                    ProgressIndicator::SpinWait(duration) => {
                        self.yielder.yield_for(duration);
                    }
                }
            }

            self.yielder.yield_process();
        }
    }
}
