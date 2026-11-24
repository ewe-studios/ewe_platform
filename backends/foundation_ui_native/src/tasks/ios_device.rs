//! Run Todo and starter tests on an iPhone simulator.
use super::{
    device::{self, cleanup, cleanup_command, text},
    output::device_query as query,
    stage, NativeBuildEnv,
};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Args)]
pub(super) struct Todo {
    device: Option<String>,
}
#[derive(Args)]
pub(super) struct Starter {
    #[arg(long)]
    local_package: Option<PathBuf>,
}

fn sim(env: &NativeBuildEnv, root: &Path) -> ProcessCommand {
    if let Some(exe) = env.devices.get("EWE_SIMCTL_BIN") {
        env.apple_command(root, exe)
    } else {
        env.apple_command(root, "xcrun").arg("simctl")
    }
}
async fn test_app(
    ctx: &TaskContext,
    root: &Path,
    products: &Path,
    app: &str,
    scheme: &str,
    id: &str,
    urls: Option<(&str, &str)>,
) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let mut cmd = env
        .apple_command(root, "xcodebuild")
        .arg("-project")
        .arg(root.join(format!("examples/ui_todo/ios/{app}/{scheme}.xcodeproj")))
        .args([
            "-scheme",
            scheme,
            "-destination",
            &format!("platform=iOS Simulator,id={id}"),
            "-derivedDataPath",
        ])
        .arg(products.join(scheme))
        .arg("-clonedSourcePackagesDirPath")
        .arg(products.join("packages"))
        .args([
            "-parallel-testing-enabled",
            "NO",
            "CODE_SIGNING_ALLOWED=NO",
            "test",
        ]);
    if let Some((url, data)) = urls {
        cmd = cmd
            .env("TEST_RUNNER_EWE_TODO_URL", url)
            .env("TEST_RUNNER_EWE_DATA_URL", data);
    }
    ctx.exec(cmd).await
}

pub(super) async fn todo(ctx: TaskContext, args: Todo) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let root = env.root().await?;
    let id = if let Some(id) = args.device {
        id
    } else {
        let bytes = query(
            &ctx,
            sim(&env, &root).args(["list", "devices", "available", "-j"]),
        )
        .await?;
        let view: Value = serde_json::from_slice(&bytes).map_err(TaskError::operation)?;
        view["devices"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, _)| k.contains(".iOS-"))
            .flat_map(|(_, v)| v.as_array().into_iter().flatten())
            .find(|v| v["name"].as_str().is_some_and(|n| n.starts_with("iPhone")))
            .and_then(|v| v["udid"].as_str())
            .ok_or_else(|| TaskError::operation("No available iPhone simulator"))?
            .to_owned()
    };
    let products = root.join("target/native-ui/ios-todo-tests");
    fs::create_dir_all(&products).map_err(TaskError::operation)?;
    super::todo::prepare(
        ctx.clone(),
        super::todo::Prepare {
            sync_bridge: false,
            output: None,
        },
    )
    .await?;
    ctx.exec(sim(&env, &root).args(["bootstatus", &id, "-b"]))
        .await?;
    test_app(
        &ctx,
        &root,
        &products,
        "todo_on_device",
        "TodoOnDevice",
        &id,
        None,
    )
    .await?;
    let exe = device::host(&ctx, &root).await?;
    let (ctx, root, products, id, exe) = (
        &ctx,
        root.as_path(),
        products.as_path(),
        id.as_str(),
        exe.as_path(),
    );
    device::server(ctx, root, exe, false, |base| async move {
        let first = products.join("first.bin");
        fetch(ctx, &format!("{base}/initial"), &first).await?;
        device::server(ctx, root, exe, true, |data| async move {
            test_app(
                ctx,
                root,
                products,
                "todo_server",
                "TodoServer",
                id,
                Some((&base, &data)),
            )
            .await?;
            let second = products.join("second.bin");
            fetch(ctx, &format!("{base}/initial"), &second).await?;
            for (text, expected) in [
                ("Server integration task", true),
                ("Device-only task", false),
            ] {
                match ctx
                    .exec(
                        ProcessCommand::new("grep")
                            .env("LC_ALL", "C")
                            .args(["-aFq", text])
                            .arg(&second),
                    )
                    .await
                {
                    Ok(()) if expected => {}
                    Err(TaskError::Exit { code: 1, .. }) if !expected => {}
                    Err(e) if !matches!(e, TaskError::Exit { code: 1, .. }) => return Err(e),
                    _ => {
                        return Err(TaskError::operation(format!(
                            "Unexpected server frame for {text}"
                        )))
                    }
                }
            }
            Ok(())
        })
        .await
    })
    .await
}

async fn fetch(ctx: &TaskContext, url: &str, file: &Path) -> TaskResult {
    ctx.exec(
        ProcessCommand::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--max-time",
                "5",
                url,
                "-o",
            ])
            .arg(file),
    )
    .await?;
    if fs::metadata(file).map_err(TaskError::operation)?.len() == 0 {
        return Err(TaskError::operation(
            "The test server returned an empty frame",
        ));
    }
    Ok(())
}

async fn driver(ctx: &TaskContext, root: &Path, temp: &Path) -> Result<PathBuf, TaskError> {
    let env = ctx.require::<NativeBuildEnv>()?;
    if let Some(exe) = env.devices.get("EWE_AXE_BIN") {
        return Ok(PathBuf::from(exe));
    }
    let archive = temp.join("axe.tgz");
    ctx.exec(env.apple_command(root,"curl").args(["--fail","--silent","--show-error","--location","https://github.com/cameroncooke/AXe/releases/download/v2.1.1/AXe-macOS-homebrew-v2.1.1.tar.gz","-o"]).arg(&archive)).await?;
    if stage::digest(&archive).await?
        != "e8a4d873fe49e38d70b8fe1bea180658b5b997f5501561755e304641b1395256"
    {
        return Err(TaskError::operation("Invalid simulator driver checksum"));
    }
    let dest = temp.join("driver");
    fs::create_dir(&dest).map_err(TaskError::operation)?;
    ctx.exec(
        env.apple_command(root, "tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&dest),
    )
    .await?;
    let sign = |file: &Path| {
        env.apple_command(root, "codesign")
            .args(["--force", "--sign", "-", "--timestamp=none"])
            .arg(file)
    };
    for entry in dest
        .join("Frameworks")
        .read_dir()
        .map_err(TaskError::operation)?
    {
        let path = entry.map_err(TaskError::operation)?.path();
        if path.extension().is_some_and(|v| v == "framework") {
            ctx.exec(sign(&path)).await?;
        }
    }
    let exe = dest.join("axe");
    ctx.exec(sign(&exe)).await?;
    Ok(exe)
}

async fn new_device(ctx: &TaskContext, root: &Path, stage: &Path) -> Result<String, TaskError> {
    let env = ctx.require::<NativeBuildEnv>()?;
    let json = query(ctx, sim(&env, root).args(["list", "-j", "runtimes"])).await?;
    let info: Value = serde_json::from_slice(&json).map_err(TaskError::operation)?;
    let runtime = info["runtimes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = r["identifier"].as_str()?;
            if r["isAvailable"] != true || !id.contains(".iOS-") {
                return None;
            }
            let device = r["supportedDeviceTypes"]
                .as_array()?
                .iter()
                .find(|d| d["productFamily"] == "iPhone")?["identifier"]
                .as_str()?;
            let version = r["version"]
                .as_str()?
                .split('.')
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            Some((version, id, device))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .ok_or_else(|| TaskError::operation("Install an iOS runtime with iPhone support"))?;
    let name = format!(
        "Ewe package test {}",
        stage.file_name().unwrap_or_default().to_string_lossy()
    );
    text(
        ctx,
        sim(&env, root).args(["create", &name, runtime.2, runtime.1]),
    )
    .await
}

#[derive(Deserialize)]
struct Control {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "AXLabel")]
    label: Option<String>,
    #[serde(default)]
    children: Vec<Self>,
}
fn has(nodes: &[Control], kind: &str, label: &str) -> bool {
    nodes.iter().any(|c| {
        c.kind.as_deref() == Some(kind) && c.label.as_deref() == Some(label)
            || has(&c.children, kind, label)
    })
}
async fn count(ctx: &TaskContext, axe: &Path, id: &str, n: &str) -> TaskResult {
    device::wait_for(
        ctx,
        Duration::from_secs(60),
        Duration::from_millis(350),
        "the starter count",
        || async {
            let snapshot = query(
                ctx,
                ProcessCommand::new(axe).args(["describe-ui", "--udid", id]),
            )
            .await?;
            let controls: Vec<Control> =
                serde_json::from_slice(&snapshot).map_err(TaskError::operation)?;
            Ok(has(&controls, "StaticText", n) && has(&controls, "Button", "Refresh"))
        },
    )
    .await
}

pub(super) async fn starter(ctx: TaskContext, args: Starter) -> TaskResult {
    let env = ctx.require::<NativeBuildEnv>()?;
    let root = env.root().await?;
    if !cfg!(target_os = "macos") {
        return Err(TaskError::operation(
            "The iOS test requires macOS and Xcode",
        ));
    }
    let temp = tempfile::tempdir().map_err(TaskError::operation)?;
    let stage = temp.path();
    let consumer = stage.join("consumer");
    fs::create_dir(&consumer).map_err(TaskError::operation)?;
    let example = root.join("examples/mobile_starter/ios");
    ctx.exec(
        env.apple_command(&root, "cp")
            .arg("-R")
            .arg(example.join("Starter.xcodeproj"))
            .arg(example.join("App"))
            .arg(&consumer),
    )
    .await?;
    let axe = driver(&ctx, &root, stage).await?;
    let exe = device::host(&ctx, &root).await?;

    let config = stage::config(&root)?;
    let (ctx, root, env, axe, consumer) = (
        &ctx,
        root.as_path(),
        env.as_ref(),
        axe.as_path(),
        consumer.as_path(),
    );
    device::server(ctx, root, &exe, false, |url| async move {
        fetch(
            ctx,
            &format!("{url}/open-count"),
            &stage.join("initial.bin"),
        )
        .await?;
        let project = consumer.join("Starter.xcodeproj");
        let file = project.join("project.pbxproj");
        let text = fs::read_to_string(&file).map_err(TaskError::operation)?;
        let section = text
            .split_once("/* Begin XCSwiftPackageProductDependency")
            .ok_or_else(|| TaskError::operation("Missing public package product"))?
            .1;
        let pattern = regex::Regex::new(r"productName = (\w+);").map_err(TaskError::operation)?;
        if pattern
            .captures_iter(section)
            .map(|v| v[1].to_owned())
            .collect::<Vec<_>>()
            != ["EweUI"]
        {
            return Err(TaskError::operation(
                "The starter must use only the public EweUI product",
            ));
        }
        let package = args
            .local_package
            .map(|p| root.join(p).canonicalize())
            .transpose()
            .map_err(TaskError::operation)?;
        super::swift::point_project(
            &file,
            &config.repository,
            &config.version,
            package.as_deref(),
        )?;
        let app = consumer.join("App/App.swift");
        let source = fs::read_to_string(&app).map_err(TaskError::operation)?;
        if source.matches("http://127.0.0.1:8080").count() != 1 {
            return Err(TaskError::operation(
                "Cannot identify the example server address",
            ));
        }
        fs::write(app, source.replace("http://127.0.0.1:8080", &url))
            .map_err(TaskError::operation)?;
        let supplied = env.devices.get("EWE_IOS_DEVICE");
        let id = if let Some(id) = supplied {
            id.to_string_lossy().into_owned()
        } else {
            new_device(ctx, root, stage).await?
        };
        let run = async {
            ctx.exec(sim(env, root).args(["bootstatus", &id, "-b"]))
                .await?;
            let products = stage.join("products");
            let arch = if std::env::consts::ARCH == "aarch64" {
                "arm64"
            } else {
                std::env::consts::ARCH
            };
            ctx.exec(
                env.apple_command(root, "xcodebuild")
                    .arg("-project")
                    .arg(project)
                    .args([
                        "-scheme",
                        "Starter",
                        "-destination",
                        "generic/platform=iOS Simulator",
                        "-derivedDataPath",
                    ])
                    .arg(&products)
                    .args([
                        &format!("ARCHS={arch}"),
                        "ONLY_ACTIVE_ARCH=YES",
                        "CODE_SIGNING_ALLOWED=NO",
                        "build",
                    ]),
            )
            .await?;
            let app = products.join("Build/Products/Debug-iphonesimulator/Starter.app");
            ctx.exec(sim(env, root).args(["install", &id]).arg(&app))
                .await?;
            let bundle = text_query(
                ctx,
                env.apple_command(root, "/usr/libexec/PlistBuddy")
                    .args(["-c", "Print :CFBundleIdentifier"])
                    .arg(app.join("Info.plist")),
            )
            .await?;
            ctx.exec(sim(env, root).args(["launch", &id, &bundle]))
                .await?;
            count(ctx, axe, &id, "0").await?;
            ctx.exec(
                ProcessCommand::new("node")
                    .current_dir(root)
                    .args(["scripts/ui_native/release/seed-starter.mjs", &url]),
            )
            .await?;
            ctx.exec(
                env.apple_command(root, axe)
                    .args(["tap", "--label", "Refresh", "--udid", &id]),
            )
            .await?;
            count(ctx, axe, &id, "1").await
        }
        .await;
        if supplied.is_some() {
            return run;
        }
        let shutdown = cleanup_command(ctx, sim(env, root).args(["shutdown", &id])).await;
        let delete = cleanup_command(ctx, sim(env, root).args(["delete", &id])).await;
        cleanup(run, cleanup(shutdown, delete))
    })
    .await
}

use super::device::text as text_query;
