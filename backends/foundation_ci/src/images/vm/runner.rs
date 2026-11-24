//! Check the runner process in a guest through its loopback port.
use super::Guest;
use crate::{CiEnvironment, execution::sleep};
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet,
};
use std::{path::PathBuf, time::Duration};

#[derive(Args)]
struct Connection {
    #[arg(long, value_enum)]
    os: Guest,
    #[arg(long)]
    key: PathBuf,
    #[arg(long, default_value_t = 2222, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    /// Maximum time to establish a loopback connection.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    connect_seconds: u32,
}

#[derive(Args)]
struct Wait {
    #[command(flatten)]
    connection: Connection,
    /// Maximum wait for the runner, in milliseconds.
    #[arg(long, default_value_t = 21_600_000)]
    timeout_ms: u64,
    /// Time between process checks, in milliseconds.
    #[arg(long, default_value_t = 30_000, value_parser = clap::value_parser!(u64).range(1..))]
    interval_ms: u64,
}

async fn running(ctx: &TaskContext, connection: &Connection) -> Result<bool, TaskError> {
    let host = ctx.require::<CiEnvironment>()?;
    // Each probe uses exit code 44 only after a successful process search finds no runner.
    let (default_user, probe) = match connection.os {
        Guest::Macos => (
            "ci",
            "pgrep -f '[R]unner.Listener' >/dev/null; s=$?; if [ \"$s\" = 1 ]; then exit 44; fi; exit \"$s\"",
        ),
        Guest::Windows => (
            "Docker",
            "$ErrorActionPreference = 'Stop'; if (Get-Process | Where-Object ProcessName -eq 'Runner.Listener') { exit 0 }; exit 44",
        ),
    };
    let user = host
        .text("PI_VM_USER")
        .unwrap_or_else(|| default_user.into());
    let command = ProcessCommand::new("ssh")
        .arg("-i")
        .arg(host.root.join(&connection.key))
        .arg("-p")
        .arg(connection.port.to_string())
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "LogLevel=ERROR",
            "-o",
        ])
        .arg(format!("ConnectTimeout={}", connection.connect_seconds))
        .args(["-o", "ServerAliveInterval=30", "-l"])
        .arg(user)
        .args(["127.0.0.1", probe]);
    match ctx.exec(command).await {
        Ok(()) => Ok(true),
        Err(TaskError::Exit { code: 44, .. }) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn alive(ctx: TaskContext, args: Connection) -> TaskResult {
    let line: &[u8] = if running(&ctx, &args).await? {
        b"true\n"
    } else {
        b"false\n"
    };
    ctx.require::<TaskEnv>()?
        .output
        .write(OutputStream::Stdout, line)
        .await
        .map_err(TaskError::operation)
}

async fn wait(ctx: TaskContext, args: Wait) -> TaskResult {
    let clock = ctx.context().clock();
    let start = clock.now();
    let timeout = Duration::from_millis(args.timeout_ms);
    let interval = Duration::from_millis(args.interval_ms);
    while running(&ctx, &args.connection).await? {
        let elapsed = clock.now().saturating_duration_since(start);
        if elapsed >= timeout {
            return Err(TaskError::operation("The guest runner is still busy"));
        }
        sleep(&ctx, interval.min(timeout.saturating_sub(elapsed))).await?;
    }
    ctx.require::<TaskEnv>()?
        .output
        .write(OutputStream::Stdout, b"runner finished\n")
        .await
        .map_err(TaskError::operation)
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("vm-runner-alive", "Check the guest runner process", alive)?;
    set.add(
        "vm-wait-runner",
        "Wait for the guest runner to finish",
        wait,
    )
}
