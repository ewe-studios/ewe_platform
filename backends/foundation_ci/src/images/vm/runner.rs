//! Start and check the runner process in a guest through its loopback port.
use super::{
    connection::{command, Connection},
    failure, Guest,
};
use crate::{
    execution::sleep,
    workspace::{capture, capture_to},
    CiEnvironment,
};
use clap::Args;
use foundation_tasks::{OutputStream, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet};
use std::{fs, io::Read, path::PathBuf, time::Duration};

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

#[derive(Args)]
pub(super) struct Runner {
    #[command(flatten)]
    connection: Connection,
    /// Host file with the encoded JIT configuration.
    #[arg(long)]
    jitconfig_file: PathBuf,
    /// Time to wait before the first runner check, in milliseconds. Default 20000.
    #[arg(long, default_value_t = 20_000)]
    start_wait_ms: u64,
    /// Log lines to report when the runner exits at once. Default 30.
    #[arg(long, default_value_t = 30)]
    log_lines: usize,
}

// The JIT config goes into a file only SYSTEM and Administrators can read.
// A small launcher passes the file to run.cmd. The `set /p` command would cut it short.
const WINDOWS_START: &[&str] = &[
    "$jit = [Console]::In.ReadToEnd().Trim()",
    "Set-Content -Path C:/actions-runner/.jitconfig -Value $jit -NoNewline",
    "icacls C:/actions-runner/.jitconfig /inheritance:r /grant 'SYSTEM:F' /grant 'Administrators:F' | Out-Null",
    "Set-Content -Path C:/actions-runner/run-jit.ps1 -Value '$jit = (Get-Content -Raw C:/actions-runner/.jitconfig).Trim(); & C:/actions-runner/run.cmd --jitconfig $jit *> C:/actions-runner/runner.log; exit $LASTEXITCODE'",
    "$action = New-ScheduledTaskAction -Execute 'C:\\Program Files\\PowerShell\\7\\pwsh.exe' -Argument '-NoProfile -ExecutionPolicy Bypass -File C:\\actions-runner\\run-jit.ps1' -WorkingDirectory 'C:\\actions-runner'",
    "$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest",
    "$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero)",
    "Register-ScheduledTask -TaskName platform-images-runner -Action $action -Principal $principal -Settings $settings -Force | Out-Null",
    "Start-ScheduledTask -TaskName platform-images-runner",
];

// `;`, not `&&`, before the `&`: a trailing `&` backgrounds the whole chain.
const MACOS_START: &str = r#"umask 077; cd /usr/local/actions-runner || exit 1; cat > .jitconfig || exit 1; [ -s .jitconfig ] || { echo 'empty JIT config' >&2; exit 1; }; nohup bash -lc './run.sh --jitconfig "$(cat .jitconfig)"' > runner.log 2>&1 < /dev/null & echo started"#;

pub(super) async fn running(ctx: &TaskContext, connection: &Connection) -> Result<bool, TaskError> {
    // Each probe uses exit code 44 only after a successful process search finds no runner.
    let probe = match connection.os {
        Guest::Macos => {
            "pgrep -f '[R]unner.Listener' >/dev/null; s=$?; if [ \"$s\" = 1 ]; then exit 44; fi; exit \"$s\""
        }
        Guest::Windows => {
            "$ErrorActionPreference = 'Stop'; if (Get-Process | Where-Object ProcessName -eq 'Runner.Listener') { exit 0 }; exit 44"
        }
    };
    match ctx.exec(command(ctx, connection, probe)?).await {
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

pub(super) async fn start(ctx: TaskContext, args: Runner) -> TaskResult {
    let jit = ctx
        .require::<CiEnvironment>()?
        .root
        .join(&args.jitconfig_file);
    let jit = fs::read_to_string(jit).map_err(TaskError::operation)?;
    let jit = jit.trim();
    if jit.is_empty() {
        return Err(TaskError::operation(format!(
            "{} is empty",
            args.jitconfig_file.display()
        )));
    }
    let start = match args.connection.os {
        Guest::Windows => WINDOWS_START.join("; "),
        Guest::Macos => MACOS_START.to_owned(),
    };
    let output = tempfile::tempfile().map_err(TaskError::operation)?;
    let start = command(&ctx, &args.connection, &start)?.private_input(jit.as_bytes().to_vec());
    if let Err(error) = capture_to(
        &ctx,
        start,
        output.try_clone().map_err(TaskError::operation)?,
        true,
    )
    .await
    {
        if matches!(error, TaskError::Cancelled) {
            return Err(error);
        }
        return Err(failure(&ctx, "starting the runner failed", error, output).await);
    }
    sleep(&ctx, Duration::from_millis(args.start_wait_ms)).await?;
    if !running(&ctx, &args.connection).await? {
        let tail = match args.connection.os {
            Guest::Windows => format!(
                "Get-Content C:/actions-runner/runner.log -Tail {}",
                args.log_lines
            ),
            Guest::Macos => format!(
                "tail -{} /usr/local/actions-runner/runner.log",
                args.log_lines
            ),
        };
        let mut log = capture(&ctx, command(&ctx, &args.connection, &tail)?).await?;
        let mut text = String::new();
        log.read_to_string(&mut text)
            .map_err(TaskError::operation)?;
        return Err(TaskError::operation(format!(
            "The runner exited right after starting:\n{text}"
        )));
    }
    ctx.require::<TaskEnv>()?
        .output
        .write(OutputStream::Stdout, b"runner started\n")
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
