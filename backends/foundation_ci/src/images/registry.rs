//! Inspect image manifests and attach tags with the installed Docker client.
use super::report;
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskError, TaskOutput, TaskResult, TaskSet,
};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Seek, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Args)]
struct Exists {
    reference: String,
    #[arg(long)]
    github_output: bool,
    /// Maximum registry attempts. The default matches the image release workflow.
    #[arg(long, default_value_t = 5)]
    attempts: u32,
    /// Initial delay in seconds. Each rate limit adds one delay interval.
    #[arg(long, default_value_t = 15)]
    retry_seconds: u64,
}
#[derive(Args)]
struct Promote {
    reference: String,
    #[arg(long)]
    tags: String,
}
#[derive(Serialize)]
pub(super) struct Inspection {
    pub exists: bool,
    pub digest: String,
}
#[derive(Deserialize)]
struct Manifest {
    digest: String,
}

struct Capture {
    stdout: Mutex<File>,
    stderr: Mutex<File>,
}
impl TaskOutput for Capture {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            let file = match stream {
                OutputStream::Stdout => &self.stdout,
                OutputStream::Stderr => &self.stderr,
            };
            file.lock()
                .map_err(|_| io::Error::other("Image output unavailable"))?
                .write_all(bytes)
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct Failure {
    limited: bool,
    missing: bool,
    denied: bool,
}
fn classify(reader: File) -> io::Result<Failure> {
    let mut result = Failure::default();
    let mut reader = BufReader::new(reader);
    let mut tail = Vec::new();
    let groups: [(&[&[u8]], &mut bool); 3] = [
        (
            &[b"429", b"too many requests", b"toomanyrequests"],
            &mut result.limited,
        ),
        (
            &[
                b"not found",
                b"manifest unknown",
                b"name unknown",
                b"no such manifest",
            ],
            &mut result.missing,
        ),
        (
            &[b"unauthorized", b"denied", b"authentication required"],
            &mut result.denied,
        ),
    ];
    let keep = groups
        .iter()
        .flat_map(|(keys, _)| keys.iter())
        .map(|s| s.len())
        .max()
        .unwrap_or(1)
        - 1;
    let mut groups = groups;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            break;
        }
        let count = chunk.len();
        tail.extend(chunk.iter().map(u8::to_ascii_lowercase));
        for (keys, flag) in &mut groups {
            **flag |= keys
                .iter()
                .any(|key| tail.windows(key.len()).any(|w| w == *key));
        }
        tail.drain(..tail.len().saturating_sub(keep));
        reader.consume(count);
    }
    Ok(result)
}

async fn inspect(ctx: &TaskContext, args: &Exists) -> Result<Inspection, TaskError> {
    if args.attempts == 0 {
        return Err(TaskError::operation(
            "Registry inspection needs at least one attempt",
        ));
    }
    let cmd = ProcessCommand::new("docker").args([
        "buildx",
        "imagetools",
        "inspect",
        "--format",
        "{{json .Manifest}}",
        "--",
        &args.reference,
    ]);
    for attempt in 1..=args.attempts {
        let mut stdout = tempfile::tempfile().map_err(TaskError::operation)?;
        let mut stderr = tempfile::tempfile().map_err(TaskError::operation)?;
        let capture = Capture {
            stdout: Mutex::new(stdout.try_clone().map_err(TaskError::operation)?),
            stderr: Mutex::new(stderr.try_clone().map_err(TaskError::operation)?),
        };
        match ctx.exec_with_output(cmd.clone(), Arc::new(capture)).await {
            Ok(()) => {
                stdout.rewind().map_err(TaskError::operation)?;
                let manifest: Manifest = serde_json::from_reader(BufReader::new(stdout))
                    .map_err(TaskError::operation)?;
                if manifest.digest.is_empty() {
                    return Err(TaskError::operation(
                        "Image manifest contains an empty digest",
                    ));
                }
                return Ok(Inspection {
                    exists: true,
                    digest: manifest.digest,
                });
            }
            Err(error @ TaskError::Exit { .. }) => {
                stderr.rewind().map_err(TaskError::operation)?;
                let reason = classify(stderr.try_clone().map_err(TaskError::operation)?)
                    .map_err(TaskError::operation)?;
                if reason.limited && attempt < args.attempts {
                    let wait = Duration::from_secs(args.retry_seconds)
                        .checked_mul(attempt)
                        .ok_or_else(|| TaskError::operation("Invalid registry retry interval"))?;
                    eprintln!(
                        "::warning::Registry rate limit for {}. Retry in {wait:?}.",
                        args.reference
                    );
                    crate::execution::sleep(ctx, wait).await?;
                    continue;
                }
                if reason.denied || reason.missing {
                    if reason.denied {
                        eprintln!("::warning::Registry access denied for {}. Treating the image as missing.",args.reference);
                    }
                    return Ok(Inspection {
                        exists: false,
                        digest: String::new(),
                    });
                }
                stderr.rewind().map_err(TaskError::operation)?;
                io::copy(&mut stderr, &mut io::stderr().lock()).map_err(TaskError::operation)?;
                return Err(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(TaskError::operation(
        "Registry inspection exhausted its attempts",
    ))
}

pub(super) async fn lookup(ctx: &TaskContext, reference: &str) -> Result<Inspection, TaskError> {
    let options = Exists {
        reference: reference.to_owned(),
        github_output: false,
        attempts: 5,
        retry_seconds: 15,
    };
    inspect(ctx, &options).await
}

async fn exists(ctx: TaskContext, args: Exists) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    report(&host, &inspect(&ctx, &args).await?, args.github_output)
}

async fn promote(ctx: TaskContext, args: Promote) -> TaskResult {
    let image = args.reference.split('@').next().unwrap_or_default();
    let start = image.rfind('/').map_or(0, |index| index + 1);
    let end = image[start..].find(':').map_or(image.len(), |i| start + i);
    let repo = &image[..end];
    if repo.is_empty() {
        return Err(TaskError::operation("Missing image repository"));
    }
    let mut tags = args.tags.split_whitespace().peekable();
    if tags.peek().is_none() {
        return Err(TaskError::operation("--tags is empty"));
    }
    let mut cmd = ProcessCommand::new("docker").args([
        "buildx",
        "imagetools",
        "create",
        "--prefer-index=false",
    ]);
    for tag in tags {
        cmd = cmd.arg("-t").arg(format!("{repo}:{tag}"));
    }
    ctx.exec(cmd.arg("--").arg(args.reference)).await
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("exists", "Inspect an image manifest", exists)?;
    set.add("promote", "Attach tags to an existing image", promote)?;
    set.expose("exists", &["image-exists"])?;
    set.expose("promote", &["image-promote"])
}
