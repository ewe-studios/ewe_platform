use foundation_tasks::{task, TaskResult, TaskRunner};
mod generate;
mod local;
mod models;
mod platform_tasks;
mod tcp_capture;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    runner = runner.with_name("platform", "The Ewe platform toolset");
    runner.register(platform_tasks::set()?)?;
    runner.register(platform_tasks::codegen()?)?;
    runner.register(platform_tasks::wasm_bins()?)?;
    runner.register(platform_tasks::wasm_bundle()?)?;
    runner.register(foundation_wasm_ui::tasks::set()?)?;
    runner
        .context()
        .store(foundation_ui_native::tasks::NativeBuildEnv::from_workspace());
    runner.register(foundation_ui_native::tasks::set()?)?;
    runner.run().await
}
