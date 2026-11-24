//! Stage application-owned bindings and libraries in their declared native module.
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use foundation_ui_project::application::{AndroidProject, AppleProject, NativeLibrary};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

async fn bindings(
    ctx: &TaskContext,
    root: &Path,
    lib: &NativeLibrary,
    language: &str,
    out: &Path,
) -> Result<PathBuf, TaskError> {
    let project = root.join(&lib.directory);
    let target = project.join("target");
    ctx.exec(
        ProcessCommand::new("cargo")
            .current_dir(&project)
            .args([
                "build",
                "--features",
                "bindgen",
                "--bin",
                &lib.bindings_bin,
                "--lib",
                "--target-dir",
            ])
            .arg(&target),
    )
    .await?;
    let artifact = target.join("debug").join(format!(
        "{}{}{}",
        std::env::consts::DLL_PREFIX,
        lib.library,
        std::env::consts::DLL_SUFFIX
    ));
    let generator = target.join("debug").join(format!(
        "{}{}",
        lib.bindings_bin,
        std::env::consts::EXE_SUFFIX
    ));
    ctx.exec(
        ProcessCommand::new(generator)
            .current_dir(&project)
            .args(["generate", "--library"])
            .arg(artifact)
            .args([
                "--crate",
                &lib.package,
                "--language",
                language,
                "--no-format",
                "--out-dir",
            ])
            .arg(out),
    )
    .await?;
    Ok(target)
}

pub(super) async fn android(
    ctx: &TaskContext,
    root: &Path,
    app: &AndroidProject,
    lib: &NativeLibrary,
    release: bool,
) -> TaskResult {
    let module = root.join(&app.directory).join(&app.module);
    let temp = tempfile::tempdir_in(&module).map_err(TaskError::operation)?;
    let output = temp.path().join("generated");
    let target = bindings(ctx, root, lib, "kotlin", &output.join("kotlin")).await?;
    let directory = root.join(&lib.directory);
    let mut cmd =
        ProcessCommand::new("cargo")
            .current_dir(&directory)
            .args(["ndk", "--platform", "26"]);
    for abi in &app.architectures {
        cmd = cmd.arg("-t").arg(abi);
    }
    cmd = cmd
        .arg("-o")
        .arg(temp.path().join("libs"))
        .args(["build", "--lib", "--target-dir"])
        .arg(target);
    if release {
        cmd = cmd.arg("--release");
    }
    ctx.exec(cmd).await?;
    for abi in &app.architectures {
        let name = format!("lib{}.so", lib.library);
        let out = output.join("jniLibs").join(abi);
        fs::create_dir_all(&out).map_err(TaskError::operation)?;
        fs::copy(
            temp.path().join("libs").join(abi).join(&name),
            out.join(name),
        )
        .map_err(TaskError::operation)?;
    }
    super::super::staging::replace(
        &output,
        &module.join("generated"),
        &ctx.require::<crate::tasks::NativeBuildEnv>()?.temporary_dir,
    )
    .map_err(TaskError::operation)
}
pub(super) async fn apple(
    ctx: &TaskContext,
    root: &Path,
    app: &AppleProject,
    lib: &NativeLibrary,
    release: bool,
) -> TaskResult {
    let dir = root.join(&app.directory);
    let temp = tempfile::tempdir_in(&dir).map_err(TaskError::operation)?;
    let output = temp.path().join("generated");
    let raw = temp.path().join("bindings");
    let target = bindings(ctx, root, lib, "swift", &raw).await?;
    let headers = output.join(&lib.swift_ffi);
    fs::create_dir_all(&headers).map_err(TaskError::operation)?;
    for (src, dest) in [
        (
            format!("{}.h", lib.swift_ffi),
            format!("{}.h", lib.swift_ffi),
        ),
        (
            format!("{}.modulemap", lib.swift_ffi),
            "module.modulemap".into(),
        ),
    ] {
        fs::copy(raw.join(src), headers.join(dest)).map_err(TaskError::operation)?;
    }
    let file = format!("{}.swift", lib.swift_module);
    let mut swift = fs::File::create(output.join(&file)).map_err(TaskError::operation)?;
    use std::io::Write;
    swift
        .write_all(b"import EweCore\nimport EweCoreFFI\n")
        .map_err(TaskError::operation)?;
    io::copy(
        &mut fs::File::open(raw.join(file)).map_err(TaskError::operation)?,
        &mut swift,
    )
    .map_err(TaskError::operation)?;
    let simulator = match std::env::consts::ARCH {
        "aarch64" => "aarch64-apple-ios-sim",
        "x86_64" => "x86_64-apple-ios",
        other => {
            return Err(TaskError::operation(format!(
                "Unsupported Apple host architecture: {other}"
            )))
        }
    };
    let mut assemble = ProcessCommand::new("xcodebuild").arg("-create-xcframework");
    for slice in ["aarch64-apple-ios", simulator] {
        let mut cmd = ProcessCommand::new("cargo")
            .current_dir(root.join(&lib.directory))
            .env("IPHONEOS_DEPLOYMENT_TARGET", "17.0")
            .args(["build", "--lib", "--target", slice, "--target-dir"])
            .arg(&target);
        if release {
            cmd = cmd.arg("--release");
        }
        ctx.exec(cmd).await?;
        let profile = if release { "release" } else { "debug" };
        assemble = assemble
            .arg("-library")
            .arg(
                target
                    .join(slice)
                    .join(profile)
                    .join(format!("lib{}.a", lib.library)),
            )
            .arg("-headers")
            .arg(&headers);
    }
    ctx.exec(
        assemble
            .arg("-output")
            .arg(output.join(format!("{}.xcframework", lib.swift_ffi))),
    )
    .await?;
    super::super::staging::replace(
        &output,
        &dir.join("generated"),
        &ctx.require::<crate::tasks::NativeBuildEnv>()?.temporary_dir,
    )
    .map_err(TaskError::operation)
}
