//! Install the pinned release tools and select the compiler for each release job.
use crate::{workspace::capture, CiEnvironment};
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Deserialize;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Publish,
    PullRequest,
}
#[derive(Args)]
struct Setup {
    #[arg(long)]
    mode: Mode,
    #[arg(long)]
    version: String,
    #[arg(long)]
    cache: PathBuf,
    #[arg(long)]
    github_env: bool,
}
#[derive(Deserialize)]
struct File {
    toolchain: Toolchain,
}
#[derive(Deserialize)]
struct Toolchain {
    channel: String,
}

pub(super) fn register(tasks: &mut TaskSet) -> TaskResult {
    tasks.add(
        "release",
        "Install the release tool and select its compiler",
        run,
    )?;
    tasks.expose("release", &["toolchain-release"])
}

fn channel(value: &str) -> Result<&str, TaskError> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('-')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
    {
        return Err(TaskError::operation("Invalid Rust toolchain channel"));
    }
    Ok(value)
}
fn append(path: Option<std::ffi::OsString>, line: &str) -> TaskResult {
    let path = path
        .filter(|f| !f.is_empty())
        .ok_or_else(|| TaskError::operation("Missing GitHub environment file"))?;
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .map_err(TaskError::operation)?,
        "{line}"
    )
    .map_err(TaskError::operation)
}
async fn install(ctx: &TaskContext, root: &Path, channel: &str) -> TaskResult {
    ctx.exec(ProcessCommand::new("rustup").current_dir(root).args([
        "toolchain",
        "install",
        channel,
        "--profile",
        "minimal",
        "--no-self-update",
    ]))
    .await
}
async fn cached(ctx: &TaskContext, path: &Path, expected: &str) -> Result<bool, TaskError> {
    if !path.is_file() {
        return Ok(false);
    }
    let mut line = String::new();
    let file = match capture(ctx, ProcessCommand::new(path).arg("--version")).await {
        Ok(f) => f,
        Err(TaskError::Exit { .. }) => return Ok(false),
        Err(error) => return Err(error),
    };
    BufReader::new(file)
        .read_line(&mut line)
        .map_err(TaskError::operation)?;
    Ok(line.split_whitespace().eq(["release-plz", expected]))
}
async fn run(ctx: TaskContext, args: Setup) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let root = &host.root;
    let text =
        fs::read_to_string(root.join("ci/release-rust-toolchain")).map_err(TaskError::operation)?;
    let stable = channel(&text)?;
    let selected = match args.mode {
        Mode::Publish => stable.to_owned(),
        Mode::PullRequest => {
            let tc: File = toml::from_str(
                &fs::read_to_string(root.join("rust-toolchain.toml"))
                    .map_err(TaskError::operation)?,
            )
            .map_err(TaskError::operation)?;
            channel(&tc.toolchain.channel)?.to_owned()
        }
    };
    install(&ctx, root, &selected).await?;
    let cache = root.join(&args.cache);
    let bin = cache.join("bin");
    let executable = bin.join(format!("release-plz{}", std::env::consts::EXE_SUFFIX));
    if !cached(&ctx, &executable, &args.version).await? {
        if selected != stable {
            install(&ctx, root, stable).await?;
        }
        ctx.exec(
            ProcessCommand::new("cargo")
                .current_dir(root)
                .env("RUSTUP_TOOLCHAIN", stable)
                .args([
                    "install",
                    "--force",
                    "release-plz",
                    "--version",
                    &args.version,
                    "--locked",
                    "--root",
                ])
                .arg(&cache),
        )
        .await?;
        if !cached(&ctx, &executable, &args.version).await? {
            return Err(TaskError::operation(
                "Installed release-plz version differs from its pin",
            ));
        }
    }
    if args.github_env {
        append(host.get("GITHUB_PATH"), &bin.display().to_string())?;
        append(
            host.get("GITHUB_ENV"),
            &format!("RUSTUP_TOOLCHAIN={selected}"),
        )?;
        if matches!(args.mode, Mode::PullRequest) {
            append(
                host.get("GITHUB_ENV"),
                "CARGO_UNSTABLE_CODEGEN_BACKEND=true",
            )?;
        }
    }
    println!("release-plz {} uses {}", args.version, selected);
    Ok(())
}
