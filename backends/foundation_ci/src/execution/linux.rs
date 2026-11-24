//! Run the package checks in the same Linux container as the hosted jobs.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{
    Cancellation, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet,
};
use std::{
    io::{BufRead, BufReader},
    path::Path,
};

#[derive(Args)]
struct Linux {
    /// A package name. The default selects every enabled package.
    package: Option<String>,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("linux", "Run the checks in the Linux container", run)?;
    set.expose("linux", &["linux"])
}

async fn run(ctx: TaskContext, args: Linux) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let image = host
        .text("CI_IMAGE")
        .unwrap_or_else(|| "ewestudios/ewe-platform-ci:latest".into());
    let platform = host
        .text("CI_PLATFORM")
        .unwrap_or_else(|| "linux/amd64".into());
    let volume = host.text("CI_TARGET_VOLUME").unwrap_or_else(|| {
        if platform.contains("amd64") {
            "ewe-cargo-amd64"
        } else {
            "ewe-cargo"
        }
        .into()
    });
    let selected = args
        .package
        .or_else(|| host.text("PACKAGE"))
        .unwrap_or_else(|| "all".into());
    let git = capture(
        &ctx,
        ProcessCommand::new("git").current_dir(&host.root).args([
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ]),
    )
    .await?;
    let mut common = String::new();
    BufReader::new(git)
        .read_line(&mut common)
        .map_err(TaskError::operation)?;
    let common = Path::new(common.trim_end());
    let scratch = tempfile::tempdir().map_err(TaskError::operation)?;
    let receipt = scratch.path().join("container-id");
    let mut create = ProcessCommand::new("docker")
        .args(["create", "--cidfile"])
        .arg(&receipt)
        .args(["--platform", &platform, "-v"])
        .arg(format!("{}:/w", host.root.display()))
        .args(["-w", "/w", "-v"])
        .arg(format!("{volume}:/target"))
        .args(["-e", "CARGO_TARGET_DIR=/target"]);
    if common != host.root.join(".git") {
        create = create.arg("-v").arg(format!("{0}:{0}", common.display()));
    }
    let guest = r#"cargo run --locked --profile uat -p foundation_ci -- link-submodules
exec cargo run --locked --profile uat -p foundation_ci -- "$1" "$2" --profile uat"#;
    let operation = async {
        ctx.exec(create.args([
            image.as_str(),
            "bash",
            "-ec",
            guest,
            "ci-linux",
            if selected == "all" { "run-many" } else { "run" },
            &selected,
        ]))
        .await?;
        let id = std::fs::read_to_string(&receipt).map_err(TaskError::operation)?;
        ctx.exec(ProcessCommand::new("docker").args(["start", "--attach", id.trim()]))
            .await
    }
    .await;
    match std::fs::read_to_string(&receipt) {
        Ok(id) => {
            let env = ctx.require::<TaskEnv>()?;
            env.process
                .run(
                    ProcessCommand::new("docker").args(["rm", "--force", id.trim()]),
                    env.output.clone(),
                    Cancellation::new(),
                )
                .await?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && operation.is_err() => {}
        Err(error) => return Err(TaskError::operation(error)),
    }
    operation
}
