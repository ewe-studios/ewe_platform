//! Compile and inspect a temporary Android application.
use super::CiEnvironment;
use crate::workspace::capture;
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
};

#[derive(Args)]
pub(super) struct Build {}

pub(super) async fn run(ctx: TaskContext, _: Build) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let required = |key: &str| {
        host.text(key)
            .ok_or_else(|| TaskError::operation(format!("Missing {key}. Run this task with mise.")))
    };
    let platform = required("ANDROID_COMPILE_SDK")?;
    let tools_version = required("ANDROID_BUILD_TOOLS")?;
    let gradle = required("ANDROID_GRADLE_PLUGIN")?;
    let kotlin = required("ANDROID_KOTLIN_VERSION")?;
    let tools = PathBuf::from(required("ANDROID_HOME")?)
        .join("build-tools")
        .join(&tools_version);
    let aapt = tools.join(if cfg!(windows) { "aapt.exe" } else { "aapt" });
    let signer = tools.join(if cfg!(windows) {
        "apksigner.bat"
    } else {
        "apksigner"
    });
    if !aapt.is_file() || !signer.is_file() {
        return Err(TaskError::operation(
            "Run mise run toolchain:android to install build tools",
        ));
    }
    let project = tempfile::tempdir().map_err(TaskError::operation)?;
    let dir = project.path();
    let app = dir.join("app/src/main");
    let source = app.join("kotlin/org/eweplatform/toolchain");
    fs::create_dir_all(&source).map_err(TaskError::operation)?;
    for (file, contents) in [
        (
            dir.join("settings.gradle"),
            include_str!("android-proof/settings.gradle").into(),
        ),
        (
            dir.join("build.gradle"),
            include_str!("android-proof/build.gradle")
                .replace("$ANDROID_GRADLE_PLUGIN", &gradle)
                .replace("$ANDROID_KOTLIN_VERSION", &kotlin),
        ),
        (
            dir.join("app/build.gradle"),
            include_str!("android-proof/app.gradle")
                .replace("$ANDROID_COMPILE_SDK", &platform)
                .replace("$ANDROID_BUILD_TOOLS", &tools_version),
        ),
        (
            app.join("AndroidManifest.xml"),
            include_str!("android-proof/AndroidManifest.xml").into(),
        ),
        (
            source.join("MainActivity.kt"),
            include_str!("android-proof/MainActivity.kt").into(),
        ),
    ] {
        fs::write(file, contents).map_err(TaskError::operation)?;
    }
    ctx.exec(
        ProcessCommand::new("gradle")
            .args([
                "--no-daemon",
                "--console=plain",
                "--warning-mode",
                "all",
                "--project-dir",
            ])
            .arg(dir)
            .arg(":app:assembleDebug"),
    )
    .await?;
    let apk = dir.join("app/build/outputs/apk/debug/app-debug.apk");
    verify_manifest(&ctx, aapt, &apk).await?;
    ctx.exec(
        ProcessCommand::new("unzip")
            .arg("-t")
            .arg(&apk)
            .args(["AndroidManifest.xml", "classes.dex"]),
    )
    .await?;
    ctx.exec(
        ProcessCommand::new(signer)
            .args(["-J-enable-native-access=ALL-UNNAMED", "verify"])
            .arg(apk),
    )
    .await?;
    project.close().map_err(TaskError::operation)?;
    println!("PASS android build (Android Gradle Plugin {gradle}, platform {platform})");
    Ok(())
}

async fn verify_manifest(ctx: &TaskContext, aapt: PathBuf, apk: &std::path::Path) -> TaskResult {
    let info = capture(
        ctx,
        ProcessCommand::new(aapt).args(["dump", "badging"]).arg(apk),
    )
    .await?;
    let (mut package, mut activity) = (false, false);
    for line in BufReader::new(info).lines() {
        let line = line.map_err(TaskError::operation)?;
        package |= line.contains("package: name='org.eweplatform.toolchain'");
        activity |=
            line.contains("launchable-activity: name='org.eweplatform.toolchain.MainActivity'");
    }
    if !(package && activity) {
        return Err(TaskError::operation("The package or launcher is absent"));
    }
    Ok(())
}
