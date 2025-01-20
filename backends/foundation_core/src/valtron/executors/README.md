# Executors

Detailed is the overall design decision selected in the design and implementation of the executor and it's variants where each provides a specific feature, behavior of each type of executor.

Executors have two core concepts:

### Tasks
This describes an execution represented by a rust iterator which provides an adequate means of representing an infinite and asynchronous work that needs to be managed unto completion. Tasks generally can have what you would consider one direct dependent task.

Generally Tasks are Iterators with callbacks that allow tasks to supply their result when the `Iter::next()` is called, which means each result is delivered as received to the callback for processing, but generally you can also do blocking calls like `collect()` that indicate to the executor you are looking to collecting all the result at surface points (enty and exit calls).

Since all Tasks are really Iterators, most times your implementations will really work at the Iterator level, where you implement iterators or have functions wrapped as iterators from other iterators which means generally asynchronous, strema like behaviour is backed into your implementation by design.

This does not mean a task can trigger another, in the concept of the executors describe below, Task A can generate a Task B but the relationship between these tasks can only ever be in two modes

#### Lift
This describes the relationship where a task basically says I am dependent on this task completing before I continue, this means when such a command is received, an executed will:

1. Take the provided task and allocate it to the top of the executing queue
2. Take the current task and move it below the newly lifted task
3. Create a direct connection from Task A to Task B to directly correlate that a task that was paused or asleep with a direct upward dependency should also be moved into sleep till the task it lifted is done.

Generally the iron clad rule is a task can never lift up more than one task because its evidently indicating to the executor it wants to priortize the lifted task above itself, given it the remaining execution slot till that task completes.

Which might seem limiting since then we loose the ability of mutual progress where we want to progress in the same accordance and order (e.g managing memory by iterator just from next to next which allows a more refined memory management than waiting for a list to be completely collected) but  with such a paradigm we simplify the underlying core operations and lift such ability to a higher level above the core behaviour e.g custom iterators that allow such behaviour, though the default callback system we wish to define also allows this with ease.

#### Schedule

This describes the ability of tasks to schedule other tasks after them which might not be immediately after them (but you get the idea). In such a case, a task is not saying its dependent on this task for its operation even if technically it might be, but rather its clear indicating this is deferrable work it can leave for later handling by the executor and is not urgent to it's executing operation and hence this can be left for later.

#### Distribute

This describes the ability of tasks to opt out to instead of executing a task in the local queue of their own executor but instead to send tasks to the global tasks queue which moves those tasks to a new thread which will own them for execution.

This allows you intentionally opt out of local queue execution, which then requires that task to be `Send` without `Sync` which means what the given task can't keep a reference to any local values in that thread else must copy them.

But local tasks cant wait for such tasks to finish to continue execution in anyway, its no different from `Schedule` with the difference being explicity knowing this task will likely not execute in the same thread (depending on what executor is running).

### Sleeps
This describe the fact that tasks have the ability to communicate their desire to be put to sleep at some point in time until their sleep cycle is over upon which they should be awoken to continue operation.

## Executors

### LocalThreadExecutor

A Non threaded foundation executor that executes in the thread it is created in, generally most more complicated executors might use this executor underneath or the ConcurrentExecutor underneath. The core behaviour this executor outlines is that it will only ever execute a singular task unto completion. Now to be clear this does not mean the executor waste cycles by doing nothing when a task in the queue is sleeping, delayed or communicates its unreadiness which then allows the executor prioritize other pending tasks but generally a executing task that can make progress will be executed all the way to completion.

One core things we must note here is no task should ever block the queue else there will be a deadlock and no work
can be done.

I see benefit for this type of executor in enviroments like WebAssembly.

Outline below are different scenarious and expectations for how the executor should work and the overall behaviour to be implemented:

#### Task A to Completion
A scenario where a task can execute to completion.

1. Task A gets scheduled by executor and can make progress.
2. Executor executes Task A `next()` checking returned `State` and continues to execute till completion.

#### Task A Goes to Sleep (PriorityOrder: On)
In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks taking it place to utilize resources better.

- PriorityOrder: means the executor will ensure to maintain existing priority of a task even if it goes to sleep, when the sleep period has expired no matter if another task is executing that task will be demoted for the previous task to become priority.

##### Scenario
1. Task A gets scheduled by executor and can make progress
2. Task A wants to sleep for some duration of time
3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
4. Executor pulls new task from global queue and queues it for execution and progress (with: CoExecutionAllowed).
5. When Task A sleep expires, executor lift Task A as priority with no dependency and continues executing Task A (with: PriorityOrder).

#### Task A Goes to Sleep (PriorityOrder: Off)
In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks taking it place to utilize resources better.

##### Scenario
1. Task A gets scheduled by executor and can make progress
2. Task A wants to sleep for some duration of time
3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
4. Executor pulls new task from global queue and queues it for execution and progress (with: CoExecutionAllowed)
5. When Task A sleep expires, executor schedules Task A to bottom of queue to wait its turn again.


#### Task A spawns Task B which then Spawns Task C that wants to go to sleep
In such a scenario an executing spawns a task as priority that spawns another task as priority which spawns a final one that wishes to sleep.

##### Scenario
1. Task A gets scheduled by executor and can make progress
2. Task A spawns Task B as priority
2. Task B spawns Task C as priority
2. Task C wants to sleep for some duration of time
3. Executor removes Task C to Task A due to task graph (Task A -> (depends) Task B -> (depends) Task C) from queue and puts Task C to sleep (register sleep waker) and moves Task A and B from queue till Task C returns from sleep.
4. Executor goes on to execute other tasks and depending on state of PriorityOrder will either add Task C to Task A back to end of queue or start of queue.

##### Concept

- Task Graph: internally the executor should keep a graph (HashMap really) that maps Task to it's Dependents (the lifter) in this case, this allows us to do the following:

1. Task A lifts Task B so we store in Map: {B: Vec[A]}
2. Task B lifts Task C so we store in Map: {C: Vec[B], B: Vec[A]}
3. With Above we can identify the dependency tree by going Task C -> Task B -> Task A to understand the relationship graph and understand which tasks we need to move out of processing since Task C is now sleeping for some period of time.

##### Dev Notes
I am skeptical if this really is of value but for now it will be supported.


### Concurrent Executor

ConcurrentExecutor generally will be executed in a multi-threaded environment which allows more larger control and performance has it can spread tasks to dedicated threads who really are each managing a LocalThreadExecutor in a given thread with CoExecution turned on.

One thing must be clear, executors can never be sent across threads, they are designed that way, they exists in the
thread they were created in which means their overall operations are serialized as you can't have some other thread do something with the task executor that requires `Sync`.

ConcurrentExecutors have the exact same scenarios as the `LocalThreadExecutor` but it instead coordinates multiple instances of them in as many OS threads as you want.

## Higher Order Iterator (HOI) Tasks

### Grouped Iterator

Grouped a HOI task type that sits on the provided executor in that it allows us group a series of tasks in that they make progress together. Unlike TimeSlice iterator where each task in the group gets a `TimeSlice`, the Grouped iterator simply just calls `Iter::next()` on all sub-tasks

### TimeSlice Iterator

TimeSlice iterator implements a variant that sits on the provided executor in that it allows us create a series of sub-tasks that can make progress for a given time slice, noting that it internally manages this tasks in it's `next()` call.

It achieves this by wrapping each task (basically are type `Iterator`) in a `TimeSlice` structure that grant each iterator a giving time slice within which the task `next()` keeps getting called till the `TimeSlice` issues a `State::Reschedule` event that indicates that the executor should move the `TimeSlice` to the bottom of the queue.

The executor will keep executing those tasks concurrently for their time slice until they reach completion at which point the completed task is removed from the queue and another task is added in.

#### Task A spawns a TimeSlice Iterator type Task with Task [B, C, D]

In such a scenario an executing spawns the TimeSlice Iterator as a task making progress in the queue and calls it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D) till its TimeSlice wrapper indicates to be
rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes.

The Executor just keeps receiving `State::Progress` from TimeSlice iterator indicating its making progress.

#### Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] but C wants to reschedule

In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue and calls it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D) till its TimeSlice wrapper indicates to be
rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes .

When sub-task C indicates it wants to reschedule, TimeSlice iterator moves C out into sleep register with a waker and continues executing task (B, D) within their time slices, and till C indicates its ready to make progress `State::Progress` to the TimeSlice iterator else keeps skipping C.

If All sub-task exclaim `State::Reschedule` then TimeSlice forwards that to Executor to reschedule TimeSlice as a whole.

The Executor just keeps receiving `State::Progress` from TimeSlice iterator indicating its making progress.

#### Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] but C wants to sleep

In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue and calls it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D) till its TimeSlice wrapper indicates to be
rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes .

When sub-task C indicates it wants to sleep, TimeSlice iterator moves C out into sleep register with a waker and continues executing task (B, D) within their time slices, and till C wakes up, it keeps sending `State::Progress` to executor.

If sub-task (B, D) finishes before C wakes up then TimeSlice iterator now yields `State::Pending(time::Duration)` to indicate it also should be put to sleep for that given duration time upon which when it wakes will check if it has any sleepers who are ready to be woken and if so moves them into its internal execution queue for progress else issues another `State::Pending(time::Duration)`.

#### Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] and all sub-tasks wants to sleep

In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue and calls it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D) till all task indicates they wish to sleep.

The TimeSlice iterator then puts all into its internal sleeping tracker and issues `State::Reschedule` until one of the task is ready to make progress.

#### Task A spawns a TimeSlice Iterator type Task with Task [B, C, D] and B wants to lift a new sub-task

In such a scenario an executing task spawns the TimeSlice Iterator as a task making progress in the queue and which one of the tasks (Task B) would like to lift up another task as priority at which point the TimeSlice iterator will replace Task B positionally with new lifted Task (with the same time slice settings as Task B)  in essence putting Task B to sleep till it's lifted Task is done at which point only will Task B enters the group again to continue execution.

It continues executing the other tasks in their time slice till their completion, applying same logic to them if they wish to lift their own sub-tasks.
