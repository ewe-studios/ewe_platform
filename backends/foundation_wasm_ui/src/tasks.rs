//! Repository tasks owned by the browser user interface.
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult, TaskSet};
use std::path::PathBuf;
use std::prelude::rust_2021::*;

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
    Ok(set)
}
