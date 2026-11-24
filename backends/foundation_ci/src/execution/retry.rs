//! Retry a release command while it continues to publish new packages.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput, TaskResult, TaskSet,
};
use futures_util::future::BoxFuture;
use regex::bytes::Regex;
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

#[derive(Args)]
struct PublishAttempts {
    #[arg(long, default_value_t = 5)]
    max_failures: u32,
    #[arg(long, default_value_t = 30.0)]
    base_delay: f64,
    #[arg(long, default_value_t = 900.0)]
    max_delay: f64,
    #[arg(long, default_value_t = 5.0)]
    retry_margin: f64,
    #[arg(long, default_value_t = 12000.0)]
    deadline: f64,
    #[arg(long, default_value = "git ls-remote --tags origin")]
    progress_cmd: String,
    #[arg(long)]
    dry_sleep: bool,
    #[arg(last = true)]
    command: Vec<OsString>,
}

#[derive(Default)]
struct Observed {
    tails: [Vec<u8>; 2],
    retry_at: Option<SystemTime>,
    limited: bool,
}
struct Output {
    forward: Arc<dyn TaskOutput>,
    watch: Mutex<Observed>,
    hint: Regex,
    rate: Regex,
}
impl Output {
    fn new(forward: Arc<dyn TaskOutput>) -> Result<Self, TaskError> {
        Ok(Self{forward,watch:Mutex::default(),hint:Regex::new(r"try again after ([A-Z][a-z]{2}, \d{1,2} [A-Z][a-z]{2} \d{4} \d{2}:\d{2}:\d{2} GMT)").map_err(TaskError::operation)?,
            rate:Regex::new(r"\b429\b|Too Many Requests").map_err(TaskError::operation)?})
    }
    fn observe(&self, stream: OutputStream, bytes: &[u8]) -> io::Result<()> {
        let mut watch = self
            .watch
            .lock()
            .map_err(|_| io::Error::other("Unavailable retry output"))?;
        let index = usize::from(stream == OutputStream::Stderr);
        let mut data = std::mem::take(&mut watch.tails[index]);
        data.extend_from_slice(bytes);
        watch.limited |= self.rate.is_match(&data);
        if let Some(found) = self.hint.captures_iter(&data).last() {
            let text = std::str::from_utf8(&found[1]).map_err(io::Error::other)?;
            watch.retry_at = Some(
                chrono::DateTime::parse_from_rfc2822(text)
                    .map_err(io::Error::other)?
                    .into(),
            );
        }
        // This is the length of the longest supported hint before its last byte arrives.
        let keep = "try again after Wed, 31 Dec 9999 23:59:59 GMT".len() - 1;
        watch.tails[index] = data[data.len().saturating_sub(keep)..].to_vec();
        Ok(())
    }
    fn hint(&self) -> Result<(Option<SystemTime>, bool), TaskError> {
        let w = self
            .watch
            .lock()
            .map_err(|_| TaskError::operation("Unavailable retry output"))?;
        Ok((w.retry_at, w.limited))
    }
}
impl TaskOutput for Output {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.observe(stream, bytes)?;
            self.forward.write(stream, bytes).await
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.forward.flush()
    }
}

async fn progress(ctx: &TaskContext, cmd: &ProcessCommand) -> Result<Option<usize>, TaskError> {
    let data = match capture(ctx, cmd.clone()).await {
        Ok(file) => file,
        Err(TaskError::Exit { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut count = 0;
    for line in BufReader::new(data).lines() {
        if !line.map_err(TaskError::operation)?.trim().is_empty() {
            count += 1;
        }
    }
    Ok(Some(count))
}

fn seconds(value: f64) -> Result<Duration, TaskError> {
    Duration::try_from_secs_f64(value).map_err(TaskError::operation)
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "publish-with-retry",
        "Retry a release command while it makes progress",
        run,
    )?;
    set.expose("publish-with-retry", &["publish-with-retry"])
}

fn commands(
    args: &PublishAttempts,
    root: &std::path::Path,
) -> Result<(ProcessCommand, ProcessCommand), TaskError> {
    let (program, arguments) = args
        .command
        .split_first()
        .ok_or_else(|| TaskError::operation("Give the publish command after --"))?;
    let command = ProcessCommand::new(program)
        .args(arguments.iter().cloned())
        .current_dir(root);
    let words = shlex::split(&args.progress_cmd)
        .ok_or_else(|| TaskError::operation("Invalid progress command"))?;
    let (program, arguments) = words
        .split_first()
        .ok_or_else(|| TaskError::operation("Missing progress command"))?;
    let query = ProcessCommand::new(program)
        .args(arguments.iter().cloned())
        .current_dir(root);
    Ok((command, query))
}

async fn run(ctx: TaskContext, args: PublishAttempts) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let (command, query) = commands(&args, &host.root)?;
    let base = seconds(args.base_delay)?;
    let max = seconds(args.max_delay)?;
    let margin = seconds(args.retry_margin)?;
    let deadline = seconds(args.deadline)?;
    if args.max_failures == 0 || base.is_zero() || max < base {
        return Err(TaskError::operation("Invalid failure or delay settings"));
    }
    let clock = ctx.context().clock();
    let start = clock.now();
    let mut skipped = Duration::ZERO;
    let mut backoff = base;
    let mut failures = 0;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let before = progress(&ctx, &query).await?;
        let sink = Arc::new(Output::new(ctx.require::<TaskEnv>()?.output.clone())?);
        println!("::group::publish attempt {attempt}");
        let result = ctx.exec_with_output(command.clone(), sink.clone()).await;
        println!("::endgroup::");
        let error = match result {
            Ok(()) => {
                println!("publish succeeded on attempt {attempt}");
                return Ok(());
            }
            Err(error @ TaskError::Exit { .. }) => error,
            Err(error) => return Err(error),
        };
        let after = progress(&ctx, &query).await?;
        let delta = before.zip(after).map(|(a, b)| b.saturating_sub(a));
        if delta.is_some_and(|value| value > 0) {
            failures = 0;
            backoff = base;
        } else {
            failures += 1;
        }
        println!("attempt {attempt} failed with code {}; published {} new; {failures} failed attempt(s) in a row with no progress",error.exit_code(),delta.map_or_else(||"?".into(),|n|n.to_string()));
        if failures >= args.max_failures {
            println!("::error::publish failed {failures} times in a row with no progress");
            return Err(error);
        }
        let (hint, limited) = sink.hint()?;
        let (wait, reason) = if let Some(time) = hint {
            (
                time.duration_since(clock.wall_now())
                    .unwrap_or_default()
                    .saturating_add(margin),
                "the server's retry time",
            )
        } else {
            let wait = backoff;
            backoff = backoff
                .checked_mul(2)
                .filter(|next| *next <= max)
                .unwrap_or(base);
            (
                wait,
                if limited {
                    "rate limited, no retry time"
                } else {
                    "backoff"
                },
            )
        };
        if clock
            .now()
            .saturating_duration_since(start)
            .saturating_add(skipped)
            .saturating_add(wait)
            > deadline
        {
            println!(
                "::warning::the next wait ({:.0}s, {reason}) ends after the deadline ({:.0}s)",
                wait.as_secs_f64(),
                deadline.as_secs_f64()
            );
            return Err(error);
        }
        println!("waiting {:.0}s ({reason})", wait.as_secs_f64());
        if args.dry_sleep {
            skipped = skipped.saturating_add(wait);
        } else {
            super::sleep(&ctx, wait).await?;
        }
    }
}
