//! Wait for a guest connection and its provisioning result.
use super::{
    Guest,
    connection::{Connection, command},
    state,
};
use crate::execution::sleep;
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet,
};
use std::time::Duration;

#[derive(Args)]
struct Ready {
    #[command(flatten)]
    connection: Connection,
    #[arg(long, default_value = "pi-vm")]
    name: String,
    /// Require only a successful guest connection.
    #[arg(long)]
    ssh_only: bool,
    #[arg(long, default_value_t = 7_200_000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 30_000, value_parser=clap::value_parser!(u64).range(1..))]
    interval_ms: u64,
    #[arg(long, default_value_t = 300_000, value_parser=clap::value_parser!(u64).range(1..))]
    log_interval_ms: u64,
    #[arg(long, default_value_t = 5)]
    progress_lines: usize,
    #[arg(long, default_value_t = 50)]
    stopped_lines: usize,
}

async fn logs(ctx: &TaskContext, id: &str, count: usize) -> TaskResult {
    ctx.exec(
        ProcessCommand::new("docker")
            .args(["logs", "--tail"])
            .arg(count.to_string())
            .arg(id),
    )
    .await
}

async fn ready(ctx: TaskContext, args: Ready) -> TaskResult {
    let probe = match (args.connection.os, args.ssh_only) {
        (Guest::Windows, true) => "exit 0",
        (Guest::Macos, true) => "true",
        (Guest::Windows, false) => {
            "$ErrorActionPreference = 'Stop'; if (Test-Path 'C:/OEM/status/provisioned.ok') { exit 0 }; if (Test-Path 'C:/OEM/status/provision.failed') { exit 3 }; exit 2"
        }
        (Guest::Macos, false) => {
            "if [ -f /usr/local/platform-images/status/provisioned.ok ]; then exit 0; elif [ -f /usr/local/platform-images/status/provision.failed ]; then exit 3; else exit 2; fi"
        }
    };
    let clock = ctx.context().clock();
    let start = clock.now();
    let mut last_log = start;
    let timeout = Duration::from_millis(args.timeout_ms);
    let interval = Duration::from_millis(args.interval_ms);
    let output = &ctx.require::<TaskEnv>()?.output;
    let Some((id, mut status)) = state(&ctx, &args.name).await? else {
        return Err(TaskError::operation(format!(
            "Missing guest container {}",
            args.name
        )));
    };
    loop {
        if status != "running" {
            logs(&ctx, &id, args.stopped_lines).await?;
            return Err(TaskError::operation(format!(
                "The guest container {} is {status}",
                args.name
            )));
        }
        let code = match ctx.exec(command(&ctx, &args.connection, probe)?).await {
            Ok(()) => {
                return output
                    .write(
                        OutputStream::Stdout,
                        format!("{} is ready\n", args.name).as_bytes(),
                    )
                    .await
                    .map_err(TaskError::operation);
            }
            Err(TaskError::Exit { code: 3, .. }) => {
                return Err(TaskError::operation(format!(
                    "Provisioning failed in {}. Check provision.log",
                    args.name
                )));
            }
            Err(TaskError::Exit {
                code: code @ (2 | 255),
                ..
            }) => code,
            Err(error) => return Err(error),
        };
        let now = clock.now();
        let elapsed = now.saturating_duration_since(start);
        if elapsed >= timeout {
            return Err(TaskError::operation(format!(
                "The guest {} did not become ready within {} ms (SSH status {code})",
                args.name, args.timeout_ms
            )));
        }
        if now.saturating_duration_since(last_log) >= Duration::from_millis(args.log_interval_ms) {
            logs(&ctx, &id, args.progress_lines).await?;
            last_log = now;
        }
        sleep(&ctx, interval.min(timeout.saturating_sub(elapsed))).await?;
        let current = state(&ctx, &args.name).await?;
        match current {
            Some((current_id, current_status)) if current_id == id => status = current_status,
            _ => {
                return Err(TaskError::operation(format!(
                    "The guest container {} was replaced or removed",
                    args.name
                )));
            }
        }
    }
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("vm-wait", "Wait for the local guest to become ready", ready)
}
