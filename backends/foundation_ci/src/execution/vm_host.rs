//! Verify a virtual machine host container before running its connectivity tests.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    io::{BufRead, BufReader},
    time::Duration,
};

#[derive(Args)]
struct VmHost {
    service: String,
    container: String,
    web_port: u16,
    #[arg(default_value_t = 600)]
    boot_seconds: u64,
    #[arg(long, default_value_t = 10)]
    boot_poll_seconds: u64,
    #[arg(long, default_value_t = 6)]
    missing_attempts: usize,
    #[arg(long, default_value_t = 180)]
    console_seconds: u64,
    #[arg(long, default_value_t = 5)]
    console_poll_seconds: u64,
    #[arg(long, default_value_t = 100)]
    log_lines: usize,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "vm-host-smoke",
        "Verify the virtual machine host container",
        run,
    )?;
    set.expose("vm-host-smoke", &["vm-host-smoke"])
}

async fn check(ctx: &TaskContext, args: &VmHost, compose: &ProcessCommand) -> TaskResult {
    let clock = ctx.context().clock();
    let start = clock.now();
    let mut missing = 0;
    loop {
        if clock.now().saturating_duration_since(start) >= Duration::from_secs(args.boot_seconds) {
            return Err(TaskError::operation(format!(
                "{} did not start in {}s",
                args.container, args.boot_seconds
            )));
        }
        let state = capture(
            ctx,
            ProcessCommand::new("docker").args([
                "inspect",
                "-f",
                "{{.State.Running}}",
                &args.container,
            ]),
        )
        .await;
        match state {
            Ok(file) => {
                let mut status = String::new();
                BufReader::new(file)
                    .read_line(&mut status)
                    .map_err(TaskError::operation)?;
                if status.trim() == "true" {
                    break;
                }
                return Err(TaskError::operation(format!(
                    "{} is not running",
                    args.container
                )));
            }
            Err(TaskError::Exit { .. }) => {
                missing += 1;
                if missing >= args.missing_attempts {
                    return Err(TaskError::operation(format!(
                        "{} does not exist",
                        args.container
                    )));
                }
            }
            Err(error) => return Err(error),
        }
        super::sleep(ctx, Duration::from_secs(args.boot_poll_seconds)).await?;
    }
    let start = clock.now();
    let port = args.web_port.to_string();
    loop {
        let result = ctx
            .exec(ProcessCommand::new("bash").args([
                "-c",
                r#"exec 3<>"/dev/tcp/127.0.0.1/$1""#,
                "vm-console",
                &port,
            ]))
            .await;
        match result {
            Ok(()) => break,
            Err(TaskError::Exit { .. })
                if clock.now().saturating_duration_since(start)
                    < Duration::from_secs(args.console_seconds) =>
            {
                super::sleep(ctx, Duration::from_secs(args.console_poll_seconds)).await?;
            }
            Err(error) => return Err(error),
        }
    }
    ctx.exec(ProcessCommand::new("docker").args([
        "exec",
        &args.container,
        "test",
        "-f",
        "/shared/Cargo.toml",
    ]))
    .await?;
    ctx.exec(compose.clone().arg("ps")).await?;
    println!("The {} VM host passed the smoke test", args.service);
    Ok(())
}

async fn run(ctx: TaskContext, args: VmHost) -> TaskResult {
    if args.missing_attempts == 0 || args.boot_poll_seconds == 0 || args.console_poll_seconds == 0 {
        return Err(TaskError::operation(
            "Polling intervals and missing attempts must be positive",
        ));
    }
    let host = ctx.require::<CiEnvironment>()?;
    let compose = ProcessCommand::new("docker")
        .current_dir(&host.root)
        .args(["compose", "-f"])
        .arg(
            host.text("COMPOSE_FILE")
                .unwrap_or_else(|| "docker-compose.yaml".into()),
        );
    ctx.exec(compose.clone().args(["up", "-d", &args.service]))
        .await?;
    match check(&ctx, &args, &compose).await {
        Ok(()) => Ok(()),
        Err(TaskError::Cancelled) => Err(TaskError::Cancelled),
        Err(error) => {
            let mut result = error;
            for (title, cmd) in [
                ("docker compose ps", compose.arg("ps")),
                (
                    "docker logs",
                    ProcessCommand::new("docker").args([
                        "logs",
                        "--tail",
                        &args.log_lines.to_string(),
                        &args.container,
                    ]),
                ),
            ] {
                println!("::group::{title}");
                if let Err(diagnostics) = ctx.exec(cmd).await {
                    result = TaskError::Cleanup {
                        source: Box::new(result),
                        cleanup: Box::new(diagnostics),
                    };
                }
                println!("::endgroup::");
            }
            Err(result)
        }
    }
}
