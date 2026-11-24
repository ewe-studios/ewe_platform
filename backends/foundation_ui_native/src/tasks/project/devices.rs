//! List and start mobile devices by their platform identifiers.
use crate::tasks::{
    device::{sleep, text},
    output::device_query,
    NativeBuildEnv,
};
use foundation_tasks::{ProcessCommand, TaskContext, TaskEnv, TaskError};
use foundation_ui_project::application::Target;
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub state: String,
}
pub(super) fn simctl(ctx: &TaskContext) -> Result<ProcessCommand, TaskError> {
    let env = ctx.require::<NativeBuildEnv>()?;
    let mut command = if let Some(path) = env.devices.get("EWE_SIMCTL_BIN") {
        ProcessCommand::new(path)
    } else {
        ProcessCommand::new("xcrun").arg("simctl")
    };
    if let Some(directory) = &env.developer_dir {
        command = command.env("DEVELOPER_DIR", directory);
    }
    Ok(command)
}
pub(super) fn adb(ctx: &TaskContext) -> Result<ProcessCommand, TaskError> {
    let env = ctx.require::<NativeBuildEnv>()?;
    let bin = env.devices.get("ANDROID_HOME").map(PathBuf::from).map(|p| {
        p.join("platform-tools")
            .join(if cfg!(windows) { "adb.exe" } else { "adb" })
    });
    Ok(ProcessCommand::new(bin.unwrap_or_else(|| "adb".into())))
}
fn emulator(ctx: &TaskContext) -> Result<ProcessCommand, TaskError> {
    let env = ctx.require::<NativeBuildEnv>()?;
    let bin = env.devices.get("ANDROID_HOME").map(PathBuf::from).map(|p| {
        p.join("emulator").join(if cfg!(windows) {
            "emulator.exe"
        } else {
            "emulator"
        })
    });
    Ok(ProcessCommand::new(
        bin.unwrap_or_else(|| "emulator".into()),
    ))
}

pub async fn list(ctx: &TaskContext, target: Target) -> Result<Vec<Device>, TaskError> {
    match target {
        Target::Android => {
            let data = text(ctx, adb(ctx)?.arg("devices")).await?;
            let mut devices = Vec::new();
            for line in data.lines().skip(1) {
                if let Some((id, state)) = line.split_once('\t') {
                    let name = if state == "device" && id.starts_with("emulator-") {
                        text(ctx, adb(ctx)?.args(["-s", id, "emu", "avd", "name"]))
                            .await?
                            .lines()
                            .next()
                            .unwrap_or(id)
                            .to_owned()
                    } else {
                        id.to_owned()
                    };
                    devices.push(Device {
                        id: id.into(),
                        name,
                        state: state.into(),
                    });
                }
            }
            let avds = text(ctx, emulator(ctx)?.arg("-list-avds")).await?;
            for name in avds.lines().filter(|s| !s.is_empty()) {
                if !devices.iter().any(|d| d.name == name) {
                    devices.push(Device {
                        id: name.to_owned(),
                        name: name.to_owned(),
                        state: "Stopped".into(),
                    });
                }
            }
            Ok(devices)
        }
        Target::Ios => {
            #[derive(Deserialize)]
            struct List {
                devices: BTreeMap<String, Vec<Sim>>,
            }
            #[derive(Deserialize)]
            struct Sim {
                name: String,
                udid: String,
                state: String,
            }
            let bytes = device_query(
                ctx,
                simctl(ctx)?.args(["list", "devices", "available", "-j"]),
            )
            .await?;
            let all: List = serde_json::from_slice(&bytes).map_err(TaskError::operation)?;
            Ok(all
                .devices
                .into_iter()
                .filter(|(runtime, _)| runtime.contains(".iOS-"))
                .flat_map(|(_, sims)| sims)
                .map(|s| Device {
                    id: s.udid,
                    name: s.name,
                    state: s.state,
                })
                .collect())
        }
        Target::Web => Err(TaskError::operation(
            "Web applications do not need a device",
        )),
    }
}
pub fn select(devices: &[Device], name: &str) -> Result<Device, TaskError> {
    let mut found = devices.iter().filter(|d| d.id == name || d.name == name);
    let first = found.next().ok_or_else(|| {
        TaskError::operation(format!(
            "No available device or system image matches {name}"
        ))
    })?;
    if found.next().is_some() {
        return Err(TaskError::operation(format!(
            "Ambiguous device {name}. Select a device identifier."
        )));
    }
    Ok(first.clone())
}
pub async fn start(
    ctx: &TaskContext,
    target: Target,
    name: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<Device, TaskError> {
    let since = ctx.context().clock().elapsed();
    let deadline = async {
        sleep(ctx, timeout).await?;
        Err(TaskError::operation(format!("Timed out looking up {name}")))
    };
    let devices = futures_lite::future::race(list(ctx, target), deadline).await?;
    let device = select(&devices, name)?;
    let remaining = timeout.saturating_sub(ctx.context().clock().elapsed().saturating_sub(since));
    let deadline = async {
        sleep(ctx, remaining).await?;
        Err(TaskError::operation(format!("Timed out starting {name}")))
    };
    if target == Target::Ios {
        let owned = device.state == "Shutdown";
        let boot = async {
            if owned {
                ctx.exec(simctl(ctx)?.args(["boot", &device.id])).await?;
            }
            ctx.exec(simctl(ctx)?.args(["bootstatus", &device.id, "-b"]))
                .await?;
            Ok(Device {
                state: "Booted".into(),
                ..device.clone()
            })
        };
        let result = futures_lite::future::race(boot, deadline).await;
        if let Err(source) = result {
            if owned {
                let host = ctx.require::<TaskEnv>()?;
                if let Err(cleanup) = host
                    .process
                    .run(
                        simctl(ctx)?.args(["shutdown", &device.id]),
                        host.output.clone(),
                        foundation_tasks::Cancellation::new(),
                    )
                    .await
                {
                    return Err(TaskError::Cleanup {
                        source: Box::new(source),
                        cleanup: Box::new(cleanup),
                    });
                }
            }
            return Err(source);
        }
        return result;
    }
    futures_lite::future::race(start_android(ctx, device, interval), deadline).await
}

async fn start_android(
    ctx: &TaskContext,
    device: Device,
    interval: Duration,
) -> Result<Device, TaskError> {
    let mut owned = if device.state == "Stopped" {
        let cmd = emulator(ctx)?.args(["-avd", &device.name]);
        Some(
            ctx.require::<TaskEnv>()?
                .process
                .service(cmd, ctx.cancellation().clone())
                .await?,
        )
    } else {
        None
    };
    loop {
        if let Some(child) = owned.as_mut() {
            if let Some(code) = child.status().map_err(TaskError::operation)? {
                return Err(TaskError::operation(format!(
                    "Emulator {} exited with {code}",
                    device.name
                )));
            }
        }
        for candidate in list(ctx, Target::Android).await? {
            if (candidate.id == device.id || candidate.name == device.name)
                && candidate.state == "device"
                && text(
                    ctx,
                    adb(ctx)?.args([
                        "-s",
                        &candidate.id,
                        "shell",
                        "getprop",
                        "sys.boot_completed",
                    ]),
                )
                .await?
                    == "1"
            {
                if let Some(child) = owned {
                    child.detach().map_err(TaskError::operation)?;
                }
                return Ok(candidate);
            }
        }
        sleep(ctx, interval).await?;
    }
}
