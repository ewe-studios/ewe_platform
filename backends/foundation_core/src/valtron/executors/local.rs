use std::{
    cell,
    collections::{HashMap, VecDeque},
    rc,
    sync::{
        self,
        atomic::{self, AtomicBool},
    },
    time,
};

use crate::{
    synca::{DurationWaker, Entry, EntryList, IdleMan, Sleepers, Waiter},
    valtron::{AnyResult, ExecutionEngine, ExecutionIterator, State},
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use concurrent_queue::{ConcurrentQueue, PushError};

use super::{
    BoxedLocalExecutionIterator, ExecutionAction, ExecutionTaskIteratorBuilder, ExecutorError,
    ProcessController, TaskIterator, TaskReadyResolver, TaskStatusMapper,
};

/// PriorityOrder defines how wake up tasks should placed once woken up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PriorityOrder {
    Top,
    Bottom,
}

/// `Sleepable` defines specific holders that help
/// indicate the readiness of a task via it's entry.
pub enum Sleepable {
    /// Timable are tasks that can sleep via duration and
    /// will communicate their readiness using when a duration
    /// and time is expired/over.
    Timable(DurationWaker<Entry>),

    /// Flag represents a task that connect a task with a `AtomicBool`
    /// signal will communicate when a giving task is ready.
    Atomic(sync::Arc<AtomicBool>, Entry),
}

impl Waiter for Sleepable {
    fn is_ready(&self) -> bool {
        match self {
            Sleepable::Timable(inner) => inner.is_ready(),
            Sleepable::Atomic(inner, _) => inner.load(atomic::Ordering::SeqCst),
        }
    }
}

#[derive(PartialEq, Eq)]
pub(crate) enum SpawnType {
    Lifted,
    LiftedWithParent,
    Broadcast,
    Scheduled,
}

/// Underlying stats shared by all executors.
pub struct ExecutorState<T: ExecutionIterator> {
    /// what priority should waking task be placed.
    pub(crate) priority: PriorityOrder,

    /// global_tasks are the shared tasks coming from the main thread
    /// they generally will always come in fifo order and will be processed
    /// in the order received.
    pub(crate) global_tasks: sync::Arc<ConcurrentQueue<T>>,

    /// indicates to us which if any spawn operation occurred.
    pub(crate) spawn_op: rc::Rc<cell::RefCell<Option<SpawnType>>>,

    /// indicates the current task currently being handled for
    /// safety checks.
    pub(crate) current_task: rc::Rc<cell::RefCell<Option<Entry>>>,

    /// tasks owned by the executor which should be processed first
    /// before taking on the next task from the global queue,
    /// task generally come in here from the current task taking
    /// from the global queue.
    /// This allows us keep the overall execution of any async
    /// iterator within the same thread and keep a one thread
    /// execution.
    pub(crate) local_tasks: rc::Rc<cell::RefCell<EntryList<T>>>,

    /// task_graph provides dependency mapping of a Task (Key) lifted
    /// by another Task (Value) allowing us to go from lifted task
    /// to it's tree of dependents.
    pub(crate) task_graph: rc::Rc<cell::RefCell<HashMap<Entry, Entry>>>,

    /// map used to identify that a task was packed.
    pub(crate) packed_tasks: rc::Rc<cell::RefCell<HashMap<Entry, bool>>>,

    /// the queue used for performing perioritization, usually
    /// the global task taking will first be stored in the local
    /// tasks list and have it's related Entry pointer added
    /// into the queue for prioritization, and we will never
    /// take any new tasks from the global queue until all
    /// the entries in this queue is empty, this ensures
    /// we can consistently complete every received tasks
    /// till it finishes.
    pub(crate) processing: rc::Rc<cell::RefCell<VecDeque<Entry>>>,

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
    pub(crate) sleepers: Sleepers<Sleepable>,

    /// sleepy provides a managed indicator of how many times we've been idle
    /// and recommends how much sleep should the executor take next.
    pub(crate) idler: rc::Rc<cell::RefCell<IdleMan>>,
}

// --- constructors

static DEQUEUE_CAPACITY: usize = 10;

impl<T: ExecutionIterator> ExecutorState<T> {
    pub fn new(
        global_tasks: sync::Arc<ConcurrentQueue<T>>,
        priority: PriorityOrder,
        rng: ChaCha8Rng,
        idler: IdleMan,
    ) -> Self {
        Self {
            priority,
            global_tasks,
            sleepers: Sleepers::new(),
            rng: rc::Rc::new(cell::RefCell::new(rng)),
            idler: rc::Rc::new(cell::RefCell::new(idler)),
            spawn_op: rc::Rc::new(cell::RefCell::new(None)),
            current_task: rc::Rc::new(cell::RefCell::new(None)),
            task_graph: rc::Rc::new(cell::RefCell::new(HashMap::new())),
            packed_tasks: rc::Rc::new(cell::RefCell::new(HashMap::new())),
            local_tasks: rc::Rc::new(cell::RefCell::new(EntryList::new())),
            processing: rc::Rc::new(cell::RefCell::new(VecDeque::with_capacity(
                DEQUEUE_CAPACITY,
            ))),
        }
    }
}

// --- implementations

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

// --- Task Dependences, Rng and Helper methods

impl<T: ExecutionIterator> Clone for ExecutorState<T> {
    fn clone(&self) -> Self {
        Self {
            rng: self.rng.clone(),
            idler: self.idler.clone(),
            sleepers: self.sleepers.clone(),
            priority: self.priority.clone(),
            spawn_op: self.spawn_op.clone(),
            current_task: self.current_task.clone(),
            global_tasks: self.global_tasks.clone(),
            local_tasks: self.local_tasks.clone(),
            task_graph: self.task_graph.clone(),
            packed_tasks: self.packed_tasks.clone(),
            processing: self.processing.clone(),
        }
    }
}

impl<T: ExecutionIterator> ExecutorState<T> {
    /// Returns a borrowed immutable
    pub fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.rng.clone()
    }

    fn number_of_local_tasks(&self) -> usize {
        self.local_tasks.borrow().active_slots()
    }

    fn number_of_inprocess(&self) -> usize {
        self.processing.borrow().len()
    }

    /// Returns the list of other task dependent on this giving tasks
    /// and their dependents in order.
    #[inline]
    pub fn get_task_dependents(&self, target: Entry) -> Vec<Entry> {
        let mut deps = Vec::new();

        let mut previous = Some(target);

        while previous.is_some() {
            let key = previous.clone().unwrap();
            match self.task_graph.borrow().get(&key) {
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
    pub fn is_packed(&self, target: &Entry) -> bool {
        match self.packed_tasks.borrow().get(target) {
            Some(value) => value.clone(),
            None => false,
        }
    }

    /// De-registers this task and it's dependents from the packed hashmap.
    #[inline]
    pub fn unpack_task_and_dependents(&self, target: Entry) {
        tracing::debug!("Unpacking: tasks and dependents for: {:?}", &target);
        self.packed_tasks.borrow_mut().remove(&target);
        for dependent in self.get_task_dependents(target.clone()).into_iter() {
            tracing::debug!(
                "Unpacking: task's: {:?} dependent : {:?}",
                &dependent,
                &target
            );
            self.packed_tasks.borrow_mut().remove(&dependent);
        }
    }

    /// Register this task and its dependents in the packed hashmap.
    #[inline]
    pub fn pack_task_and_dependents(&self, target: Entry) {
        tracing::debug!("Packing: tasks and dependents for: {:?}", &target);
        self.packed_tasks.borrow_mut().insert(target.clone(), true);
        for dependent in self.get_task_dependents(target.clone()).into_iter() {
            tracing::debug!(
                "Packing: task's: {:?} dependent : {:?}",
                &target,
                &dependent,
            );
            self.packed_tasks.borrow_mut().insert(dependent, true);
        }
    }

    /// wake_up adds the entry into the list of wakers
    /// that should be woken up by the executor.
    #[inline]
    pub fn wake_up(&self, target: Entry) {
        // get all the list of dependents and add back into queue.
        let deps = self.get_task_dependents(target.clone());

        // remove packed registry
        self.packed_tasks.borrow_mut().remove(&target);

        match self.priority {
            PriorityOrder::Top => {
                for dependent in deps.into_iter().rev() {
                    self.packed_tasks.borrow_mut().remove(&dependent);
                    self.processing.borrow_mut().push_front(dependent.clone());
                }
                self.processing.borrow_mut().push_front(target.clone());
            }
            PriorityOrder::Bottom => {
                self.processing.borrow_mut().push_back(target.clone());
                for dependent in deps.into_iter() {
                    self.packed_tasks.borrow_mut().remove(&dependent);
                    self.processing.borrow_mut().push_back(dependent.clone());
                }
            }
        }
    }

    /// next_wakeup checks all registered sleepers to see if they
    /// matured to the age of being woken up and placed back into
    /// the processing queue.
    #[inline]
    pub fn wakeup_ready_sleepers(&self) {
        for matured in self.sleepers.get_matured() {
            match matured {
                Sleepable::Timable(wakeable) => self.wake_up(wakeable.handle.clone()),
                Sleepable::Atomic(_, entry) => self.wake_up(entry.clone()),
            }
        }
    }

    #[inline]
    pub(crate) fn number_of_sleepers(&self) -> usize {
        self.sleepers.count()
    }

    #[inline]
    pub(crate) fn has_sleeping_tasks(&self) -> bool {
        self.sleepers.has_pending_tasks()
    }

    /// Returns True/False indicative if the executor has any local
    /// task still processing
    #[inline]
    pub fn has_local_tasks(&self) -> bool {
        self.local_tasks.borrow().active_slots() > 0
    }

    /// Returns if there is any local task is active
    /// which means:
    ///
    /// 1. Not sleeping
    /// 2. In local task queue
    ///
    pub fn has_active_tasks(&self) -> bool {
        self.total_active_tasks() > 0
    }

    /// Returns true/false if processing queue has task.
    pub fn has_inflight_task(&self) -> bool {
        self.processing.borrow().len() > 0
    }

    /// Returns the total remaining tasks that are
    /// active and not sleeping.
    pub fn total_active_tasks(&self) -> usize {
        let local_task_count = self.local_tasks.borrow().active_slots();
        let sleeping_task_count = self.sleepers.count();
        let in_process_task_count = self.number_of_inprocess();
        let active_task_count = local_task_count - sleeping_task_count;
        tracing::debug!(
            "Local TaskCount={} and Sleeping TaskCount={}, InProcessTasks={}, ActiveTasks={}",
            local_task_count,
            sleeping_task_count,
            in_process_task_count,
            active_task_count,
        );
        active_task_count
    }

    /// schedule_next will attempt to pull a new task from the
    /// global queue if no task is pending on the local queue
    /// and if so returns true to indicate success else a false
    /// to indicate no task was taking from the global queue
    /// as the local queue had a task or no task was found.
    #[inline]
    pub fn schedule_next(&self) -> ScheduleOutcome {
        if self.local_tasks.borrow().active_slots() > 0 && self.processing.borrow().len() > 0 {
            return ScheduleOutcome::LocalTaskRunning;
        }

        match self.global_tasks.pop() {
            Ok(task) => {
                let task_entry = self.local_tasks.borrow_mut().insert(task);
                self.processing.borrow_mut().push_front(task_entry.clone());
                ScheduleOutcome::GlobalTaskAcquired
            }
            Err(_) => ScheduleOutcome::NoTaskRunningOrAcquired,
        }
    }

    #[inline]
    pub fn request_global_task(&self) -> ProgressIndicator {
        if self.has_active_tasks() {
            tracing::debug!("Still have active tasks");
            return ProgressIndicator::CanProgress;
        }

        match self.schedule_next() {
            ScheduleOutcome::GlobalTaskAcquired => {
                tracing::debug!("Succesfully acquired new tasks for processing");
                ProgressIndicator::CanProgress
            }
            ScheduleOutcome::NoTaskRunningOrAcquired => {
                if self.has_sleeping_tasks() {
                    tracing::debug!(
                        "No new tasks, but we have sleeping tasks, so we can make progress"
                    );
                    return ProgressIndicator::CanProgress;
                }

                tracing::debug!("No new tasks, no need to perform work");
                return ProgressIndicator::NoWork;
            }
            ScheduleOutcome::LocalTaskRunning => {
                tracing::debug!("Invalid state reached, no local task should have been in queue");
                unreachable!("No local task should be running at this point")
            }
        }
    }

    #[inline]
    pub fn schedule_and_do_work(&self, engine: T::Executor) -> ProgressIndicator {
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

        self.wakeup_ready_sleepers();

        match self.do_work(engine) {
            ProgressIndicator::CanProgress => {
                tracing::debug!("Received CanProgress indicator from task");
                // TODO: I feel like I am missing something here
                ProgressIndicator::CanProgress
            }
            ProgressIndicator::NoWork => {
                tracing::debug!("Received NoWork indicator from task");

                // empty the current task marker
                self.current_task.borrow_mut().take();

                match self.idler.borrow_mut().increment() {
                    Some(next_dur) => ProgressIndicator::SpinWait(next_dur),
                    None => ProgressIndicator::NoWork,
                }
            }
            ProgressIndicator::SpinWait(duration) => {
                tracing::debug!("Received SpinWait({:?}) indicator from task", &duration);

                if self.has_inflight_task() {
                    return ProgressIndicator::CanProgress;
                }

                // empty the current task marker
                self.current_task.borrow_mut().take();

                // if the current task indicates it wants to spin wait,
                // attempt to get global task else return
                // duration as is.
                match self.schedule_next() {
                    ScheduleOutcome::GlobalTaskAcquired => {
                        tracing::debug!(
                            "Global task indicate we can make progress, possible acquired task"
                        );
                        ProgressIndicator::CanProgress
                    }
                    ScheduleOutcome::NoTaskRunningOrAcquired => {
                        tracing::debug!("No new task from global queue");
                        ProgressIndicator::SpinWait(duration)
                    }
                    ScheduleOutcome::LocalTaskRunning => {
                        tracing::debug!("Unexpected state with local task available");
                        unreachable!("global task never spinWaits")
                    }
                }
            }
        }
    }

    pub fn check_processing_queue(&self) -> Option<ProgressIndicator> {
        let has_sleeping_tasks = self.has_sleeping_tasks();
        let handle = self.processing.borrow_mut();
        // if after wake up, no task still enters
        // the processing queue then no work is available
        if handle.is_empty() {
            tracing::debug!("Task queue is empty: {:?}", handle.is_empty());
            if has_sleeping_tasks {
                return Some(ProgressIndicator::CanProgress);
            }
            return Some(ProgressIndicator::NoWork);
        }
        None
    }

    /// do_work attempts to call the current iterator to progress
    /// executing the next operation internally till it's ready for
    /// work to begin.
    #[inline]
    pub fn do_work(&self, engine: T::Executor) -> ProgressIndicator {
        // if after wake up, no task still enters
        // the processing queue then no work is available
        match self.check_processing_queue() {
            Some(inner) => match inner {
                ProgressIndicator::NoWork => return ProgressIndicator::NoWork,
                ProgressIndicator::CanProgress => return ProgressIndicator::CanProgress,
                _ => unreachable!("check_processing_queue should never reach here"),
            },
            None => {}
        }

        let top_entry = self.processing.borrow_mut().pop_front().unwrap();
        let remaining_tasks = self.processing.borrow().len();

        if self.is_packed(&top_entry) {
            return ProgressIndicator::CanProgress;
        }

        self.current_task.borrow_mut().replace(top_entry.clone());

        let iter_container = self.local_tasks.borrow_mut().park(&top_entry);
        match iter_container {
            Some(mut iter) => {
                match iter.next(top_entry.clone(), engine) {
                    Some(state) => {
                        tracing::debug!("Task delivered state: {:?}", &state);
                        match state {
                            State::SpawnFailed => {
                                unreachable!("Executor should never fail to spawn a task");
                            }
                            State::SpawnFinished => {
                                tracing::debug!(
                                    "Spawned new task successfully over current {:?} (rem_tasks: {})",
                                    &top_entry,
                                    remaining_tasks,
                                );

                                // unpack the entry in the task list
                                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                                // unless we just lifted with a parent then add entry back.
                                if let Some(op) = self.spawn_op.borrow().as_ref() {
                                    if op != &SpawnType::LiftedWithParent {
                                        // push entry back into processing mut
                                        self.processing.borrow_mut().push_front(top_entry);
                                    }
                                }

                                // no need to push entry since it must have
                                ProgressIndicator::CanProgress
                            }
                            State::Done => {
                                tracing::debug!(
                                    "Task as finished with State::Done (task: {:?}, rem_tasks: {})",
                                    &top_entry,
                                    remaining_tasks
                                );

                                // now unpack and take entry out of local tasks
                                self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                                self.local_tasks.borrow_mut().take(&top_entry);

                                tracing::debug!(
                                    "Finished unparking and taking task (task: {:?}, rem_tasks: {})",
                                    &top_entry,
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
                                // unpack the entry in the task list
                                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                                // push entry back into processing mut
                                self.processing.borrow_mut().push_front(top_entry);
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

                                        // unpack the entry in the task list
                                        self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                                        // pack this entry and it's dependents into our packed registry.
                                        self.pack_task_and_dependents(top_entry.clone());

                                        // I do not think I need to use the sleeper entry.
                                        let _ = self.sleepers.insert(Sleepable::Timable(
                                            DurationWaker::from_now(top_entry.clone(), inner),
                                        ));

                                        if self.processing.borrow().len() > 0 {
                                            return ProgressIndicator::CanProgress;
                                        }

                                        ProgressIndicator::SpinWait(inner)
                                    }
                                    None => {
                                        // unpack the entry in the task list
                                        self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                                        // push back to top
                                        self.processing.borrow_mut().push_front(top_entry);
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

                                // unpack the entry in the task list
                                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                                // add back task into queue
                                self.processing.borrow_mut().push_back(top_entry.clone());

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

// --- End of: Task Dependences, Rng and Helper methods

// --- Task spawn methods: Lift, Schedule & Broadcast

impl<T: ExecutionIterator> ExecutorState<T> {
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
    pub fn lift(&self, task: T, parent: Option<Entry>) -> AnyResult<Entry, ExecutorError> {
        // if there is a parent then you need to be
        // the top of the executing set.
        if let Some(parent_handle) = &parent {
            match self.current_task.borrow().clone() {
                Some(current_task) => {
                    if !current_task.eq(parent_handle) {
                        return Err(ExecutorError::ParentMustBeExecutingToLift);
                    }
                }
                None => unreachable!("Current task should never truly be empty at this point"),
            }
        }

        let task_entry = self.local_tasks.borrow_mut().insert(task);

        // if we have parent then queue parent as well
        // as next before current task, so that the next queue
        // task will be the lifted task just right after parent.
        if let Some(parent_handle) = &parent {
            self.processing
                .borrow_mut()
                .push_front(parent_handle.clone());

            self.spawn_op
                .borrow_mut()
                .replace(SpawnType::LiftedWithParent);
        } else {
            self.spawn_op.borrow_mut().replace(SpawnType::Lifted);
        }

        self.processing.borrow_mut().push_front(task_entry.clone());

        // create dependent graph map.
        if let Some(parent_handle) = &parent {
            self.task_graph
                .borrow_mut()
                .insert(task_entry.clone(), parent_handle.clone());
        }

        tracing::debug!("Lift: new task {:?} for parent: {:?}", task_entry, parent);
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
    pub fn schedule(&self, task: T) -> AnyResult<Entry, ExecutorError> {
        let task_entry = self.local_tasks.borrow_mut().insert(task);
        self.processing.borrow_mut().push_back(task_entry.clone());
        self.spawn_op.borrow_mut().replace(SpawnType::Scheduled);
        Ok(task_entry)
    }

    /// Delivers a task (Iterator) type to the global execution queue
    /// and in such a case you do not have an handle to the task as we
    /// no more have control as to where it gets allocated.
    pub fn broadcast(&self, task: T) -> AnyResult<(), ExecutorError> {
        match self.global_tasks.push(task) {
            Ok(_) => {
                self.spawn_op.borrow_mut().replace(SpawnType::Broadcast);
                Ok(())
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }
}

// --- End of: Task spawn methods: Lift, Schedule & Broadcast

pub struct ReferencedExecutorState<T: ExecutionIterator> {
    inner: rc::Rc<ExecutorState<T>>,
}

impl<T: ExecutionIterator> From<rc::Rc<ExecutorState<T>>> for ReferencedExecutorState<T> {
    fn from(value: rc::Rc<ExecutorState<T>>) -> Self {
        ReferencedExecutorState { inner: value }
    }
}

impl<T: ExecutionIterator> Clone for ReferencedExecutorState<T> {
    fn clone(&self) -> Self {
        ReferencedExecutorState {
            inner: rc::Rc::clone(&self.inner),
        }
    }
}

#[allow(unused)]
impl<T: ExecutionIterator> ReferencedExecutorState<T> {
    pub(crate) fn clone_state(&self) -> rc::Rc<ExecutorState<T>> {
        self.inner.clone()
    }

    fn get_ref(&self) -> &rc::Rc<ExecutorState<T>> {
        &self.inner
    }

    fn get_ref_mut(&mut self) -> &mut rc::Rc<ExecutorState<T>> {
        &mut self.inner
    }

    fn number_of_local_tasks(&self) -> usize {
        self.inner.number_of_local_tasks()
    }

    fn number_of_inprocess(&self) -> usize {
        self.inner.number_of_inprocess()
    }
}

impl<T: ExecutionIterator> ReferencedExecutorState<T> {
    #[inline]
    pub(crate) fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.inner.get_rng()
    }

    #[inline]
    pub(crate) fn number_of_sleepers(&self) -> usize {
        self.inner.number_of_sleepers()
    }

    /// Returns true/false if processing queue has task.
    pub fn has_inflight_task(&self) -> bool {
        self.inner.has_inflight_task()
    }

    pub fn schedule_and_do_work(&self, engine: T::Executor) -> ProgressIndicator {
        self.inner.schedule_and_do_work(engine)
    }
}

// --- LocalExecutor for ExecutionIterator::Executor

pub struct LocalExecutorEngine {
    inner: rc::Rc<ExecutorState<BoxedLocalExecutionIterator>>,
}

impl Clone for LocalExecutorEngine {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl LocalExecutorEngine {
    pub fn new(inner: rc::Rc<ExecutorState<BoxedLocalExecutionIterator>>) -> Self {
        Self { inner }
    }
}

impl LocalExecutorEngine {
    /// typed_task allows you to create a task builder but requiring specific
    /// definitions for your `Task`, `Action` and `Resolver` types.
    pub fn typed_task<Task, Action, Resolver>(
        &self,
    ) -> ExecutionTaskIteratorBuilder<
        Task::Done,
        Task::Pending,
        LocalExecutorEngine,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner>>,
        Resolver,
        Task,
    >
    where
        Task: TaskIterator<Spawner = Action> + 'static,
        Action: ExecutionAction<Executor = LocalExecutorEngine> + 'static,
        Resolver: TaskReadyResolver<LocalExecutorEngine, Task::Spawner, Task::Done, Task::Pending>
            + 'static,
    {
        ExecutionTaskIteratorBuilder::new(self.clone())
    }

    /// any_task allows you to create a task builder with less restrictive type
    /// requirements for the builder, specifically resolvers are Boxed.
    pub fn any_task<Task, Action>(
        &self,
    ) -> ExecutionTaskIteratorBuilder<
        Task::Done,
        Task::Pending,
        LocalExecutorEngine,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner>>,
        Box<dyn TaskReadyResolver<LocalExecutorEngine, Task::Spawner, Task::Done, Task::Pending>>,
        Task,
    >
    where
        Task: TaskIterator<Spawner = Action> + 'static,
        Action: ExecutionAction<Executor = LocalExecutorEngine> + 'static,
    {
        ExecutionTaskIteratorBuilder::new(self.clone())
    }
}

impl ExecutionEngine for LocalExecutorEngine {
    type Executor = LocalExecutorEngine;

    fn lift(
        &self,
        task: Box<dyn ExecutionIterator<Executor = Self::Executor>>,
        parent: Option<Entry>,
    ) -> AnyResult<(), ExecutorError> {
        let entry = self.inner.lift(task, parent.clone())?;
        tracing::debug!(
            "Lifted: new task with entry: {:?} from parent: {:?}",
            entry,
            parent
        );
        Ok(())
    }

    fn schedule(
        &self,
        task: Box<dyn ExecutionIterator<Executor = Self::Executor>>,
    ) -> AnyResult<(), ExecutorError> {
        let entry = self.inner.schedule(task)?;
        tracing::debug!("schedule: new task into Executor with entry: {:?}", entry);
        Ok(())
    }

    fn broadcast(
        &self,
        task: Box<dyn ExecutionIterator<Executor = Self::Executor>>,
    ) -> AnyResult<(), ExecutorError> {
        self.inner.broadcast(task)?;
        tracing::debug!("broadcast: new task into Executor");
        Ok(())
    }
}

impl ReferencedExecutorState<BoxedLocalExecutionIterator> {
    pub fn local_executor_engine(&self) -> LocalExecutorEngine {
        LocalExecutorEngine::new(self.inner.clone())
    }
}

// --- End of: LocalExecutor for ExecutionIterator::Executor

/// `SameThreadExecutor` is an implementation of an executor
/// which is only ever exists in the thread it was created in.
///
/// It is most suitable to be used in instances where multi-threading
/// is not possible such as WebAssembly.
///
/// `SameThreadExecutor` are not Send (!Send) so you cant
/// send them over to another thread but can potentially be
/// shared through an Arc.
pub struct LocalThreadExecutor<T: ProcessController> {
    state: ReferencedExecutorState<BoxedLocalExecutionIterator>,
    yielder: rc::Rc<T>,
}

// --- constructors

impl<T: ProcessController> Clone for LocalThreadExecutor<T> {
    fn clone(&self) -> Self {
        LocalThreadExecutor {
            state: self.state.clone(),
            yielder: self.yielder.clone(),
        }
    }
}

// -- Constructors

#[allow(unused)]
impl<T: ProcessController> LocalThreadExecutor<T> {
    pub fn new(
        tasks: sync::Arc<ConcurrentQueue<BoxedLocalExecutionIterator>>,
        rng: ChaCha8Rng,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
    ) -> Self {
        Self {
            yielder: rc::Rc::new(yielder),
            state: rc::Rc::new(ExecutorState::new(tasks, priority, rng, idler)).into(),
        }
    }

    /// creates a new local executor which uses the provided
    /// seed for ChaCha8Rng generator.
    pub fn from_seed(
        seed: u64,
        tasks: sync::Arc<ConcurrentQueue<BoxedLocalExecutionIterator>>,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
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
        tasks: sync::Arc<ConcurrentQueue<BoxedLocalExecutionIterator>>,
        rng: &mut R,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
    ) -> Self {
        Self::from_seed(rng.next_u64(), tasks, idler, priority, yielder)
    }
}

// -- LocalExecutor builder

#[allow(unused)]
impl<T: ProcessController> LocalThreadExecutor<T> {
    pub(crate) fn local_executor_engine(&self) -> LocalExecutorEngine {
        self.state.local_executor_engine()
    }

    /// typed_task allows you to create a task builder but requiring specific
    /// definitions for your `Task`, `Action` and `Resolver` types.
    pub fn typed_task<Task, Action, Resolver>(
        &self,
    ) -> ExecutionTaskIteratorBuilder<
        Task::Done,
        Task::Pending,
        LocalExecutorEngine,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner>>,
        Resolver,
        Task,
    >
    where
        Task: TaskIterator<Spawner = Action> + 'static,
        Action: ExecutionAction<Executor = LocalExecutorEngine> + 'static,
        Resolver: TaskReadyResolver<LocalExecutorEngine, Task::Spawner, Task::Done, Task::Pending>
            + 'static,
    {
        ExecutionTaskIteratorBuilder::new(LocalExecutorEngine::new(self.state.clone_state()))
    }

    /// any_task allows you to create a task builder with less restrictive type
    /// requirements for the builder, specifically resolvers are Boxed.
    pub fn any_task<Task, Action>(
        &self,
    ) -> ExecutionTaskIteratorBuilder<
        Task::Done,
        Task::Pending,
        LocalExecutorEngine,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner>>,
        Box<dyn TaskReadyResolver<LocalExecutorEngine, Task::Spawner, Task::Done, Task::Pending>>,
        Task,
    >
    where
        Task: TaskIterator<Spawner = Action> + 'static,
        Action: ExecutionAction<Executor = LocalExecutorEngine> + 'static,
    {
        ExecutionTaskIteratorBuilder::new(LocalExecutorEngine::new(self.state.clone_state()))
    }
}

// --- LocalExecutor run once

#[allow(unused)]
impl<T: ProcessController> LocalThreadExecutor<T> {
    #[inline]
    pub fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.state.get_rng()
    }

    pub fn run_once(&self) -> ProgressIndicator {
        tracing::debug!("Creating local executor from state");
        let local_executor = self.state.local_executor_engine();

        tracing::debug!("run: ReferencedExecutorState::schedule_and_do_work with local executor");
        self.state.schedule_and_do_work(local_executor)
    }

    pub fn block_on(&self) {
        loop {
            for _ in 0..200 {
                match self.run_once() {
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

    pub(crate) fn number_of_sleepers(&self) -> usize {
        self.state.number_of_sleepers()
    }

    pub(crate) fn number_of_local_tasks(&self) -> usize {
        self.state.number_of_local_tasks()
    }

    pub(crate) fn number_of_inprocess(&self) -> usize {
        self.state.number_of_inprocess()
    }
}

// --- LocalExecutor run once

#[cfg(test)]
mod test_local_thread_executor {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    use crate::{
        panic_if_failed,
        retries::ExponentialBackoffDecider,
        synca::SleepyMan,
        valtron::{
            ExecutionAction, NoSpawner, OnNext, ProcessController, TaskIterator, TaskStatus,
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

    struct Counter(&'static str, usize, usize, usize);

    impl TaskIterator for Counter {
        type Done = usize;
        type Spawner = NoSpawner;
        type Pending = time::Duration;

        fn next(&mut self) -> Option<TaskStatus<Self::Done, Self::Pending, Self::Spawner>> {
            let old_count = self.1;
            let new_count = old_count + 1;
            self.1 = new_count;

            tracing::debug!(
                "Counter({}) has current count {} from old count {}",
                self.0,
                new_count,
                old_count,
            );

            if new_count == self.2 {
                return None;
            }

            if new_count == self.3 {
                return Some(TaskStatus::Delayed(time::Duration::from_millis(5)));
            }

            Some(TaskStatus::Ready(new_count))
        }
    }

    #[test]
    #[traced_test]
    fn scenario_one_task_a_runs_to_completion() {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            NoYielder::default(),
        );

        let count_clone = Rc::clone(&counts);
        let on_next = OnNext::on_next(
            Counter("Counter1", 0, 3, 3),
            move |next, _engine| count_clone.borrow_mut().push(next),
            None,
        );

        panic_if_failed!(global.push(on_next.into()));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
        assert_eq!(executor.number_of_sleepers(), 0);

        let count_list = counts.clone().take();
        assert_eq!(
            count_list,
            vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_one_can_use_local_executor_builder_to_queue_task() {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            NoYielder::default(),
        );

        let count_clone = Rc::clone(&counts);
        panic_if_failed!(executor
            .typed_task()
            .with_task(Counter("Counter1", 0, 3, 3))
            .on_next(move |next, _| count_clone.borrow_mut().push(next))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
        assert_eq!(executor.number_of_sleepers(), 0);

        let count_list = counts.clone().take();
        assert_eq!(
            count_list,
            vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_2_task_a_goes_to_sleep_as_only_task_in_queue() {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            NoYielder::default(),
        );

        let count_clone = Rc::clone(&counts);
        panic_if_failed!(global.push(Box::new(OnNext::on_next(
            Counter("Counter1", 10, 20, 12),
            move |next, _engine| { count_clone.borrow_mut().push(next) },
            None,
        ))));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(counts.borrow().clone(), vec![TaskStatus::Ready(11),]);

        assert_eq!(
            executor.run_once(),
            ProgressIndicator::SpinWait(time::Duration::from_millis(5))
        );
        assert_eq!(counts.borrow().clone(), vec![TaskStatus::Ready(11),]);

        // wait for 5ms and validate we made progress
        thread::sleep(time::Duration::from_millis(5));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![TaskStatus::Ready(11), TaskStatus::Ready(13),]
        );
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.borrow().clone(),
            vec![
                TaskStatus::Ready(11),
                TaskStatus::Ready(13),
                TaskStatus::Ready(14),
            ]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_3_task_goes_to_sleep_as_highest_priority_on_wakeup_with_other_tasks() {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<(&'static str, TaskStatus<usize, time::Duration, NoSpawner>)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Top,
            NoYielder::default(),
        );

        let count_clone = Rc::clone(&counts);
        panic_if_failed!(global.push(Box::new(OnNext::on_next(
            Counter("Counter1", 0, 4, 2),
            move |next, _| count_clone.borrow_mut().push(("Counter1", next)),
            None,
        ))));

        let count_clone2 = Rc::clone(&counts);
        panic_if_failed!(global.push(
            OnNext::on_next(
                Counter("Counter2", 0, 20, 10),
                move |next, _| count_clone2.borrow_mut().push(("Counter2", next)),
                None,
            )
            .into()
        ));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.borrow().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.borrow().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.number_of_sleepers(), 1);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
            ]
        );

        // wait for 5ms and validate we made progress
        tracing::debug!("Sleeping thread for 5ms");
        thread::sleep(time::Duration::from_millis(5));
        tracing::debug!("Finished sleeping thread for 5ms");

        // Counter1 is brought back in as priority
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter1", TaskStatus::Ready(3)),
            ]
        );

        // Counter1 finishes and removed from queue, so count is same.
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter1", TaskStatus::Ready(3)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter1", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(3)),
            ]
        );
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter1", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(4)),
            ]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_4_task_goes_to_sleep_as_lowest_priority_on_wakeup_with_other_tasks() {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<(&'static str, TaskStatus<usize, time::Duration, NoSpawner>)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            NoYielder::default(),
        );

        let count_clone = Rc::clone(&counts);
        panic_if_failed!(global.push(
            OnNext::on_next(
                Counter("Counter1", 0, 4, 2),
                move |next, _| count_clone.borrow_mut().push(("Counter1", next)),
                None
            )
            .into()
        ));

        let count_clone2 = Rc::clone(&counts);
        panic_if_failed!(global.push(
            OnNext::on_next(
                Counter("Counter2", 0, 5, 10),
                move |next, _| count_clone2.borrow_mut().push(("Counter2", next)),
                None
            )
            .into()
        ));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.borrow().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.borrow().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.number_of_sleepers(), 1);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
            ]
        );

        // wait for 5ms and validate we made progress
        tracing::debug!("Sleeping thread for 5ms");
        thread::sleep(time::Duration::from_millis(5));
        tracing::debug!("Finished sleeping thread for 5ms");

        // Counter1 is brought back in as priority
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter2", TaskStatus::Ready(3)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter2", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(4)),
            ]
        );

        // Counter2 triggers Done signal
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter2", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(4)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter2", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(4)),
                ("Counter1", TaskStatus::Ready(3)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::NoWork);

        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter2", TaskStatus::Ready(3)),
                ("Counter2", TaskStatus::Ready(4)),
                ("Counter1", TaskStatus::Ready(3)),
            ]
        );
    }

    #[derive(Clone)]
    enum DaemonSpawner {
        NoSpawning,
        InThread,
        TopOfThread,
        OutofThread,
    }

    impl Default for DaemonSpawner {
        fn default() -> Self {
            Self::NoSpawning
        }
    }

    impl ExecutionAction for DaemonSpawner {
        type Executor = LocalExecutorEngine;

        fn apply(self, key: Entry, executor: Self::Executor) -> crate::valtron::GenericResult<()> {
            match self {
                DaemonSpawner::NoSpawning => Ok(()),
                DaemonSpawner::TopOfThread => {
                    match executor
                        .any_task()
                        .with_parent(key.clone())
                        .with_task(SimpleCounter("SubTask1", 0, 5))
                        .lift()
                    {
                        Ok(_) => Ok(()),
                        Err(err) => Err(Box::new(err)),
                    }
                }
                DaemonSpawner::InThread => {
                    match executor
                        .any_task()
                        .with_task(SimpleCounter("SubTask1", 0, 5))
                        .schedule()
                    {
                        Ok(_) => Ok(()),
                        Err(err) => Err(Box::new(err)),
                    }
                }
                DaemonSpawner::OutofThread => {
                    match executor
                        .any_task()
                        .with_task(SimpleCounter("SubTask2", 0, 5))
                        .broadcast()
                    {
                        Ok(_) => Ok(()),
                        Err(err) => Err(Box::new(err)),
                    }
                }
            }
        }
    }

    struct DaemonCounter(Rc<AtomicUsize>);
    impl TaskIterator for DaemonCounter {
        type Done = ();
        type Pending = ();
        type Spawner = DaemonSpawner;

        fn next(&mut self) -> Option<TaskStatus<Self::Done, Self::Pending, Self::Spawner>> {
            let current = self.0.load(Ordering::SeqCst);
            if current == 0 {
                return Some(TaskStatus::Pending(()));
            }

            if current == 1 {
                return Some(TaskStatus::Ready(()));
            }

            if current == 2 {
                return Some(TaskStatus::Spawn(DaemonSpawner::InThread));
            }

            if current == 3 {
                return Some(TaskStatus::Spawn(DaemonSpawner::TopOfThread));
            }

            Some(TaskStatus::Spawn(DaemonSpawner::OutofThread))
        }
    }

    struct SimpleCounter(&'static str, usize, usize);

    impl TaskIterator for SimpleCounter {
        type Done = usize;
        type Pending = ();
        type Spawner = NoSpawner;

        fn next(&mut self) -> Option<TaskStatus<Self::Done, Self::Pending, Self::Spawner>> {
            let old_count = self.1;
            let new_count = old_count + 1;
            self.1 = new_count;

            tracing::debug!(
                "Counter({}) has current count {} from old count {}",
                self.0,
                new_count,
                old_count,
            );

            if new_count == self.2 {
                tracing::debug!("Counter({}) ending task at {}", self.0, new_count,);
                return None;
            }

            if new_count == 3 {
                tracing::debug!("Counter({}) going to sleep at {}", self.0, new_count,);
                return Some(TaskStatus::Delayed(time::Duration::from_millis(10)));
            }

            Some(TaskStatus::Ready(new_count))
        }
    }

    #[test]
    #[traced_test]
    fn scenario_5_task_can_spawn_task_via_actions() {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<(&'static str, TaskStatus<(), (), DaemonSpawner>)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            NoYielder::default(),
        );

        let gen_state = rc::Rc::new(AtomicUsize::new(0));

        let count_clone = counts.clone();
        panic_if_failed!(executor
            .typed_task()
            .with_task(DaemonCounter(gen_state.clone()))
            .on_next(move |next, _| count_clone.borrow_mut().push(("DaemonCounter", next)))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(counts.borrow().clone(), vec![]);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(1, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(2, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 2);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 5);

        gen_state.store(0, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 5);
    }

    #[test]
    #[traced_test]
    fn scenario_5_task_a_spawns_task_b_that_that_goes_to_sleep_but_also_ties_task_a_to_its_readiness(
    ) {
        let global: Arc<ConcurrentQueue<BoxedLocalExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Rc<RefCell<Vec<(&'static str, TaskStatus<(), (), DaemonSpawner>)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let seed = rand::thread_rng().next_u64();

        let executor = LocalThreadExecutor::from_seed(
            seed,
            global.clone(),
            IdleMan::new(
                3,
                None,
                SleepyMan::new(3, ExponentialBackoffDecider::default()),
            ),
            PriorityOrder::Bottom,
            NoYielder::default(),
        );

        let gen_state = rc::Rc::new(AtomicUsize::new(0));

        let count_clone = counts.clone();
        panic_if_failed!(executor
            .typed_task()
            .with_task(DaemonCounter(gen_state.clone()))
            .on_next(move |next, _| count_clone.borrow_mut().push(("DaemonCounter", next)))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(counts.borrow().clone(), vec![]);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(1, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(3, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 2);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        gen_state.store(1, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_inprocess(), 2);
        assert_eq!(executor.number_of_local_tasks(), 2);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        tracing::debug!("Before: Checking tasks counts");
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        tracing::debug!("After: Checking tasks counts");
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.borrow().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        tracing::debug!("Checking tasks counts");
        assert_eq!(executor.number_of_sleepers(), 1);
        assert_eq!(executor.number_of_inprocess(), 0);
        assert_eq!(executor.number_of_local_tasks(), 2);

        tracing::debug!("Sleep for 10ms");
        thread::sleep(time::Duration::from_millis(10));
        tracing::debug!("Finished sleeping for 10ms");

        assert_eq!(executor.number_of_sleepers(), 1);
        assert_eq!(executor.number_of_inprocess(), 0);
        assert_eq!(executor.number_of_local_tasks(), 2);

        tracing::debug!("Wake up tasks from sleep");
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_inprocess(), 2);
        assert_eq!(executor.number_of_local_tasks(), 2);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_inprocess(), 1);
        assert_eq!(executor.number_of_local_tasks(), 1);

        tracing::debug!("Task A is now ready to continue");
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("DaemonCounter", TaskStatus::Ready(())),
                ("DaemonCounter", TaskStatus::Ready(())),
            ]
        );

        tracing::debug!("Task A is now spawns task to global queue but still continues");
        gen_state.store(4, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_inprocess(), 1);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(1, Ordering::SeqCst);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        tracing::debug!("Task A emits another state");
        assert_eq!(
            counts.borrow().clone(),
            vec![
                ("DaemonCounter", TaskStatus::Ready(())),
                ("DaemonCounter", TaskStatus::Ready(())),
                ("DaemonCounter", TaskStatus::Ready(())),
            ]
        );
    }
}
