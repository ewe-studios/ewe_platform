//! Own a local Wrangler process and stop it through its project directory.
use crate::{execution::sleep, CiEnvironment};
use clap::Args;
use foundation_tasks::{
    Cancellation, OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput,
    TaskResult,
};
use futures_util::future::{select, BoxFuture, Either};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Args, Clone)]
struct Options {
    /// Port used by the local Cloudflare explorer.
    #[arg(long, default_value_t = 8789, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    /// Maximum time for startup, shutdown, or another controller to release its lock.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_seconds: u64,
    /// Interval between local readiness or process checks.
    #[arg(long, default_value_t = 250, value_parser = clap::value_parser!(u64).range(1..))]
    poll_milliseconds: u64,
    /// Maximum time for one local explorer request.
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u64).range(1..))]
    probe_seconds: u64,
}

#[derive(Args)]
struct Worker {
    #[command(flatten)]
    options: Options,
    /// Instance name that the start command creates.
    #[arg(long)]
    instance: String,
}

impl Options {
    fn deadline(&self, ctx: &TaskContext) -> Result<std::time::Instant, TaskError> {
        ctx.context()
            .clock()
            .now()
            .checked_add(Duration::from_secs(self.timeout_seconds))
            .ok_or_else(|| TaskError::operation("Wrangler deadline exceeds the clock range"))
    }

    async fn poll(&self, ctx: &TaskContext) -> TaskResult {
        sleep(ctx, Duration::from_millis(self.poll_milliseconds)).await
    }
}

fn control(ctx: &TaskContext) -> Result<PathBuf, TaskError> {
    Ok(ctx
        .require::<CiEnvironment>()?
        .root
        .join(".wrangler/ewe-tests"))
}

fn owned_directory(root: &Path, name: &str) -> Result<PathBuf, TaskError> {
    if !name.starts_with("instance-")
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err(TaskError::operation("Invalid local Wrangler instance name"));
    }
    Ok(root.join(name))
}

fn lock_file(path: &Path) -> Result<File, TaskError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(TaskError::operation)
}

async fn lock(ctx: &TaskContext, path: &Path, options: &Options) -> Result<File, TaskError> {
    let file = lock_file(path)?;
    let deadline = options.deadline(ctx)?;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::Error(error)) => return Err(TaskError::operation(error)),
            Err(TryLockError::WouldBlock) => {}
        }
        if ctx.context().clock().now() >= deadline {
            return Err(TaskError::operation(format!(
                "Timed out waiting for {}",
                path.display()
            )));
        }
        options.poll(ctx).await?;
    }
}

async fn stop_current(ctx: &TaskContext, root: &Path, options: &Options) -> TaskResult {
    let current = root.join("current");
    let name = match fs::read_to_string(&current) {
        Ok(name) => name,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(TaskError::operation(e)),
    };
    let dir = owned_directory(root, &name)?;
    if dir.try_exists().map_err(TaskError::operation)? {
        fs::write(dir.join("stop"), []).map_err(TaskError::operation)?;
        drop(lock(ctx, &dir.join("owner.lock"), options).await?);
        fs::remove_dir_all(dir).map_err(TaskError::operation)?;
    }
    fs::remove_file(current).map_err(TaskError::operation)
}

async fn stop(ctx: TaskContext, options: Options) -> TaskResult {
    let root = control(&ctx)?;
    if !root.try_exists().map_err(TaskError::operation)? {
        return Ok(());
    }
    let _controller = lock(&ctx, &root.join("controller.lock"), &options).await?;
    stop_current(&ctx, &root, &options).await
}

async fn start(ctx: TaskContext, options: Options) -> TaskResult {
    let root = control(&ctx)?;
    fs::create_dir_all(&root).map_err(TaskError::operation)?;
    let _controller = lock(&ctx, &root.join("controller.lock"), &options).await?;
    start_current(&ctx, &root, &options).await
}

async fn start_current(ctx: &TaskContext, root: &Path, options: &Options) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    stop_current(ctx, root, options).await?;
    let instance = tempfile::Builder::new()
        .prefix("instance-")
        .tempdir_in(root)
        .map_err(TaskError::operation)?;
    let name = instance
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| TaskError::operation("Invalid generated Wrangler instance"))?;
    let mut current = tempfile::NamedTempFile::new_in(root).map_err(TaskError::operation)?;
    current
        .write_all(name.as_bytes())
        .map_err(TaskError::operation)?;
    current
        .persist(root.join("current"))
        .map_err(TaskError::operation)?;
    let cmd = ProcessCommand::new(&host.binary)
        .current_dir(&host.root)
        .args(["task", "run", "cf:service", "--instance", name, "--port"])
        .arg(options.port.to_string())
        .arg("--timeout-seconds")
        .arg(options.timeout_seconds.to_string())
        .arg("--poll-milliseconds")
        .arg(options.poll_milliseconds.to_string())
        .arg("--probe-seconds")
        .arg(options.probe_seconds.to_string());
    let dir = instance.keep();
    let deadline = options.deadline(ctx)?;
    let result = async {
        let worker = ctx
            .require::<TaskEnv>()?
            .process
            .service(cmd, ctx.cancellation().clone())
            .await?;
        worker.detach().map_err(TaskError::operation)?;
        fs::write(dir.join("authorized"), []).map_err(TaskError::operation)?;
        loop {
            if dir
                .join("finished")
                .try_exists()
                .map_err(TaskError::operation)?
            {
                return Err(TaskError::operation(format!(
                    "Local Wrangler exited. See {}",
                    host.root.join("wrangler.log").display()
                )));
            }
            if dir
                .join("ready")
                .try_exists()
                .map_err(TaskError::operation)?
            {
                return Ok(());
            }
            if ctx.context().clock().now() >= deadline {
                return Err(TaskError::operation(
                    "Local Wrangler did not become ready before the deadline",
                ));
            }
            options.poll(ctx).await?;
        }
    }
    .await;
    if let Err(source) = result {
        if let Err(cleanup) = stop_current(ctx, root, options).await {
            return Err(TaskError::Cleanup {
                source: Box::new(source),
                cleanup: Box::new(cleanup),
            });
        }
        return Err(source);
    }
    Ok(())
}

struct Log(Mutex<File>);
impl TaskOutput for Log {
    fn write<'a>(&'a self, _: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.0
                .lock()
                .map_err(|_| io::Error::other("Wrangler log is unavailable"))?
                .write_all(bytes)
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async {
            self.0
                .lock()
                .map_err(|_| io::Error::other("Wrangler log is unavailable"))?
                .flush()
        })
    }
}

async fn monitor(ctx: &TaskContext, dir: &Path, options: &Options) -> TaskResult {
    let deadline = options.deadline(ctx)?;
    let mut ready = false;
    loop {
        if dir
            .join("stop")
            .try_exists()
            .map_err(TaskError::operation)?
        {
            return Ok(());
        }
        if !ready {
            let probe = ProcessCommand::new("curl")
                .args([
                    "--silent",
                    "--fail",
                    "--write-out",
                    "%{http_code}",
                    "--output",
                    if cfg!(windows) { "NUL" } else { "/dev/null" },
                    "--max-time",
                ])
                .arg(options.probe_seconds.to_string())
                .arg(format!(
                    "http://127.0.0.1:{}/cdn-cgi/local/explorer/api/d1/database",
                    options.port
                ));
            match crate::workspace::capture(ctx, probe).await {
                Ok(status) => {
                    let mut code = String::new();
                    status
                        .take(4)
                        .read_to_string(&mut code)
                        .map_err(TaskError::operation)?;
                    if code == "200" {
                        fs::write(dir.join("ready"), []).map_err(TaskError::operation)?;
                        ready = true;
                    }
                }
                Err(TaskError::Exit {
                    code: 7 | 22 | 28, ..
                }) => {}
                Err(error) => return Err(error),
            }
            if !ready && ctx.context().clock().now() >= deadline {
                return Err(TaskError::operation(
                    "Wrangler explorer did not respond before the deadline",
                ));
            }
        }
        options.poll(ctx).await?;
    }
}

async fn worker(ctx: TaskContext, args: Worker) -> TaskResult {
    let root = control(&ctx)?;
    let dir = owned_directory(&root, &args.instance)?;
    let owner = lock_file(&dir.join("owner.lock"))?;
    owner.try_lock().map_err(TaskError::operation)?;
    if fs::read_to_string(root.join("current")).map_err(TaskError::operation)? != args.instance
        || dir
            .join("stop")
            .try_exists()
            .map_err(TaskError::operation)?
    {
        return Ok(());
    }
    let deadline = args.options.deadline(&ctx)?;
    while !dir
        .join("authorized")
        .try_exists()
        .map_err(TaskError::operation)?
    {
        if dir
            .join("stop")
            .try_exists()
            .map_err(TaskError::operation)?
        {
            return Ok(());
        }
        if ctx.context().clock().now() >= deadline {
            return Err(TaskError::operation(
                "Wrangler startup authorization expired",
            ));
        }
        args.options.poll(&ctx).await?;
    }
    let result = serve(&ctx, &dir, &args.options).await;
    let stamp = fs::write(dir.join("finished"), []).map_err(TaskError::operation);
    match (result, stamp) {
        (Err(source), Err(cleanup)) => Err(TaskError::Cleanup {
            source: Box::new(source),
            cleanup: Box::new(cleanup),
        }),
        (result, Ok(())) | (Ok(()), result) => result,
    }
}

async fn serve(ctx: &TaskContext, dir: &Path, options: &Options) -> TaskResult {
    let log = Arc::new(Log(Mutex::new(
        File::create(ctx.require::<CiEnvironment>()?.root.join("wrangler.log"))
            .map_err(TaskError::operation)?,
    )));
    let host = ctx.require::<CiEnvironment>()?;
    let version = host
        .text("WRANGLER_VERSION")
        .unwrap_or_else(|| "4.156.2".into());
    let cmd = ProcessCommand::new("npx")
        .current_dir(&host.root)
        .arg("--yes")
        .arg(format!("wrangler@{version}"))
        .args(["dev", "--local", "--ip", "127.0.0.1", "--port"])
        .arg(options.port.to_string());
    let cancel = Cancellation::new();
    let process = ctx.require::<TaskEnv>()?;
    let child = process.process.run(cmd, log, cancel.clone());
    let completed = select(child, Box::pin(monitor(ctx, dir, options))).await;
    match completed {
        Either::Left((result, _)) => {
            result?;
            Err(TaskError::operation(
                "Wrangler exited before a stop request",
            ))
        }
        Either::Right((result, child)) => {
            cancel.cancel();
            match child.await {
                Ok(()) | Err(TaskError::Cancelled) => result,
                Err(cleanup) => match result {
                    Ok(()) => Err(cleanup),
                    Err(source) => Err(TaskError::Cleanup {
                        source: Box::new(source),
                        cleanup: Box::new(cleanup),
                    }),
                },
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Suite {
    ObjectStore,
    Database,
    ObjectFiles,
    DatabaseFiles,
}

impl Suite {
    fn command(self) -> ProcessCommand {
        let cargo = ProcessCommand::new("cargo").args(["test", "--locked", "--package"]);
        match self {
            Self::ObjectStore => cargo.args(["foundation_db", "--test", "r2_blobstore_tests"]),
            Self::Database => cargo.args(["foundation_db", "--test", "d1_kvstore_tests"]),
            Self::ObjectFiles => cargo.args([
                "foundation_nativeapis",
                "--features",
                "vfs-native,vfs-r2",
                "--test",
                "r2_delta_vfs",
            ]),
            Self::DatabaseFiles => cargo.args([
                "foundation_nativeapis",
                "--features",
                "vfs-native,vfs-d1",
                "--test",
                "d1_delta_vfs",
            ]),
        }
    }
}

async fn test(ctx: TaskContext, options: Options, suites: &'static [Suite]) -> TaskResult {
    let root = control(&ctx)?;
    fs::create_dir_all(&root).map_err(TaskError::operation)?;
    let _controller = lock(&ctx, &root.join("controller.lock"), &options).await?;
    start_current(&ctx, &root, &options).await?;
    let result = async {
        let host = ctx.require::<CiEnvironment>()?;
        let base = format!(
            "http://127.0.0.1:{}/cdn-cgi/local/explorer/api",
            options.port
        );
        for suite in suites {
            let cmd = suite
                .command()
                .args(["--", "--show-output"])
                .current_dir(&host.root)
                .env("CF_INTEGRATION_TEST", "1")
                .env("LOCAL_CF_API_BASE", &base)
                .env("LOCAL_R2_BUCKET", "test-bucket-preview")
                .env("LOCAL_D1_DATABASE_ID", "test-db-preview");
            ctx.exec(cmd).await?;
        }
        Ok(())
    }
    .await;
    match (result, stop_current(&ctx, &root, &options).await) {
        (Err(source), Err(cleanup)) => Err(TaskError::Cleanup {
            source: Box::new(source),
            cleanup: Box::new(cleanup),
        }),
        (result, Ok(())) | (Ok(()), result) => result,
    }
}

pub(super) fn register(tasks: &mut foundation_tasks::TaskSet) -> TaskResult {
    use Suite::{Database as DatabaseStore, DatabaseFiles, ObjectFiles, ObjectStore};
    tasks.add("start", "Start the local Cloudflare test service", start)?;
    tasks.add("stop", "Stop this project's local Cloudflare service", stop)?;
    tasks.add("service", "Run the local Cloudflare worker", worker)?;
    for (name, description, suites) in [
        (
            "test",
            "Test both Cloudflare stores",
            &[ObjectStore, DatabaseStore][..],
        ),
        (
            "test:r2",
            "Test the Cloudflare object store",
            &[ObjectStore],
        ),
        ("test:d1", "Test the Cloudflare database", &[DatabaseStore]),
        (
            "test:vfs",
            "Test both Cloudflare file systems",
            &[ObjectFiles, DatabaseFiles],
        ),
        (
            "test:vfs:r2",
            "Test the Cloudflare object file system",
            &[ObjectFiles],
        ),
        (
            "test:vfs:d1",
            "Test the Cloudflare database file system",
            &[DatabaseFiles],
        ),
    ] {
        tasks.add(name, description, move |ctx, args| test(ctx, args, suites))?;
    }
    Ok(())
}
