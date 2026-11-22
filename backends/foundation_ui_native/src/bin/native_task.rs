use foundation_tasks::{task, TaskResult, TaskRunner};
use foundation_ui_native::tasks::{self, NativeBuildEnv};

#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    runner.context().store(NativeBuildEnv::from_workspace());
    runner.register(tasks::set()?)?;
    runner.run().await
}
