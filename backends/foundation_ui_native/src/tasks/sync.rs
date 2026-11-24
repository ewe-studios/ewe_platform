use super::{bindings, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct Check {
    #[arg(long)]
    pub check: bool,
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

pub async fn android(ctx: TaskContext, args: Check) -> TaskResult {
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
    ] {
        sync(&ctx, &root, &from, &into, args.check).await?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

pub async fn sync(
    ctx: &TaskContext,
    root: &Path,
    from: &Path,
    into: &Path,
    check: bool,
) -> TaskResult {
    let source = root.join(from);
    let destination = root.join(into);
    let locks = ctx.require::<NativeBuildEnv>()?.temporary_dir.clone();
    let cancel = ctx.cancellation().clone();
    blocking::unblock(move || super::staging::sync(&source, &destination, check, &locks, &cancel))
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
