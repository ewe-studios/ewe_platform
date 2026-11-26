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
    runner.register(foundation_tasks::install::task_set(vec![
        foundation_tasks::install::Capability {
            name: "platform-codegen",
            package: "foundation_platform",
            version: "0.0.7",
            path: None,
            features: &[],
            function: "codegen::generate_platform_code",
            fallible: false,
        },
        foundation_tasks::install::Capability {
            name: "browser-runtime",
            package: "foundation_wasm_ui",
            version: "0.1.13",
            path: None,
            features: &["build-hook"],
            function: "build_hook::generate_from_cargo",
            fallible: true,
        },
        foundation_tasks::install::Capability {
            name: "resources",
            package: "foundation_resources",
            version: "0.1.1",
            path: None,
            features: &["build"],
            function: "build::stage_from_cargo",
            fallible: true,
        },
    ])?)?;
    foundation_ci::install_host(&mut runner)?;
    runner.register(foundation_ci::set()?)?;
    runner.register(foundation_ci::toolchain::set()?)?;
    runner.register(foundation_ci::wire::set()?)?;
    runner.register(foundation_ci::images::set()?)?;
    runner.register(foundation_ci::development::set()?)?;
    runner.register(foundation_ci::cloudflare::set()?)?;
    runner.register(platform_tasks::set()?)?;
    runner.register(platform_tasks::codegen()?)?;
    runner.register(platform_tasks::wasm_bins()?)?;
    runner.register(platform_tasks::wasm_bundle()?)?;
    foundation_wasm_ui::tasks::install_host(&runner);
    runner.register(foundation_wasm_ui::tasks::set()?)?;
    runner.context().store(ui_todo_tasks::environment());
    let mut native = foundation_ui_native::tasks::release_set()?;
    ui_todo_tasks::compatibility(&mut native)?;
    runner.register(native)?;
    runner.run().await
}
