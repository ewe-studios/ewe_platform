use super::{android, apple, output::query, NativeBuildEnv, Templates};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(super) struct Prepare {
    #[arg(long, default_value = "release/native-ui.toml")]
    config: PathBuf,
    #[arg(long, default_value = "target/native-ui/candidate")]
    output: PathBuf,
}

pub(super) async fn prepare(ctx: TaskContext, args: Prepare) -> TaskResult {
    if args.config != Path::new("release/native-ui.toml") {
        return Err(TaskError::Arguments(clap::Error::raw(
            clap::error::ErrorKind::InvalidValue,
            "Use release/native-ui.toml",
        )));
    }
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let revision = clean_revision(&ctx, &root).await?;
    for scope in ["apple", "android"] {
        ctx.exec(ProcessCommand::new("mise").current_dir(&root).args([
            "exec",
            "--",
            "cargo",
            "run",
            "--locked",
            "--profile",
            "uat",
            "-p",
            "foundation_ci",
            "--",
            "toolchain-check",
            "--scope",
            scope,
        ]))
        .await?;
    }
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    let apple = stage.path().join("apple");
    let maven = stage.path().join("maven");
    apple::build_at(&ctx, &root, &apple).await?;
    android::build_at(&ctx, &root, false).await?;
    install_maven(&ctx, &root, &maven).await?;
    if clean_revision(&ctx, &root).await? != revision {
        return Err(TaskError::operation(
            "The core revision changed during the build",
        ));
    }
    super::stage::release(
        &ctx,
        &root,
        &maven,
        &apple.join("EweCoreFFI.xcframework.zip"),
        &root.join(args.output),
        std::str::from_utf8(&revision)
            .map_err(TaskError::operation)?
            .trim(),
    )
    .await?;
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

pub(super) async fn clean_revision(ctx: &TaskContext, root: &Path) -> Result<Vec<u8>, TaskError> {
    let status = query(
        ctx,
        ProcessCommand::new("git")
            .current_dir(root)
            .args(["status", "--porcelain"]),
    )
    .await?;
    if !status.is_empty() {
        return Err(TaskError::operation(
            "Commit the release inputs before building",
        ));
    }
    query(
        ctx,
        ProcessCommand::new("git").current_dir(root).args([
            "rev-parse",
            "--verify",
            "HEAD^{commit}",
        ]),
    )
    .await
}

#[derive(Args)]
pub(super) struct Verify {
    #[arg(long, default_value = "target/native-ui/candidate")]
    candidate: PathBuf,
    #[arg(long, value_enum, default_value = "both")]
    templates: Templates,
    #[arg(long)]
    public: bool,
}

pub(super) async fn verify(ctx: TaskContext, args: Verify) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let root = env.root().await?;
    let revision = check_candidate(&root, &args.candidate, false).await?;
    let candidate = root.join(args.candidate);
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    if !matches!(args.templates, Templates::Android) {
        verify_apple(
            &ctx,
            &root,
            &candidate,
            stage.path(),
            &revision,
            args.public,
        )
        .await?;
    }
    if !matches!(args.templates, Templates::Ios) {
        verify_android(
            &ctx,
            &root,
            &candidate,
            stage.path(),
            &revision,
            args.public,
        )
        .await?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

async fn unpack_revision(
    ctx: &TaskContext,
    root: &Path,
    stage: &Path,
    into: &Path,
    revision: &str,
    paths: &[&str],
) -> TaskResult {
    let archive = stage.join("source.tar");
    let destination = into.to_path_buf();
    blocking::unblock(move || std::fs::create_dir_all(destination))
        .await
        .map_err(TaskError::operation)?;
    ctx.exec(
        ProcessCommand::new("git")
            .current_dir(root)
            .args(["archive", "--format=tar", "--output"])
            .arg(&archive)
            .arg(revision)
            .args(paths.iter().copied()),
    )
    .await?;
    ctx.exec(
        ProcessCommand::new("tar")
            .current_dir(root)
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(into),
    )
    .await
}

async fn verify_apple(
    ctx: &TaskContext,
    root: &Path,
    candidate: &Path,
    stage: &Path,
    revision: &str,
    public: bool,
) -> TaskResult {
    if !cfg!(target_os = "macos") {
        return Err(TaskError::operation(
            "Apple verification requires Xcode on macOS",
        ));
    }
    let env = ctx.require::<NativeBuildEnv>()?;
    let cmd = |program| env.apple_command(root, program);
    unpack_revision(
        ctx,
        root,
        stage,
        stage,
        revision,
        &["examples/mobile_starter/ios"],
    )
    .await?;
    let project = stage.join("examples/mobile_starter/ios/Starter.xcodeproj");
    let destinations: &[&str] = if public {
        &["generic/platform=iOS", "generic/platform=iOS Simulator"]
    } else {
        let swift = stage.join("swift");
        unpack_revision(
            ctx,
            root,
            stage,
            &swift,
            revision,
            &["Package.swift", "mobile/ios"],
        )
        .await?;
        ctx.exec(
            cmd("unzip")
                .arg("-q")
                .arg(candidate.join("EweCoreFFI.xcframework.zip"))
                .arg("-d")
                .arg(swift.join(".build-inputs")),
        )
        .await?;
        let record = candidate.join("native-ui.json");
        let package = swift.clone();
        let project_file = project.join("project.pbxproj");
        blocking::unblock(move || super::swift::local(&record, &package, &project_file)).await?;
        ctx.exec(cmd("swift").args(["test", "--package-path"]).arg(swift))
            .await?;
        &["generic/platform=iOS Simulator"]
    };
    for destination in destinations {
        ctx.exec(
            cmd("xcodebuild")
                .arg("-project")
                .arg(&project)
                .args(["-scheme", "Starter", "-destination", destination])
                .arg("-derivedDataPath")
                .arg(stage.join("products"))
                .args(["CODE_SIGNING_ALLOWED=NO", "build"]),
        )
        .await?;
    }
    Ok(())
}

async fn verify_android(
    ctx: &TaskContext,
    root: &Path,
    candidate: &Path,
    stage: &Path,
    revision: &str,
    public: bool,
) -> TaskResult {
    unpack_revision(
        ctx,
        root,
        stage,
        stage,
        revision,
        &["examples/mobile_starter/android"],
    )
    .await?;
    let mut gradle = ProcessCommand::new("mise")
        .current_dir(root)
        .env("GRADLE_USER_HOME", stage.join("gradle"))
        .args(["exec", "--", "gradle", "--no-daemon", "-p"])
        .arg(stage.join("examples/mobile_starter/android"));
    if !public {
        let packages = stage.join("maven");
        ctx.exec(
            ProcessCommand::new("unzip")
                .current_dir(root)
                .arg("-q")
                .arg(candidate.join("android-publication.zip"))
                .arg("-d")
                .arg(&packages),
        )
        .await?;
        let mut repository = OsString::from("-PeweRepository=");
        repository.push(packages);
        gradle = gradle.arg(repository);
    }
    ctx.exec(gradle.arg(":app:assembleDebug")).await
}

#[derive(Args)]
pub(super) struct Publish {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    resume: bool,
    #[arg(long)]
    public_key: Option<PathBuf>,
    #[arg(long, default_value_t = 1800, value_parser = clap::value_parser!(u64).range(1..))]
    timeout: u64,
    #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..))]
    poll_interval: u64,
}

pub(super) async fn publish(ctx: TaskContext, args: Publish) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    check_candidate(&root, &args.candidate, true).await?;
    let candidate = root.join(&args.candidate);
    let lock = blocking::unblock(move || {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(candidate.join(".publish.lock"))
            .map_err(TaskError::operation)?;
        file.try_lock().map_err(TaskError::operation)?;
        Ok::<_, TaskError>(file)
    })
    .await?;
    let result = super::publish::run(
        &ctx,
        &root,
        &root.join(args.candidate),
        args.timeout,
        args.poll_interval,
        args.public_key.as_deref(),
    )
    .await;
    drop(lock);
    result
}

pub(super) async fn check_candidate(
    root: &Path,
    candidate: &Path,
    publishing: bool,
) -> Result<String, TaskError> {
    let candidate = root.join(candidate);
    blocking::unblock(move || {
        let record: serde_json::Value = serde_json::from_reader(
            std::fs::File::open(candidate.join("native-ui.json")).map_err(TaskError::operation)?,
        )
        .map_err(TaskError::operation)?;
        let revision = record
            .get("source_revision")
            .and_then(serde_json::Value::as_str)
            .filter(|value| {
                matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(|| TaskError::operation("The candidate requires a full source revision"))?;
        let files = record
            .get("sha256")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| TaskError::operation("The candidate has no artifact digests"))?;
        if publishing
            && record
                .get("central_signed")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
        {
            return Err(TaskError::operation(
                "The Android candidate requires signatures",
            ));
        }
        let expected = ["EweCoreFFI.xcframework.zip", "android-publication.zip"];
        if files.len() != expected.len() {
            return Err(TaskError::operation(
                "The candidate has an unexpected artifact list",
            ));
        }
        for name in expected {
            let expected_digest = files
                .get(name)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    TaskError::operation(format!("The candidate has no digest for {name}"))
                })?;
            if super::stage::digest_file(&candidate.join(name))? != expected_digest {
                return Err(TaskError::operation(format!(
                    "Artifact digest mismatch: {name}"
                )));
            }
        }
        Ok(revision.to_owned())
    })
    .await
}

#[derive(Args)]
pub(super) struct Maven {
    #[arg(long)]
    repository: PathBuf,
}

pub(super) async fn maven(ctx: TaskContext, args: Maven) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    install_maven(&ctx, &root, &root.join(args.repository)).await
}

async fn install_maven(ctx: &TaskContext, root: &Path, maven: &Path) -> TaskResult {
    let mut setting = OsString::from("-Dmaven.repo.local=");
    setting.push(maven);
    ctx.exec(
        ProcessCommand::new("mise")
            .current_dir(root)
            .args([
                "exec",
                "--",
                "mobile/android/gradlew",
                "-p",
                "mobile/android",
                ":core:test",
                ":ui:testDebugUnitTest",
                "publishToMavenLocal",
            ])
            .arg(setting),
    )
    .await
}
