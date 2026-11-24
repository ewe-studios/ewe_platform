use foundation_tasks::{TaskResult, TaskRunner, task};

#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    runner = runner.with_name("foundation-ci", "Plan and run workspace checks");
    foundation_ci::install_host(&mut runner)?;
    runner.register(foundation_ci::set()?)?;
    runner.register(foundation_ci::toolchain::set()?)?;
    runner.register(foundation_ci::wire::set()?)?;
    runner.register(foundation_ci::images::set()?)?;
    runner.register(foundation_ci::development::set()?)?;
    runner.run().await
}
