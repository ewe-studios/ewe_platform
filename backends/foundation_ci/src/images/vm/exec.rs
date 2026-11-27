//! Run a script in the selected guest through its loopback port.
use super::{
    connection::{command, Connection},
    failure, Guest,
};
use crate::{workspace::capture_to, CiEnvironment};
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult};
use std::{fs, io, path::PathBuf};

#[derive(Args)]
pub(super) struct Exec {
    #[command(flatten)]
    connection: Connection,
    /// Remote working directory.
    #[arg(long)]
    cwd: String,
    /// Host file that holds the commands to run.
    #[arg(long)]
    script: PathBuf,
}

/// Build the host file that becomes the guest script.
fn stage(body: &mut fs::File, prologue: &str) -> Result<tempfile::NamedTempFile, TaskError> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().map_err(TaskError::operation)?;
    file.write_all(prologue.as_bytes())
        .map_err(TaskError::operation)?;
    io::copy(body, file.as_file_mut()).map_err(TaskError::operation)?;
    file.as_file_mut().flush().map_err(TaskError::operation)?;
    Ok(file)
}

pub(super) async fn run(ctx: TaskContext, args: Exec) -> TaskResult {
    let script = ctx.require::<CiEnvironment>()?.root.join(&args.script);
    let mut body = fs::File::open(&script).map_err(TaskError::operation)?;
    let (remote, cwd) = match args.connection.os {
        Guest::Windows => ("C:/platform-images/exec.ps1", args.cwd.replace('\'', "''")),
        Guest::Macos => (
            "/tmp/platform-images-exec.sh",
            args.cwd.replace('\'', "'\\''"),
        ),
    };
    let prologue = match args.connection.os {
        Guest::Windows => format!(
            "$ErrorActionPreference = 'Stop'\n$PSNativeCommandUseErrorActionPreference = $true\nSet-Location -LiteralPath '{cwd}'\n"
        ),
        Guest::Macos => format!("set -euo pipefail\ncd '{cwd}'\n"),
    };
    let host = stage(&mut body, &prologue)?;
    let write = match args.connection.os {
        Guest::Windows => {
            format!("Set-Content -Path '{remote}' -Value ([Console]::In.ReadToEnd())")
        }
        Guest::Macos => format!("cat > '{remote}'"),
    };
    let capture = tempfile::tempfile().map_err(TaskError::operation)?;
    let write = command(&ctx, &args.connection, &write)?.file_input(host.path());
    if let Err(error) = capture_to(
        &ctx,
        write,
        capture.try_clone().map_err(TaskError::operation)?,
        true,
    )
    .await
    {
        if matches!(error, TaskError::Cancelled) {
            return Err(error);
        }
        return Err(failure(
            &ctx,
            "writing the script into the guest failed",
            error,
            capture,
        )
        .await);
    }
    // The command streams both output streams and propagates the guest exit status.
    let run = match args.connection.os {
        Guest::Windows => format!("pwsh -NoProfile -File '{remote}'"),
        Guest::Macos => format!("bash -l '{remote}'"),
    };
    ctx.exec(command(&ctx, &args.connection, &run)?).await
}
