//! Verify the fingerprint and content of each signed Maven artifact.
use super::{output::query, release::check_candidate, stage, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

pub(super) const ANDROID: &str = "android-publication.zip";
const SUMS: [&str; 4] = [".md5", ".sha1", ".sha256", ".sha512"];

#[derive(Args)]
pub(super) struct Sign {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    key: String,
    #[arg(long)]
    previous: Option<PathBuf>,
}

async fn extract(path: &Path, into: &Path) -> Result<BTreeSet<String>, TaskError> {
    let (path, into) = (path.to_owned(), into.to_owned());
    blocking::unblock(move || extract_files(&path, &into)).await
}

fn extract_files(path: &Path, into: &Path) -> Result<BTreeSet<String>, TaskError> {
    let mut archive = ZipArchive::new(File::open(path).map_err(TaskError::operation)?)
        .map_err(TaskError::operation)?;
    // The ZIP reader indexes files by name. Count all central headers before using that index.
    let mut central = File::open(path).map_err(TaskError::operation)?;
    use std::io::{Read as _, Seek as _, SeekFrom};
    central
        .seek(SeekFrom::Start(archive.central_directory_start()))
        .map_err(TaskError::operation)?;
    let mut entries = 0;
    loop {
        let mut signature = [0; 4];
        central
            .read_exact(&mut signature)
            .map_err(TaskError::operation)?;
        if &signature != b"PK\x01\x02" {
            break;
        }
        let mut header = [0; 42];
        central
            .read_exact(&mut header)
            .map_err(TaskError::operation)?;
        let variable_length = [24, 26, 28]
            .into_iter()
            .map(|at| i64::from(u16::from_le_bytes([header[at], header[at + 1]])))
            .sum();
        central
            .seek(SeekFrom::Current(variable_length))
            .map_err(TaskError::operation)?;
        entries += 1;
    }
    if entries != archive.len() {
        return Err(TaskError::operation("Android bundle has duplicate members"));
    }
    let mut names = BTreeSet::new();
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).map_err(TaskError::operation)?;
        let name = member.name().map_err(TaskError::operation)?.into_owned();
        let safe = member
            .enclosed_name()
            .ok_or_else(|| TaskError::operation("Invalid Maven archive path"))?;
        if !member.is_file()
            || member.is_symlink()
            || safe.to_str() != Some(&name)
            || !names.insert(name)
        {
            return Err(TaskError::operation(
                "Invalid or duplicate Maven archive member",
            ));
        }
        let target = into.join(safe);
        fs::create_dir_all(
            target
                .parent()
                .ok_or_else(|| TaskError::operation("Missing archive parent"))?,
        )
        .map_err(TaskError::operation)?;
        let mut file = File::create_new(target).map_err(TaskError::operation)?;
        io::copy(&mut member, &mut file).map_err(TaskError::operation)?;
    }
    Ok(names)
}

async fn verify(
    ctx: &TaskContext,
    folder: &Path,
    names: &BTreeSet<String>,
    fingerprint: &str,
    keyring: Option<&Path>,
) -> TaskResult {
    let products: BTreeSet<_> = names
        .iter()
        .filter(|n| !n.ends_with(".asc") && !SUMS.iter().any(|s| n.ends_with(s)))
        .map(String::as_str)
        .collect();
    let signatures: BTreeSet<_> = names
        .iter()
        .filter_map(|n| n.strip_suffix(".asc"))
        .collect();
    if products.is_empty() || products != signatures {
        return Err(TaskError::operation(
            "Android bundle requires a signature for each product",
        ));
    }
    for name in products {
        let status = query(
            ctx,
            gpg(keyring)
                .args(["--batch", "--status-fd", "1", "--verify"])
                .arg(folder.join(format!("{name}.asc")))
                .arg(folder.join(name)),
        )
        .await?;
        let text = std::str::from_utf8(&status).map_err(TaskError::operation)?;
        let matches = text
            .lines()
            .filter_map(|line| line.strip_prefix("[GNUPG:] VALIDSIG "))
            .any(|rest| {
                let mut parts = rest.split_whitespace();
                parts
                    .next()
                    .is_some_and(|k| k.eq_ignore_ascii_case(fingerprint))
                    || parts
                        .next_back()
                        .is_some_and(|k| k.eq_ignore_ascii_case(fingerprint))
            });
        if !matches {
            return Err(TaskError::operation(format!(
                "Unexpected signer for {name}"
            )));
        }
    }
    Ok(())
}

pub(super) async fn verified(
    ctx: &TaskContext,
    path: &Path,
    fingerprint: &str,
    keyring: Option<&Path>,
) -> TaskResult {
    let temp = tempfile::tempdir().map_err(TaskError::operation)?;
    let names = extract(path, temp.path()).await?;
    verify(ctx, temp.path(), &names, fingerprint, keyring).await
}

pub(super) async fn run(ctx: TaskContext, args: Sign) -> TaskResult {
    sign(&ctx, args, None).await
}
async fn sign(ctx: &TaskContext, args: Sign, keyring: Option<&Path>) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    check_candidate(&root, &args.candidate, false).await?;
    let dir = root.join(args.candidate);
    let out = root.join(args.out);
    if out.exists() {
        return Err(TaskError::operation(
            "Use a new directory for the signed candidate",
        ));
    }
    let parent = out
        .parent()
        .ok_or_else(|| TaskError::operation("Missing signed candidate parent"))?;
    fs::create_dir_all(parent).map_err(TaskError::operation)?;
    let temp = tempfile::tempdir_in(parent).map_err(TaskError::operation)?;
    let ready = temp.path().join("signed");
    fs::create_dir(&ready).map_err(TaskError::operation)?;
    let config = stage::config(&root)?;
    stage::copy(
        &dir.join(&config.apple_archive),
        &ready.join(&config.apple_archive),
    )
    .await?;
    let repo = temp.path().join("maven");
    let names = extract(&dir.join(ANDROID), &repo).await?;
    let android = if let Some(previous) = args.previous {
        let saved = temp.path().join("saved");
        let previous = root.join(previous);
        let members = extract(&previous, &saved).await?;
        let mut expected = names.clone();
        expected.extend(
            names
                .iter()
                .filter(|n| !SUMS.iter().any(|s| n.ends_with(s)))
                .map(|n| format!("{n}.asc")),
        );
        if members != expected {
            return Err(TaskError::operation("Signed bundle has different members"));
        }
        for name in names {
            if stage::digest(&repo.join(&name)).await? != stage::digest(&saved.join(&name)).await? {
                return Err(TaskError::operation(format!(
                    "Signed file differs from the preview: {name}"
                )));
            }
        }
        verify(ctx, &saved, &members, &args.key, keyring).await?;
        let target = ready.join(ANDROID);
        stage::copy(&previous, &target).await?;
        target
    } else {
        stage::maven(ctx, &repo, &config, &ready, Some(&args.key), keyring).await?
    };
    let mut record: Value = serde_json::from_reader(
        File::open(dir.join("native-ui.json")).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    record["sha256"][ANDROID] = json!(stage::digest(&android).await?);
    record["central_signed"] = json!(true);
    stage::write_json(&ready.join("native-ui.json"), &record)?;
    fs::rename(ready, out).map_err(TaskError::operation)
}

pub(super) fn gpg(keyring: Option<&Path>) -> ProcessCommand {
    let cmd = ProcessCommand::new("gpg");
    if let Some(path) = keyring {
        cmd.arg("--homedir").arg(path)
    } else {
        cmd
    }
}

#[derive(Clone)]
pub(super) struct JobEnv {
    key: Vec<u8>,
    fingerprint: String,
    tag: String,
    repository: String,
}
impl JobEnv {
    pub(super) fn from_host() -> Option<Self> {
        Some(Self {
            key: std::env::var("SIGNING_KEY").ok()?.into_bytes(),
            fingerprint: std::env::var("FINGERPRINT").ok()?,
            tag: std::env::var("GITHUB_REF_NAME").ok()?,
            repository: std::env::var("GITHUB_REPOSITORY").ok()?,
        })
    }
}

#[derive(Args)]
pub(super) struct Job {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    public_key: PathBuf,
}

pub(super) async fn job(ctx: TaskContext, args: Job) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let job = env.signing_job.as_ref().ok_or_else(|| {
        TaskError::operation("Set the release key, fingerprint, repository, and tag")
    })?;
    let root = env.root().await?;
    let cfg = stage::config(&root)?;
    if job.repository != cfg.repository || job.tag != format!("native-ui-v{}", cfg.version) {
        return Err(TaskError::operation(
            "The signing job requires the configured repository and release tag",
        ));
    }
    let keyring = tempfile::tempdir().map_err(TaskError::operation)?;
    let home = Some(keyring.path());
    ctx.exec(
        gpg(home)
            .args(["--batch", "--import"])
            .private_input(job.key.clone()),
    )
    .await?;
    ctx.exec(
        gpg(home)
            .args(["--batch", "--armor", "--output"])
            .arg(args.public_key)
            .args(["--export", &job.fingerprint]),
    )
    .await?;
    let route = format!("repos/{}/releases/tags/{}", job.repository, job.tag);
    let release = super::publish::github_lookup(&ctx, &route, "[.assets[].name]").await?;
    let archive = if release
        .as_ref()
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|v| v == ANDROID))
    {
        ctx.exec(
            ProcessCommand::new("gh")
                .args([
                    "release",
                    "download",
                    &job.tag,
                    "--repo",
                    &job.repository,
                    "--pattern",
                    ANDROID,
                    "--dir",
                ])
                .arg(keyring.path()),
        )
        .await?;
        Some(keyring.path().join(ANDROID))
    } else {
        None
    };
    sign(
        &ctx,
        Sign {
            candidate: args.candidate,
            out: args.out,
            key: job.fingerprint.clone(),
            previous: archive,
        },
        home,
    )
    .await
}
