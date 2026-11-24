use crate::{Application, Target};
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult, TaskSet};
use foundation_ui_native::tasks::project;
use std::{path::PathBuf, time::Duration};

#[derive(Args)]
struct Build {
    /// Platform declared in ewe-ui.toml.
    #[arg(long, value_enum)]
    target: Target,
    /// Build an optimized web, iOS, or signed Android release.
    #[arg(long)]
    release: bool,
    /// Use cached Cargo and npm dependencies for a web build.
    #[arg(long)]
    offline: bool,
}
#[derive(Args)]
struct Profile {
    /// Build with the release profile.
    #[arg(long)]
    release: bool,
}
#[derive(Args)]
struct DeviceTarget {
    /// Select Android, which needs its SDK, or iOS, which needs Xcode on macOS.
    #[arg(long, value_enum)]
    target: Target,
}
#[derive(Args)]
struct Device {
    #[command(flatten)]
    platform: DeviceTarget,
    /// Device identifier from devices list.
    #[arg(long)]
    device: String,
    /// Maximum Android emulator startup time in seconds.
    #[arg(long, default_value_t = 300)]
    timeout_seconds: u64,
    /// Android readiness polling interval in milliseconds.
    #[arg(long, default_value_t = 1000)]
    poll_milliseconds: u64,
}
pub struct Directory(pub PathBuf);
async fn build(ctx: TaskContext, args: Build) -> TaskResult {
    let dir = ctx.require::<Directory>()?;
    let app = Application::load(&dir.0).map_err(TaskError::operation)?;
    if !app.targets.contains(&args.target) {
        return Err(TaskError::operation(format!(
            "Undeclared target {}",
            args.target.directory()
        )));
    }
    let artifact = if args.target == Target::Web {
        let web = dir.0.join("web");
        let dest = web.join("dist");
        foundation_wasm_ui::tasks::application::build(
            &ctx,
            &web.join("Cargo.toml"),
            &dest,
            args.release,
            args.offline,
        )
        .await?;
        dest
    } else {
        if args.offline {
            return Err(TaskError::operation("--offline applies to web builds"));
        }
        project::build(&ctx, &dir.0, &app, args.target, args.release, None).await?
    };
    println!(
        "{} {} {}",
        args.target.directory(),
        if args.release { "release" } else { "debug" },
        artifact.display()
    );
    Ok(())
}
async fn devices(ctx: TaskContext, args: DeviceTarget) -> TaskResult {
    for device in project::devices::list(&ctx, args.target).await? {
        println!("{}\t{}\t{}", device.id, device.state, device.name);
    }
    Ok(())
}
async fn open(ctx: TaskContext, args: Device, run: bool) -> TaskResult {
    let target = args.platform.target;
    let loaded = if run {
        let dir = ctx.require::<Directory>()?;
        let app = Application::load(&dir.0).map_err(TaskError::operation)?;
        if !app.targets.contains(&target) {
            return Err(TaskError::operation(
                "This project does not declare the requested target",
            ));
        }
        Some((dir, app))
    } else {
        None
    };
    if args.poll_milliseconds == 0 || args.timeout_seconds == 0 {
        return Err(TaskError::operation(
            "Device timeout and polling interval must be positive",
        ));
    }
    let device = project::devices::start(
        &ctx,
        target,
        &args.device,
        Duration::from_secs(args.timeout_seconds),
        Duration::from_millis(args.poll_milliseconds),
    )
    .await?;
    if let Some((dir, app)) = loaded {
        let artifact = project::build(&ctx, &dir.0, &app, target, false, Some(&device.id)).await?;
        let id = app
            .app_id
            .as_deref()
            .ok_or_else(|| TaskError::operation("Missing application identifier"))?;
        let component = app
            .android
            .as_ref()
            .map_or("", |v| v.launch_component.as_str());
        project::launch(&ctx, target, &device.id, id, component, &artifact).await?;
    }
    println!("{} {} {}", device.id, device.state, device.name);
    Ok(())
}
pub fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "prepare",
        "Stage application Rust bindings and libraries for Xcode or Gradle",
        |ctx, args: Build| async move {
            let root = ctx.require::<Directory>()?;
            let app = Application::load(&root.0).map_err(TaskError::operation)?;
            project::prepare(&ctx, &root.0, &app, args.target, args.release).await
        },
    )?;
    set.add(
        "build",
        "Build the selected target into the application output directory",
        build,
    )?;
    set.expose("build", &["build"])?;
    set.add(
        "devices-list",
        "List installed simulators, emulators, and connected devices",
        devices,
    )?;
    set.expose("devices-list", &["devices", "list"])?;
    set.add(
        "devices-start",
        "Start the selected simulator or emulator and wait until it is ready",
        |ctx, args: Device| open(ctx, args, false),
    )?;
    set.expose("devices-start", &["devices", "start"])?;
    set.add(
        "run",
        "Build, install, and launch an application on the selected device",
        |ctx, args: Device| open(ctx, args, true),
    )?;
    set.expose("run", &["run"])
}
pub fn native() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_ui_native").with_namespace("native");
    for (name, target) in [
        ("build-ios", Target::Ios),
        ("build-android", Target::Android),
    ] {
        set.add(
            name,
            "Build the native application declared in ewe-ui.toml",
            move |ctx, profile: Profile| {
                build(
                    ctx,
                    Build {
                        target,
                        release: profile.release,
                        offline: false,
                    },
                )
            },
        )?;
    }
    Ok(set)
}
