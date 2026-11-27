//! Collect the host and guest logs without retaining their bodies in memory.
use super::{
    connection::{command, Connection},
    state, Guest,
};
use crate::{workspace::capture_to, CiEnvironment};
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Logs {
    #[command(flatten)]
    connection: Connection,
    #[arg(long, default_value = "pi-vm")]
    name: String,
    #[arg(long)]
    out: PathBuf,
}

async fn store(
    ctx: &TaskContext,
    dir: &Path,
    name: &str,
    cmd: ProcessCommand,
    combine: bool,
) -> TaskResult {
    let file = tempfile::NamedTempFile::new_in(dir).map_err(TaskError::operation)?;
    capture_to(
        ctx,
        cmd,
        file.reopen().map_err(TaskError::operation)?,
        combine,
    )
    .await?;
    file.persist(dir.join(name)).map_err(TaskError::operation)?;
    Ok(())
}

async fn run(ctx: TaskContext, args: Logs) -> TaskResult {
    let out = ctx.require::<CiEnvironment>()?.root.join(args.out);
    fs::create_dir_all(&out).map_err(TaskError::operation)?;
    let (id, _) = state(&ctx, &args.name)
        .await?
        .ok_or_else(|| TaskError::operation(format!("Missing guest container {}", args.name)))?;
    store(
        &ctx,
        &out,
        "container.log",
        ProcessCommand::new("docker").args(["logs", &id]),
        true,
    )
    .await?;
    let paths = match args.connection.os {
        Guest::Windows => [
            "C:/OEM/status/provision.log",
            "C:/OEM/status/versions.txt",
            "C:/actions-runner/runner.log",
        ],
        Guest::Macos => [
            "/tmp/platform-images-provision.log",
            "/usr/local/platform-images/status/versions.txt",
            "/usr/local/actions-runner/runner.log",
        ],
    };
    for (source, name) in paths
        .into_iter()
        .zip(["provision.log", "versions.txt", "runner.log"])
    {
        let probe = match args.connection.os {
            Guest::Windows => format!(
                "$ErrorActionPreference = 'Stop'; if (!(Test-Path -LiteralPath '{source}')) {{ exit 44 }}; $file = [IO.File]::OpenRead('{source}'); try {{ $file.CopyTo([Console]::OpenStandardOutput()) }} finally {{ $file.Dispose() }}"
            ),
            Guest::Macos => {
                format!("if [ ! -e '{source}' ]; then exit 44; fi; exec cat '{source}'")
            }
        };
        match store(
            &ctx,
            &out,
            name,
            command(&ctx, &args.connection, &probe)?,
            false,
        )
        .await
        {
            Ok(()) => {}
            Err(TaskError::Exit { code: 44, .. }) => {
                match fs::remove_file(out.join(name)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(TaskError::operation(error)),
                }
                ctx.require::<TaskEnv>()?
                    .output
                    .write(
                        OutputStream::Stdout,
                        format!("(no {source} in the VM)\n").as_bytes(),
                    )
                    .await
                    .map_err(TaskError::operation)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("vm-logs", "Collect host and guest diagnostic files", run)
}
