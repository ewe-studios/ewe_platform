Files Context:
  - /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/valtron/task.rs
  - /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/valtron/executors/local.rs
  - /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/synca/sleepers.rs
  -  /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/valtron/executors/threads.rs

## Idea 1: TaskStatus - Add Depends state

I have been thinking about the fact valtron's task status delayed state which is great to elaborate the need for a task to wait a giving duration before it can continue this provides us a great way to not endless wait cpu and with the improvements in /home/darkvoid/Boxxed/@dev/ewe_platform/specifications/23-valtron-executor-deep-dive and /home/darkvoid/Boxxed/@dev/ewe_platform/specifications/25-valtron-quality-improvements, we better improved task sleeping and thread yielding without alot of wasted cpu spinning go on.

But there is still one more situation that is not accounted for. A task may not always and evidently correctly perform well if it uses TaskStatus::Delayed when it may undershoot or overshoot the condition it needs to wait for, timing may not be the issue or the best way to communicate this to the execution engine which will wake it up.

Worst if we depend on timing, we may miss important signals that require low latency executions. But we do not want to bring in more heavy burden logic for these even channels or mutex guarded condvars.

So we will extend TaskStatus with a Depends state, this state will have a Arc<AtomicBool> value:

pub enum TaskStatus {

  ...

  Depends(Arc<AtomicBool>)

  ...
}

The idea is simple:

1. A task that has some dependent operation it needs to wait for can (however it will do so, spawn a thread, send something to some other service, not our business) will yield the new TaskStatus::Depends which will contain a Arc<AtomicBool>.

The atomic bool must be in a false state, if it returns one in a true state then its no different than it just returned TaskStatus::Pending and in such a case, the task will be treated as such and called again in the next loop.

But if the conditions are met, the  atomic bool is false, then we will add the task entry in local.rs into a new hasmap representing Entry (entry id for the task as key) and Arc<AtomicBool> as value.

Just like how we do with the sleepers, before the next sequence begin, we check if:

1. Sleepers are ready to wakeup and return to processing queue
2. SignalWaiters (what we call the second group), have their Arc<AtomicBool> changed from a false state to a true state, we dont care how many times it changes, we just care when we check, is it now true and if so, consider its time to wakeup the task and add it back to the processing queue just like the sleepers.

We will add a new construct like the Sleepers in /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/synca/sleepers.rs into the /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/synca/ module which will own and manager these signal waiters and will keep the internal hashmap containing the tasks ids and atomic bool signals and will provide three methods:

Note: foundation_errstack is used for all errors

### add(entry_id, Arc<AtomicBool>) -> Result<bool, ErrorReport<CustomErrror>>
Register a new task id and atomic bool, ensuring the key does not exists before, else returns an Err() if key already is taken.

### update(entry_id, Arc<AtomicBool>) -> Result<bool, ErrorReport<CustomErrror>>
Update an new task id with a new atomic bool, ensuring the key does exists before, else returns an Err() if key already is taken.

### get_ready -> Option<Vec<Entry>>
Returns a Some(Vec<Entry>) for all the entries whoes Arc<AtomicBool> are now true during checking (ensuring to use the right atomic ordering) to ensure strong read after write guarantees (not sequential but one that works) and removes these entries from the hashmap (in essence de-registers them). If none, returns None.


In the Local Executor's `ExecutorState` we had this new type and use it to track these SignalWaiters and add them back into the queue when the get_ready() returns us tasks that are ready to go back into the processing queue.

This way we support tasks that will happily signal their readiness without using TaskStatus::Duration.

We understand this will not kickstart them immediately has they must wait for each run of the schedule_and_do_work but that is ok.


## Idea 2: Task workers faireness

I have been thinking today about our discussion on task distribution faireness in valtron. valtron does not implement a work stealing process but rather lets a task be owned by its original thread until its completion.

A task can sleep freeing a thread to take on additional work but once is awake resumes ownership and processing on that thread depending on its wake up semantics e,g lift (put to the top), schedule (put at the bottom).

But its seems reasonable to improve fearness guarantees to some extent, I know we cant overcome and guarantee faireness all the time e.g

Every task a thread takes on goes to sleep for an extended period of time till it ends up taking alot more tasks that most other threads, starving others of work.

So we want to implement a light faireness tracker at the time of getting new work from the global queue, this will control if a thread/worker gets a new tasks from the global queue depending on its currently owned tasks, whats tasks are sleeping or with the new signal waiters, waiting on signals and what other workers are free for work.

I initially wanted a construct that has a hashmap with a wrapper Mutex or RWMutex to improve reads and ony block on writes but the truth is this process will be write heavy than read heavy has workers will consistently update the owning construct about their state on every new run of schedule_and_do_work(..) which owns the loop for every execution of a valtron LocalThreadExecutor (the core worker in valtron).

So i realized we have another way, one that is ideal for such situation, that may not gurantee 100% fiareness and might have a little skew but not too much to affect faireness of work distribution but also allow heavy writes and reads via CAS.

So instead every LocalThreadExecutor executor will have a field called the worker_state that is a AtomicValue<T> where T is a construct we will create that records: e.g WorkerStats

1. Total tasks owned by worker
2. Total active tasks owned by worker
3. Total tasks sleeping
4. Total tasks waiting for signal (signal watchers)
5. Total tasks awaiting awake (total sleepers + total signal watchers waiters)
7. Lastest total time of oldest tasks in worker, for each task we store when it was taken and remove it when it finishes, we can create a task time watcher that has method to add a task Entry, Instant time, then it can provide methods to tell us whats the oldest tasks in the list that is still present (we should also add this construct to /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/synca) then others can use it. Once every tasks finishes it is removed, it can then internally record how quickly a tasks was removed (which tells us how long it takes to execute to finish) and tell us the average time each tasks for a giving worker LocalTaskExecutor really takes to know where long running tasks have likely gone to.


This will be stored via Atomic CAS to the AtomicValue, i will prefer we put this new construct in /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_nostd and have it have generic types to support what is stored and provide  convenience functions to update an AtomicValue successfully (we can take lessons from ConcurrentQueue on how to do this successfully and efficiently), this way it can be used by others as well.

The AtomicValue will contain a Option<T>, and we ensure in threads.rs that a worker when it dies will have its atomic construct replace with a None, this ensures the tracker when it seems a workers tracker handle contain None, will just remove it from its list/hashmap internally and not consider it anymore.

Now we will create a secondary construct that the ThreadRegistry will and will be in a Arc<Trackers> (Trackers) being the one tracking the state of all workers and it stores it as the workers AtomicValue storage construct that contains  the Option<WorkerStats> which must always be a Some(WorkerStats) while the worker is alive.

Important: Before the worker runs any process in schedule_and_do_work(), it will call a method to get its current WorkerState, and update its own stat tracker with the latest state.

This ensures every workers state is update to date before it does anything.

Then in the function of the LocalThreadExecutor to pull a new tasks from the global queue, it calls the global Arc<Trackers> which will have a method like:

Trackers::can_take_worker(worker_entry_id) - with its entry id representing the worker so the tracker can know which worker it is and who is asking.

In my head, the tracker will follow the following priority rules:

1. The workers with zero tasks take top precendence, so a worker with existing tasks gets told No, until all workers have taken on tasks
2. if all workers have work, then the worker with the no waiters+ sleepers win, since it means it has not taken on more than it can chew. 
2. if all workers have work, and tasks with sleepers and signal waiters, then the worker with the least sleeping + waiting for wake up signal gets the task
3. If all 3 conditions are true, has worker, as active tasks, has sleepers + signal waiters, then the one with worker with the shortest tasks execution time wins, because the time tells us where the ones with long running work went to and we can prioritize those with the least tasks total execution time  to not send tasks to workers with long running jobs that will cause more starvation since a tasks will likely never get to finish with another tasks owning the worker.


Its important to note, valtron never lets workers take on more tasks that 1, but because a task can sleep and be waiting for a signal, and to optimize workers usage, workers are allowed to pick 1 more tasks so they are not idle, this means there is a chance for starvation to happen.

So the goal of this is to add some fiareness component in there to reduce this.


Review these two new addition and lets expand it, fill areas that are lacking or need improvement and once its all good with the user, lets spawn the specification management agents in ./agents/agents to create a spec  within this directory with features.
