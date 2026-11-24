//! Build a generated browser project through its Cargo resource hook.
use foundation_tasks::{ProcessCommand, TaskContext, TaskResult};
use std::path::Path;

/// Build the selected Cargo manifest and stage the finished web assets.
///
/// # Errors
/// Returns the failure from Cargo or the packaging hook.
pub async fn build(
    ctx: &TaskContext,
    manifest: &Path,
    destination: &Path,
    release: bool,
    offline: bool,
) -> TaskResult {
    let mut command = ProcessCommand::new("cargo")
        .args(["build", "--lib", "--manifest-path"])
        .arg(manifest)
        .env("EWE_RESOURCE_TARGET", "web")
        .env("EWE_RESOURCE_DEST", destination);
    if release {
        command = command.arg("--release");
    }
    if offline {
        command = command.arg("--offline").env("EWE_UI_OFFLINE", "1");
    }
    ctx.exec(command).await
}
