//! Execute one package or a shard under its configured deadline.
use crate::{CiEnvironment, Settings, Workspace};
use clap::Args;
use foundation_tasks::{
    Cancellation, OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput,
    TaskResult, TaskSet,
};
use futures_util::{FutureExt, future::BoxFuture};
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

mod chromium;
mod guards;
mod recipes;

#[derive(Args)]
struct Options {
    #[arg(long,value_parser=["fmt","build","clippy","test","wasm"])]
    step: Vec<String>,
    #[arg(long)]
    profile: Option<String>,
    #[arg(long)]
    if_affected: bool,
    #[arg(long)]
    base: Option<String>,
    #[arg(long)]
    head: Option<String>,
}
#[derive(Args)]
struct Run {
    package: String,
    #[command(flatten)]
    options: Options,
}
#[derive(Args)]
struct Many {
    #[arg(required=true,num_args=1..)]
    packages: Vec<String>,
    #[command(flatten)]
    options: Options,
}
pub fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "run",
        "Run the configured checks for one package",
        |ctx, args: Run| execute(ctx, vec![args.package], args.options, false),
    )?;
    set.add(
        "run-many",
        "Run each package in a shard and summarize failures",
        |ctx, args: Many| {
            execute(
                ctx,
                args.packages
                    .iter()
                    .flat_map(|s| s.split_whitespace().map(str::to_owned))
                    .collect(),
                args.options,
                true,
            )
        },
    )?;
    for name in ["run", "run-many"] {
        set.expose(name, &[name])?;
    }
    chromium::register(set)?;
    guards::register(set)
}
struct CommandStep {
    cmd: ProcessCommand,
    expected_failure: Option<String>,
    deny: bool,
}
impl CommandStep {
    fn new(
        program: impl Into<std::ffi::OsString>,
        argv: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
    ) -> Self {
        Self {
            cmd: ProcessCommand::new(program).args(argv),
            expected_failure: None,
            deny: false,
        }
    }
}
struct Lines {
    pending: Vec<u8>,
    after_panic: bool,
    over_limit: bool,
    overlap: Vec<u8>,
}
struct Diagnostics {
    lines: [Lines; 2],
    kept: Vec<String>,
    matched: bool,
}
struct Output {
    forward: Arc<dyn TaskOutput>,
    inner: Mutex<Diagnostics>,
    expected: Option<Vec<u8>>,
    limit: usize,
    line_bytes: usize,
}
impl Output {
    fn new(forward: Arc<dyn TaskOutput>, expected: Option<&str>, settings: &Settings) -> Self {
        Self {
            forward,
            expected: expected.map(|s| s.as_bytes().to_vec()),
            inner: Mutex::new(Diagnostics {
                lines: std::array::from_fn(|_| Lines {
                    pending: Vec::new(),
                    after_panic: false,
                    over_limit: false,
                    overlap: Vec::new(),
                }),
                kept: Vec::new(),
                matched: false,
            }),
            limit: settings.failure_lines,
            line_bytes: settings.failure_line_bytes,
        }
    }
    fn consume(&self, stream: OutputStream, bytes: &[u8], finish: bool) -> io::Result<()> {
        let mut d = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("output lock poisoned"))?;
        let Diagnostics {
            lines,
            kept,
            matched,
        } = &mut *d;
        let line = &mut lines[usize::from(stream == OutputStream::Stderr)];
        if let Some(pattern) = &self.expected {
            if !*matched && !pattern.is_empty() {
                line.overlap.extend_from_slice(bytes);
                *matched = line.overlap.windows(pattern.len()).any(|w| w == pattern);
                let consumed = line.overlap.len().saturating_sub(pattern.len() - 1);
                line.overlap.drain(..consumed);
            }
        }
        for part in bytes.split_inclusive(|b| *b == b'\n') {
            let keep = self
                .line_bytes
                .saturating_sub(line.pending.len())
                .min(part.len());
            line.pending.extend_from_slice(&part[..keep]);
            line.over_limit |= keep < part.len();
            if part.ends_with(b"\n") || finish {
                let raw = String::from_utf8_lossy(&line.pending);
                let raw = raw.trim_end_matches(['\r', '\n']);
                let panic = raw.contains("panicked at");
                if kept.len() < self.limit
                    && (panic
                        || line.after_panic
                        || (raw.starts_with("test ") && raw.ends_with(" ... FAILED"))
                        || (raw.starts_with("---- ") && raw.ends_with(" stdout ----")))
                {
                    kept.push(if line.over_limit {
                        format!("{raw}…")
                    } else {
                        raw.to_string()
                    });
                }
                line.after_panic = panic;
                line.pending.clear();
                line.over_limit = false;
            }
        }
        Ok(())
    }
}
impl TaskOutput for Output {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.consume(stream, bytes, false)?;
            self.forward.write(stream, bytes).await
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            for s in [OutputStream::Stdout, OutputStream::Stderr] {
                self.consume(s, b"\n", true)?;
            }
            self.forward.flush().await
        })
    }
}
async fn execute(ctx: TaskContext, names: Vec<String>, options: Options, many: bool) -> TaskResult {
    let ws = Workspace::load(&ctx).await?;
    let env = ctx.require::<CiEnvironment>()?;
    let steps = if options.step.is_empty() {
        ["fmt", "build", "clippy", "test", "wasm"]
            .map(str::to_owned)
            .to_vec()
    } else {
        options.step
    };
    let profile = options
        .profile
        .or_else(|| env.text("CI_PROFILE"))
        .unwrap_or_else(|| "uat".into());
    let changed = if options.if_affected {
        match ws
            .affected(&ctx, options.base.as_deref(), options.head.as_deref())
            .await
        {
            Ok((_, reasons)) => Some(reasons),
            Err(TaskError::Cancelled) => return Err(TaskError::Cancelled),
            Err(e) => {
                eprintln!("cannot tell what changed ({e}); running");
                None
            }
        }
    } else {
        None
    };
    let mut failures = Vec::new();
    let github = env.text("GITHUB_ACTIONS").is_some();
    for name in &names {
        if many {
            println!("==== {name} ====");
        }
        if changed.as_ref().is_some_and(|r| !r.contains_key(name)) {
            println!("{name}: not affected, skipping");
            continue;
        }
        let settings = ws
            .settings
            .get(name)
            .ok_or_else(|| TaskError::operation(format!("unknown package: {name}")))?;
        if let Some(why) = &settings.skip {
            println!("skipping {name}: {why}");
            continue;
        }
        let os = std::env::consts::OS;
        if matches!(os, "macos" | "windows") && settings.platform_runs(os).is_none() {
            println!("skipping {name}: no test runs for {os}");
            continue;
        }
        let package = PackageRun {
            ctx: &ctx,
            ws: &ws,
            env: &env,
            settings,
            name,
            os,
            github,
            profile: &profile,
        };
        let reports = package.run(&steps).await?;
        for (msg, lines) in &reports {
            eprintln!("{msg}");
            for line in lines {
                eprintln!("    {line}");
            }
        }
        if !reports.is_empty() {
            failures.push((name, reports));
        }
    }
    if many {
        println!(
            "==== summary: {} package(s), {} failed ====",
            names.len(),
            failures.len()
        );
    }
    if failures.is_empty() {
        return Ok(());
    }
    let mut names = Vec::new();
    for (name, reports) in failures {
        names.push(name.as_str());
        if many {
            for (note, lines) in reports {
                eprintln!("{note}");
                for line in lines {
                    eprintln!("    {line}");
                }
            }
        }
    }
    Err(TaskError::operation(format!(
        "failed packages: {}",
        names.join(", ")
    )))
}

type Report = (String, Vec<String>);

struct PackageRun<'a> {
    ctx: &'a TaskContext,
    ws: &'a Workspace,
    env: &'a CiEnvironment,
    settings: &'a Settings,
    name: &'a str,
    os: &'a str,
    github: bool,
    profile: &'a str,
}

impl PackageRun<'_> {
    async fn run(&self, steps: &[String]) -> Result<Vec<Report>, TaskError> {
        let minutes = u64::try_from(self.settings.timeout_minutes).map_err(TaskError::operation)?;
        let duration = Duration::from_secs(
            minutes
                .checked_mul(60)
                .ok_or_else(|| TaskError::operation("Invalid package timeout"))?,
        );
        let deadline = self
            .ctx
            .context()
            .clock()
            .now()
            .checked_add(duration)
            .ok_or_else(|| TaskError::operation("Invalid package deadline"))?;
        let target = self
            .env
            .get("CARGO_TARGET_DIR")
            .filter(|s| !s.is_empty())
            .map_or_else(
                || self.ws.root.join("target"),
                |p| Path::new(&p).join(self.ws.root.file_name().unwrap_or_default()),
            );
        println!("{}: target dir {}", self.name, target.display());
        let mut reports = Vec::new();
        for step in steps {
            let mode = match step.as_str() {
                "fmt" => &self.settings.fmt,
                "build" => &self.settings.build,
                "clippy" => &self.settings.clippy,
                "wasm" => &self.settings.wasm,
                _ => &self.settings.test,
            };
            if mode == "off" {
                continue;
            }
            for entry in recipes::commands(
                &self.ws.root,
                &self.ws.packages[self.name],
                self.settings,
                self.profile,
                self.os,
                step,
                self.env,
            )? {
                let (report, expired) = self
                    .run_command(entry, step, mode, &target, deadline)
                    .await?;
                if let Some(report) = report {
                    reports.push(report);
                }
                if expired {
                    return Ok(reports);
                }
            }
        }
        Ok(reports)
    }

    fn prepare_command(
        &self,
        command: ProcessCommand,
        target: &Path,
        deny: bool,
    ) -> Result<(ProcessCommand, String), TaskError> {
        let mut cmd = command.current_dir(&self.ws.root);
        for (key, value) in self.settings.platform_env(self.os) {
            if self.env.get(key).is_none() {
                cmd = cmd.env(key, value);
            }
        }
        cmd = cmd.env("CARGO_TARGET_DIR", target);
        if deny {
            cmd = cmd.env("RUSTFLAGS", "-D warnings");
        }
        let label = shlex::try_join(
            std::iter::once(&cmd.program)
                .chain(&cmd.args)
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .iter()
                .map(AsRef::as_ref),
        )
        .map_err(TaskError::operation)?;
        Ok((cmd, label))
    }

    async fn run_command(
        &self,
        entry: CommandStep,
        step: &str,
        mode: &str,
        target: &Path,
        deadline: std::time::Instant,
    ) -> Result<(Option<Report>, bool), TaskError> {
        let (cmd, label) = self.prepare_command(entry.cmd, target, entry.deny)?;
        if self.github {
            println!("::group::{step}: {label}");
        } else {
            println!("$ {label}");
        }
        let task_env = self.ctx.require::<TaskEnv>()?;
        let sink = Arc::new(Output::new(
            task_env.output.clone(),
            entry.expected_failure.as_deref(),
            self.settings,
        ));
        let timer = self
            .ctx
            .context()
            .env()
            .ok_or(TaskError::MissingService("timer"))?
            .timer();
        let cancel = Cancellation::new();
        let mut child = task_env
            .process
            .run(cmd, sink.clone(), cancel.clone())
            .fuse();
        let mut timeout = timer
            .sleep_until(self.ctx.context().clone(), deadline)
            .fuse();
        let mut interrupted = self.ctx.cancellation().cancelled().boxed().fuse();
        let (result, expired) = futures_util::select_biased! {
            () = interrupted => {
                cancel.cancel();
                return match child.await {
                    Ok(()) | Err(TaskError::Cancelled) => Err(TaskError::Cancelled),
                    Err(cleanup) => Err(cleanup),
                };
            }
            () = timeout => {
                cancel.cancel();
                (child.await,true)
            }
            result = child => (result,false),
        };
        if self.github {
            println!("::endgroup::");
        }
        let diag = sink
            .inner
            .lock()
            .map_err(|_| TaskError::operation("output lock poisoned"))?;
        let name = self.name;
        let message = if expired {
            let minutes = self.settings.timeout_minutes;
            match result {
                Err(error) if !matches!(error, TaskError::Cancelled) => {
                    format!("{name}: {label} timed out after {minutes} minutes ({error})")
                }
                _ => format!("{name}: {label} timed out after {minutes} minutes"),
            }
        } else {
            match (result, entry.expected_failure) {
                (Ok(()), None) => return Ok((None, false)),
                (Err(TaskError::Exit { .. }), Some(text)) if text.is_empty() || diag.matched => {
                    return Ok((None, false));
                }
                (Ok(()), Some(_)) => format!("{name}: {label} unexpectedly succeeded"),
                (Err(error), _) => format!("{name}: {label} failed ({error})"),
            }
        };
        if mode == "enforce" || expired {
            eprintln!(
                "{}{message}",
                if self.github { "::error::" } else { "error: " }
            );
            Ok((Some((message, diag.kept.clone())), expired))
        } else {
            let reason = self
                .settings
                .reason
                .as_deref()
                .unwrap_or("report-only step");
            eprintln!(
                "{}{message} ({reason})",
                if self.github {
                    "::warning::"
                } else {
                    "warning: "
                }
            );
            Ok((None, false))
        }
    }
}
