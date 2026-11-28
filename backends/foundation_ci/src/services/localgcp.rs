//! Run localgcp, a local Google Cloud emulator, as a task.
//!
//! WHY: the Google Cloud Key Management Service (KMS) gRPC tests of
//! `foundation_gcp_kms` need a real service. localgcp (MIT license) emulates
//! Cloud KMS over gRPC on a local port, with no credential and no bill, so a
//! free continuous integration (CI) job runs the same tests as a developer.
//!
//! WHAT: `services localgcp` downloads the pinned release (`VERSION`) for the
//! host, checks its sha256 against the release `checksums.txt`, starts
//! `localgcp up --no-docker`, saves the process ID, and waits for the KMS
//! port. `--keep-running` leaves it up. `services localgcp-stop` stops the
//! saved process ID.
//!
//! HOW: the task process interface starts a long-lived service, so the
//! emulator runs as a child of the task until the task detaches it. The saved
//! process ID is the child's own ID, not a search for a program name, so the
//! stop action never stops an unrelated process.

use std::fs;
use std::io::Read as _;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput, TaskResult, TaskSet,
};
use futures_util::future::BoxFuture;
use sha2::Digest as _;

use crate::{execution::sleep, CiEnvironment};

/// The pinned release of localgcp.
const VERSION: &str = "0.6.0";

#[derive(Args)]
struct Start {
    /// Leave the emulator running after the task exits.
    /// `services localgcp-stop` stops it.
    #[arg(long)]
    keep_running: bool,
    /// The port of the Cloud KMS emulator.
    #[arg(long, default_value_t = 8091, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    /// The longest wait for the KMS port to answer.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_seconds: u64,
    /// The interval between port checks.
    #[arg(long, default_value_t = 250, value_parser = clap::value_parser!(u64).range(1..))]
    poll_milliseconds: u64,
    /// An installed localgcp binary. The task then skips the download.
    #[arg(long)]
    binary: Option<PathBuf>,
    /// The release to download when `--binary` is absent.
    #[arg(long, default_value = VERSION)]
    release: String,
}

#[derive(Args)]
struct Stop {
    /// The longest wait for the emulator to exit.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_seconds: u64,
    /// The interval between process checks.
    #[arg(long, default_value_t = 250, value_parser = clap::value_parser!(u64).range(1..))]
    poll_milliseconds: u64,
}

/// Register the `services localgcp` task and its stop action.
///
/// # Errors
/// Returns an error when a command name is invalid or duplicated.
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "localgcp",
        "Start localgcp, the local Google Cloud emulator",
        start,
    )?;
    set.add(
        "localgcp-stop",
        "Stop the localgcp emulator by its saved process ID",
        stop,
    )?;
    // The path aliases that the plan and the workflow use. The namespace also
    // names them `services:localgcp` for `task run`.
    set.expose("localgcp", &["services", "localgcp"])?;
    set.expose("localgcp-stop", &["services", "localgcp-stop"])?;
    Ok(())
}

/// The directory that holds the downloaded release and the process ID file.
fn control(ctx: &TaskContext) -> Result<PathBuf, TaskError> {
    Ok(ctx.require::<CiEnvironment>()?.root.join(".localgcp"))
}

/// The process ID file of the running emulator.
fn pid_file(dir: &Path) -> PathBuf {
    dir.join("localgcp.pid")
}

/// The name of the release archive for this host.
///
/// # Errors
/// Returns an error for a host that the release does not name.
fn asset(version: &str) -> Result<String, TaskError> {
    let host = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin_arm64",
        ("macos", "x86_64") => "darwin_amd64",
        ("linux", "aarch64") => "linux_arm64",
        ("linux", "x86_64") => "linux_amd64",
        ("windows", _) => {
            return Err(TaskError::operation(
                "the localgcp Windows release is a zip; pass --binary with an installed localgcp.exe",
            ));
        }
        (os, arch) => {
            return Err(TaskError::operation(format!(
                "localgcp has no release for {os}-{arch}"
            )));
        }
    };
    Ok(format!("localgcp_{version}_{host}.tar.gz"))
}

/// The sha256 of a file, as lowercase hexadecimal text.
fn sha256_file(path: &Path) -> Result<String, TaskError> {
    let mut file = fs::File::open(path).map_err(TaskError::operation)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(TaskError::operation)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// The expected sha256 of `asset` from the release `checksums.txt`.
fn expected_checksum(text: &str, asset: &str) -> Result<String, TaskError> {
    text.lines()
        .find_map(|line| {
            let (hash, name) = line.split_once("  ")?;
            (name.trim() == asset).then(|| hash.trim().to_string())
        })
        .ok_or_else(|| TaskError::operation(format!("{asset} is not in the release checksums")))
}

/// Download, verify and unpack the release, and return the binary path.
async fn install(ctx: &TaskContext, dir: &Path, version: &str) -> Result<PathBuf, TaskError> {
    let asset = asset(version)?;
    let version_dir = dir.join(version);
    let binary = version_dir.join("localgcp");
    if binary.is_file() {
        println!("localgcp: using {}", binary.display());
        return Ok(binary);
    }
    fs::create_dir_all(&version_dir).map_err(TaskError::operation)?;
    let base = format!("https://github.com/slokam-ai/localgcp/releases/download/v{version}");
    let checksums = dir.join("checksums.txt");
    let archive = dir.join(&asset);
    ctx.exec(
        ProcessCommand::new("curl")
            .args(["-fsSL", &format!("{base}/checksums.txt"), "-o"])
            .arg(&checksums),
    )
    .await?;
    ctx.exec(
        ProcessCommand::new("curl")
            .args(["-fsSL", &format!("{base}/{asset}"), "-o"])
            .arg(&archive),
    )
    .await?;

    let text = fs::read_to_string(&checksums).map_err(TaskError::operation)?;
    let expected = expected_checksum(&text, &asset)?;
    let actual = sha256_file(&archive)?;
    if expected != actual {
        return Err(TaskError::operation(format!(
            "localgcp {asset}: sha256 {actual} is not the release value {expected}"
        )));
    }

    let file = fs::File::open(&archive).map_err(TaskError::operation)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    tar.unpack(&version_dir).map_err(TaskError::operation)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
            .map_err(TaskError::operation)?;
    }
    fs::remove_file(&archive).map_err(TaskError::operation)?;
    if !binary.is_file() {
        return Err(TaskError::operation(format!(
            "the localgcp release holds no binary at {}",
            binary.display()
        )));
    }
    println!("localgcp: installed {}", binary.display());
    Ok(binary)
}

/// A sink that discards a probe's output. A process check exits with an error
/// when the process is gone, and that error is the answer, not a task failure.
struct Silent;

impl TaskOutput for Silent {
    fn write<'a>(&'a self, _: OutputStream, _: &'a [u8]) -> BoxFuture<'a, std::io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn flush(&self) -> BoxFuture<'_, std::io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// Run a process check and discard its output.
async fn run_probe(ctx: &TaskContext, command: ProcessCommand) -> TaskResult {
    ctx.exec_with_output(command, std::sync::Arc::new(Silent))
        .await
}

/// Whether `pid` names a live process.
#[cfg(unix)]
async fn alive(ctx: &TaskContext, pid: u32) -> Result<bool, TaskError> {
    // Signal 0 asks whether the process exists and sends nothing.
    let probe = ProcessCommand::new("kill")
        .args(["-0"])
        .arg(pid.to_string());
    match run_probe(ctx, probe).await {
        Ok(()) => Ok(true),
        Err(TaskError::Exit { .. }) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Whether `pid` names a live process.
#[cfg(windows)]
async fn alive(ctx: &TaskContext, pid: u32) -> Result<bool, TaskError> {
    use std::io::Read as _;
    let probe = ProcessCommand::new("tasklist").args([
        "/FI",
        &format!("PID eq {pid}"),
        "/NH",
        "/FO",
        "CSV",
    ]);
    let mut file = crate::workspace::capture(ctx, probe).await?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(TaskError::operation)?;
    Ok(text.contains(&format!("\"{pid}\"")))
}

/// Ask the process `pid` to stop.
async fn terminate(ctx: &TaskContext, pid: u32) -> TaskResult {
    #[cfg(unix)]
    let command = ProcessCommand::new("kill")
        .args(["-TERM"])
        .arg(pid.to_string());
    #[cfg(windows)]
    let command = ProcessCommand::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]);
    ctx.exec(command).await
}

/// Ask the process `pid` to stop now.
async fn kill(ctx: &TaskContext, pid: u32) -> TaskResult {
    #[cfg(unix)]
    let command = ProcessCommand::new("kill")
        .args(["-KILL"])
        .arg(pid.to_string());
    #[cfg(windows)]
    let command = ProcessCommand::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]);
    ctx.exec(command).await
}

/// Stop the saved emulator, when one runs.
async fn stop_saved(
    ctx: &TaskContext,
    dir: &Path,
    timeout: Duration,
    poll: Duration,
) -> TaskResult {
    let file = pid_file(dir);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(TaskError::operation(error)),
    };
    let pid: u32 = text.trim().parse().map_err(|_| {
        TaskError::operation(format!("{} does not hold a process ID", file.display()))
    })?;
    if !alive(ctx, pid).await? {
        fs::remove_file(&file).map_err(TaskError::operation)?;
        return Ok(());
    }
    println!("localgcp: stopping PID {pid}");
    terminate(ctx, pid).await?;
    let clock = ctx.context().clock();
    let deadline = clock
        .now()
        .checked_add(timeout)
        .ok_or_else(|| TaskError::operation("the stop deadline exceeds the clock range"))?;
    while alive(ctx, pid).await? {
        if clock.now() >= deadline {
            kill(ctx, pid).await?;
            if alive(ctx, pid).await? {
                return Err(TaskError::operation(format!(
                    "localgcp PID {pid} did not stop"
                )));
            }
            break;
        }
        sleep(ctx, poll).await?;
    }
    fs::remove_file(&file).map_err(TaskError::operation)?;
    println!("localgcp: stopped PID {pid}");
    Ok(())
}

/// Whether the KMS port answers on the loopback address.
fn port_ready(port: u16) -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
}

async fn start(ctx: TaskContext, args: Start) -> TaskResult {
    let dir = control(&ctx)?;
    fs::create_dir_all(&dir).map_err(TaskError::operation)?;
    let poll = Duration::from_millis(args.poll_milliseconds);
    let timeout = Duration::from_secs(args.timeout_seconds);

    // A saved instance from an earlier run would hold the port. Stop it first.
    stop_saved(&ctx, &dir, timeout, poll).await?;

    let binary = match args.binary {
        Some(binary) => binary,
        None => install(&ctx, &dir, &args.release).await?,
    };
    let command = ProcessCommand::new(&binary)
        .arg("up")
        // The Docker-orchestrated services are not part of this task.
        .arg("--no-docker")
        // Request logging would fill the CI log with every KMS call.
        .arg("--quiet")
        .arg("--port-kms")
        .arg(args.port.to_string());
    let env = ctx.require::<TaskEnv>()?;
    let mut service = env
        .process
        .service(command, ctx.cancellation().clone())
        .await?;
    let pid = service
        .pid()
        .ok_or_else(|| TaskError::operation("this host does not report the localgcp process ID"))?;
    let started = async {
        fs::write(pid_file(&dir), pid.to_string()).map_err(TaskError::operation)?;
        let clock = ctx.context().clock();
        let deadline = clock
            .now()
            .checked_add(timeout)
            .ok_or_else(|| TaskError::operation("the startup deadline exceeds the clock range"))?;
        loop {
            if port_ready(args.port) {
                return Ok(());
            }
            if service.status().map_err(TaskError::operation)?.is_some() {
                return Err(TaskError::operation(
                    "localgcp exited before its KMS port answered",
                ));
            }
            if clock.now() >= deadline {
                return Err(TaskError::operation(format!(
                    "localgcp did not open port {} in {}s",
                    args.port, args.timeout_seconds
                )));
            }
            sleep(&ctx, poll).await?;
        }
    }
    .await;

    match started {
        Ok(()) if args.keep_running => {
            service.detach().map_err(TaskError::operation)?;
            println!(
                "localgcp: KMS on http://127.0.0.1:{} (PID {pid})",
                args.port
            );
            Ok(())
        }
        Ok(()) => {
            drop(service);
            fs::remove_file(pid_file(&dir)).map_err(TaskError::operation)?;
            println!(
                "localgcp: the KMS port {} answered; the emulator is stopped",
                args.port
            );
            Ok(())
        }
        Err(source) => {
            // Drop stops the service. Remove the saved ID so the stop action
            // does not report a process that no longer exists.
            drop(service);
            let cleanup = fs::remove_file(pid_file(&dir)).map_err(TaskError::operation);
            match cleanup {
                Ok(()) => Err(source),
                Err(cleanup) => Err(TaskError::Cleanup {
                    source: Box::new(source),
                    cleanup: Box::new(cleanup),
                }),
            }
        }
    }
}

async fn stop(ctx: TaskContext, args: Stop) -> TaskResult {
    let dir = control(&ctx)?;
    let poll = Duration::from_millis(args.poll_milliseconds);
    let timeout = Duration::from_secs(args.timeout_seconds);
    if !pid_file(&dir).exists() {
        println!("localgcp: not running");
        return Ok(());
    }
    stop_saved(&ctx, &dir, timeout, poll).await
}
