//! Build, install, and launch an application from its declared host settings.
pub mod devices;
mod libraries;
use super::device::text;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use foundation_ui_project::application::{AndroidProject, AppleProject, Application, Target};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn problem(target: Target) -> TaskError {
    TaskError::operation(format!(
        "This application does not declare the {} target",
        target.directory()
    ))
}
fn android(app: &Application) -> Result<&AndroidProject, TaskError> {
    app.android.as_ref().ok_or_else(|| problem(Target::Android))
}
fn ios(app: &Application) -> Result<&AppleProject, TaskError> {
    app.ios.as_ref().ok_or_else(|| problem(Target::Ios))
}

/// Prepare the on-device Rust library and bindings for the selected host.
///
/// # Errors
/// Returns the Cargo, binding generator, or native staging error.
pub async fn prepare(
    ctx: &TaskContext,
    root: &Path,
    app: &Application,
    target: Target,
    release: bool,
) -> TaskResult {
    match target {
        Target::Android => {
            let project = android(app)?;
            if let Some(lib) = &app.native {
                libraries::android(ctx, root, project, lib, release).await?;
            }
        }
        Target::Ios => {
            let project = ios(app)?;
            if let Some(lib) = &app.native {
                libraries::apple(ctx, root, project, lib, release).await?;
            }
        }
        Target::Web => {
            return Err(TaskError::operation(
                "Native preparation requires Android or iOS",
            ))
        }
    }
    Ok(())
}

/// Build the selected native host. Return its verified artifact path.
///
/// # Errors
/// Returns errors from tool execution, missing signing settings, and archive verification.
pub async fn build(
    ctx: &TaskContext,
    root: &Path,
    app: &Application,
    target: Target,
    release: bool,
    device: Option<&str>,
) -> Result<PathBuf, TaskError> {
    match target {
        Target::Android => {
            let project = android(app)?;
            let dir = root.join(&project.directory);
            let task = if release {
                "bundleRelease"
            } else {
                "assembleDebug"
            };
            let wrapper = dir.join(if cfg!(windows) {
                "gradlew.bat"
            } else {
                "gradlew"
            });
            ctx.exec(
                ProcessCommand::new(wrapper)
                    .current_dir(&dir)
                    .arg(format!(":{}:{task}", project.module)),
            )
            .await?;
            let output = dir.join(&project.module).join("build/outputs");
            let archive = if release {
                output.join(format!("bundle/release/{}-release.aab", project.module))
            } else {
                output.join(format!("apk/debug/{}-debug.apk", project.module))
            };
            verify_android(ctx, app, project, &archive, release).await?;
            Ok(archive)
        }
        Target::Ios => {
            let project = ios(app)?;
            let dir = root.join(&project.directory);
            if app.native.is_some() {
                prepare(ctx, root, app, Target::Ios, release).await?;
            }
            let configuration = if release { "Release" } else { "Debug" };
            let destination = device.map_or_else(
                || "generic/platform=iOS Simulator".to_owned(),
                |id| format!("platform=iOS Simulator,id={id}"),
            );
            let derived = dir.join("build");
            ctx.exec(
                ProcessCommand::new("xcodebuild")
                    .current_dir(&dir)
                    .arg("-project")
                    .arg(&project.project)
                    .args([
                        "-scheme",
                        &project.scheme,
                        "-configuration",
                        configuration,
                        "-destination",
                        &destination,
                        "-derivedDataPath",
                    ])
                    .arg(&derived)
                    .arg("EWE_UI_NATIVE_PREPARED=YES")
                    .arg("build"),
            )
            .await?;
            let path = derived.join(format!(
                "Build/Products/{configuration}-iphonesimulator/{}.app",
                project.product
            ));
            if !path.join("Info.plist").is_file() {
                return Err(TaskError::operation(format!(
                    "Missing application at {}",
                    path.display()
                )));
            }
            Ok(path)
        }
        Target::Web => Err(problem(target)),
    }
}

/// Install and launch an existing artifact with the given application identity.
///
/// # Errors
/// Returns the device identifier and command error if installation or launch fails.
pub async fn launch(
    ctx: &TaskContext,
    target: Target,
    id: &str,
    application_id: &str,
    component: &str,
    artifact: &Path,
) -> TaskResult {
    let result = async {
        match target {
            Target::Android => {
                let adb = || devices::adb(ctx).map(|c| c.args(["-s", id]));
                ctx.exec(adb()?.args(["install", "-r"]).arg(artifact))
                    .await?;
                let report = text(
                    ctx,
                    adb()?.args([
                        "shell",
                        "am",
                        "start",
                        "-W",
                        "-n",
                        &format!("{application_id}/{component}"),
                    ]),
                )
                .await?;
                let status = report
                    .lines()
                    .find_map(|line| line.trim().strip_prefix("Status: "));
                if !matches!(status, Some("ok" | "timeout")) {
                    return Err(TaskError::operation(format!(
                        "Android launch failed: {report}"
                    )));
                }
                let class = component.strip_prefix('.').map_or_else(
                    || component.to_owned(),
                    |short| format!("{application_id}.{short}"),
                );
                wait_for_window(ctx, adb()?, &format!("{application_id}/{class}"), &report).await?;
            }
            Target::Ios => {
                ctx.exec(devices::simctl(ctx)?.args(["install", id]).arg(artifact))
                    .await?;
                ctx.exec(devices::simctl(ctx)?.args(["launch", id, application_id]))
                    .await?;
            }
            Target::Web => return Err(problem(target)),
        }
        Ok(())
    }
    .await;
    result.map_err(|e| TaskError::Task {
        task: format!("Launch {} on {id}", target.directory()),
        source: Box::new(e),
    })
}

async fn wait_for_window(
    ctx: &TaskContext,
    adb: ProcessCommand,
    expected: &str,
    report: &str,
) -> TaskResult {
    let env = ctx.require::<super::NativeBuildEnv>()?;
    if env.window_poll.is_zero() {
        return Err(TaskError::operation(
            "Android window polling interval must be positive",
        ));
    }
    let clock = ctx.context().clock();
    let deadline = clock
        .now()
        .checked_add(env.window_wait)
        .ok_or_else(|| TaskError::operation("Android window deadline exceeds the clock range"))?;
    loop {
        let focus = text(
            ctx,
            adb.clone()
                .args(["shell", "dumpsys window | grep mCurrentFocus="]),
        )
        .await?;
        let mut windows = focus.lines().filter_map(|line| {
            line.trim()
                .strip_prefix("mCurrentFocus=Window{")?
                .strip_suffix('}')?
                .split_whitespace()
                .last()
        });
        if windows.any(|name| name == expected) {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(clock.now());
        if remaining.is_zero() || focus.contains("Application Not Responding:") {
            return Err(TaskError::operation(format!(
                "Android launch did not focus {expected}: {report}\n{focus}"
            )));
        }
        super::device::sleep(ctx, env.window_poll.min(remaining)).await?;
    }
}

async fn verify_android(
    ctx: &TaskContext,
    app: &Application,
    project: &AndroidProject,
    archive: &Path,
    release: bool,
) -> TaskResult {
    let mut required = project.framework_libraries.clone();
    if let Some(lib) = &app.native {
        required.push(format!("lib{}.so", lib.library));
    }
    let source = fs::File::open(archive).map_err(TaskError::operation)?;
    let zip = zip::ZipArchive::new(source).map_err(TaskError::operation)?;
    let mut names = BTreeMap::new();
    for name in zip.file_names() {
        let name = name.map_err(TaskError::operation)?.into_owned();
        *names.entry(name).or_insert(0usize) += 1;
    }
    for abi in &project.architectures {
        for lib in &required {
            let name = format!("{}lib/{abi}/{lib}", if release { "base/" } else { "" });
            if names.get(&name) != Some(&1) {
                return Err(TaskError::operation(format!(
                    "Expected one {name} in {}",
                    archive.display()
                )));
            }
        }
    }
    if release
        && !names.keys().any(|name| {
            name.starts_with("META-INF/")
                && [".RSA", ".DSA", ".EC"]
                    .iter()
                    .any(|ext| name.ends_with(ext))
        })
    {
        return Err(TaskError::operation("The Android release bundle is unsigned. Configure the release signing settings in the application module."));
    }
    if release {
        let verified = super::output::verifies_signature(
            ctx,
            ProcessCommand::new("jarsigner")
                .args(["-J-Duser.language=en", "-J-Duser.country=US", "-verify"])
                .arg(archive),
        )
        .await?;
        if !verified {
            return Err(TaskError::operation(format!(
                "Cannot verify the Android release signature in {}",
                archive.display()
            )));
        }
    }
    Ok(())
}
