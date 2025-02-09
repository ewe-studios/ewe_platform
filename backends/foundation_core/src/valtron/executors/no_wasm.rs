#![cfg(not(target_arch = "wasm32"))]

use super::{
    ExecutionAction, TaskIterator, TaskReadyResolver, TaskStatusMapper, ThreadPoolTaskBuilder,
};

use ctrlc;

/// `spawn` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<
    Task::Done,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Done, Task::Pending, Task::Spawner> + Send + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Done, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Done: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
{
    todo!()
}

/// `spawn2` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you to provide types for both Mapper and Resolver.
pub fn spawn2<Task, Action, Mapper, Resolver>(
) -> ThreadPoolTaskBuilder<Task::Done, Task::Pending, Action, Mapper, Resolver, Task>
where
    Task::Done: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Task::Done, Task::Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Task::Done, Task::Pending> + Send + 'static,
{
    todo!()
}

pub fn block_on(seed_from_rng: u64) {
    todo!()
}
