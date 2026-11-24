use super::{
    output::{artifact, query},
    sync, NativeBuildEnv,
};
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult};
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(super) struct Prepare {
    #[arg(long)]
    pub(super) sync_bridge: bool,
    #[arg(value_name = "OUTPUT")]
    pub(super) output: Option<PathBuf>,
}

pub(super) async fn prepare(ctx: TaskContext, args: Prepare) -> TaskResult {
    if !cfg!(target_os = "macos") {
        return Err(TaskError::operation(
            "The Todo Apple package requires Xcode on macOS",
        ));
    }
    let simulator = match std::env::consts::ARCH {
        "aarch64" => "aarch64-apple-ios-sim",
        "x86_64" => "x86_64-apple-ios",
        arch => {
            return Err(TaskError::operation(format!(
                "Unsupported simulator CPU {arch}"
            )))
        }
    };
    let env = ctx.require::<NativeBuildEnv>()?;
    let root = env.root().await?;
    let cmd = |program| env.apple_command(&root, program);
    let output = root.join(
        args.output
            .unwrap_or_else(|| PathBuf::from("target/native-ui/ios-todo")),
    );
    let stage = create_stage(&output).await?;
    let cargo = cmd("cargo").args([
        "build",
        "--locked",
        "--profile",
        "uat",
        "-p",
        "ui_todo",
        "-p",
        "foundation_ui_native",
        "--no-default-features",
        "--features",
        "ui_todo/mobile,foundation_ui_native/bindgen",
        "--message-format=json-render-diagnostics",
    ]);
    let library = artifact(&ctx, cargo.clone(), "ui_todo", &[".dylib"]).await?;
    let generator = artifact(&ctx, cargo, "uniffi-bindgen", &[]).await?;
    let generated = stage.path().join("generated");
    for package in ["foundation_ui_native", "ui_todo"] {
        ctx.exec(
            env.apple_command(&root, generator.as_os_str())
                .args(["generate", "--library"])
                .arg(&library)
                .args(["--crate", package, "--language", "swift", "--out-dir"])
                .arg(&generated)
                .arg("--no-format"),
        )
        .await?;
    }
    let bridge = root.join("examples/ui_todo/ios/Bridge/EweTodoCore.swift");
    let staged_bridge = stage.path().join("EweTodoCore.swift");
    prepare_bridge(&generated, &staged_bridge, &bridge, args.sync_bridge).await?;
    if !args.sync_bridge {
        ctx.exec(cmd("diff").arg("-u").arg(&bridge).arg(&staged_bridge))
            .await?;
        let headers = stage.path().join("headers");
        copy_headers(&generated, &headers).await?;
        let mut framework = cmd("xcodebuild").arg("-create-xcframework");
        for (sdk, target) in [
            ("iphoneos", "aarch64-apple-ios"),
            ("iphonesimulator", simulator),
        ] {
            let archive = slice(&ctx, &env, &root, stage.path(), sdk, target).await?;
            framework = framework
                .arg("-library")
                .arg(archive)
                .arg("-headers")
                .arg(&headers);
        }
        let built = stage.path().join("EweCoreFFI.xcframework");
        ctx.exec(framework.arg("-output").arg(&built)).await?;
        stage_package(
            &ctx,
            &root,
            &output,
            stage.path(),
            &generated,
            &bridge,
            &built,
        )
        .await?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

async fn create_stage(output: &Path) -> Result<tempfile::TempDir, TaskError> {
    let output = output.to_path_buf();
    blocking::unblock(move || {
        std::fs::create_dir_all(&output)?;
        tempfile::Builder::new()
            .prefix(".build-")
            .tempdir_in(output)
    })
    .await
    .map_err(TaskError::operation)
}

async fn prepare_bridge(
    generated: &Path,
    staged: &Path,
    bridge: &Path,
    update: bool,
) -> TaskResult {
    let source = generated.join("EweTodoCore.swift");
    let (staged, bridge) = (staged.to_path_buf(), bridge.to_path_buf());
    blocking::unblock(move || {
        use std::io::Write as _;
        let mut output = std::fs::File::create(&staged)?;
        output.write_all(b"import EweCore\nimport EweCoreFFI\n")?;
        io::copy(&mut std::fs::File::open(source)?, &mut output)?;
        output.flush()?;
        if update {
            std::fs::copy(staged, bridge)?;
        }
        Ok::<_, io::Error>(())
    })
    .await
    .map_err(TaskError::operation)
}

async fn copy_headers(generated: &Path, headers: &Path) -> TaskResult {
    let (generated, headers) = (generated.to_path_buf(), headers.to_path_buf());
    blocking::unblock(move || {
        std::fs::create_dir_all(&headers)?;
        for name in ["EweCoreFFI.h", "EweTodoCoreFFI.h"] {
            std::fs::copy(generated.join(name), headers.join(name))?;
        }
        std::fs::write(headers.join("module.modulemap"),
            "module EweCoreFFI {\n    header \"EweCoreFFI.h\"\n    header \"EweTodoCoreFFI.h\"\n    export *\n}\n")
    }).await.map_err(TaskError::operation)
}

async fn slice(
    ctx: &TaskContext,
    env: &NativeBuildEnv,
    root: &Path,
    stage: &Path,
    sdk: &str,
    target: &str,
) -> Result<PathBuf, TaskError> {
    let xcrun = env
        .apple_command(root, "xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"]);
    let sdk_path = String::from_utf8(query(ctx, xcrun).await?).map_err(TaskError::operation)?;
    let cargo = env
        .apple_command(root, "cargo")
        .env("SDKROOT", sdk_path.trim())
        .env("IPHONEOS_DEPLOYMENT_TARGET", "17.0")
        .args([
            "build",
            "--locked",
            "--release",
            "-p",
            "ui_todo",
            "--lib",
            "--no-default-features",
            "--features",
            "mobile",
            "--target",
            target,
            "--message-format=json-render-diagnostics",
        ]);
    let archive = artifact(ctx, cargo, "ui_todo", &[".a"]).await?;
    let directory = stage.join(sdk);
    blocking::unblock(move || {
        std::fs::create_dir_all(&directory)?;
        let link = directory.join("libEweCoreFFI.a");
        if std::fs::hard_link(&archive, &link).is_err() {
            std::fs::copy(archive, &link)?;
        }
        Ok::<_, io::Error>(link)
    })
    .await
    .map_err(TaskError::operation)
}

async fn stage_package(
    ctx: &TaskContext,
    root: &Path,
    output: &Path,
    stage: &Path,
    generated: &Path,
    bridge: &Path,
    framework: &Path,
) -> TaskResult {
    for directory in ["Sources", "Tests"] {
        let path = Path::new("mobile/ios").join(directory);
        sync::sync(ctx, root, &root.join(&path), &output.join(path), false).await?;
    }
    let (root, output, stage, generated, bridge, framework) = (
        root.to_path_buf(),
        output.to_path_buf(),
        stage.to_path_buf(),
        generated.to_path_buf(),
        bridge.to_path_buf(),
        framework.to_path_buf(),
    );
    blocking::unblock(move || {
        let bridge_dir = output.join("examples/ui_todo/ios/Bridge");
        std::fs::create_dir_all(&bridge_dir)?;
        std::fs::copy(bridge, bridge_dir.join("EweTodoCore.swift"))?;
        std::fs::copy(generated.join("EweCore.swift"), output.join("mobile/ios/Sources/EweCore/EweCore.swift"))?;
        std::fs::copy(root.join("Package.resolved"), output.join("Package.resolved"))?;
        let manifest = std::fs::read_to_string(root.join("Package.swift"))?;
        let manifest = manifest.replacen("products: [", "products: [.library(name: \"EweTodoCore\", targets: [\"EweTodoCore\"]), ", 1)
            .replacen("        core,", "        core,\n        .target(name: \"EweTodoCore\", dependencies: [\"EweCore\", \"EweCoreFFI\"], path: \"examples/ui_todo/ios/Bridge\"),", 1);
        std::fs::write(output.join("Package.swift"), manifest)?;
        let packages = output.join(".build-inputs");
        std::fs::create_dir_all(&packages)?;
        replace_framework(&framework, &packages.join("EweCoreFFI.xcframework"), &stage.join("previous.xcframework"))
    }).await.map_err(TaskError::operation)
}

fn replace_framework(source: &Path, target: &Path, prior: &Path) -> io::Result<()> {
    let exists = target.try_exists()?;
    if exists {
        std::fs::rename(target, prior)?;
    }
    if let Err(error) = std::fs::rename(source, target) {
        if exists {
            std::fs::rename(prior, target).map_err(|cleanup| {
                io::Error::other(format!("{error}; restore failed: {cleanup}"))
            })?;
        }
        return Err(error);
    }
    Ok(())
}
