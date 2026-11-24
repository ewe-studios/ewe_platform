//! Put the Git revisions recorded by the checkout into its submodule directories.
use crate::{workspace::capture, CiEnvironment};
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Deserialize;
use std::{
    fs,
    io::{self, BufReader, Read},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    Symlink,
    Submodule,
}
#[derive(Args)]
struct Link {
    paths: Vec<String>,
    #[arg(long)]
    lock: Option<PathBuf>,
    #[arg(long, default_value = ".")]
    workspace: PathBuf,
    #[arg(long,value_enum,default_value_t=Mode::Symlink)]
    mode: Mode,
}
#[derive(Deserialize)]
struct Lock {
    submodules: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    sha: String,
}

fn git(root: &Path) -> ProcessCommand {
    ProcessCommand::new("git")
        .arg("-c")
        .arg(format!("safe.directory={}", root.display()))
        .arg("-C")
        .arg(root)
}
async fn text(ctx: &TaskContext, cmd: ProcessCommand) -> Result<String, TaskError> {
    let mut data = String::new();
    capture(ctx, cmd)
        .await?
        .read_to_string(&mut data)
        .map_err(TaskError::operation)?;
    Ok(data)
}

fn remove_link(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        fs::remove_dir(path)
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(path)
    }
}

fn clear(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) if path.read_link().is_ok() => remove_link(path),
        Ok(meta) if meta.is_dir() => fs::remove_dir(path),
        Ok(_) => Err(io::Error::other(format!(
            "Expected an empty submodule directory at {}",
            path.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
async fn link(ctx: &TaskContext, src: &Path, dst: &Path) -> TaskResult {
    ctx.exec(
        ProcessCommand::new("cmd")
            .args([
                "/D",
                "/V:OFF",
                "/C",
                r#"mklink /J "%EWE_SUBMODULE_TARGET%" "%EWE_SUBMODULE_SOURCE%""#,
            ])
            .env("EWE_SUBMODULE_SOURCE", src)
            .env("EWE_SUBMODULE_TARGET", dst),
    )
    .await
}
#[cfg(not(windows))]
fn link(_ctx: &TaskContext, src: &Path, dst: &Path) -> std::future::Ready<TaskResult> {
    std::future::ready(std::os::unix::fs::symlink(src, dst).map_err(TaskError::operation))
}

async fn head(ctx: &TaskContext, path: &Path) -> Result<Option<String>, TaskError> {
    if !path.join(".git").exists() {
        return Ok(None);
    }
    let checkout = path.canonicalize().map_err(TaskError::operation)?;
    match text(ctx, git(&checkout).args(["rev-parse", "HEAD"])).await {
        Ok(head) => Ok(Some(head.trim().into())),
        Err(TaskError::Exit { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

async fn checkout(ctx: &TaskContext, root: &Path, path: &str, source: &Path) -> TaskResult {
    let keys = text(
        ctx,
        git(root).args([
            "config",
            "-z",
            "-f",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.path$",
        ]),
    )
    .await?;
    let key = keys
        .split('\0')
        .filter_map(|row| row.split_once('\n'))
        .find(|(_, name)| *name == path)
        .and_then(|(key, _)| key.strip_suffix(".path"))
        .ok_or_else(|| TaskError::operation(format!("No submodule name for {path}")))?;
    ctx.exec(
        git(root)
            .arg("config")
            .arg(format!("{key}.url"))
            .arg(source),
    )
    .await?;
    ctx.exec(
        git(root)
            .args(["-c", "protocol.file.allow=always", "-c"])
            .arg(format!("safe.directory={}", source.display()))
            .args(["submodule", "update", "--init", "--recursive", "--", path]),
    )
    .await
}

struct Checkout<'a> {
    ctx: &'a TaskContext,
    root: &'a Path,
    baked_root: Option<&'a Path>,
    entries: &'a [Entry],
    mode: Mode,
}
impl Checkout<'_> {
    async fn install(&self, name: &str) -> TaskResult {
        if name.is_empty()
            || !Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(TaskError::operation(format!(
                "Invalid submodule path {name:?}"
            )));
        }
        let ctx = self.ctx;
        let root = self.root;
        let entry = text(ctx, git(root).args(["ls-tree", "-z", "HEAD", "--", name])).await?;
        let sha = entry
            .trim_end_matches('\0')
            .split_once('\t')
            .filter(|(_, path)| *path == name)
            .and_then(|(fields, _)| fields.strip_prefix("160000 commit "))
            .ok_or_else(|| TaskError::operation(format!("{name} is not a submodule at HEAD")))?;
        let target = root.join(name);
        let parent = target
            .parent()
            .ok_or_else(|| TaskError::operation("Missing submodule parent"))?;
        let existing = parent
            .ancestors()
            .find(|path| path.exists())
            .ok_or_else(|| TaskError::operation("Missing workspace directory"))?;
        if !existing
            .canonicalize()
            .map_err(TaskError::operation)?
            .starts_with(root)
        {
            return Err(TaskError::operation(format!(
                "Submodule parent escapes the checkout: {name}"
            )));
        }
        let linked = target.read_link().is_ok();
        let present = head(ctx, &target).await?;
        if present.as_deref() == Some(sha) && (!linked || self.mode == Mode::Symlink) {
            println!("{name} {sha} already checked out");
            return Ok(());
        }
        let baked = self.entries.iter().find(|entry| entry.path == name);
        if let Some(source) = self
            .baked_root
            .filter(|_| baked.is_some_and(|e| e.sha == sha))
            .map(|r| r.join(name))
        {
            if head(ctx, &source).await?.as_deref() != Some(sha) {
                return Err(TaskError::operation(format!(
                    "Baked submodule {} does not contain {sha}",
                    source.display()
                )));
            }
            clear(&target).map_err(TaskError::operation)?;
            fs::create_dir_all(
                target
                    .parent()
                    .ok_or_else(|| TaskError::operation("Missing submodule parent"))?,
            )
            .map_err(TaskError::operation)?;
            match self.mode {
                Mode::Symlink => link(ctx, &source, &target).await?,
                Mode::Submodule => checkout(ctx, root, name, &source).await?,
            }
        } else {
            eprintln!("warning: {name} at {sha} is absent from the image. Fetching its recorded revision.");
            if linked {
                remove_link(&target).map_err(TaskError::operation)?;
            }
            ctx.exec(git(root).args([
                "submodule",
                "update",
                "--init",
                "--recursive",
                "--depth",
                "1",
                "--",
                name,
            ]))
            .await?;
        }
        if head(ctx, &target).await?.as_deref() != Some(sha) {
            return Err(TaskError::operation(format!(
                "Submodule {name} does not contain {sha}"
            )));
        }
        println!("{name} {sha}");
        Ok(())
    }
}

async fn run(ctx: TaskContext, args: Link) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let root = host
        .root
        .join(&args.workspace)
        .canonicalize()
        .map_err(TaskError::operation)?;
    let lock = args
        .lock
        .or_else(|| {
            host.get("PLATFORM_IMAGES_SUBMODULES_LOCK")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
        .map(|p| host.root.join(p).canonicalize())
        .transpose()
        .map_err(TaskError::operation)?;
    let entries = if let Some(path) = &lock {
        serde_json::from_reader::<_, Lock>(BufReader::new(
            fs::File::open(path).map_err(TaskError::operation)?,
        ))
        .map_err(TaskError::operation)?
        .submodules
    } else {
        Vec::new()
    };
    let paths = if args.paths.is_empty() {
        entries.iter().map(|e| e.path.clone()).collect()
    } else {
        args.paths
    };
    if paths.is_empty() {
        return Err(TaskError::operation("No submodule paths or baked entries"));
    }
    let work = Checkout {
        ctx: &ctx,
        root: &root,
        baked_root: lock.as_deref().and_then(Path::parent),
        entries: &entries,
        mode: args.mode,
    };
    for path in paths {
        work.install(&path).await?;
    }
    Ok(())
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "link-submodules",
        "Prepare the pinned workspace submodules",
        run,
    )?;
    set.expose("link-submodules", &["link-submodules"])
}
