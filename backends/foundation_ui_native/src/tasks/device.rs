//! Scope device test processes and their temporary files to one task.
use super::{
    output::{artifact, query},
    NativeBuildEnv,
};
use foundation_tasks::{
    Cancellation, OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput,
    TaskResult,
};
use futures_util::future::{select, BoxFuture, Either};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    future::Future,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Default)]
pub(super) struct Options {
    vars: BTreeMap<&'static str, OsString>,
}
impl Options {
    pub(super) fn from_host() -> Self {
        let vars = [
            "ANDROID_HOME",
            "EWE_ANDROID_BOOT_TIMEOUT",
            "EWE_ANDROID_SERIAL",
            "EWE_THEME_CAPTURES",
            "EWE_TODO_RPC",
            "EWE_TODO_SOCKET",
            "EWE_TODO_BIN",
            "EWE_IOS_DEVICE",
            "EWE_SIMCTL_BIN",
            "EWE_AXE_BIN",
        ]
        .into_iter()
        .filter_map(|name| {
            std::env::var_os(name)
                .filter(|s| !s.is_empty())
                .map(|v| (name, v))
        })
        .collect();
        Self { vars }
    }
    pub(super) fn get(&self, key: &str) -> Option<&std::ffi::OsStr> {
        self.vars.get(key).map(OsString::as_os_str)
    }
}

pub(super) async fn text(ctx: &TaskContext, cmd: ProcessCommand) -> Result<String, TaskError> {
    Ok(String::from_utf8(query(ctx, cmd).await?)
        .map_err(TaskError::operation)?
        .trim()
        .to_string())
}

pub(super) fn cleanup(result: TaskResult, cleanup: TaskResult) -> TaskResult {
    match (result, cleanup) {
        (Err(e), Err(cleanup)) => Err(TaskError::Cleanup {
            source: Box::new(e),
            cleanup: Box::new(cleanup),
        }),
        (Err(e), _) | (_, Err(e)) => Err(e),
        _ => Ok(()),
    }
}
pub(super) async fn cleanup_command(ctx: &TaskContext, cmd: ProcessCommand) -> TaskResult {
    let env = ctx.require::<TaskEnv>()?;
    env.process
        .run(cmd, env.output.clone(), Cancellation::new())
        .await
}

pub(super) async fn background(
    ctx: &TaskContext,
    cmd: ProcessCommand,
    output: Arc<dyn TaskOutput>,
    work: impl Future<Output = TaskResult>,
) -> TaskResult {
    let env = ctx.require::<TaskEnv>()?;
    let stop = Cancellation::new();
    let child = env.process.run(cmd, output, stop.clone());
    let outcome = match select(child, Box::pin(work)).await {
        Either::Left((result, work)) => {
            ctx.cancellation().cancel();
            let cleanup_result = work.await;
            let result = result.and_then(|()| {
                Err(TaskError::operation(
                    "A background process exited before the test finished",
                ))
            });
            match cleanup_result {
                Err(TaskError::Cancelled) => result,
                other => cleanup(result, other),
            }
        }
        Either::Right((result, child)) => {
            stop.cancel();
            let ended = match child.await {
                Err(TaskError::Cancelled) | Ok(()) => Ok(()),
                other => other,
            };
            cleanup(result, ended)
        }
    };
    outcome
}

struct ServerOutput {
    output: Arc<dyn TaskOutput>,
    ready: async_channel::Sender<String>,
    line: Mutex<Vec<u8>>,
    limit: usize,
}
impl TaskOutput for ServerOutput {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if stream == OutputStream::Stdout && !self.ready.is_closed() {
                let mut line = self
                    .line
                    .lock()
                    .map_err(|_| io::Error::other("Server output is unavailable"))?;
                for &b in bytes {
                    if b == b'\n' {
                        if let Ok(s) = std::str::from_utf8(&line) {
                            if let Some(port) = s
                                .trim_end()
                                .strip_prefix("Open http://127.0.0.1:")
                                .and_then(|s| s.strip_suffix('/'))
                                .and_then(|s| s.parse::<u16>().ok())
                            {
                                if port != 0 {
                                    let _ = self.ready.try_send(format!("http://127.0.0.1:{port}"));
                                    self.ready.close();
                                }
                            }
                        }
                        line.clear();
                    } else if line.len() < self.limit {
                        line.push(b);
                    }
                }
            }
            self.output.write(stream, bytes).await
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.output.flush()
    }
}

pub(super) async fn server<F, Fut>(
    ctx: &TaskContext,
    root: &Path,
    exe: &Path,
    data: bool,
    work: F,
) -> TaskResult
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = TaskResult>,
{
    let (ready, address) = async_channel::bounded(1);
    let env = ctx.require::<NativeBuildEnv>()?;
    let sink = Arc::new(ServerOutput {
        output: ctx.require::<TaskEnv>()?.output.clone(),
        ready,
        line: Mutex::new(Vec::new()),
        limit: env.query_bytes,
    });
    let mut cmd = ProcessCommand::new(exe)
        .current_dir(root)
        .arg("127.0.0.1:0");
    if data {
        cmd = cmd.arg("--data-only");
    }
    background(ctx, cmd, sink, async {
        let wait = async { address.recv().await.map_err(TaskError::operation) };
        let deadline = async {
            sleep(ctx, env.server_wait).await?;
            Err(TaskError::operation(
                "The todo server did not report an address",
            ))
        };
        let url = futures_lite::future::race(wait, deadline).await?;
        work(url).await
    })
    .await
}

pub(super) async fn host(ctx: &TaskContext, root: &Path) -> Result<PathBuf, TaskError> {
    if let Some(path) = ctx.require::<NativeBuildEnv>()?.devices.get("EWE_TODO_BIN") {
        return Ok(PathBuf::from(path));
    }
    artifact(
        ctx,
        ProcessCommand::new("cargo").current_dir(root).args([
            "build",
            "--locked",
            "--profile",
            "uat",
            "-p",
            "ui_todo",
            "--bin",
            "ui_todo",
            "--message-format=json-render-diagnostics",
        ]),
        "ui_todo",
        &[],
    )
    .await
}

pub(super) async fn sleep(ctx: &TaskContext, duration: Duration) -> TaskResult {
    let host = ctx.context();
    let deadline = host
        .clock()
        .now()
        .checked_add(duration)
        .ok_or_else(|| TaskError::operation("Timer deadline exceeds the clock range"))?;
    let timer = host.require_env().map_err(TaskError::operation)?.timer();
    let wait = async {
        timer.sleep_until(host.clone(), deadline).await;
        Ok(())
    };
    let cancelled = async {
        ctx.cancellation().cancelled().await;
        Err(TaskError::Cancelled)
    };
    futures_lite::future::race(wait, cancelled).await
}

pub(super) async fn wait_for<F, Fut>(
    ctx: &TaskContext,
    timeout: Duration,
    delay: Duration,
    description: &str,
    mut probe: F,
) -> TaskResult
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<bool, TaskError>>,
{
    let clock = ctx.context().clock();
    let start = clock.elapsed();
    while clock.elapsed().saturating_sub(start) < timeout {
        match probe().await {
            Ok(true) => return Ok(()),
            Ok(false) | Err(TaskError::Exit { .. }) => {}
            Err(error) => return Err(error),
        }
        sleep(ctx, delay).await?;
    }
    Err(TaskError::operation(format!(
        "Timed out waiting for {description}"
    )))
}
