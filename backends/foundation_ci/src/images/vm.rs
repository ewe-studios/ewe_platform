//! Start and stop a local virtual machine host container by its exact identity.
mod connection;
mod exec;
mod logs;
mod provision;
mod ready;
mod runner;
mod transfer;

use crate::{CiEnvironment, workspace::capture};
use clap::{Args, ValueEnum};
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet,
};
use std::{
    ffi::OsString,
    fs,
    io::{BufRead, BufReader, Read, Seek},
    path::PathBuf,
};

#[derive(Clone, Copy, ValueEnum)]
enum Guest {
    Windows,
    Macos,
}

#[derive(Args)]
struct Container {
    /// Docker container name.
    #[arg(long, default_value = "pi-vm")]
    name: String,
    /// Time for an orderly guest shutdown before Docker stops the container.
    #[arg(long, default_value_t = 120)]
    stop_seconds: u32,
}

#[derive(Args)]
struct Up {
    #[command(flatten)]
    container: Container,
    /// Guest operating system.
    #[arg(long = "os", value_enum)]
    _os: Guest,
    /// Docker image with the guest installer or the installed disk.
    #[arg(long)]
    image: String,
    /// Host directory for persistent guest storage.
    #[arg(long)]
    storage: PathBuf,
    /// SSH private key path. The host reads only its public companion.
    #[arg(long)]
    key: PathBuf,
    #[arg(long, default_value_t = 2222, value_parser=clap::value_parser!(u16).range(1..))]
    port: u16,
    #[arg(long, default_value_t = 8006, value_parser=clap::value_parser!(u16).range(1..))]
    web_port: u16,
    #[arg(long, default_value = "/dev/kvm")]
    kvm_device: PathBuf,
    #[arg(long, default_value = "/dev/net/tun")]
    tun_device: PathBuf,
    #[arg(long, default_value = "")]
    version: String,
    #[arg(long, default_value = "")]
    ram: String,
    #[arg(long, default_value = "")]
    cpus: String,
    #[arg(long, default_value = "")]
    disk: String,
}

async fn state(ctx: &TaskContext, name: &str) -> Result<Option<(String, String)>, TaskError> {
    if !name
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
    {
        return Err(TaskError::operation(
            "Invalid virtual machine container name",
        ));
    }
    let cmd = ProcessCommand::new("docker")
        .args([
            "container",
            "ls",
            "--all",
            "--no-trunc",
            "--format",
            "{{.ID}} {{.State}}",
            "--filter",
        ])
        .arg(format!("name=^/{}$", regex::escape(name)));
    let file = capture(ctx, cmd).await?;
    let mut lines = BufReader::new(file).lines();
    let Some(first) = lines.next().transpose().map_err(TaskError::operation)? else {
        return Ok(None);
    };
    let (id, status) = first
        .split_once(' ')
        .ok_or_else(|| TaskError::operation("Docker returned an invalid container record"))?;
    if id.len() != 64
        || !id.bytes().all(|c| c.is_ascii_hexdigit())
        || status.is_empty()
        || lines
            .next()
            .transpose()
            .map_err(TaskError::operation)?
            .is_some()
    {
        return Err(TaskError::operation(
            "Docker returned an ambiguous container record",
        ));
    }
    Ok(Some((id.into(), status.into())))
}

fn mount(path: &std::path::Path, destination: &str) -> OsString {
    let mut arg = path.as_os_str().to_owned();
    arg.push(":");
    arg.push(destination);
    arg
}

async fn up(ctx: TaskContext, args: Up) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    if !args.kvm_device.try_exists().map_err(TaskError::operation)? {
        return Err(TaskError::operation(format!(
            "The KVM device {} is unavailable",
            args.kvm_device.display()
        )));
    }
    if !args.tun_device.try_exists().map_err(TaskError::operation)? {
        return Err(TaskError::operation(format!(
            "The network tunnel device {} is unavailable",
            args.tun_device.display()
        )));
    }
    let mut public = host.root.join(&args.key).into_os_string();
    public.push(".pub");
    let key = fs::read_to_string(public).map_err(TaskError::operation)?;
    let key = key.trim();
    if key.is_empty() || key.lines().count() != 1 {
        return Err(TaskError::operation("Expected one public SSH key"));
    }
    let directory = host.root.join(&args.storage);
    fs::create_dir_all(&directory).map_err(TaskError::operation)?;
    let directory = fs::canonicalize(directory).map_err(TaskError::operation)?;
    if let Some((id, status)) = state(&ctx, &args.container.name).await? {
        if matches!(status.as_str(), "running" | "paused" | "restarting") {
            return Err(TaskError::operation(format!(
                "Container {} is already active",
                args.container.name
            )));
        }
        ctx.exec(ProcessCommand::new("docker").args(["rm", &id]))
            .await?;
    }
    let mut cmd = ProcessCommand::new("docker")
        .current_dir(&host.root)
        .args(["run", "--detach", "--name"])
        .arg(args.container.name)
        .arg("--device")
        .arg(mount(&args.kvm_device, "/dev/kvm"))
        .arg("--device")
        .arg(mount(&args.tun_device, "/dev/net/tun"))
        .args(["--cap-add", "NET_ADMIN", "--stop-timeout"])
        .arg(args.container.stop_seconds.to_string())
        .arg("--publish")
        .arg(format!("127.0.0.1:{}:22", args.port))
        .arg("--publish")
        .arg(format!("127.0.0.1:{}:8006", args.web_port))
        .arg("--volume")
        .arg(mount(&directory, "/storage"))
        .arg("--env")
        .arg(format!("PI_SSH_AUTHORIZED_KEY={key}"));
    for (name, value) in [
        ("VERSION", args.version),
        ("RAM_SIZE", args.ram),
        ("CPU_CORES", args.cpus),
        ("DISK_SIZE", args.disk),
    ] {
        if !value.is_empty() {
            cmd = cmd.arg("--env").arg(format!("{name}={value}"));
        }
    }
    ctx.exec(cmd.arg("--").arg(args.image)).await
}

async fn down(ctx: TaskContext, args: Container) -> TaskResult {
    if let Some((id, _)) = state(&ctx, &args.name).await? {
        ctx.exec(
            ProcessCommand::new("docker")
                .args(["stop", "--time"])
                .arg(args.stop_seconds.to_string())
                .arg(&id),
        )
        .await?;
        ctx.exec(ProcessCommand::new("docker").args(["rm", &id]))
            .await?;
    } else {
        ctx.require::<TaskEnv>()?
            .output
            .write(
                OutputStream::Stdout,
                format!("no container {}\n", args.name).as_bytes(),
            )
            .await
            .map_err(TaskError::operation)?;
    }
    Ok(())
}

/// Keep an SSH connection failure as its own status. Reuse the guest message for other failures.
///
/// The captured output holds the command's short diagnostic.
pub(super) async fn failure(
    ctx: &TaskContext,
    action: &str,
    error: TaskError,
    mut capture: fs::File,
) -> TaskError {
    let mut text = String::new();
    if capture.rewind().is_ok() {
        let _ = capture.read_to_string(&mut text);
    }
    let text = text.trim();
    if let TaskError::Exit { program, code: 255 } = error {
        if !text.is_empty() {
            if let Ok(env) = ctx.require::<TaskEnv>() {
                let _ = env
                    .output
                    .write(OutputStream::Stderr, format!("{text}\n").as_bytes())
                    .await;
            }
        }
        return TaskError::Exit { program, code: 255 };
    }
    if text.is_empty() {
        TaskError::operation(format!("{action}: {error}"))
    } else {
        TaskError::operation(format!("{action}: {text}"))
    }
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    logs::register(set)?;
    provision::register(set)?;
    ready::register(set)?;
    runner::register(set)?;
    transfer::register(set)?;
    set.add("vm-exec", "Run a script in the guest", exec::run)?;
    set.add(
        "vm-runner",
        "Start the GitHub Actions runner in the guest",
        runner::start,
    )?;
    set.add("vm-up", "Start the virtual machine container", up)?;
    set.add(
        "vm-down",
        "Stop and remove the virtual machine container",
        down,
    )?;
    Ok(())
}
