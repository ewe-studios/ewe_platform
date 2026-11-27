//! Transfer project files through the selected guest's loopback port.
use super::{
    connection::{command, copy, Connection},
    Guest,
};
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Push {
    #[command(flatten)]
    connection: Connection,
    #[arg(long, default_value = ".")]
    src: PathBuf,
    #[arg(long)]
    dest: String,
    /// Archive exclusions relative to the source directory.
    #[arg(long)]
    exclude: Vec<String>,
    /// Use the supplied exclusion list even when it is empty.
    #[arg(long)]
    explicit_excludes: bool,
}

#[derive(Args)]
struct Pull {
    #[command(flatten)]
    connection: Connection,
    #[arg(long)]
    src: String,
    #[arg(long)]
    dest: PathBuf,
}

fn destination(value: &str) -> TaskResult {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(TaskError::operation(
            "Expected a guest path without control characters",
        ));
    }
    Ok(())
}

async fn push(ctx: TaskContext, args: Push) -> TaskResult {
    destination(&args.dest)?;
    let source = ctx.require::<CiEnvironment>()?.root.join(&args.src);
    let tmp = tempfile::Builder::new()
        .prefix("ewe-guest-")
        .tempdir()
        .map_err(TaskError::operation)?;
    let archive = tmp.path().join("project.tar.gz");
    let mut tar = ProcessCommand::new("tar").arg("-czf").arg(&archive);
    let mut exclusions = args.exclude;
    if exclusions.is_empty() && !args.explicit_excludes {
        exclusions.push("target".into());
    }
    for pattern in exclusions {
        tar = tar.arg(format!("--exclude=./{pattern}"));
    }
    ctx.exec(tar.arg("-C").arg(&source).arg(".")).await?;
    match args.connection.os {
        Guest::Macos => {
            let path = args.dest.replace('\'', "'\\''");
            let probe = format!("mkdir -p -- '{path}' && tar -xzf - -C '{path}'");
            ctx.exec(command(&ctx, &args.connection, &probe)?.file_input(&archive))
                .await?;
        }
        Guest::Windows => {
            let name = tmp
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| TaskError::operation("Invalid temporary archive name"))?;
            let name = format!("{name}.tar.gz");
            let upload = copy(&ctx, &args.connection)?
                .arg("--")
                .arg(&archive)
                .arg(format!("127.0.0.1:{name}"));
            // The upload uses the guest home directory before the target directory exists.
            let destination = args.dest.replace('\'', "''");
            let unpack = format!(
                "$ErrorActionPreference='Stop'; $a=Join-Path $HOME '{name}'; try {{ [IO.Directory]::CreateDirectory('{destination}') | Out-Null; tar.exe -xzf $a -C '{destination}'; exit $LASTEXITCODE }} finally {{ Remove-Item -LiteralPath $a -ErrorAction Stop }}"
            );
            if let Err(source) = ctx.exec(upload).await {
                if matches!(source, TaskError::Cancelled) {
                    return Err(source);
                }
                let clean = format!(
                    "$ErrorActionPreference='Stop'; $a=Join-Path $HOME '{name}'; if (Test-Path -LiteralPath $a) {{ Remove-Item -LiteralPath $a }}"
                );
                return match ctx.exec(command(&ctx, &args.connection, &clean)?).await {
                    Ok(()) => Err(source),
                    Err(cleanup) => Err(TaskError::Cleanup {
                        source: Box::new(source),
                        cleanup: Box::new(cleanup),
                    }),
                };
            }
            ctx.exec(command(&ctx, &args.connection, &unpack)?).await?;
        }
    }
    println!("pushed {} to {}", source.display(), args.dest);
    Ok(())
}

// Merge existing directories while preserving local files outside the requested transfer.
fn collect(
    stage: &Path,
    target: &Path,
    relative: &Path,
    items: &mut Vec<(PathBuf, PathBuf)>,
) -> io::Result<()> {
    for entry in fs::read_dir(stage.join("incoming").join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        let dest = target.join(&path);
        let existing_dir = match fs::symlink_metadata(&dest) {
            Ok(info) => info.is_dir(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if entry.file_type()?.is_dir() && existing_dir {
            collect(stage, target, &path, items)?;
        } else {
            items.push((Path::new("incoming").join(path), dest));
        }
    }
    Ok(())
}

async fn pull(ctx: TaskContext, args: Pull) -> TaskResult {
    destination(&args.src)?;
    let dest = ctx.require::<CiEnvironment>()?.root.join(args.dest);
    fs::create_dir_all(&dest).map_err(TaskError::operation)?;
    let stage = tempfile::tempdir_in(&dest).map_err(TaskError::operation)?;
    let incoming = stage.path().join("incoming");
    fs::create_dir(&incoming).map_err(TaskError::operation)?;
    ctx.exec(
        copy(&ctx, &args.connection)?
            .args(["-r", "--"])
            .arg(format!("127.0.0.1:{}", args.src))
            .arg(&incoming),
    )
    .await?;
    let mut items = Vec::new();
    collect(stage.path(), &dest, Path::new(""), &mut items).map_err(TaskError::operation)?;
    crate::staging::publish(stage, &dest, items, false)
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("vm-push", "Copy a project directory into the guest", push)?;
    set.add("vm-pull", "Copy guest files into a local directory", pull)
}
