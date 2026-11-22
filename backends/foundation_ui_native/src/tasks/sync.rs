use super::{bindings, output::artifact, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(super) struct Check {
    #[arg(long)]
    check: bool,
}

pub(super) async fn ios(ctx: TaskContext, args: Check) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let wire = root.join("backends/foundation_ui_wire");
    let package = root.join("mobile/ios");
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    let generated = stage.path().join("bindings");
    bindings::generate(&ctx, &root, Some(&generated), false).await?;
    let source = wire.join("fixtures");
    let dest = stage.path().to_path_buf();
    let gpu = root.join("backends/foundation_ui_gpu/tests/shaders");
    let core = generated.join("swift/EweCore.swift");
    blocking::unblock(move || {
        copy(&[core], &dest.join("EweCore"))?;
        let fixtures = dest.join("Fixtures");
        let native = std::fs::read_dir(source.join("native"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        copy(&native, &fixtures)?;
        copy(
            &named(
                &source,
                &[
                    "frame_ops_all_forms.bin",
                    "frame_state_custom.bin",
                    "state_counter_custom.json",
                    "action_increment_custom.bin",
                ],
            ),
            &fixtures,
        )?;
        if gpu.is_dir() {
            let shaders = std::fs::read_dir(gpu)?
                .filter_map(|entry| match entry {
                    Ok(entry) if entry.path().extension().is_some_and(|ext| ext == "wgsl") => {
                        Some(Ok(entry.path()))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            copy(&shaders, &fixtures)?;
        }
        Ok::<_, std::io::Error>(())
    })
    .await
    .map_err(TaskError::operation)?;
    for (from, into) in [
        (
            wire.join("generated/swift"),
            package.join("Sources/EweWire"),
        ),
        (
            stage.path().join("EweCore"),
            package.join("Sources/EweCore"),
        ),
        (
            stage.path().join("Fixtures"),
            package.join("Tests/EweUITests/Fixtures"),
        ),
    ] {
        sync(&ctx, &root, &from, &into, args.check).await?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

pub(super) async fn android(ctx: TaskContext, args: Check) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    let wire = root.join("backends/foundation_ui_wire");
    let package = root.join("mobile/android");
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    bindings::generate(&ctx, &root, Some(stage.path()), false).await?;
    let source = wire.join("fixtures");
    let dest = stage.path().to_path_buf();
    blocking::unblock(move || {
        copy(
            &named(&source, &["frame_ops_all_forms.bin"]),
            &dest.join("forms"),
        )?;
        copy(
            &named(
                &source,
                &[
                    "frame_state_custom.bin",
                    "state_counter_custom.json",
                    "action_increment_custom.bin",
                ],
            ),
            &dest.join("signed"),
        )
    })
    .await
    .map_err(TaskError::operation)?;
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
    for (from, into) in [
        (
            wire.join("generated/kotlin"),
            package.join("core/src/generated/kotlin"),
        ),
        (
            stage.path().join("kotlin"),
            package.join("binding/src/generated/kotlin"),
        ),
        (
            wire.join("fixtures/native"),
            package.join("core/src/test/resources/native"),
        ),
        (
            stage.path().join("forms"),
            package.join("core/src/test/resources/forms"),
        ),
        (
            stage.path().join("signed"),
            package.join("core/src/test/resources/signed"),
        ),
        (todo, root.join("examples/ui_todo/android/generated")),
    ] {
        sync(&ctx, &root, &from, &into, args.check).await?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

pub(super) async fn sync(
    ctx: &TaskContext,
    root: &Path,
    from: &Path,
    into: &Path,
    check: bool,
) -> TaskResult {
    if check {
        return ctx
            .exec(
                ProcessCommand::new("diff")
                    .current_dir(root)
                    .arg("-ru")
                    .arg(from)
                    .arg(into),
            )
            .await;
    }
    let directory = into.to_path_buf();
    blocking::unblock(move || std::fs::create_dir_all(directory))
        .await
        .map_err(TaskError::operation)?;
    let mut tree = from.as_os_str().to_os_string();
    tree.push("/");
    ctx.exec(
        ProcessCommand::new("rsync")
            .current_dir(root)
            .args(["-a", "--delete"])
            .arg(tree)
            .arg(into),
    )
    .await
}

fn named(directory: &Path, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|name| directory.join(name)).collect()
}

fn copy(files: &[PathBuf], dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for file in files {
        let name = file
            .file_name()
            .ok_or_else(|| std::io::Error::other("Missing source name"))?;
        std::fs::copy(file, dest.join(name))?;
    }
    Ok(())
}
