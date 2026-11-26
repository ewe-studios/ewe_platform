//! Record release tag names and start the binary workflow for new tags.
use crate::{workspace::capture, CiEnvironment};
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, ValueEnum)]
enum Product {
    Platform,
    Browser,
}
impl Product {
    fn prefix(self) -> &'static str {
        match self {
            Self::Platform => "ewe_platform-v",
            Self::Browser => "foundation_wasm_ui-v",
        }
    }
    fn workflow(self) -> &'static str {
        match self {
            Self::Platform => "release.yaml",
            Self::Browser => "browser-runtime-release.yaml",
        }
    }
}
#[derive(Args)]
struct Tags {
    #[arg(long)]
    state: PathBuf,
    #[arg(long, value_enum, default_value = "platform")]
    product: Product,
    #[arg(long, default_value = "origin")]
    remote: String,
}
#[derive(Args)]
struct Dispatch {
    #[command(flatten)]
    tags: Tags,
    #[arg(long)]
    repo: Option<String>,
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "record-release-tags",
        "Record the existing binary release tags",
        record,
    )?;
    set.add(
        "dispatch-binary-releases",
        "Start the binary workflow for new release tags",
        dispatch,
    )?;
    set.expose("record-release-tags", &["record-release-tags"])?;
    set.expose("dispatch-binary-releases", &["dispatch-binary-releases"])
}
async fn remote_tags(
    ctx: &TaskContext,
    root: &Path,
    remote: &str,
    product: Product,
) -> Result<BTreeSet<String>, TaskError> {
    let file = capture(
        ctx,
        ProcessCommand::new("git").current_dir(root).args([
            "ls-remote",
            "--tags",
            "--refs",
            "--",
            remote,
            &format!("refs/tags/{}*", product.prefix()),
        ]),
    )
    .await?;
    let mut tags = BTreeSet::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(TaskError::operation)?;
        let tag = line
            .split_once('\t')
            .and_then(|(_, name)| name.strip_prefix("refs/tags/"))
            .filter(|n| n.starts_with(product.prefix()) && !n.ends_with("^{}"))
            .ok_or_else(|| TaskError::operation("Invalid binary release tag record"))?;
        tags.insert(tag.to_owned());
    }
    Ok(tags)
}
fn write(path: &Path, tags: &BTreeSet<String>) -> TaskResult {
    let parent = path
        .parent()
        .ok_or_else(|| TaskError::operation("Missing release tag state directory"))?;
    fs::create_dir_all(parent).map_err(TaskError::operation)?;
    let mut out = tempfile::NamedTempFile::new_in(parent).map_err(TaskError::operation)?;
    for tag in tags {
        writeln!(out, "{tag}").map_err(TaskError::operation)?;
    }
    out.as_file().sync_all().map_err(TaskError::operation)?;
    out.persist(path).map_err(TaskError::operation)?;
    Ok(())
}
async fn lock_state(state: &Path) -> Result<fs::File, TaskError> {
    let dir = state
        .parent()
        .ok_or_else(|| TaskError::operation("Missing state directory"))?
        .to_owned();
    blocking::unblock(move || {
        fs::create_dir_all(&dir)?;
        crate::staging::lock(&dir)
    })
    .await
    .map_err(TaskError::operation)
}
async fn record(ctx: TaskContext, args: Tags) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let state = host.root.join(args.state);
    let _lock = lock_state(&state).await?;
    let tags = remote_tags(&ctx, &host.root, &args.remote, args.product).await?;
    write(&state, &tags)
}
async fn dispatch(ctx: TaskContext, args: Dispatch) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let repo = args
        .repo
        .or_else(|| host.text("GITHUB_REPOSITORY"))
        .ok_or_else(|| TaskError::operation("Missing release repository"))?;
    let state = host.root.join(&args.tags.state);
    let _lock = lock_state(&state).await?;
    let mut sent = BufReader::new(fs::File::open(&state).map_err(TaskError::operation)?)
        .lines()
        .collect::<std::io::Result<BTreeSet<_>>>()
        .map_err(TaskError::operation)?;
    let now = remote_tags(&ctx, &host.root, &args.tags.remote, args.tags.product).await?;
    for tag in now {
        if sent.contains(&tag) {
            continue;
        }
        ctx.exec(ProcessCommand::new("gh").args([
            "workflow",
            "run",
            args.tags.product.workflow(),
            "--repo",
            &repo,
            "--ref",
            &tag,
        ]))
        .await?;
        sent.insert(tag);
        write(&state, &sent)?;
    }
    Ok(())
}
