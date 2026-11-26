use super::{
    output::{artifact, query},
    NativeBuildEnv,
};
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(super) struct AppleBuild {
    #[arg(value_name = "OUTPUT")]
    output: Option<PathBuf>,
}

pub(super) async fn build(ctx: TaskContext, args: AppleBuild) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let root = env.root().await?;
    let output = args
        .output
        .unwrap_or_else(|| PathBuf::from("target/native-ui/apple"));
    build_at(&ctx, &root, &root.join(output)).await
}

pub(super) async fn build_at(ctx: &TaskContext, root: &Path, out: &Path) -> TaskResult {
    if !cfg!(target_os = "macos") {
        return Err(TaskError::operation(
            "The Apple archive requires Xcode on macOS",
        ));
    }
    let env = ctx.require::<NativeBuildEnv>()?;
    let cmd = |program| env.apple_command(root, program);
    ctx.exec(cmd("xcodebuild").arg("-version")).await?;
    let output = out.to_path_buf();
    let stage = blocking::unblock(move || {
        std::fs::create_dir_all(&output).map_err(TaskError::operation)?;
        tempfile::Builder::new()
            .prefix(".build-")
            .tempdir_in(&output)
            .map_err(TaskError::operation)
    })
    .await?;
    let layout = stage.path().to_path_buf();
    let bindings = layout.join("bindings");
    super::bindings::generate(ctx, root, Some(&bindings), false).await?;
    let folders = layout.clone();
    blocking::unblock(move || {
        for folder in ["headers", "device", "simulator", "mac"] {
            std::fs::create_dir_all(folders.join(folder)).map_err(TaskError::operation)?;
        }
        for (src, dst) in [
            ("EweCoreFFI.h", "EweCoreFFI.h"),
            ("EweCoreFFI.modulemap", "module.modulemap"),
        ] {
            std::fs::copy(
                bindings.join("swift").join(src),
                folders.join("headers").join(dst),
            )
            .map_err(TaskError::operation)?;
        }
        Ok::<_, TaskError>(())
    })
    .await?;
    let products = build_slices(ctx, root, &env).await?;
    let device = layout.join("device/libEweCoreFFI.a");
    let simulator = layout.join("simulator/libEweCoreFFI.a");
    let mac = layout.join("mac/libEweCoreFFI.a");
    let (source, dest) = (products[0].clone(), device.clone());
    blocking::unblock(move || std::fs::copy(source, dest))
        .await
        .map_err(TaskError::operation)?;
    for (pair, dest) in [(1..3, &simulator), (3..5, &mac)] {
        ctx.exec(
            cmd("lipo")
                .arg("-create")
                .args(products[pair].iter().map(|path| path.as_os_str()))
                .arg("-output")
                .arg(dest),
        )
        .await?;
    }
    let mut framework = cmd("xcodebuild").arg("-create-xcframework");
    for archive in [device, simulator, mac] {
        framework = framework
            .arg("-library")
            .arg(archive)
            .arg("-headers")
            .arg(layout.join("headers"));
    }
    ctx.exec(
        framework
            .arg("-output")
            .arg(layout.join("EweCoreFFI.xcframework")),
    )
    .await?;
    let archive_layout = layout.clone();
    blocking::unblock(move || super::archive::apple(&archive_layout)).await?;
    let (archive, target) = (
        layout.join("EweCoreFFI.xcframework.zip"),
        out.join("EweCoreFFI.xcframework.zip"),
    );
    ctx.exec(
        cmd("swift")
            .args(["package", "compute-checksum"])
            .arg(&archive),
    )
    .await?;
    blocking::unblock(move || {
        std::fs::rename(archive, target).map_err(TaskError::operation)?;
        stage.close().map_err(TaskError::operation)
    })
    .await
}

async fn build_slices(
    ctx: &TaskContext,
    root: &Path,
    env: &NativeBuildEnv,
) -> Result<Vec<PathBuf>, TaskError> {
    let cmd = |program| env.apple_command(root, program);
    let mut products = Vec::new();
    for (target, sdk) in [
        ("aarch64-apple-ios", "iphoneos"),
        ("aarch64-apple-ios-sim", "iphonesimulator"),
        ("x86_64-apple-ios", "iphonesimulator"),
        ("aarch64-apple-darwin", "macosx"),
        ("x86_64-apple-darwin", "macosx"),
    ] {
        let sdk_path = query(ctx, cmd("xcrun").args(["--sdk", sdk, "--show-sdk-path"])).await?;
        let sdk_path = String::from_utf8(sdk_path).map_err(TaskError::operation)?;
        let build = cmd("cargo")
            .env("SDKROOT", sdk_path.trim())
            .env("MACOSX_DEPLOYMENT_TARGET", "14.0")
            .env("IPHONEOS_DEPLOYMENT_TARGET", "17.0")
            .args([
                "build",
                "--locked",
                "--release",
                "--lib",
                "--target",
                target,
                "-p",
                "foundation_ui_native",
                "--message-format=json-render-diagnostics",
            ]);
        products.push(artifact(ctx, build, "foundation_ui_native", &[".a"]).await?);
    }
    Ok(products)
}
