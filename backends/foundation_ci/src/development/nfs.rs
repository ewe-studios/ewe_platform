//! Run the native Network File System tests and retain their complete log.
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput, TaskResult,
};
use futures_util::future::BoxFuture;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Seek, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Args)]
pub(super) struct Options {
    /// File that receives the combined Cargo output.
    #[arg(long)]
    log: Option<PathBuf>,
}

struct Log {
    file: Mutex<File>,
    output: Arc<dyn TaskOutput>,
}
impl TaskOutput for Log {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.file
                .lock()
                .map_err(|_| io::Error::other("NFS test log is unavailable"))?
                .write_all(bytes)?;
            self.output.write(stream, bytes).await
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move {
            self.file
                .lock()
                .map_err(|_| io::Error::other("NFS test log is unavailable"))?
                .flush()?;
            self.output.flush().await
        })
    }
}

pub(super) async fn run(ctx: TaskContext, args: Options) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let path = args.log.map_or_else(
        || std::env::temp_dir().join("vfs-nfs-test.log"),
        |path| host.root.join(path),
    );
    let mut staged = tempfile::NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| TaskError::operation("Invalid NFS test log path"))?,
    )
    .map_err(TaskError::operation)?;
    let sink = Arc::new(Log {
        file: Mutex::new(staged.reopen().map_err(TaskError::operation)?),
        output: ctx.require::<TaskEnv>()?.output.clone(),
    });
    let cargo = ProcessCommand::new("cargo")
        .current_dir(&host.root)
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "test",
            "--locked",
            "--package",
            "foundation_nativeapis",
            "--features",
            "vfs-native,vfs-nfs",
            "--test",
            "nfs_vfs",
        ]);
    let result = ctx.exec_with_output(cargo, sink).await;
    staged.rewind().map_err(TaskError::operation)?;
    let mut found = false;
    for line in BufReader::new(staged.as_file()).lines() {
        let line = line.map_err(TaskError::operation)?;
        if line.starts_with("test ")
            && [" ... ok", " ... ignored", " ... FAILED"]
                .iter()
                .any(|end| line.ends_with(end))
        {
            found = true;
            break;
        }
    }
    let stored = staged.persist(path).map_err(TaskError::operation);
    match (result, stored) {
        (Err(source), Err(cleanup)) => Err(TaskError::Cleanup {
            source: Box::new(source),
            cleanup: Box::new(cleanup),
        }),
        (Err(e), _) | (_, Err(e)) => Err(e),
        (Ok(()), Ok(_)) if !found => {
            Err(TaskError::operation("Cargo did not report any NFS tests"))
        }
        (Ok(()), Ok(_)) => Ok(()),
    }
}
