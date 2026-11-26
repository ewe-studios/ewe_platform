use foundation_tasks::{task, TaskResult, TaskRunner};
#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    runner.context().store(ui_todo_tasks::environment());
    runner.register(ui_todo_tasks::set()?)?;
    runner.run().await
}
