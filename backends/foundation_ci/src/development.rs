//! Build and check the workspace with the same Cargo arguments from each host.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Deserialize;
use std::{collections::BTreeSet, io::BufReader};

#[derive(Args)]
struct Empty {}

#[derive(Args)]
struct Publish {
    #[arg(required = true)]
    packages: Vec<String>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Deserialize)]
struct Members {
    packages: Vec<Member>,
}

#[derive(Deserialize)]
struct Member {
    name: String,
    publish: Option<Vec<String>>,
}

async fn publish(ctx: TaskContext, args: Publish) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let metadata = ProcessCommand::new("cargo")
        .args([
            "metadata",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ])
        .current_dir(&host.root);
    let data: Members = serde_json::from_reader(BufReader::new(capture(&ctx, metadata).await?))
        .map_err(TaskError::operation)?;
    let eligible: BTreeSet<_> = data
        .packages
        .into_iter()
        .filter(|member| {
            member
                .publish
                .as_ref()
                .is_none_or(|registries| !registries.is_empty())
        })
        .map(|member| member.name)
        .collect();
    let mut command = ProcessCommand::new("cargo")
        .arg("publish")
        .current_dir(&host.root);
    let mut selected = BTreeSet::new();
    for package in args.packages {
        if !eligible.contains(&package) {
            return Err(TaskError::operation(format!(
                "Cannot publish {package}. Check the workspace membership and registry policy."
            )));
        }
        if selected.insert(package.clone()) {
            command = command.arg("--package").arg(package);
        }
    }
    if args.dry_run {
        command = command.arg("--dry-run");
    }
    ctx.exec(command).await
}

const RECIPES: &[(&str, &str, &[&str])] = &[
    ("build", "Build workspace packages", &["build", "--all"]),
    (
        "build-release",
        "Build workspace packages for release",
        &["build", "--all", "--release"],
    ),
    (
        "build-wasm",
        "Build foundation_nostd for a browser",
        &[
            "build",
            "--package",
            "foundation_nostd",
            "--target",
            "wasm32-unknown-unknown",
        ],
    ),
    ("check", "Check workspace packages", &["check", "--all"]),
    (
        "clippy",
        "Check all targets with strict lints",
        &[
            "clippy",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    ),
    ("fmt", "Format workspace source files", &["fmt", "--all"]),
    (
        "fmt-check",
        "Check workspace source formatting",
        &["fmt", "--all", "--", "--check"],
    ),
    ("audit", "Check workspace dependency advisories", &["audit"]),
    (
        "doc",
        "Build workspace documentation",
        &["doc", "--no-deps", "--all-features"],
    ),
    (
        "doc-open",
        "Build and open workspace documentation",
        &["doc", "--no-deps", "--all-features", "--open"],
    ),
    (
        "doc-nostd",
        "Build and open foundation_nostd documentation",
        &[
            "doc",
            "--package",
            "foundation_nostd",
            "--no-deps",
            "--all-features",
            "--open",
        ],
    ),
];

/// Register workspace development commands.
///
/// # Errors
/// Returns an error if two commands share a name.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut tasks = TaskSet::new("foundation_workspace").with_namespace("workspace");
    for &(name, description, arguments) in RECIPES {
        tasks.add(name, description, move |ctx, _: Empty| async move {
            let host = ctx.require::<CiEnvironment>()?;
            ctx.exec(
                ProcessCommand::new("cargo")
                    .args(arguments.iter().copied())
                    .current_dir(&host.root),
            )
            .await
        })?;
    }
    tasks.add(
        "build-wasm-matrix",
        "Build foundation_nostd for all four WebAssembly targets",
        |ctx, _: Empty| async move {
            let host = ctx.require::<CiEnvironment>()?;
            for target in [
                "wasm32-unknown-unknown",
                "wasm32-unknown-emscripten",
                "wasm32-wasip1",
                "wasm32-wasip2",
            ] {
                ctx.exec(
                    ProcessCommand::new("cargo")
                        .args(["build", "--package", "foundation_nostd", "--target", target])
                        .env("CARGO_PROFILE_DEV_CODEGEN_BACKEND", "llvm")
                        .current_dir(&host.root),
                )
                .await?;
            }
            Ok(())
        },
    )?;
    tasks.add(
        "publish",
        "Verify the selected crates and publish them together",
        publish,
    )?;
    Ok(tasks)
}
