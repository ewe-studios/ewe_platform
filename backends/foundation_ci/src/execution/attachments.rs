//! Validate release archives and attach them to the release for an existing tag.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Attach {
    #[arg(long)]
    tag: String,
    #[arg(long)]
    repo: Option<String>,
    #[arg(long, default_value = "dist")]
    dir: PathBuf,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "attach-binaries",
        "Verify and upload binary release archives",
        attach,
    )?;
    set.expose("attach-binaries", &["attach-binaries"])
}

fn archives(dir: &Path) -> Result<Vec<PathBuf>, TaskError> {
    let mut archives = BTreeMap::<String, (Option<PathBuf>, Option<PathBuf>)>::new();
    for item in fs::read_dir(dir).map_err(TaskError::operation)? {
        let item = item.map_err(TaskError::operation)?;
        let kind = item.file_type().map_err(TaskError::operation)?;
        if kind.is_dir() {
            continue;
        }
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| TaskError::operation("Invalid release filename"))?;
        if !kind.is_file() {
            return Err(TaskError::operation(format!(
                "Expected a regular archive file: {name}"
            )));
        }
        if name.ends_with(".tar.gz") {
            archives.entry(name).or_default().0 = Some(item.path());
        } else if let Some(archive) = name.strip_suffix(".tar.gz.sha256") {
            archives.entry(format!("{archive}.tar.gz")).or_default().1 = Some(item.path());
        } else {
            return Err(TaskError::operation(format!(
                "Unexpected release asset {name}"
            )));
        }
    }
    if archives.is_empty() {
        return Err(TaskError::operation("Missing release archives"));
    }
    let mut files = Vec::new();
    for (name, pair) in archives {
        let (Some(archive), Some(checksum)) = pair else {
            return Err(TaskError::operation(format!(
                "Missing archive or checksum for {name}"
            )));
        };
        let mut lines =
            BufReader::new(fs::File::open(&checksum).map_err(TaskError::operation)?).lines();
        let line = lines
            .next()
            .transpose()
            .map_err(TaskError::operation)?
            .ok_or_else(|| TaskError::operation("Empty release checksum"))?;
        let (hash, path) = line
            .split_once("  ")
            .ok_or_else(|| TaskError::operation("Invalid release checksum record"))?;
        if Path::new(path).file_name() != archive.file_name()
            || lines
                .next()
                .transpose()
                .map_err(TaskError::operation)?
                .is_some()
            || crate::staging::sha256(&archive).map_err(TaskError::operation)? != hash
        {
            return Err(TaskError::operation(format!("Invalid checksum for {name}")));
        }
        files.extend([archive, checksum]);
    }
    Ok(files)
}

#[derive(Deserialize)]
struct Response {
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    repository: Repository,
}
#[derive(Deserialize)]
struct Repository {
    release: Option<Release>,
}
#[derive(Deserialize)]
struct Release {
    #[serde(rename = "tagName")]
    tag: String,
}

async fn present(ctx: &TaskContext, repo: &str, tag: &str) -> Result<bool, TaskError> {
    let (owner, name) = repo
        .split_once('/')
        .filter(|(o, n)| !o.is_empty() && !n.is_empty() && !n.contains('/'))
        .ok_or_else(|| TaskError::operation("Expected owner/repository"))?;
    let query="query($owner:String!,$name:String!,$tag:String!){repository(owner:$owner,name:$name){release(tagName:$tag){tagName}}}";
    let command = ProcessCommand::new("gh")
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .arg("-f")
        .arg(format!("owner={owner}"))
        .arg("-f")
        .arg(format!("name={name}"))
        .arg("-f")
        .arg(format!("tag={tag}"));
    let result: Response = serde_json::from_reader(BufReader::new(capture(ctx, command).await?))
        .map_err(TaskError::operation)?;
    match result.data.repository.release {
        None => Ok(false),
        Some(value) if value.tag == tag => Ok(true),
        Some(_) => Err(TaskError::operation(
            "The release query returned a different tag",
        )),
    }
}
async fn attach(ctx: TaskContext, args: Attach) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let repo = args
        .repo
        .or_else(|| host.text("GITHUB_REPOSITORY"))
        .ok_or_else(|| TaskError::operation("Missing release repository"))?;
    let files = archives(&host.root.join(args.dir))?;
    if !present(&ctx, &repo, &args.tag).await? {
        let create = ProcessCommand::new("gh").args([
            "release",
            "create",
            "--draft",
            "--verify-tag",
            "--repo",
            &repo,
            "--title",
            &args.tag,
            "--notes",
            "Built by the Release binaries workflow.",
            "--",
            &args.tag,
        ]);
        if let Err(error) = ctx.exec(create).await {
            if !matches!(error, TaskError::Exit { .. }) || !present(&ctx, &repo, &args.tag).await? {
                return Err(error);
            }
        }
    }
    ctx.exec(
        ProcessCommand::new("gh")
            .args([
                "release",
                "upload",
                "--repo",
                &repo,
                "--clobber",
                "--",
                &args.tag,
            ])
            .args(files),
    )
    .await
}
