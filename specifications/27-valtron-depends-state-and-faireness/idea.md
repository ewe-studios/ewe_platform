Files Context:
  - /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/valtron/task.rs
  - /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/valtron/executors/local.rs
  - /home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/synca/sleepers.rs

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
