//! Repository tasks owned by the browser user interface.
pub mod application;
mod npm;
mod npm_release;
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult, TaskSet};
use std::path::PathBuf;
use std::prelude::rust_2021::*;

type Lookup = dyn Fn(&str) -> Option<std::ffi::OsString> + Send + Sync;

/// Read a host variable only when a task requests it.
pub struct BrowserTaskEnv(pub std::sync::Arc<Lookup>);

/// Supply the process environment to the browser task host.
pub fn install_host(runner: &foundation_tasks::TaskRunner) {
    if runner.context().get::<BrowserTaskEnv>().is_none() {
        runner
            .context()
            .store(BrowserTaskEnv(std::sync::Arc::new(|key| {
                std::env::var_os(key)
            })));
    }
}

#[derive(Args)]
pub(crate) struct RuntimeArgs {
    /// Directory that contains runtime-entry.js.
    #[arg(long, default_value = "backends/foundation_wasm_ui/runtimes")]
    runtime_dir: PathBuf,
    /// Destination directory. The default is the runtime source directory.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Compare both generated files with the destination.
    #[arg(long)]
    check: bool,
}
impl RuntimeArgs {
    pub(crate) fn generate(&self) -> std::io::Result<()> {
        super::runtime_build::generate(
            &self.runtime_dir,
            self.output.as_deref().unwrap_or(&self.runtime_dir),
            self.check,
        )?;
        Ok(())
    }
}

async fn runtime(_: TaskContext, args: RuntimeArgs) -> TaskResult {
    blocking::unblock(move || args.generate())
        .await
        .map_err(TaskError::operation)
}

/// Export the browser runtime task for an executable host.
///
/// # Errors
/// Returns an error for a conflicting task name or path.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_wasm_ui").with_namespace("wasm-bundle");
    set.add(
        "runtime",
        "Generate or verify the classic browser runtime scripts",
        runtime,
    )?;
    set.expose("runtime", &["wasm-bundle", "runtime"])?;
    npm::register(&mut set)?;
    npm_release::register(&mut set)?;
    Ok(set)
}
