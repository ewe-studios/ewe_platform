//! Check every tracked Cargo lockfile outside the root workspace.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput, TaskResult, TaskSet,
};
use futures_util::future::BoxFuture;
use std::{
    io::{self, BufRead, BufReader},
    path::Path,
    sync::Arc,
};

#[derive(Args)]
struct Locks {}

struct Diagnostics(Arc<dyn TaskOutput>);

impl TaskOutput for Diagnostics {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        match stream {
            OutputStream::Stdout => Box::pin(async { Ok(()) }),
            OutputStream::Stderr => self.0.write(stream, bytes),
        }
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        self.0.flush()
    }
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "check-lockfiles",
        "Verify the nested Cargo lockfiles",
        check,
    )?;
    set.expose("check-lockfiles", &["check-lockfiles"])
}

async fn check(ctx: TaskContext, _: Locks) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let git = capture(
        &ctx,
        ProcessCommand::new("git").current_dir(&host.root).args([
            "ls-files",
            "-z",
            "--",
            ":(glob)**/Cargo.lock",
        ]),
    )
    .await?;
    let output = Arc::new(Diagnostics(ctx.require::<TaskEnv>()?.output.clone()));
    for name in BufReader::new(git).split(0) {
        let bytes = name.map_err(TaskError::operation)?;
        let path = Path::new(std::str::from_utf8(&bytes).map_err(TaskError::operation)?);
        if path == Path::new("Cargo.lock") {
            continue;
        }
        let manifest = path.with_file_name("Cargo.toml");
        println!("checking {}", manifest.display());
        ctx.exec_with_output(
            ProcessCommand::new("cargo")
                .current_dir(&host.root)
                .args([
                    "metadata",
                    "--locked",
                    "--format-version",
                    "1",
                    "--manifest-path",
                ])
                .arg(manifest),
            output.clone(),
        )
        .await?;
    }
    Ok(())
}
