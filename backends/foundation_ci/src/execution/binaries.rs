//! Build the selected Cargo executables and stage a release archive with its checksum.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Binaries {
    /// Package names from the release workflow.
    #[arg(long, required=true, value_delimiter=' ', num_args=1..)]
    packages: Vec<String>,
    #[arg(long)]
    tag: String,
    #[arg(long, default_value = "dist")]
    out: PathBuf,
    #[arg(long)]
    cuda: bool,
    #[arg(long)]
    github_output: bool,
}
#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Member>,
}
#[derive(Deserialize)]
struct Member {
    id: String,
    name: String,
    targets: Vec<Target>,
}
#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
}
impl Target {
    fn binary(&self) -> bool {
        self.kind.iter().any(|kind| kind == "bin")
    }
}
#[derive(Deserialize)]
struct Message {
    reason: String,
    package_id: Option<String>,
    target: Option<Target>,
    executable: Option<PathBuf>,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "release-binaries",
        "Build a release archive with its checksum",
        run,
    )?;
    set.expose("release-binaries", &["release-binaries"])
}

async fn targets(
    ctx: &TaskContext,
    root: &Path,
    packages: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, TaskError> {
    let file = capture(
        ctx,
        ProcessCommand::new("cargo")
            .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
            .current_dir(root),
    )
    .await?;
    let meta: Metadata =
        serde_json::from_reader(BufReader::new(file)).map_err(TaskError::operation)?;
    let mut found = BTreeSet::new();
    let mut binaries = BTreeMap::new();
    for member in meta.packages.iter().filter(|p| packages.contains(&p.name)) {
        let count = binaries.len();
        for target in member.targets.iter().filter(|t| t.binary()) {
            if binaries
                .insert(target.name.clone(), member.id.clone())
                .is_some()
            {
                return Err(TaskError::operation(format!(
                    "Duplicate release binary {}",
                    target.name
                )));
            }
        }
        if binaries.len() == count {
            return Err(TaskError::operation(format!(
                "No binary targets in {}",
                member.name
            )));
        }
        found.insert(member.name.clone());
    }
    if let Some(missing) = packages.difference(&found).next() {
        return Err(TaskError::operation(format!(
            "Unknown release package {missing}"
        )));
    }
    Ok(binaries)
}
async fn collect(
    ctx: &TaskContext,
    root: &Path,
    stage: &Path,
    packages: &BTreeSet<String>,
    cuda: bool,
    platform: &str,
) -> TaskResult {
    let mut expected = targets(ctx, root, packages).await?;
    let mut cmd = ProcessCommand::new("cargo")
        .args([
            "build",
            "--release",
            "--locked",
            "--bins",
            "--message-format=json-render-diagnostics",
            "--target",
            platform,
        ])
        .current_dir(root);
    if cuda {
        cmd = cmd.args(["--features", "foundation_ai/cuda"]);
    }
    for name in packages {
        cmd = cmd.arg("-p").arg(name);
    }
    for line in BufReader::new(capture(ctx, cmd).await?).lines() {
        let message: Message = serde_json::from_str(&line.map_err(TaskError::operation)?)
            .map_err(TaskError::operation)?;
        if message.reason != "compiler-artifact" {
            continue;
        }
        let (Some(target), Some(file), Some(id)) =
            (message.target, message.executable, message.package_id)
        else {
            continue;
        };
        if target.binary() && expected.get(&target.name) == Some(&id) {
            let name = file
                .file_name()
                .ok_or_else(|| TaskError::operation("Missing binary filename"))?;
            fs::copy(&file, stage.join(name)).map_err(TaskError::operation)?;
            expected.remove(&target.name);
        }
    }
    if !expected.is_empty() {
        return Err(TaskError::operation(format!(
            "Cargo did not build {}",
            expected.into_keys().collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(())
}
fn archive_files(dir: &Path, name: &str, dst: &Path) -> std::io::Result<()> {
    let gzip =
        flate2::write::GzEncoder::new(fs::File::create(dst)?, flate2::Compression::default());
    let mut tar = tar::Builder::new(gzip);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_size(0);
    header.set_mode(0o755);
    tar.append_data(&mut header, name, std::io::empty())?;
    let files = fs::read_dir(dir)?
        .map(|entry| entry.map(|item| (item.file_name(), item.path())))
        .collect::<std::io::Result<BTreeMap<_, _>>>()?;
    for (filename, path) in files {
        let file = fs::File::open(path)?;
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(file.metadata()?.len());
        header.set_mode(0o755);
        tar.append_data(&mut header, Path::new(name).join(filename), file)?;
    }
    tar.into_inner()?.finish()?.sync_all()
}

async fn run(ctx: TaskContext, args: Binaries) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let compiler = capture(
        &ctx,
        ProcessCommand::new("rustc")
            .arg("-vV")
            .current_dir(&host.root),
    )
    .await?;
    let mut triple = None;
    for line in BufReader::new(compiler).lines() {
        let line = line.map_err(TaskError::operation)?;
        if let Some(value) = line.strip_prefix("host: ") {
            triple = Some(value.to_owned());
        }
    }
    let platform = triple.ok_or_else(|| TaskError::operation("Missing compiler host target"))?;
    let tag = args.tag.replace(['/', '\\'], "-");
    if tag.is_empty() || tag.chars().any(char::is_control) {
        return Err(TaskError::operation("Invalid release tag"));
    }
    let variant = if args.cuda { "-cuda12" } else { "" };
    let name = format!("ewe_platform-{tag}-{platform}{variant}");
    let out = host.root.join(&args.out);
    fs::create_dir_all(&out).map_err(TaskError::operation)?;
    let stage = tempfile::tempdir_in(&out).map_err(TaskError::operation)?;
    let dir = stage.path().join(&name);
    fs::create_dir(&dir).map_err(TaskError::operation)?;
    let packages = args
        .packages
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if BTreeSet::is_empty(&packages) {
        return Err(TaskError::operation("Missing release packages"));
    }
    collect(&ctx, &host.root, &dir, &packages, args.cuda, &platform).await?;
    let archive = format!("{name}.tar.gz");
    let checksum = format!("{archive}.sha256");
    archive_files(&dir, &name, &stage.path().join(&archive)).map_err(TaskError::operation)?;
    let hash =
        crate::staging::sha256(&stage.path().join(&archive)).map_err(TaskError::operation)?;
    fs::write(
        stage.path().join(&checksum),
        format!("{hash}  {}\n", args.out.join(&archive).display()),
    )
    .map_err(TaskError::operation)?;
    let pairs = [name.as_str(), archive.as_str(), checksum.as_str()].map(|f| (f, out.join(f)));
    crate::staging::publish(stage, &out, pairs, false)?;
    if args.github_output {
        let file = host
            .get("GITHUB_OUTPUT")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| TaskError::operation("Missing GITHUB_OUTPUT"))?;
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(file)
                .map_err(TaskError::operation)?,
            "name={name}"
        )
        .map_err(TaskError::operation)?;
    }
    println!("{name}");
    Ok(())
}
