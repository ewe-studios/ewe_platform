use std::{
    cell,
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

pub type BoxedStateIterator = Box<dyn Iterator<Item = State>>;

/// PriorityOrder defines how wake up tasks should placed once woken up.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    pub(crate) sleepers: Sleepers<Entry>,

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
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
        for matured in self.sleepers.get_matured() {
            self.wake_up(matured.handle.clone())
        }
    }

    #[inline]
    pub(crate) fn has_sleeping_tasks(&self) -> bool {
        self.sleepers.has_pending_tasks()
    }

    /// Returns True/False indicative if the executor has any local
    /// task still processing
    #[inline]
    pub fn has_local_tasks(&self) -> bool {
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
        ReferencedExecutorState(rc::Rc::clone(&self.0))
    }
}

#[allow(unused)]
impl<T: Iterator<Item = State>> ReferencedExecutorState<T> {
    fn get_ref(&self) -> &rc::Rc<cell::RefCell<ExecutorState<T>>> {
        &self.0
    }

    fn get_ref_mut(&mut self) -> &mut rc::Rc<cell::RefCell<ExecutorState<T>>> {
        &mut self.0
    }

    fn borrow(&self) -> cell::Ref<'_, ExecutorState<T>> {
        self.0.borrow()
    }

    fn borrow_mut(&self) -> cell::RefMut<'_, ExecutorState<T>> {
        self.0.borrow_mut()
    }
}

impl<T: Iterator<Item = State>> ReferencedExecutorState<T> {
    #[inline]
    pub(crate) fn request_global_task(&self) -> ProgressIndicator {
        let mut handle = self.0.borrow_mut();
        if handle.has_local_tasks() {
            return ProgressIndicator::CanProgress;
        }

        match handle.schedule_next() {
            ScheduleOutcome::GlobalTaskAcquired => {
                tracing::debug!("Succesfully acquired new tasks for processing");
                ProgressIndicator::CanProgress
            }
            ScheduleOutcome::NoTaskRunningOrAcquired => {
                tracing::debug!("No new tasks, no need to perform work");
                if handle.has_sleeping_tasks() {
                    return ProgressIndicator::CanProgress;
                }
                return ProgressIndicator::NoWork;
            }
            ScheduleOutcome::LocalTaskRunning => {
                tracing::debug!("Invalid state reached, no local task should have been in queue");
                unreachable!("No local task should be running at this point")
            }
        }
    }

    #[inline]
    pub(crate) fn request_sleeping_tasks_wake(&self) {
        let mut handle = self.0.borrow_mut();
        handle.wakeup_ready_sleepers();
    }

    #[inline]
    pub fn schedule_and_do_work(&self) -> ProgressIndicator {
        match self.request_global_task() {
            ProgressIndicator::CanProgress => {
                tracing::debug!(
                    "Attempt to get global task shows we can progress, call do_work next"
                );
            }
            ProgressIndicator::NoWork => {
                return ProgressIndicator::NoWork;
            }
            ProgressIndicator::SpinWait(_) => {
                unreachable!("Requesting global task should never spin wait")
            }
        }

        self.request_sleeping_tasks_wake();

        match self.do_work() {
            ProgressIndicator::CanProgress => {
                tracing::debug!("Received CanProgress indicator from task");
                // TODO: I feel like I am missing something here
                ProgressIndicator::CanProgress
            }
            ProgressIndicator::NoWork => {
                tracing::debug!("Received NoWork indicator from task");
                let mut state = self.0.borrow_mut();
                match state.sleepers.min_duration() {
                    None => match state.idler.increment() {
                        Some(next_dur) => ProgressIndicator::SpinWait(next_dur),
                        None => ProgressIndicator::NoWork,
                    },
                    Some(duration) => ProgressIndicator::SpinWait(duration),
                }
            }
            ProgressIndicator::SpinWait(duration) => {
                tracing::debug!("Received SpinWait({:?}) indicator from task", &duration);
                ProgressIndicator::SpinWait(duration)
            }
        }
    }

    /// do_work attempts to call the current iterator to progress
    /// executing the next operation internally till it's ready for
    /// work to begin.
    #[inline]
    pub fn do_work(&self) -> ProgressIndicator {
        let mut handle = self.0.borrow_mut();

        // if after wake up, no task still enters
        // the processing queue then no work is available
        if handle.processing.is_empty() {
            tracing::debug!("Task queue is empty: {:?}", handle.processing.is_empty());
            if handle.has_sleeping_tasks() {
                return ProgressIndicator::CanProgress;
            }
            return ProgressIndicator::NoWork;
        }

        let top_entry = handle.processing.pop_front().unwrap();
        let remaining_tasks = handle.processing.len();

        if handle.is_packed(&top_entry) {
            return ProgressIndicator::CanProgress;
        }

        match handle.local_tasks.get_mut(&top_entry) {
            Some(iter) => {
                match iter.next() {
                    Some(state) => {
                        tracing::debug!("Task delivered state: {:?}", &state);
                        match state {
                            State::Done => {
                                tracing::debug!(
                                    "Task as finished with State::Done (rem_tasks: {})",
                                    remaining_tasks
                                );
                                // Task Iterator is really done
                                if remaining_tasks == 0 {
                                    ProgressIndicator::NoWork
                                } else {
                                    ProgressIndicator::CanProgress
                                }
                            }
                            State::Progressed => {
                                tracing::debug!("Task is progressing with State::Progressed");
                                handle.processing.push_front(top_entry);
                                ProgressIndicator::CanProgress
                            }
                            State::Pending(duration) => {
                                tracing::debug!(
                                    "Task indicates it is in pending state: State::Pending({:?})",
                                    &duration
                                );
                                // if we have a duration then we check if
                                // we have other tasks pending, and if so
                                // we indicate we can make progress else we
                                // tell the executor to pause us for that duration
                                // till the task is ready.
                                //
                                // The task that wants to sleep gets removed from
                                // the processing queue and gets registered with the
                                // sleepers (which monitors task that are sleeping).
                                let final_state = match duration {
                                    Some(inner) => {
                                        tracing::debug!("Task provided duration: {:?}", &inner);

                                        // pack this entry and it's dependents into our packed registry.
                                        handle.pack_task_and_dependents(top_entry.clone());

                                        // I do not think I need to use the sleeper entry.
                                        let _ = handle
                                            .sleepers
                                            .insert(Wakeable::from_now(top_entry.clone(), inner));

                                        if handle.processing.len() > 0 {
                                            return ProgressIndicator::CanProgress;
                                        }

                                        ProgressIndicator::SpinWait(inner)
                                    }
                                    None => {
                                        handle.processing.push_front(top_entry);
                                        ProgressIndicator::CanProgress
                                    }
                                };

                                tracing::debug!("Sending out state: {:?}", &final_state);
                                final_state
                            }
                            State::Reschedule => {
                                tracing::debug!(
                                    "Task is wishes to reschedule with State::Reschedule"
                                );
                                handle.processing.push_back(top_entry.clone());
                                ProgressIndicator::CanProgress
                            }
                        }
                    }
                    None => {
                        tracing::debug!(
                            "Task returned None (has finished) (rem_tasks: {})",
                            remaining_tasks
                        );
                        // Task Iterator is really done
                        if remaining_tasks == 0 {
                            ProgressIndicator::NoWork
                        } else {
                            ProgressIndicator::CanProgress
                        }
                    }
                }
            }
            None => unreachable!("An entry must always have a task attached"),
        }
    }
}

pub trait ProcessController {
    fn yield_process(&self);
    fn yield_for(&self, dur: time::Duration);
}

pub trait CloneProcessController: ProcessController {
    fn clone_process_controller(&self) -> Box<dyn CloneProcessController>;
}

impl<F> CloneProcessController for F
where
    F: ProcessController + Clone + 'static,
{
    fn clone_process_controller(&self) -> Box<dyn CloneProcessController> {
        Box::new(self.clone())
    }
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
    yielder: Box<dyn CloneProcessController>,
}

// --- constructors

impl<T: Iterator<Item = State>> Clone for LocalThreadExecutor<T> {
    fn clone(&self) -> Self {
        LocalThreadExecutor {
            state: self.state.clone(),
            yielder: self.yielder.clone_process_controller(),
        }
    }
}

impl<T: Iterator<Item = State>> LocalThreadExecutor<T> {
    pub fn new(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: ChaCha8Rng,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: Box<dyn CloneProcessController>,
    ) -> Self {
        Self {
            yielder,
            state: rc::Rc::new(cell::RefCell::new(ExecutorState::new(
                tasks, priority, rng, idler,
            )))
            .into(),
        }
    }

    /// creates a new local executor which uses the provided
    /// seed for ChaCha8Rng generator.
    pub fn from_seed(
        seed: u64,
        tasks: sync::Arc<ConcurrentQueue<T>>,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: Box<dyn CloneProcessController>,
    ) -> Self {
        Self::new(
            tasks,
            ChaCha8Rng::seed_from_u64(seed),
            idler,
            priority,
            yielder,
        )
    }

    /// Allows supplying a custom Rng generator for creating the initial
    /// ChaCha8Rng seed.
    pub fn from_rng<R: rand::Rng>(
        tasks: sync::Arc<ConcurrentQueue<T>>,
        rng: &mut R,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: Box<dyn CloneProcessController>,
    ) -> Self {
        Self::from_seed(rng.next_u64(), tasks, idler, priority, yielder)
    }
}

// --- implementations

impl<T: Iterator<Item = State>> LocalThreadExecutor<T> {
    pub fn make_progress(&self) -> ProgressIndicator {
        self.state.schedule_and_do_work()
    }

    pub fn block_on(&self) {
        loop {
            for _ in 0..200 {
                match self.make_progress() {
                    ProgressIndicator::CanProgress => continue,
                    ProgressIndicator::NoWork => {
                        self.yielder.yield_process();
                        continue;
                    }
                    ProgressIndicator::SpinWait(duration) => {
                        self.yielder.yield_for(duration);
                        continue;
                    }
                }
            }
            self.yielder.yield_process();
        }
    }
}

#[cfg(test)]
mod test_local_thread_executor {
    use crate::{
        panic_if_failed,
        retries::ExponentialBackoffDecider,
        synca::SleepyMan,
        valtron::{
            task_iterator::{TaskIterator, TaskStatus},
            SimpleScheduledTask,
        },
    };

    use super::*;
    use cell::*;
    use rand::prelude::*;
    use rc::Rc;
    use sync::Arc;
    use tracing_test::traced_test;

    #[derive(Default, Clone)]
    struct NoYielder;

    impl ProcessController for NoYielder {
        fn yield_process(&self) {
            return;
        }

        fn yield_for(&self, _: time::Duration) {
            return;
        }
    }

    struct Counter(usize, usize);

    impl TaskIterator for Counter {
        type Done = usize;
        type Pending = time::Duration;

        fn next(
            &mut self,
        ) -> Option<crate::valtron::task_iterator::TaskStatus<Self::Done, Self::Pending>> {
            let old_count = self.0;
            let new_count = old_count + 1;
            self.0 = new_count;

            if new_count == self.1 {
                return None;
            }

            if new_count % 3 == 0 {
                return Some(TaskStatus::Delayed(time::Duration::from_millis(5)));
            }

            Some(TaskStatus::Ready(new_count))
        }
    }

    #[test]
    #[traced_test]
    fn scenario_one_task_a_runs_to_completion() {
        let global: ConcurrentQueue<BoxedStateIterator> = ConcurrentQueue::bounded(10);

        let counts: Rc<RefCell<Vec<TaskStatus<usize, time::Duration>>>> =
            Rc::new(RefCell::new(Vec::new()));

        let count_clone = Rc::clone(&counts);
        panic_if_failed!(global.push(Box::new(SimpleScheduledTask::on_next_mut(
            Counter(0, 3),
            move |next| count_clone.borrow_mut().push(next)
        ))));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            Arc::new(global),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            Box::new(NoYielder::default()),
        );

        assert_eq!(executor.make_progress(), ProgressIndicator::CanProgress);
        assert_eq!(executor.make_progress(), ProgressIndicator::CanProgress);
        assert_eq!(executor.make_progress(), ProgressIndicator::NoWork);

        let count_list: Vec<TaskStatus<usize, time::Duration>> = counts.clone().take();
        assert_eq!(
            count_list,
            vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_2_task_a_goes_to_sleep_as_only_task_in_queue() {
        let global: ConcurrentQueue<BoxedStateIterator> = ConcurrentQueue::bounded(10);

        let counts: Rc<RefCell<Vec<TaskStatus<usize, time::Duration>>>> =
            Rc::new(RefCell::new(Vec::new()));

        let count_clone = Rc::clone(&counts);
        panic_if_failed!(global.push(Box::new(SimpleScheduledTask::on_next_mut(
            Counter(10, 20),
            move |next| count_clone.borrow_mut().push(next)
        ))));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            Arc::new(global),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            Box::new(NoYielder::default()),
        );

        assert_eq!(executor.make_progress(), ProgressIndicator::CanProgress);

        assert_eq!(
            executor.make_progress(),
            ProgressIndicator::SpinWait(time::Duration::from_millis(5))
        );

        // wait for 5ms and validate we made progress
        thread::sleep(time::Duration::from_millis(5));

        assert_eq!(executor.make_progress(), ProgressIndicator::CanProgress);
        assert_eq!(executor.make_progress(), ProgressIndicator::CanProgress);

        let count_list: Vec<TaskStatus<usize, time::Duration>> = counts.clone().take();
        assert_eq!(
            count_list,
            vec![
                TaskStatus::Ready(11),
                TaskStatus::Ready(13),
                TaskStatus::Ready(14),
            ]
        );
    }
}
