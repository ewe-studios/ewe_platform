use super::{output::artifact, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(super) struct Generate {
    #[arg(long, required_unless_present = "check")]
    out: Option<PathBuf>,
    #[arg(long)]
    check: bool,
}

pub(super) async fn run(ctx: TaskContext, args: Generate) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    generate(&ctx, &root, args.out.as_deref(), args.check).await
}

pub(super) async fn generate(
    ctx: &TaskContext,
    root: &Path,
    out: Option<&Path>,
    check: bool,
) -> TaskResult {
    let cargo = ProcessCommand::new("cargo").current_dir(root).args([
        "build",
        "--locked",
        "--profile",
        "uat",
        "-p",
        "foundation_ui_native",
        "--features",
        "bindgen",
        "--message-format=json-render-diagnostics",
    ]);
    let shared = artifact(
        ctx,
        cargo.clone().arg("--lib"),
        "foundation_ui_native",
        &[".so", ".dylib", ".dll"],
    )
    .await?;
    let exe = artifact(
        ctx,
        cargo.args(["--bin", "uniffi-bindgen"]),
        "uniffi-bindgen",
        &[],
    )
    .await?;
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    let current = stage.path().join("current");
    let out = out.map_or_else(|| stage.path().join("repeated"), |path| root.join(path));
    let targets = if check && out.starts_with(stage.path()) {
        vec![&current, &out]
    } else {
        vec![&current]
    };
    for directory in targets {
        let path = directory.clone();
        blocking::unblock(move || std::fs::create_dir_all(&path))
            .await
            .map_err(TaskError::operation)?;
        for language in ["swift", "kotlin"] {
            ctx.exec(
                ProcessCommand::new(&exe)
                    .current_dir(root)
                    .arg("generate")
                    .arg("--library")
                    .arg(&shared)
                    .args(["--language", language, "--out-dir"])
                    .arg(directory.join(language))
                    .arg("--no-format"),
            )
            .await?;
        }
    }
    if check {
        for language in ["swift", "kotlin"] {
            ctx.exec(
                ProcessCommand::new("diff")
                    .current_dir(root)
                    .arg("-ru")
                    .arg(out.join(language))
                    .arg(current.join(language)),
            )
            .await?;
        }
    } else {
        blocking::unblock(move || copy_directory(&current, &out))
            .await
            .map_err(TaskError::operation)?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

fn copy_directory(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
