#![allow(clippy::too_many_lines)]
#![allow(clippy::type_complexity)]
#![allow(clippy::return_self_not_must_use)]

use std::{
    cell,
    collections::{HashMap, VecDeque},
    rc,
    sync::{
        self,
        atomic::{self, AtomicBool},
        Arc,
    },
    time,
};

use crate::{
    synca::{mpp, DurationWaker, Entry, EntryList, IdleMan, OnSignal, RunOnDrop, Sleepers, Waiter},
    valtron::{AnyResult, ExecutionEngine, ExecutionIterator, State},
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use concurrent_queue::{ConcurrentQueue, PushError};

#[allow(unused)]
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[allow(unused)]
#[cfg(target_arch = "wasm32")]
use wasm_sync::Mutex;

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedSendExecutionIterator, ExecutionAction,
    ExecutionTaskIteratorBuilder, ExecutorError, ProcessController, SharedTaskQueue, TaskIterator,
    TaskReadyResolver, TaskStatusMapper, ThreadActivity,
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SpawnType {
    Lifted,
    LiftedWithParent,
    Broadcast,
    Scheduled,
}

/// Underlying state of an executor.
/// It is the inner structure used by a `LocalExecutorEngine` to manage
/// all relevant tasks it takes on.
///
/// It's important to note that if a task panic, so will the ExecutorState
/// which can't be restored and all tasks owned will be lost, so its important
/// you always handle panic in the actual task itself by using either the `DoNext`,
/// `CollectNext` and `OnNext` and if you implement your own ExecutionIterator to
/// ensure to follow the pattern demonstrated in those implementations to correctly
/// handle panic from tasks even at the trade of some runtime cost from Mutex which
/// allow you use [`std::panic::catch_unwind`].
pub struct ExecutorState {
    /// what priority should waking task be placed.
    pub(crate) priority: PriorityOrder,

    /// global_tasks are the shared tasks coming from the main thread
    /// they generally will always come in fifo order and will be processed
    /// in the order received.
    pub(crate) global_tasks: sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,

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
    pub(crate) local_tasks: rc::Rc<cell::RefCell<EntryList<BoxedExecutionIterator>>>,

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

impl ExecutorState {
    pub fn new(
        global_tasks: sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,
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

impl Clone for ExecutorState {
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

impl ExecutorState {
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
    /// is considered packed.
    #[inline]
    pub fn is_packed(&self, target: &Entry) -> bool {
        *self.packed_tasks.borrow().get(target).unwrap_or(&false)
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

    /// Returns totla
    pub fn total_inprocess_tasks(&self) -> usize {
        self.number_of_inprocess()
    }

    pub fn total_sleeping_tasks(&self) -> usize {
        self.sleepers.count()
    }

    /// Returns the total remaining tasks that are
    /// active and not sleeping.
    pub fn total_active_tasks(&self) -> usize {
        let local_task_count = self.local_tasks.borrow().active_slots();
        let sleeping_task_count = self.sleepers.count();
        let in_process_task_count = self.number_of_inprocess();
        let active_task_count = local_task_count - sleeping_task_count;
        tracing::debug!(
            "Local TaskCount={} and SleepingTaskCount={}, InProcessTasks={}, ActiveTasks={}",
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
                tracing::debug!("Successfully acquired new tasks for processing");
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
                ProgressIndicator::NoWork
            }
            ScheduleOutcome::LocalTaskRunning => {
                tracing::debug!("Invalid state reached, no local task should have been in queue");
                unreachable!("No local task should be running at this point")
            }
        }
    }

    #[inline]
    pub fn schedule_and_do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
        match self.request_global_task() {
            ProgressIndicator::CanProgress => {}
            ProgressIndicator::NoWork => {
                return ProgressIndicator::NoWork;
            }
            ProgressIndicator::SpinWait(_) => {
                unreachable!("Requesting global task should never spin wait")
            }
        }

        self.wakeup_ready_sleepers();

        // reset the spawn_ops to None
        RunOnDrop::new(|| {
            self.spawn_op.borrow_mut().take();
        });

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

    // fn execute_task(
    //     &self,
    //     key: Entry,
    //     iter: BoxedExecutionIterator,
    //     engine: BoxedExecutionEngine,
    // ) -> Option<State> {
    //     let iter_exec = Mutex::new(iter);
    //     match std::panic::catch_unwind(|| iter_exec.lock().unwrap().next(key, engine)) {
    //         Ok(_) => todo!(),
    //         Err(_) => todo!(),
    //     }
    // }

    /// do_work attempts to call the current iterator to progress
    /// executing the next operation internally till it's ready for
    /// work to begin.
    #[inline]
    pub fn do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
        // if after wake up, no task still enters
        // the processing queue then no work is available
        if let Some(inner) = self.check_processing_queue() {
            match inner {
                ProgressIndicator::NoWork => return ProgressIndicator::NoWork,
                ProgressIndicator::CanProgress => return ProgressIndicator::CanProgress,
                _ => unreachable!("check_processing_queue should never reach here"),
            }
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
                            State::Panicked => {
                                tracing::debug!(
                                    "Task just panicked and communicated that with State::Panicked, will remove immediately"
                                );

                                // unpack the entry in the task list
                                self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                                self.local_tasks.borrow_mut().take(&top_entry);

                                // no need to push entry since it must have
                                tracing::debug!("Task is removed from queue due to panic");

                                // Task Iterator is really done
                                if remaining_tasks == 0 {
                                    ProgressIndicator::NoWork
                                } else {
                                    ProgressIndicator::CanProgress
                                }
                            }
                            State::SpawnFinished => {
                                let active_tasks = self.total_active_tasks();
                                let sleeping_tasks = self.total_active_tasks();
                                let in_process_tasks = self.total_inprocess_tasks();

                                tracing::debug!(
                                    "Spawned new task successfully over current {:?} (rem_tasks: {}, active_tasks: {}, sleeping_tasks: {}, in_process_tasks: {})",
                                    &top_entry,
                                    remaining_tasks,
                                    active_tasks,
                                    sleeping_tasks,
                                    in_process_tasks,
                                );

                                // unpack the entry in the task list
                                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                                // unless we just lifted with a parent then add entry back.
                                if let Some(op) = self.spawn_op.borrow().as_ref() {
                                    tracing::info!("Spawned process with op: {:?}", op);
                                    if op != &SpawnType::LiftedWithParent {
                                        // push entry back into processing mut
                                        tracing::info!(
                                            "Adding task {:?} back into top of queue: {:?}",
                                            top_entry,
                                            op
                                        );
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

impl ExecutorState {
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
    pub fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<Entry, ExecutorError> {
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
    pub fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<Entry, ExecutorError> {
        let task_entry = self.local_tasks.borrow_mut().insert(task);
        self.processing.borrow_mut().push_back(task_entry.clone());
        self.spawn_op.borrow_mut().replace(SpawnType::Scheduled);
        Ok(task_entry)
    }
}

impl ExecutorState {
    /// Delivers a task (Iterator) type to the global execution queue
    /// and in such a case you do not have an handle to the task as we
    /// no more have control as to where it gets allocated.
    pub fn broadcast(&self, task: BoxedSendExecutionIterator) -> AnyResult<(), ExecutorError> {
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

pub struct ReferencedExecutorState {
    inner: rc::Rc<ExecutorState>,
    activities: Option<mpp::Sender<ThreadActivity>>,
}

impl From<rc::Rc<ExecutorState>> for ReferencedExecutorState {
    fn from(value: rc::Rc<ExecutorState>) -> Self {
        ReferencedExecutorState {
            inner: value,
            activities: None,
        }
    }
}

impl Clone for ReferencedExecutorState {
    fn clone(&self) -> Self {
        ReferencedExecutorState {
            inner: rc::Rc::clone(&self.inner),
            activities: self.activities.clone(),
        }
    }
}

#[allow(unused)]
impl ReferencedExecutorState {
    pub fn new(
        inner: rc::Rc<ExecutorState>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self { inner, activities }
    }

    pub(crate) fn clone_queue(&self) -> SharedTaskQueue {
        self.inner.global_tasks.clone()
    }

    pub(crate) fn clone_state(&self) -> rc::Rc<ExecutorState> {
        self.inner.clone()
    }

    fn use_activities(&mut self, activities: mpp::Sender<ThreadActivity>) {
        self.activities = Some(activities)
    }

    fn get_ref(&self) -> &rc::Rc<ExecutorState> {
        &self.inner
    }

    fn get_ref_mut(&mut self) -> &mut rc::Rc<ExecutorState> {
        &mut self.inner
    }

    fn number_of_local_tasks(&self) -> usize {
        self.inner.number_of_local_tasks()
    }

    fn number_of_inprocess(&self) -> usize {
        self.inner.number_of_inprocess()
    }
}

impl ReferencedExecutorState {
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

    pub fn schedule_and_do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
        self.inner.schedule_and_do_work(engine)
    }
}

// --- LocalExecutor for ExecutionIterator::Executor

pub struct LocalExecutionEngine {
    inner: rc::Rc<ExecutorState>,
    activities: Option<mpp::Sender<ThreadActivity>>,
}

impl Clone for LocalExecutionEngine {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            activities: self.activities.clone(),
        }
    }
}

impl LocalExecutionEngine {
    pub fn new(
        inner: rc::Rc<ExecutorState>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self { inner, activities }
    }

    #[allow(unused)]
    fn use_activities(&mut self, activities: mpp::Sender<ThreadActivity>) {
        self.activities = Some(activities)
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a> ExecutionEngine for Box<&'a LocalExecutionEngine> {
    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<(), ExecutorError> {
        (**self).lift(task, parent)
    }

    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<(), ExecutorError> {
        (**self).schedule(task)
    }

    fn broadcast(&self, task: BoxedSendExecutionIterator) -> AnyResult<(), ExecutorError> {
        (**self).broadcast(task)
    }

    fn shared_queue(&self) -> SharedTaskQueue {
        (**self).shared_queue()
    }

    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        (**self).rng()
    }
}

impl ExecutionEngine for Box<LocalExecutionEngine> {
    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<(), ExecutorError> {
        (**self).lift(task, parent)
    }

    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<(), ExecutorError> {
        (**self).schedule(task)
    }

    fn broadcast(&self, task: BoxedSendExecutionIterator) -> AnyResult<(), ExecutorError> {
        (**self).broadcast(task)
    }

    fn shared_queue(&self) -> SharedTaskQueue {
        (**self).shared_queue()
    }

    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        (**self).rng()
    }
}

impl ExecutionEngine for LocalExecutionEngine {
    fn lift(
        &self,
        task: BoxedExecutionIterator,
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

    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<(), ExecutorError> {
        let entry = self.inner.schedule(task)?;
        tracing::debug!("schedule: new task into Executor with entry: {:?}", entry);
        Ok(())
    }

    fn broadcast(&self, task: Box<dyn ExecutionIterator + Send>) -> AnyResult<(), ExecutorError> {
        self.inner.broadcast(task)?;
        tracing::debug!("broadcast: new task into Executor");
        if let Some(sender) = &self.activities {
            if let Err(err) = sender.send(ThreadActivity::BroadcastedTask) {
                return Err(ExecutorError::FailedToSendThreadActivity(Box::new(err)));
            }
        }
        Ok(())
    }

    /// shared_queue returns access to the global queue.
    fn shared_queue(&self) -> SharedTaskQueue {
        self.inner.global_tasks.clone()
    }

    /// rng returns the local executor random number generator.
    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.inner.get_rng()
    }
}

impl ReferencedExecutorState {
    #[inline]
    pub fn local_engine(&self) -> LocalExecutionEngine {
        LocalExecutionEngine::new(self.inner.clone(), self.activities.clone())
    }

    #[inline]
    pub fn boxed_engine(&self) -> BoxedExecutionEngine {
        Box::new(self.local_engine())
    }
}

// --- End of: LocalExecutor for ExecutionIterator::Executor

/// # LocalThreadExecutor
///
/// Detailed is the overall design decision selected in the design and implementation of the executor and
/// it's variants
/// where each provides a specific feature, behavior of each type of executor.
///
/// Executors have two core concepts:
///
/// ### Tasks
/// This describes an execution represented by a rust iterator which provides an adequate means of representing an
/// infinite and asynchronous work that needs to be managed unto completion. Tasks generally can have what you would
/// consider one direct dependent task.
///
/// Generally Tasks are Iterators with callbacks that allow tasks to supply their result when the `Iter::next()` is
/// called, which means each result is delivered as received to the callback for processing, but generally you can also
/// do blocking calls like `collect()` that indicate to the executor you are looking to collecting all the result at
/// surface points (entry and exit calls).
///
/// Since all Tasks are really Iterators, most times your implementations will really work at the Iterator level,
/// where you implement iterators or have functions wrapped as iterators from other iterators which means
/// generally asynchronous, stream like behaviour is backed into your implementation by design.
///
///  This does not mean a task can trigger another, in the concept of the executors describe below,
/// Task A can generate a
/// Task B but the relationship between these tasks can only ever be in two modes
///
///  #### Lift
///  This describes the relationship where a task basically says I am dependent on this task completing
/// before I
/// continue, this means when such a command is received, an executed will:
///
///  1. Take the provided task and allocate it to the top of the executing queue
///  2. Take the current task and move it below the newly lifted task
///  3. Create a direct connection from Task A to Task B to directly correlate that a task that was paused
///     or asleep with a direct upward dependency should also be moved into sleep till
///     the task it lifted is done.
///
///  Generally the iron clad rule is a task can never lift up more than one task because its evidently
/// indicating to the executor it wants to prioritize the lifted task above itself,
/// given it the remaining execution slot till that task completes.
///
///
///  Which might seem limiting since then we loose the ability of mutual progress where we want to progress in the same
/// accordance and order (e.g managing memory by iterator just from next to next which allows a more refined memory
/// management than waiting for a list to be completely collected) but  with such a paradigm we simplify the underlying
/// core operations and lift such ability to a higher level above the core behaviour e.g custom iterators
/// that allow such behaviour, though the default callback system we wish to define also allows this with ease.
///
///  #### Schedule
///
///  This describes the ability of tasks to schedule other tasks after them which might not be immediately after them /// (but you get the idea). In such a case, a task is not saying its dependent on this task for its operation even if
/// technically it might be, but rather its clear indicating this is deferrable work it can leave
/// for later handling by the executor and is not urgent to it's executing
/// operation and hence this can be left for later.
///
/// #### Distribute
///
/// This describes the ability of tasks to opt out to instead of executing a task in the local queue of their own
/// executor but instead to send tasks to the global tasks queue which moves those tasks to a new thread which will own
/// them for execution.
///
/// This allows you intentionally opt out of local queue execution, which then requires that task to be `Send`
/// without `Sync` which means what the given task
/// can't keep a reference to any local values in that thread else must copy them.
///
/// But local tasks cant wait for such tasks to finish to continue execution in anyway, its no different
/// from `Schedule` with the difference being explicitly knowing this task will likely
/// not execute in the same thread (depending on what
/// executor is running).
///
/// ### Sleeps
/// This describe the fact that tasks have the ability to communicate their desire to be put to sleep at some point in
/// time until their sleep cycle is over upon which they should be awoken to continue operation.
///
/// ## Executors
///
/// ### LocalThreadExecutor
///
/// A Non threaded foundation executor that executes in the thread it is created in, generally most more complicated
/// executors might use this executor underneath or the ConcurrentExecutor underneath. The core behaviour this executor
/// outlines is that it will only ever execute a singular task unto completion. Now to be clear this does not mean the
/// executor waste cycles by doing nothing when a task in the queue is sleeping, delayed or communicates its unreadiness
/// which then allows the executor prioritize other pending tasks but generally a executing task that can make progress
/// will be executed all the way to completion.
///
/// One core things we must note here is no task should ever block the queue else there will be a deadlock and no work
/// can be done.
///
/// *I see benefit for this type of executor in environments like WebAssembly.*
///
/// Outline below are different scenarios their related expectations for
/// how this executor should behave in implemented:
///
/// #### Scenario Concepts (You will meet)
///
/// Below are concepts you will meet and should keep in mind as you reason about these scenarios:
///
/// - [`PriorityOrder`]: means the executor will ensure to maintain existing priority of a task even if it goes to sleep,
///     when the sleep period has expired no matter if another task is executing that task will be demoted for the previous
///     task to become priority.
///
/// - Task Graph: internally the executor should keep a graph (HashMap really) that maps Task to it's
///     Dependents (the lifter) in this case, this allows us to do the following:
///     1. Task A lifts Task B so we store in Map: {B: Vec[A]}
///     2. Task B lifts Task C so we store in Map: {C: Vec[B], B: Vec[A]}
///     3. With Above we can identify the dependency tree by going Task C -> Task B -> Task A to
///        understand the relationship graph and understand which tasks we need to move out of
///        processing since Task C is now sleeping for some period of time.
///
/// #### Scenario 1: Task A to Completion
/// A scenario where a task can execute to completion.
///
/// 1. Task A gets scheduled by executor and can make progress.
/// 2. Executor executes Task A `next()` checking returned `State` and continues to execute till completion.
///
/// #### Scenario 2: Task A Goes to Sleep (Only task in queue)
/// In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks
/// taking it place to utilize resources better.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A wants to sleep for some duration of time
/// 3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
/// 4. Executor pulls new task from global queue and queues it for execution and progress (with: CoExecutionAllowed).
/// 5. When Task A sleep expires, executor lift Task A as priority with no dependency
///    and continues executing Task A ///(with: PriorityOrder).
///
/// #### Scenario 3: Task A Goes to Sleep (PriorityOrder: On)
/// In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks
/// taking it place to utilize resources better.
///
/// - PriorityOrder: means the executor will ensure to maintain existing priority of a task even if it goes to sleep,
///     when the sleep period has expired no matter if another task is executing that task will be demoted for the previous
///     task to become priority.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A wants to sleep for some duration of time
/// 3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
/// 4. Executor pulls new task from global queue and queues it for execution and progress (with: CoExecutionAllowed).
/// 5. When Task A sleep expires, executor lift Task A as priority with no dependency and
///    continues executing Task A ///(with: PriorityOrder).
///
/// #### Scenario 4:  Task A Goes to Sleep (PriorityOrder: Off)
/// In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks
/// taking it place to utilize resources better.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A wants to sleep for some duration of time
/// 3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
/// 4. Executor pulls new task from global queue and queues it for execution and progress (with: CoExecutionAllowed)
/// 5. When Task A sleep expires, executor schedules Task A to bottom of queue to wait its turn again.
///
/// #### Scenario 5:  Task A spawns Task B which then wants to go to sleep
/// In such a scenario an executing spawns a task as priority that spawns another task as priority which spawns a final
/// one that wishes to sleep.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A spawns Task B as priority
/// 2. Task B wants to sleep for some duration of time
/// 3. Executor removes Task B to Task A due to task graph (Task A -> (depends) Task B) from queue
///    and puts Task B to sleep (register sleep waker) and moves Task A from queue till Task B returns from sleep.
/// 4. Executor goes on to execute other tasks and depending on state of PriorityOrder will either add Task C to Task A
///    back to end of queue or start of queue.
///
/// ##### Dev Notes
/// I am skeptical if this really is of value but for now it will be supported.
///
/// ### Concurrent Executor
///
/// ConcurrentExecutor generally will be executed in a multi-threaded environment which allows more larger control and
/// performance has it can spread tasks to dedicated threads who really are each
/// managing a LocalThreadExecutor in a given
/// thread with CoExecution turned on.
///
/// One thing must be clear, executors can never be sent across threads, they are designed that way, they exists in the
/// thread they were created in which means their overall operations are serialized as you can't have some other thread
/// do something with the task executor that requires `Sync`.
///
/// ConcurrentExecutors have the exact same scenarios as the `LocalThreadExecutor` but it instead coordinates multiple
/// instances of them in as many OS threads as you want.
///
/// ## Higher Order Iterator (HOI) Tasks
///
/// ### Grouped Iterator
///
/// Grouped a HOI task type that sits on the provided executor in that it allows us group a series of tasks in that they
/// make progress together. Unlike TimeSlice iterator where each task in the group gets
/// a `TimeSlice`, the Grouped iterator simply just calls `Iter::next()` on all sub-tasks
///
/// ### TimeSlice Iterator
///
/// TimeSlice iterator implements a variant that sits on the provided executor in that it allows us create a
/// series of sub-tasks that can make progress for a given time slice,
/// noting that it internally manages this tasks in it's `next()` call.
///
/// It achieves this by wrapping each task (basically are type `Iterator`) in a `TimeSlice` structure that grant each
/// iterator a giving time slice within which the task `next()` keeps getting called till the `TimeSlice` issues a
/// State::Reschedule` event that indicates that the executor should move the `TimeSlice` to the bottom of the queue.
///
/// The executor will keep executing those tasks concurrently for their time slice until they reach completion at which
/// point the completed task is removed from the queue and another task is added in.
///
/// #### Scenario 1:  Task A spawns a TimeSlice Iterator type Task with Task [B, C, D]
///
/// In such a scenario an executing spawns the TimeSlice Iterator as a task making progress in the queue and calls
/// it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D)
/// till its TimeSlice wrapper indicates to be
/// rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes.
///
/// The Executor just keeps receiving `State::Progress` from TimeSlice iterator indicating its making progress.
///
/// #### Scenario 2:  Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] but C wants to reschedule
///
/// In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue
/// and calls it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D)
/// till its TimeSlice wrapper indicates
/// to be rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes .
///
/// When sub-task C indicates it wants to reschedule, TimeSlice iterator moves C out into sleep register with a waker
/// and continues executing task (B, D) within their time slices, and till C indicates its
/// ready to make progress ///`State::Progress` to the TimeSlice iterator else keeps skipping C.
///
/// If All sub-task exclaim `State::Reschedule` then TimeSlice forwards that to Executor to reschedule TimeSlice as a
/// whole.
///
/// The Executor just keeps receiving `State::Progress` from TimeSlice iterator indicating its making progress.
///
/// #### Scenario 3:  Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] but C wants to sleep
///
/// In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue and calls
/// it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D) till
/// its TimeSlice wrapper indicates
/// to be rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes .
///
/// When sub-task C indicates it wants to sleep, TimeSlice iterator moves C out into sleep register with a waker and
/// continues executing task (B, D) within their time slices, and till C wakes up, it keeps sending `State::Progress` to
/// executor.
///
/// If sub-task (B, D) finishes before C wakes up then TimeSlice iterator now yields `State::Pending(time::Duration)` to
/// indicate it also should be put to sleep for that given duration time upon which when it
/// wakes will check if it has any
/// sleepers who are ready to be woken and if so moves them into its internal execution queue for progress else issues
/// another `State::Pending(time::Duration)`.
///
/// #### Scenario 4:  Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] and all sub-tasks wants to sleep
///
/// In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in
/// the queue and calls it's `next()` which in turn calls the `next()` method of each
/// sub-tasks (B, C, D) till all task indicates they wish to sleep.
///
/// The TimeSlice iterator then puts all into its internal sleeping tracker and issues
/// `State::Reschedule` until one of the task is ready to make progress.
///
/// #### Scenario 5:  Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] and B wants to lift sub-task
///
/// In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue and which
/// one of the tasks (Task B) would like to lift up another task as priority at which point the TimeSlice iterator will
/// replace Task B positionally with new lifted Task (with the same time slice settings as Task B)  in essence
/// putting Task B to sleep till it's lifted Task is done at which point only will
/// Task B enters the group again to continue execution.
///
/// It continues executing the other tasks in their time slice till their completion, applying same logic to them if
/// they wish to lift their own sub-tasks.
///
pub struct LocalThreadExecutor<T: ProcessController + Clone> {
    kill_signal: Option<Arc<OnSignal>>,
    state: ReferencedExecutorState,
    yielder: T,
}

// --- constructors

impl<T: ProcessController + Clone> Clone for LocalThreadExecutor<T> {
    fn clone(&self) -> Self {
        LocalThreadExecutor {
            state: self.state.clone(),
            yielder: self.yielder.clone(),
            kill_signal: self.kill_signal.clone(),
        }
    }
}

// -- Constructors

#[allow(unused)]
impl<T: ProcessController + Clone> LocalThreadExecutor<T> {
    pub fn new(
        tasks: sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,
        rng: ChaCha8Rng,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
        kill_signal: Option<Arc<OnSignal>>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self {
            yielder,
            kill_signal,
            state: ReferencedExecutorState::new(
                rc::Rc::new(ExecutorState::new(tasks, priority, rng, idler)),
                activities,
            ),
        }
    }

    /// creates a new local executor which uses the provided
    /// seed for ChaCha8Rng generator.
    pub fn from_seed(
        seed: u64,
        tasks: sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
        kill_signal: Option<Arc<OnSignal>>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self::new(
            tasks,
            ChaCha8Rng::seed_from_u64(seed),
            idler,
            priority,
            yielder,
            kill_signal,
            activities,
        )
    }

    /// Allows supplying a custom Rng generator for creating the initial
    /// ChaCha8Rng seed.
    pub fn from_rng<R: rand::Rng>(
        tasks: sync::Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,
        rng: &mut R,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
        kill_signal: Option<Arc<OnSignal>>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self::from_seed(
            rng.next_u64(),
            tasks,
            idler,
            priority,
            yielder,
            kill_signal,
            activities,
        )
    }
}

// -- LocalExecutor builder

#[allow(unused)]
impl<T: ProcessController + Clone> LocalThreadExecutor<T> {
    #[inline]
    pub fn local_engine(&self) -> LocalExecutionEngine {
        self.state.local_engine()
    }

    #[inline]
    pub fn boxed_engine(&self) -> BoxedExecutionEngine {
        self.state.boxed_engine()
    }

    #[inline]
    pub(crate) fn number_of_sleepers(&self) -> usize {
        self.state.number_of_sleepers()
    }

    #[inline]
    pub(crate) fn number_of_local_tasks(&self) -> usize {
        self.state.number_of_local_tasks()
    }

    #[inline]
    pub(crate) fn number_of_inprocess(&self) -> usize {
        self.state.number_of_inprocess()
    }
}

// --- LocalExecutor rng and run methods

#[allow(unused)]
impl<T: ProcessController + Clone> LocalThreadExecutor<T> {
    /// [`get_rng`] returns the `LocalThreadExecutor` random number
    /// generator that allows you generate predictable and repeatable
    /// random numbers consistently where such a property is very useful
    /// e.g When doing [DST](https://docs.tigerbeetle.com/about/vopr/).
    #[inline]
    pub fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.state.get_rng()
    }

    /// [`run_once`] provides a fine-grained control at what you can
    /// think of as a single tick of progress by the executor.
    /// It will ask the executor to make one singular move in work time
    /// allowing the top task to make progress, this is super useful when
    /// you an an environment where fine-grained control is super important
    /// and you do not care about the `ProcessController` or wish to call
    /// and handle that portion yourself unlike in [`block_on`].
    ///
    /// [`block_on`] and [`block_until_finished`] call [`run_once`] internally
    /// as well.
    #[inline]
    pub fn run_once(&self) -> ProgressIndicator {
        let span = tracing::trace_span!("LocalThreadExecutor::run_once");
        let _enter = span.enter();

        tracing::debug!("Creating local executor from state");
        let local_executor = self.state.local_engine();

        tracing::debug!("run: ReferencedExecutorState::schedule_and_do_work with local executor");
        self.state.schedule_and_do_work(Box::new(local_executor))
    }

    /// [`block_until_finished`] defers from [`block_on`] in that it will
    /// continue to execute the executor till the task is ideally finished
    /// and no more work remains in this task.
    ///
    /// This exists for environments like WebAssembly where spawning multiple
    /// threads do not exists but still need a way to make progress for tasks ]
    /// added till they are completed.
    ///
    /// More so, this provides fine grained control to the user to decide at what
    /// point in time they will call the executor to begin progressing on tasks
    /// which allows them to add one or more tasks into the queue before triggering
    /// processing via [`block_until_finished`].
    #[inline]
    pub fn block_until_finished(&self) {
        let span = tracing::trace_span!("LocalThreadExecutor::block_until_finished");
        let _enter = span.enter();

        'main_loop: loop {
            for _ in 0..200 {
                match self.run_once() {
                    ProgressIndicator::NoWork => {
                        break 'main_loop;
                    }
                    ProgressIndicator::SpinWait(duration) => {
                        self.yielder.yield_for(duration);
                        continue;
                    }
                    ProgressIndicator::CanProgress => continue,
                }
            }
            self.yielder.yield_process();
        }
    }

    /// block_on usually should in a separate thread where it will block
    /// the thread until either a panic occurs or the kill signal is sent
    /// to the thread.
    #[inline]
    pub fn block_on(&self) {
        let span = tracing::trace_span!("LocalThreadExecutor::kill");
        let _enter = span.enter();

        /// require kill_signal to be provided.
        let kill_signal = self.kill_signal
            .clone()
            .expect("Calling LocalThreadExecutor::block_on requires kill_signal to be provided for termination");

        loop {
            if kill_signal.probe() {
                tracing::debug!("Received signal to stop and die");
                return;
            }

            for _ in 0..200 {
                if kill_signal.probe() {
                    tracing::debug!("Received signal to stop and die");
                    return;
                }

                match self.run_once() {
                    ProgressIndicator::CanProgress => continue,
                    ProgressIndicator::NoWork => {
                        if kill_signal.probe() {
                            tracing::debug!("Received signal to stop and die");
                            return;
                        }
                        self.yielder.yield_process();
                        continue;
                    }
                    ProgressIndicator::SpinWait(duration) => {
                        if kill_signal.probe() {
                            tracing::debug!("Received signal to stop and die");
                            return;
                        }

                        self.yielder.yield_for(duration);
                        continue;
                    }
                }
            }
            self.yielder.yield_process();
        }
    }
}

// --- Access Methods run once

/// typed_task allows you to create a task builder but requiring specific
/// definitions for your `Task`, `Action` and `Resolver` types.
pub fn typed_task<Task, Action, Resolver>(
    engine: BoxedExecutionEngine,
) -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + 'static>,
    Resolver,
    Task,
>
where
    Task::Ready: Send,
    Task::Pending: Send,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Resolver: TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + 'static,
{
    ExecutionTaskIteratorBuilder::new(engine)
}

/// any_task allows you to create a task builder with less restrictive type
/// requirements for the builder, specifically resolvers are Boxed.
pub fn any_task<Task, Action>(
    engine: BoxedExecutionEngine,
) -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + 'static>,
    Task,
>
where
    Task::Ready: Send,
    Task::Pending: Send,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
{
    ExecutionTaskIteratorBuilder::new(engine)
}

/// send_any_task will unlike [`any_task`] deliver the provided
/// task to the global queue instead of the local queue via the provided
/// `ExecutionEngine`.
pub fn send_any_task<Task, Action>(
    engine: BoxedExecutionEngine,
) -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Ready: Send,
    Task::Pending: Send,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
{
    ExecutionTaskIteratorBuilder::new(engine)
}

/// send_typed_task will unlike [`type_task`] deliver the provided
/// typed task to the global queue instead of the local queue via the provided
/// `ExecutionEngine`.
pub fn send_typed_task<Task, Action, Resolver>(
    engine: BoxedExecutionEngine,
) -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
    Resolver,
    Task,
>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Resolver: TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static,
{
    ExecutionTaskIteratorBuilder::new(engine)
}

#[cfg(test)]
mod test_local_thread_executor {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread,
        time::Duration,
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
    use rand::prelude::*;
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
        type Ready = usize;
        type Spawner = NoSpawner;
        type Pending = time::Duration;

        fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
    fn scenario_0_can_kill_local_executor_via_kill_signal() {
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();
        let kill_signal = Arc::new(OnSignal::new());

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
            Some(kill_signal.clone()),
            None,
        );

        let count_clone = Arc::clone(&counts);
        panic_if_failed!(send_typed_task(executor.boxed_engine())
            .with_task(Counter("Counter1", 0, 3, 3))
            .on_next(move |next, _| count_clone.lock().unwrap().push(next))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        let thread_handle = thread::spawn(move || {
            tracing::debug!("Starting sleep timer for 100ms to kill executor");
            thread::sleep(Duration::from_millis(100));
            tracing::debug!("Trigger kill signal");
            kill_signal.turn_on();
            tracing::debug!("Executor kill signal sent");
        });

        tracing::debug!("Blocking main thread with block_on()");
        executor.block_on();
        tracing::debug!("Thread released from block_on()");

        thread_handle.join().expect("should end correctly");
    }

    #[test]
    #[traced_test]
    fn scenario_one_task_a_runs_to_completion() {
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let count_clone = Arc::clone(&counts);
        let on_next = OnNext::on_next(
            Counter("Counter1", 0, 3, 3),
            move |next, _engine| count_clone.lock().unwrap().push(next),
            None,
        );

        panic_if_failed!(global.push(on_next.into()));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
        assert_eq!(executor.number_of_sleepers(), 0);

        let count_list = counts.lock().unwrap().clone();
        assert_eq!(
            count_list,
            vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_one_can_use_local_executor_builder_to_queue_task() {
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let count_clone = Arc::clone(&counts);
        panic_if_failed!(send_typed_task(executor.boxed_engine())
            .with_task(Counter("Counter1", 0, 3, 3))
            .on_next(move |next, _| count_clone.lock().unwrap().push(next))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(executor.run_once(), ProgressIndicator::NoWork);
        assert_eq!(executor.number_of_sleepers(), 0);

        let count_list = counts.lock().unwrap().clone();
        assert_eq!(
            count_list,
            vec![TaskStatus::Ready(1), TaskStatus::Ready(2),]
        );
    }

    #[test]
    #[traced_test]
    fn scenario_2_task_a_goes_to_sleep_as_only_task_in_queue() {
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<TaskStatus<usize, time::Duration, NoSpawner>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let count_clone = Arc::clone(&counts);
        panic_if_failed!(global.push(Box::new(OnNext::on_next(
            Counter("Counter1", 10, 20, 12),
            move |next, _engine| { count_clone.lock().unwrap().push(next) },
            None,
        ))));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(counts.lock().unwrap().clone(), vec![TaskStatus::Ready(11),]);

        assert_eq!(
            executor.run_once(),
            ProgressIndicator::SpinWait(time::Duration::from_millis(5))
        );
        assert_eq!(counts.lock().unwrap().clone(), vec![TaskStatus::Ready(11),]);

        // wait for 5ms and validate we made progress
        thread::sleep(time::Duration::from_millis(5));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![TaskStatus::Ready(11), TaskStatus::Ready(13),]
        );
        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.lock().unwrap().clone(),
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
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration, NoSpawner>)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let count_clone = Arc::clone(&counts);
        panic_if_failed!(global.push(Box::new(OnNext::on_next(
            Counter("Counter1", 0, 4, 2),
            move |next, _| count_clone.lock().unwrap().push(("Counter1", next)),
            None,
        ))));

        let count_clone2 = Arc::clone(&counts);
        panic_if_failed!(global.push(
            OnNext::on_next(
                Counter("Counter2", 0, 20, 10),
                move |next, _| count_clone2.lock().unwrap().push(("Counter2", next)),
                None,
            )
            .into()
        ));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.number_of_sleepers(), 1);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter1", TaskStatus::Ready(3)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<usize, time::Duration, NoSpawner>)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let count_clone = Arc::clone(&counts);
        panic_if_failed!(global.push(
            OnNext::on_next(
                Counter("Counter1", 0, 4, 2),
                move |next, _| count_clone.lock().unwrap().push(("Counter1", next)),
                None
            )
            .into()
        ));

        let count_clone2 = Arc::clone(&counts);
        panic_if_failed!(global.push(
            OnNext::on_next(
                Counter("Counter2", 0, 5, 10),
                move |next, _| count_clone2.lock().unwrap().push(("Counter2", next)),
                None
            )
            .into()
        ));

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("Counter1", TaskStatus::Ready(1)),]
        );

        assert_eq!(executor.number_of_sleepers(), 1);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
            vec![
                ("Counter1", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(1)),
                ("Counter2", TaskStatus::Ready(2)),
                ("Counter2", TaskStatus::Ready(3)),
            ]
        );

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
        fn apply(
            self,
            key: Entry,
            executor: BoxedExecutionEngine,
        ) -> crate::valtron::GenericResult<()> {
            match self {
                DaemonSpawner::NoSpawning => Ok(()),
                DaemonSpawner::TopOfThread => {
                    tracing::debug!("Spawning task as TopOfThread");
                    match any_task(executor)
                        .with_parent(key.clone())
                        .with_task(SimpleCounter("SubTask1", 0, 5))
                        .lift()
                    {
                        Ok(_) => Ok(()),
                        Err(err) => Err(Box::new(err)),
                    }
                }
                DaemonSpawner::InThread => {
                    tracing::debug!("Spawning task as InThread");
                    match any_task(executor)
                        .with_task(SimpleCounter("SubTask1", 0, 5))
                        .schedule()
                    {
                        Ok(_) => Ok(()),
                        Err(err) => Err(Box::new(err)),
                    }
                }
                DaemonSpawner::OutofThread => {
                    tracing::debug!("Spawning task as OutOfThread");
                    match send_any_task(executor)
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

    struct DaemonCounter(Arc<AtomicUsize>);
    impl TaskIterator for DaemonCounter {
        type Ready = ();
        type Pending = ();
        type Spawner = DaemonSpawner;

        fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
        type Ready = usize;
        type Pending = ();
        type Spawner = NoSpawner;

        fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<(), (), DaemonSpawner>)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let gen_state = Arc::new(AtomicUsize::new(0));

        let count_clone = counts.clone();
        panic_if_failed!(send_typed_task(executor.boxed_engine())
            .with_task(DaemonCounter(gen_state.clone()))
            .on_next(move |next, _| count_clone.lock().unwrap().push(("DaemonCounter", next)))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(counts.lock().unwrap().clone(), vec![]);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(1, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(2, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 2);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
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
        let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::bounded(10));

        let counts: Arc<Mutex<Vec<(&'static str, TaskStatus<(), (), DaemonSpawner>)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let seed = rand::rng().next_u64();

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
            None,
            None,
        );

        let gen_state = Arc::new(AtomicUsize::new(0));

        let count_clone = counts.clone();
        panic_if_failed!(send_typed_task(executor.boxed_engine())
            .with_task(DaemonCounter(gen_state.clone()))
            .on_next(move |next, _| count_clone.lock().unwrap().push(("DaemonCounter", next)))
            .broadcast());

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);

        assert_eq!(counts.lock().unwrap().clone(), vec![]);

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(1, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 1);

        gen_state.store(3, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        assert_eq!(executor.number_of_sleepers(), 0);
        assert_eq!(executor.number_of_local_tasks(), 2);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
            vec![("DaemonCounter", TaskStatus::Ready(())),]
        );

        gen_state.store(1, Ordering::SeqCst);

        assert_eq!(executor.run_once(), ProgressIndicator::CanProgress);
        assert_eq!(
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
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
            counts.lock().unwrap().clone(),
            vec![
                ("DaemonCounter", TaskStatus::Ready(())),
                ("DaemonCounter", TaskStatus::Ready(())),
                ("DaemonCounter", TaskStatus::Ready(())),
            ]
        );
    }
}
