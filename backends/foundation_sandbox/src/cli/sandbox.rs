//! The `agent-sandbox` command (feature 06).
//!
//! The command is a boundary: it parses arguments, calls one control plane and
//! copies bytes. It holds no rule about sandboxes (invariant I10). With no
//! `--connect`, `run` and `doctor` build a local manager; the other
//! sub-commands need a remote control plane, because a local sandbox ends with
//! the process.

#![allow(
    clippy::needless_pass_by_value,
    reason = "the program entry and its helpers take ownership of the parsed arguments"
)]

use std::io::{IsTerminal as _, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use foundation_context::Ctx;
use foundation_core::valtron::{block_on_future, Stream as ValtronStream};
use foundation_http::ShutdownSignal;
use foundation_nativeapis::signal::{signal_task, SignalEvent, SignalKind};
use futures::{FutureExt as _, StreamExt as _};

use crate::cli::{parse_connect, parse_listen};
use crate::native::record::FileRecordStore;
use crate::native::serve::{bind_control, serve_bound, ServeConfig, ServeError};
use crate::shared::command::{Command, ExecEvent, ExitStatus, StdinMode};
use crate::shared::control::{SandboxHandle, SandboxManager, SandboxProvider, SandboxRpcClient};
use crate::shared::driver::SandboxDriver;
use crate::shared::error::SandboxError;
use crate::shared::handles::ExecEvents;
use crate::shared::host::SandboxHost;
use crate::shared::ids::DriverId;
use crate::shared::limits::SandboxLimits;
use crate::shared::listen::ListenAddress;
#[cfg(all(feature = "artifacts", target_os = "linux"))]
use crate::shared::spec::GuestArch;
use crate::shared::spec::{NetworkMode, RootFs, RootMode, SandboxSpec};
use crate::shared::Secret;

/// A usage error (sysexits.h `EX_USAGE`).
const EXIT_USAGE: i32 = 64;
/// A listener that another server holds (sysexits.h `EX_UNAVAILABLE`).
const EXIT_UNAVAILABLE: i32 = 69;
/// The sandbox or the control plane failed.
const EXIT_FAILURE: i32 = 125;

/// The environment variable that names the control plane.
const CONNECT_ENV: &str = "EWE_SANDBOX_CONNECT";
/// The environment variable that holds the bearer token.
const TOKEN_ENV: &str = "EWE_SANDBOX_TOKEN";

/// The `agent-sandbox` program.
#[derive(Debug, Parser)]
#[command(name = "agent-sandbox", about = "Start and use sandboxes")]
pub struct SandboxCli {
    /// The sub-command.
    #[command(subcommand)]
    pub command: SandboxCommand,
}

/// The sub-commands.
#[derive(Debug, Subcommand)]
pub enum SandboxCommand {
    /// Create one sandbox, run one command in it and print the output.
    Run(RunArgs),
    /// Serve the control plane on a Unix domain socket or TCP.
    Serve(ServeArgs),
    /// Create one sandbox and print its identifier.
    Create(CreateArgs),
    /// Run one command in an existing sandbox.
    Exec(ExecArgs),
    /// List the live sandboxes.
    List(ConnectArgs),
    /// Stop one sandbox.
    Stop(IdArgs),
    /// Kill one sandbox.
    Kill(IdArgs),
    /// Report what this host can run.
    Doctor(DoctorArgs),
}

/// The options of a sandbox request.
#[derive(Debug, Clone, Args)]
pub struct SpecArgs {
    /// The driver: `firecracker`, `libkrun` or `docker`.
    #[arg(long)]
    pub driver: Option<String>,
    /// The root file system: a directory, a `.ext4` file or a `.squashfs` file.
    #[arg(long)]
    pub rootfs: Option<PathBuf>,
    /// A container image name.
    #[arg(long)]
    pub image: Option<String>,
    /// The number of virtual CPUs.
    #[arg(long, default_value_t = 1)]
    pub vcpus: u8,
    /// The memory in mebibytes.
    #[arg(long = "memory-mib", default_value_t = 512)]
    pub memory_mib: u32,
    /// The network mode: `none` or `host`.
    #[arg(long, default_value = "none")]
    pub network: String,
    /// A base environment entry, as `KEY=VALUE`. Repeat it for each entry.
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,
    /// The working directory in the sandbox.
    #[arg(long, default_value = "/workspace")]
    pub workdir: String,
    /// Mount the root read only: only `/tmp`, `/run` and the work directory are
    /// writable.
    #[arg(long = "read-only")]
    pub read_only: bool,
}

/// Where the control plane is.
#[derive(Debug, Clone, Default, Args)]
pub struct ConnectArgs {
    /// The control plane: `unix:<path>`, `tcp:<host>:<port>` or
    /// `http(s)://<address>`. The default is `EWE_SANDBOX_CONNECT`.
    #[arg(long)]
    pub connect: Option<String>,
    /// The file whose first line is the bearer token of a TCP control plane.
    #[arg(long = "token-file")]
    pub token_file: Option<PathBuf>,
}

/// `run`: one sandbox, one command.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// The sandbox request.
    #[command(flatten)]
    pub spec: SpecArgs,
    /// Where the control plane is.
    #[command(flatten)]
    pub connect: ConnectArgs,
    /// The program and its arguments, after `--`.
    #[arg(last = true, required = true, num_args = 1..)]
    pub program: Vec<String>,
}

/// `create`: one sandbox, no command.
#[derive(Debug, Args)]
pub struct CreateArgs {
    /// The sandbox request.
    #[command(flatten)]
    pub spec: SpecArgs,
    /// Where the control plane is.
    #[command(flatten)]
    pub connect: ConnectArgs,
}

/// `exec`: one command in an existing sandbox.
#[derive(Debug, Args)]
pub struct ExecArgs {
    /// Where the control plane is.
    #[command(flatten)]
    pub connect: ConnectArgs,
    /// The sandbox identifier.
    pub sandbox_id: String,
    /// A command environment entry, as `KEY=VALUE`.
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,
    /// The working directory of the command.
    #[arg(long)]
    pub workdir: Option<String>,
    /// The program and its arguments, after `--`.
    #[arg(last = true, required = true, num_args = 1..)]
    pub program: Vec<String>,
}

/// `stop` and `kill`.
#[derive(Debug, Args)]
pub struct IdArgs {
    /// Where the control plane is.
    #[command(flatten)]
    pub connect: ConnectArgs,
    /// The sandbox identifier.
    pub sandbox_id: String,
}

/// `serve`.
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// The listen address: `unix:<path>` or `tcp:<host>:<port>`.
    #[arg(long)]
    pub listen: String,
    /// The file whose first line is the token of a TCP listener.
    #[arg(long = "token-file")]
    pub token_file: Option<PathBuf>,
    /// The driver order, as a comma list.
    #[arg(long = "driver-order")]
    pub driver_order: Option<String>,
}

/// `doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// The driver order, as a comma list.
    #[arg(long = "driver-order")]
    pub driver_order: Option<String>,
}

/// Runs one sub-command and returns its exit code.
#[must_use]
pub fn run(ctx: Ctx, cli: SandboxCli) -> i32 {
    let limits = SandboxLimits::default();
    match cli.command {
        SandboxCommand::Run(args) => run_command(ctx, args, limits),
        SandboxCommand::Serve(args) => serve_command(ctx, args, limits),
        SandboxCommand::Create(args) => create_command(ctx, args, limits),
        SandboxCommand::Exec(args) => exec_command(ctx, args, limits),
        SandboxCommand::List(args) => list_command(ctx, args, limits),
        SandboxCommand::Stop(args) => stop_or_kill(ctx, args, limits, false),
        SandboxCommand::Kill(args) => stop_or_kill(ctx, args, limits, true),
        SandboxCommand::Doctor(args) => doctor_command(ctx, args),
    }
}

/// Prints a usage error and returns `EXIT_USAGE`.
fn usage(message: &str) -> i32 {
    eprintln!("agent-sandbox: {message}");
    EXIT_USAGE
}

/// Prints a control-plane failure and returns `EXIT_FAILURE`.
fn failure(error: &foundation_errstacks::ErrorTrace<SandboxError>) -> i32 {
    eprintln!("agent-sandbox: {error}");
    EXIT_FAILURE
}

/// `run`: create, execute, copy the output, stop.
fn run_command(ctx: Ctx, args: RunArgs, limits: SandboxLimits) -> i32 {
    let connect = resolve_connect(&args.connect);
    let local = connect.is_none();
    let provider = match control_plane(&ctx, &args.connect, &limits) {
        Ok(provider) => provider,
        Err(code) => return code,
    };
    let spec = match build_spec(&ctx, &args.spec, local) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    let stdin_piped = !std::io::stdin().is_terminal();
    let mut command = Command::new(&args.program);
    command.stdin = if stdin_piped {
        StdinMode::Piped
    } else {
        StdinMode::Null
    };
    eprintln!("agent-sandbox: creating the sandbox");
    let sandbox = match wait_create(provider.create_with_progress(spec)) {
        Ok(sandbox) => sandbox,
        Err(error) => return failure(&error),
    };
    let signals = start_signal_task(&ctx);
    block_on_future(run_events(ctx, sandbox, command, limits, signals))
}

/// `serve`: bind, serve and stop every sandbox at the end.
fn serve_command(ctx: Ctx, args: ServeArgs, limits: SandboxLimits) -> i32 {
    let listen = match parse_listen(&args.listen) {
        Ok(listen) => listen,
        Err(error) => return usage(&error),
    };
    if matches!(listen, ListenAddress::Vsock { .. }) {
        return usage("serve accepts a unix: or tcp: address");
    }
    let auth = match serve_auth(&listen, args.token_file.as_deref()) {
        Ok(auth) => auth,
        Err(code) => return code,
    };
    let order = match args.driver_order.as_deref().map(parse_driver_order) {
        Some(Err(error)) => return usage(&error),
        Some(Ok(order)) => Some(order),
        None => None,
    };
    let manager = match local_manager(&ctx, order) {
        Ok(manager) => manager,
        Err(code) => return code,
    };
    let restore = Arc::clone(&manager);
    if let Err(error) = block_on_future(async move { restore.restore().await }) {
        eprintln!("agent-sandbox: the restore failed: {error}");
    }
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = match bind_control(&listen, Arc::clone(&shutdown), &ServeConfig::default()) {
        Ok(bound) => bound,
        Err(ServeError::SocketPathTooLong { .. } | ServeError::Unsupported(_)) => {
            eprintln!("agent-sandbox: {}", serve_error_text(&listen, &args.listen));
            return EXIT_USAGE;
        }
        Err(error) => {
            eprintln!("agent-sandbox: {error}");
            return if matches!(error, ServeError::AddressInUse) {
                EXIT_UNAVAILABLE
            } else {
                EXIT_FAILURE
            };
        }
    };
    match bound.local_addr() {
        Some(addr) => eprintln!("agent-sandbox: listening on tcp:{addr}"),
        None => eprintln!("agent-sandbox: listening on {listen}"),
    }
    if let Some(queue) = start_signal_task(&ctx) {
        let shutdown = Arc::clone(&shutdown);
        std::thread::Builder::new()
            .name("agent-sandbox-signal".to_string())
            .spawn(move || loop {
                let event = futures::executor::block_on(next_signal(queue.clone()));
                if matches!(
                    event.kind,
                    SignalKind::Interrupt | SignalKind::Terminate | SignalKind::Quit
                ) {
                    shutdown.turn_on();
                    return;
                }
            })
            .ok();
    }

    let control: Arc<dyn SandboxProvider> = Arc::clone(&manager) as Arc<dyn SandboxProvider>;
    serve_bound(ctx.clone(), control, &bound, auth, &limits);

    // The listener stopped. Stop every sandbox that is still live.
    let listing = Arc::clone(&manager);
    if let Ok(sandboxes) = block_on_future(async move { listing.list().await }) {
        for info in sandboxes {
            if let Ok(id) = info.id.parse() {
                let stopper = Arc::clone(&manager);
                let _ = block_on_future(async move { stopper.stop(id).await });
            }
        }
    }
    0
}

/// The text of a bind failure that is a usage error.
fn serve_error_text(listen: &ListenAddress, raw: &str) -> String {
    match listen {
        ListenAddress::Vsock { .. } => "serve accepts a unix: or tcp: address".to_string(),
        _ => format!("the listen address is not usable: {raw}"),
    }
}

/// `create`: one sandbox, remote only.
fn create_command(ctx: Ctx, args: CreateArgs, limits: SandboxLimits) -> i32 {
    if resolve_connect(&args.connect).is_none() {
        return usage("create needs --connect or EWE_SANDBOX_CONNECT");
    }
    let provider = match control_plane(&ctx, &args.connect, &limits) {
        Ok(provider) => provider,
        Err(code) => return code,
    };
    let spec = match build_spec(&ctx, &args.spec, false) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    match wait_create(provider.create_with_progress(spec)) {
        Ok(sandbox) => {
            println!("{}", sandbox.id());
            0
        }
        Err(error) => failure(&error),
    }
}

/// `exec`: one command in an existing sandbox.
fn exec_command(ctx: Ctx, args: ExecArgs, limits: SandboxLimits) -> i32 {
    if resolve_connect(&args.connect).is_none() {
        return usage("exec needs --connect or EWE_SANDBOX_CONNECT");
    }
    let provider = match control_plane(&ctx, &args.connect, &limits) {
        Ok(provider) => provider,
        Err(code) => return code,
    };
    let id = match args.sandbox_id.parse() {
        Ok(id) => id,
        Err(error) => return usage(&format!("the sandbox identifier is not valid: {error}")),
    };
    let mut command = Command::new(&args.program);
    command.env = match parse_env(&args.env) {
        Ok(env) => env,
        Err(error) => return usage(&error),
    };
    command.cwd = args.workdir;
    command.stdin = if std::io::stdin().is_terminal() {
        StdinMode::Null
    } else {
        StdinMode::Piped
    };
    let getter = Arc::clone(&provider);
    let sandbox = match block_on_future(async move { getter.get(id).await }) {
        Ok(sandbox) => sandbox,
        Err(error) => return failure(&error),
    };
    let signals = start_signal_task(&ctx);
    block_on_future(run_events(ctx, sandbox, command, limits, signals))
}

/// `list`: one line for each live sandbox.
fn list_command(ctx: Ctx, args: ConnectArgs, limits: SandboxLimits) -> i32 {
    if resolve_connect(&args).is_none() {
        return usage("list needs --connect or EWE_SANDBOX_CONNECT");
    }
    let provider = match control_plane(&ctx, &args, &limits) {
        Ok(provider) => provider,
        Err(code) => return code,
    };
    let lister = Arc::clone(&provider);
    match block_on_future(async move { lister.list().await }) {
        Ok(sandboxes) => {
            for sandbox in sandboxes {
                println!(
                    "{} {} {}",
                    sandbox.id,
                    sandbox.driver,
                    state_name(sandbox.state)
                );
            }
            0
        }
        Err(error) => failure(&error),
    }
}

/// `stop` and `kill`.
fn stop_or_kill(ctx: Ctx, args: IdArgs, limits: SandboxLimits, kill: bool) -> i32 {
    if resolve_connect(&args.connect).is_none() {
        let name = if kill { "kill" } else { "stop" };
        return usage(&format!("{name} needs --connect or EWE_SANDBOX_CONNECT"));
    }
    let provider = match control_plane(&ctx, &args.connect, &limits) {
        Ok(provider) => provider,
        Err(code) => return code,
    };
    let id = match args.sandbox_id.parse() {
        Ok(id) => id,
        Err(error) => return usage(&format!("the sandbox identifier is not valid: {error}")),
    };
    let actor = Arc::clone(&provider);
    let result = if kill {
        block_on_future(async move { actor.kill(id).await })
    } else {
        block_on_future(async move { actor.stop(id).await })
    };
    match result {
        Ok(_) => 0,
        Err(error) => failure(&error),
    }
}

/// `doctor`: report what each driver can do on this host.
fn doctor_command(ctx: Ctx, args: DoctorArgs) -> i32 {
    let order = match args.driver_order.as_deref().map(parse_driver_order) {
        Some(Err(error)) => return usage(&error),
        Some(Ok(order)) => Some(order),
        None => None,
    };
    let manager = match local_manager(&ctx, order) {
        Ok(manager) => manager,
        Err(code) => return code,
    };
    let prober = Arc::clone(&manager);
    let reports = match block_on_future(async move { prober.probe().await }) {
        Ok(reports) => reports,
        Err(error) => return failure(&error),
    };
    let mut usable = false;
    for report in &reports {
        usable |= report.usable;
        println!(
            "{}: {}",
            report.driver,
            if report.usable { "ok" } else { "not usable" }
        );
        for check in &report.checks {
            println!(
                "  {}: {} {}",
                check.name,
                if check.ok { "ok" } else { "failed" },
                check.message
            );
        }
    }
    i32::from(!usable)
}
/// The name of one sandbox state.
fn state_name(state: crate::shared::control::SandboxState) -> &'static str {
    use crate::shared::control::SandboxState;
    match state {
        SandboxState::Creating => "creating",
        SandboxState::Ready => "ready",
        SandboxState::Stopping => "stopping",
        SandboxState::Stopped => "stopped",
        SandboxState::Failed => "failed",
    }
}

/// The `--connect` value, or the environment variable.
fn resolve_connect(args: &ConnectArgs) -> Option<String> {
    args.connect
        .clone()
        .or_else(|| std::env::var(CONNECT_ENV).ok())
}

/// The control plane of a call: remote when `--connect` names one, else local.
fn control_plane(
    ctx: &Ctx,
    args: &ConnectArgs,
    limits: &SandboxLimits,
) -> Result<Arc<dyn SandboxProvider>, i32> {
    let Some(value) = resolve_connect(args) else {
        return Err(usage("--connect or EWE_SANDBOX_CONNECT is needed"));
    };
    let addr = parse_connect(&value).map_err(|error| usage(&error))?;
    let mut client = SandboxRpcClient::connect(ctx.clone(), addr, limits.clone())
        .map_err(|error| failure(&error))?;
    if let Some(token) = read_token(args.token_file.as_deref())? {
        client = client.with_token(token);
    }
    Ok(Arc::new(client))
}

/// A local manager over the drivers that this build has.
fn local_manager(ctx: &Ctx, order: Option<Vec<DriverId>>) -> Result<Arc<SandboxManager>, i32> {
    let host = SandboxHost::from_ctx(ctx).map_err(|error| failure(&error))?;
    let order = order.unwrap_or_else(|| host.config.driver_order.clone());
    #[allow(
        unused_mut,
        reason = "the Firecracker driver is compiled only on Linux; another target has no push"
    )]
    let mut drivers: Vec<Arc<dyn SandboxDriver>> = Vec::new();
    for id in order {
        match id {
            DriverId::Firecracker => {
                #[cfg(all(feature = "firecracker", target_os = "linux"))]
                drivers.push(Arc::new(
                    crate::native::firecracker::driver::FirecrackerDriver::new(),
                ));
            }
            DriverId::Docker => {
                #[cfg(feature = "docker")]
                drivers.push(Arc::new(crate::native::docker::driver::DockerDriver::new()));
            }
            // Feature 09 adds the libkrun driver; a nested child and the
            // Cloudflare drivers are not part of a native local host.
            DriverId::Nested
            | DriverId::Libkrun
            | DriverId::CloudflareContainer
            | DriverId::CloudflareDo => {}
        }
    }
    let store = FileRecordStore::new(Arc::clone(&host.fs), host.config.state_dir.clone())
        .map_err(|error| failure(&error))?;
    Ok(Arc::new(SandboxManager::new(
        ctx.clone(),
        drivers,
        host.connector.clone(),
        Arc::new(store),
        crate::shared::control::ManagerConfig::default(),
    )))
}

/// The bearer token: the file first, then the environment variable.
fn read_token(path: Option<&Path>) -> Result<Option<Secret>, i32> {
    let from_file = if let Some(path) = path {
        let text = std::fs::read_to_string(path)
            .map_err(|error| usage(&format!("the token file {}: {error}", path.display())))?;
        let line = text.lines().next().unwrap_or("").trim();
        if line.is_empty() {
            return Err(usage("the token file is empty"));
        }
        Some(Secret::new(line))
    } else {
        None
    };
    if from_file.is_some() {
        return Ok(from_file);
    }
    match std::env::var(TOKEN_ENV) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(Secret::new(value.trim()))),
        _ => Ok(None),
    }
}

/// The token of a listener: a Unix socket needs none; TCP needs one.
fn serve_auth(listen: &ListenAddress, token_file: Option<&Path>) -> Result<Option<Secret>, i32> {
    match listen {
        ListenAddress::Unix { .. } => Ok(None),
        ListenAddress::Tcp { .. } => match read_token(token_file)? {
            Some(token) => Ok(Some(token)),
            None => Err(usage(
                "a TCP listener needs --token-file (or EWE_SANDBOX_TOKEN)",
            )),
        },
        ListenAddress::Vsock { .. } => Err(usage("serve accepts a unix: or tcp: address")),
    }
}

/// Parses a comma list of driver identifiers.
fn parse_driver_order(value: &str) -> Result<Vec<DriverId>, String> {
    value
        .split(',')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| DriverId::from_str(entry.trim()).map_err(|error| error.to_string()))
        .collect()
}

/// Parses `KEY=VALUE` entries.
fn parse_env(values: &[String]) -> Result<Vec<(String, String)>, String> {
    values
        .iter()
        .map(|entry| {
            let (key, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("an environment entry must be KEY=VALUE: {entry}"))?;
            if key.is_empty() {
                return Err(format!("the environment key is empty: {entry}"));
            }
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

/// Builds a sandbox request from the arguments.
fn build_spec(ctx: &Ctx, args: &SpecArgs, local: bool) -> Result<SandboxSpec, i32> {
    if args.rootfs.is_some() && args.image.is_some() {
        return Err(usage("--rootfs and --image are alternatives"));
    }
    let driver = match args.driver.as_deref() {
        Some(value) => match DriverId::from_str(value) {
            Ok(driver) => Some(driver),
            Err(error) => return Err(usage(&error.to_string())),
        },
        None => None,
    };
    let rootfs = if let Some(path) = &args.rootfs {
        rootfs_from_path(path)?
    } else if let Some(image) = &args.image {
        RootFs::ContainerImage(image.clone())
    } else if local {
        // Only the Firecracker driver ships a default image.
        if matches!(driver, Some(id) if id != DriverId::Firecracker) {
            return Err(usage("this driver needs --rootfs or --image"));
        }
        RootFs::SquashfsImage(default_rootfs(ctx)?)
    } else {
        return Err(usage(
            "run and create with --connect need --rootfs or --image",
        ));
    };
    let network = match args.network.as_str() {
        "none" => NetworkMode::None,
        "host" => NetworkMode::Host,
        other => return Err(usage(&format!("the network must be none or host: {other}"))),
    };
    let env = parse_env(&args.env).map_err(|error| usage(&error))?;
    let mut spec = SandboxSpec::new(rootfs);
    spec.driver = driver;
    spec.root_mode = if args.read_only {
        RootMode::ReadOnly
    } else {
        RootMode::Overlay
    };
    spec.vcpus = args.vcpus;
    spec.memory_mib = args.memory_mib;
    spec.network = network;
    spec.env = env;
    spec.workdir.clone_from(&args.workdir);
    Ok(spec)
}

/// The root file system that a path names.
fn rootfs_from_path(path: &Path) -> Result<RootFs, i32> {
    if path.is_dir() {
        return Ok(RootFs::Directory(path.to_path_buf()));
    }
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("ext4") => Ok(RootFs::Ext4Image(path.to_path_buf())),
        Some("squashfs") => Ok(RootFs::SquashfsImage(path.to_path_buf())),
        _ => Err(usage(&format!(
            "--rootfs needs a directory, a .ext4 file or a .squashfs file: {}",
            path.display()
        ))),
    }
}

/// The default continuous integration (CI) root file system of Firecracker.
#[cfg(all(feature = "artifacts", target_os = "linux"))]
fn default_rootfs(ctx: &Ctx) -> Result<PathBuf, i32> {
    let host = SandboxHost::from_ctx(ctx).map_err(|error| failure(&error))?;
    // `block_on_future` runs the future on a task, so the future must own the
    // store. An `Arc` gives it that ownership.
    let store = Arc::new(crate::native::artifacts::ArtifactStore::new(
        ctx.clone(),
        host.fs.clone(),
        host.config.cache_dir.clone(),
    ));
    eprintln!("agent-sandbox: fetching the default kernel and root file system");
    let kernel_store = Arc::clone(&store);
    let arch = GuestArch::host();
    let _kernel = block_on_future(async move { kernel_store.firecracker_kernel(arch).await })
        .map_err(|error| failure(&error))?;
    let rootfs_store = Arc::clone(&store);
    let arch = GuestArch::host();
    block_on_future(async move { rootfs_store.firecracker_ci_rootfs(arch).await })
        .map_err(|error| failure(&error))
}

/// Another build or target has no default image.
#[cfg(not(all(feature = "artifacts", target_os = "linux")))]
fn default_rootfs(_ctx: &Ctx) -> Result<PathBuf, i32> {
    Err(usage(
        "this build has no default image; give --rootfs or --image",
    ))
}

/// Drives a create stream and prints its progress.
fn wait_create(
    mut stream: crate::shared::handles::CreateStream,
) -> Result<SandboxHandle, foundation_errstacks::ErrorTrace<SandboxError>> {
    loop {
        match stream.next() {
            Some(ValtronStream::Pending(progress)) => {
                eprintln!("agent-sandbox: {progress:?}");
            }
            Some(ValtronStream::Next(result)) => return result,
            Some(_) => {}
            None => return Err(SandboxError::StreamLost.into()),
        }
    }
}

/// Runs one command and copies its output, with signal handling.
async fn run_events(
    ctx: Ctx,
    sandbox: SandboxHandle,
    command: Command,
    limits: SandboxLimits,
    signals: Option<foundation_core::valtron::EventQueue<SignalEvent>>,
) -> i32 {
    let mut handle = match sandbox.exec(command).await {
        Ok(handle) => handle,
        Err(error) => {
            let _ = sandbox.kill().await;
            eprintln!("agent-sandbox: {error}");
            return EXIT_FAILURE;
        }
    };
    if let Some(stdin) = handle.stdin() {
        spawn_input(stdin);
    }
    let mut events = handle.events();
    loop {
        let next = match &signals {
            Some(queue) => next_event_or_signal(&mut events, queue).await,
            None => Next::Event(events.next().await),
        };
        match next {
            Next::Event(Some(Ok(ExecEvent::Stdout(data)))) => write_stdout(&data),
            Next::Event(Some(Ok(ExecEvent::Stderr(data)))) => write_stderr(&data),
            Next::Event(Some(Ok(ExecEvent::Exit(status)))) => {
                if let Err(error) = sandbox.stop().await {
                    eprintln!("agent-sandbox: the stop failed: {error}");
                    return EXIT_FAILURE;
                }
                return exit_code(status);
            }
            Next::Event(Some(Err(error))) => {
                let _ = sandbox.kill().await;
                eprintln!("agent-sandbox: {error}");
                return EXIT_FAILURE;
            }
            Next::Event(None) => {
                let _ = sandbox.kill().await;
                eprintln!("agent-sandbox: the event stream ended without an exit");
                return EXIT_FAILURE;
            }
            Next::Signal(event) => {
                return interrupted(
                    &ctx,
                    &sandbox,
                    &handle,
                    &mut events,
                    signals.as_ref(),
                    event,
                    &limits,
                )
                .await;
            }
        }
    }
}

/// Copies the standard input of the program to its input writer.
fn spawn_input(stdin: crate::shared::handles::ExecStdin) {
    let _ = std::thread::Builder::new()
        .name("agent-sandbox-stdin".to_string())
        .spawn(move || {
            let mut input = std::io::stdin();
            let mut buffer = vec![0_u8; 64 * 1024];
            loop {
                match input.read(&mut buffer) {
                    Ok(count) if count > 0 => {
                        let data = buffer[..count].to_vec();
                        let writer = stdin.clone();
                        let result = block_on_future(async move {
                            let mut writer = writer;
                            writer.write(&data).await
                        });
                        if result.is_err() {
                            return;
                        }
                    }
                    _ => break,
                }
            }
            let _ = block_on_future(async move { stdin.close().await });
        });
}

/// Handles the first SIGINT or SIGTERM of a run.
async fn interrupted(
    ctx: &Ctx,
    sandbox: &SandboxHandle,
    handle: &crate::shared::handles::ExecHandle,
    events: &mut ExecEvents,
    signals: Option<&foundation_core::valtron::EventQueue<SignalEvent>>,
    event: SignalEvent,
    limits: &SandboxLimits,
) -> i32 {
    let original = event.kind;
    if let Err(error) = handle.signal(15).await {
        let _ = sandbox.kill().await;
        eprintln!("agent-sandbox: the signal failed: {error}");
        return EXIT_FAILURE;
    }
    let deadline = ctx
        .require_env()
        .ok()
        .map(|_| ctx.clock().now() + limits.shutdown_grace);
    loop {
        let next = match signals {
            Some(queue) => {
                next_event_signal_or_timeout(events, queue, signal_grace_until(ctx, deadline)).await
            }
            None => match events.next().await {
                Some(item) => Timed::Event(Some(item)),
                None => Timed::Event(None),
            },
        };
        match next {
            Timed::Event(Some(Ok(ExecEvent::Stdout(data)))) => write_stdout(&data),
            Timed::Event(Some(Ok(ExecEvent::Stderr(data)))) => write_stderr(&data),
            Timed::Event(Some(Ok(ExecEvent::Exit(_)))) => {
                let _ = sandbox.stop().await;
                return interrupted_code(original);
            }
            Timed::Event(Some(Err(error))) => {
                let _ = sandbox.kill().await;
                eprintln!("agent-sandbox: {error}");
                return EXIT_FAILURE;
            }
            Timed::Event(None) => {
                let _ = sandbox.kill().await;
                return interrupted_code(original);
            }
            Timed::Signal => {
                eprintln!("agent-sandbox: a second signal; killing the sandbox");
                let _ = sandbox.kill().await;
                return EXIT_FAILURE;
            }
            Timed::Timeout => {
                eprintln!("agent-sandbox: the program did not stop; killing the sandbox");
                let _ = sandbox.kill().await;
                return interrupted_code(original);
            }
        }
    }
}

/// The future of the shutdown grace. It never finishes when the context has no
/// timer or no deadline.
fn signal_grace_until(
    ctx: &Ctx,
    deadline: Option<foundation_compact::Instant>,
) -> impl std::future::Future<Output = ()> + Send {
    let ctx = ctx.clone();
    async move {
        let Some(deadline) = deadline else {
            futures::future::pending::<()>().await;
            return;
        };
        let Ok(env) = ctx.require_env() else {
            futures::future::pending::<()>().await;
            return;
        };
        env.timer().sleep_until(ctx.clone(), deadline).await;
    }
}

/// The exit code of a run that a signal ended: `128 + the signal`.
fn interrupted_code(kind: SignalKind) -> i32 {
    let number = match kind {
        SignalKind::Interrupt => 2,
        SignalKind::Terminate => 15,
        SignalKind::Quit => 3,
        SignalKind::Hangup => 1,
    };
    128 + number
}

/// The exit code of the program.
fn exit_code(status: ExitStatus) -> i32 {
    status
        .exit_code()
        .unwrap_or_else(|| 128 + status.terminating_signal().unwrap_or(0))
}

/// Writes bytes to standard output and ignores a closed pipe.
fn write_stdout(data: &[u8]) {
    let _ = std::io::stdout().write_all(data);
    let _ = std::io::stdout().flush();
}

/// Writes bytes to standard error and ignores a closed pipe.
fn write_stderr(data: &[u8]) {
    let _ = std::io::stderr().write_all(data);
    let _ = std::io::stderr().flush();
}

/// The next item of a run: an event or a signal.
enum Next {
    Event(Option<Result<ExecEvent, foundation_errstacks::ErrorTrace<SandboxError>>>),
    Signal(SignalEvent),
}

/// The next item of a run that is waiting for the program to stop.
enum Timed {
    Event(Option<Result<ExecEvent, foundation_errstacks::ErrorTrace<SandboxError>>>),
    Signal,
    Timeout,
}

/// Waits for the next event or signal.
async fn next_event_or_signal(
    events: &mut ExecEvents,
    queue: &foundation_core::valtron::EventQueue<SignalEvent>,
) -> Next {
    let event = events.next().fuse();
    let signal = next_signal(queue.clone()).fuse();
    futures::pin_mut!(event, signal);
    futures::select! {
        item = event => Next::Event(item),
        event = signal => Next::Signal(event),
    }
}

/// Waits for the next event, signal or the grace timeout.
async fn next_event_signal_or_timeout(
    events: &mut ExecEvents,
    queue: &foundation_core::valtron::EventQueue<SignalEvent>,
    grace: impl std::future::Future<Output = ()>,
) -> Timed {
    let event = events.next().fuse();
    let signal = next_signal(queue.clone()).fuse();
    let grace = grace.fuse();
    futures::pin_mut!(event, signal, grace);
    futures::select! {
        item = event => Timed::Event(item),
        _event = signal => Timed::Signal,
        () = grace => Timed::Timeout,
    }
}

/// Waits for one signal on the bus queue.
fn next_signal(
    queue: foundation_core::valtron::EventQueue<SignalEvent>,
) -> impl std::future::Future<Output = SignalEvent> + Send {
    use foundation_core::valtron::{EventReadiness as _, ReadinessWaker};
    futures::future::poll_fn(move |context| {
        if let Ok(event) = queue.pop() {
            return std::task::Poll::Ready(event);
        }
        let readiness = queue.readiness();
        if readiness.is_ready(None) {
            if let Ok(event) = queue.pop() {
                return std::task::Poll::Ready(event);
            }
        }
        let _ = readiness.register(&ReadinessWaker::from_waker(context.waker().clone()));
        match queue.pop() {
            Ok(event) => std::task::Poll::Ready(event),
            Err(_) => std::task::Poll::Pending,
        }
    })
}

/// Starts the signal task on the pool and gives its queue.
fn start_signal_task(ctx: &Ctx) -> Option<foundation_core::valtron::EventQueue<SignalEvent>> {
    if !foundation_core::valtron::multi::is_pool_initialized() {
        return None;
    }
    match signal_task(ctx.clone()) {
        Ok((task, bus)) => {
            if foundation_core::valtron::send(task).is_err() {
                return None;
            }
            Some(bus.subscribe_events())
        }
        Err(error) => {
            eprintln!("agent-sandbox: the signal handlers are not available: {error}");
            None
        }
    }
}
