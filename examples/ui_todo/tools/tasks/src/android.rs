use super::{output::artifact, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use foundation_ui_native::tasks::{
    android::{build_at, Library},
    sync,
};
#[derive(Args)]
pub(super) struct Build {
    #[arg(long)]
    with_todo: bool,
}
pub(super) async fn build(ctx: TaskContext, args: Build) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let mut libraries = vec![Library::framework()];
    if args.with_todo {
        libraries.push(Library {
            package: "ui_todo",
            library: "ui_todo",
            arguments: &["--lib", "--no-default-features", "--features", "mobile"],
            destinations: ["todo_server", "todo_on_device"]
                .map(|name| {
                    std::path::Path::new("examples/ui_todo/android")
                        .join(name)
                        .join("app/src/main/jniLibs")
                })
                .into(),
        });
    }
    build_at(&ctx, &root, &libraries).await
}
pub(super) async fn sync(ctx: TaskContext, args: sync::Check) -> TaskResult {
    let check = args.check;
    sync::android(ctx.clone(), args).await?;
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let stage = tempfile::tempdir().map_err(TaskError::operation)?;
    let cargo = ProcessCommand::new("cargo").current_dir(&root).args([
        "build",
        "--locked",
        "--profile",
        "uat",
        "--message-format=json-render-diagnostics",
    ]);
    let native = artifact(
        &ctx,
        cargo.clone().args([
            "-p",
            "ui_todo",
            "--lib",
            "--no-default-features",
            "--features",
            "mobile",
        ]),
        "ui_todo",
        &[".so", ".dylib", ".dll"],
    )
    .await?;
    let bindgen = artifact(
        &ctx,
        cargo.args([
            "-p",
            "foundation_ui_native",
            "--features",
            "bindgen",
            "--bin",
            "uniffi-bindgen",
        ]),
        "uniffi-bindgen",
        &[],
    )
    .await?;
    let todo = stage.path().join("todo");
    ctx.exec(
        ProcessCommand::new(bindgen)
            .current_dir(&root)
            .args(["generate", "--library"])
            .arg(native)
            .args(["--crate", "ui_todo", "--language", "kotlin", "--out-dir"])
            .arg(&todo)
            .arg("--no-format"),
    )
    .await?;
    sync::sync(
        &ctx,
        &root,
        &todo,
        &root.join("examples/ui_todo/android/generated"),
        check,
    )
    .await?;
    stage.close().map_err(TaskError::operation)
}
