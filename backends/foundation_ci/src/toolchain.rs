//! Check the native tools selected by the workspace configuration.
mod android;
mod android_build;
mod android_lock;
mod host_setup;
mod proof;
mod release;
mod versions;

use crate::CiEnvironment;
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Args)]
struct Check {
    #[arg(long, value_enum, default_value_t = Scope::Full)]
    scope: Scope,
    #[arg(long)]
    without_android_packages: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Scope {
    Full,
    Wire,
    Android,
    Apple,
}

#[derive(serde::Deserialize)]
struct Pins {
    tools: BTreeMap<String, toml::Value>,
    env: BTreeMap<String, toml::Value>,
}

impl Pins {
    fn tool(&self, key: &str) -> Result<&str, TaskError> {
        self.tools
            .get(key)
            .and_then(|value| value.as_str().or_else(|| value.get("version")?.as_str()))
            .ok_or_else(|| TaskError::operation(format!("Missing tool version for {key}")))
    }

    fn setting(&self, key: &str) -> Result<&str, TaskError> {
        self.env
            .get(key)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| TaskError::operation(format!("Missing toolchain setting {key}")))
    }
}

struct Report {
    failed: bool,
}
impl Report {
    fn entry(&mut self, name: &str, result: Result<String, String>) {
        match result {
            Ok(value) => println!("ok {name}: {value}"),
            Err(reason) => {
                println!("FAIL {name}: {reason}");
                self.failed = true;
            }
        }
    }
}

/// Register toolchain checks for the independent command and the platform command.
///
/// # Errors
/// Returns an error when the catalog already contains this command.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut tasks = TaskSet::new("foundation_toolchain").with_namespace("toolchain");
    tasks.add("check", "Check the pinned native tools", check)?;
    tasks.expose("check", &["toolchain-check"])?;
    tasks.add(
        "wire",
        "Compile the wire toolchain test in four languages",
        wire,
    )?;
    tasks.expose("wire", &["toolchain-wire"])?;
    tasks.add(
        "android",
        "Install the required Android packages",
        android::install,
    )?;
    tasks.expose("android", &["toolchain-android"])?;
    tasks.add("android-lock", "Verify and lock Android tool archives", android_lock::run)?;
    tasks.expose("android-lock", &["toolchain-android-lock"])?;
    tasks.add(
        "android-build",
        "Build and verify an Android application",
        android_build::run,
    )?;
    tasks.expose("android-build", &["toolchain-android-build"])?;
    tasks.add(
        "proof",
        "Verify the Swift and Kotlin foreign language bindings",
        proof::run,
    )?;
    tasks.expose("proof", &["toolchain-proof"])?;
    release::register(&mut tasks)?;
    host_setup::register(&mut tasks)?;
    Ok(tasks)
}

async fn check(ctx: TaskContext, args: Check) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let pins: Pins = toml::from_str(
        &fs::read_to_string(host.root.join("mise.toml")).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    let mut report = Report { failed: false };
    if matches!(args.scope, Scope::Full | Scope::Wire) {
        versions::exact(
            &ctx,
            &mut report,
            "flatc",
            pins.tool("github:google/flatbuffers")?,
            "flatc",
            &["--version"],
            r"^flatc(?: version)? (\S+)",
        )
        .await?;
    }
    if matches!(args.scope, Scope::Full | Scope::Android) {
        android::tools(&ctx, &host, &pins, &mut report).await?;
    }
    if args.scope != Scope::Wire {
        let apple = if cfg!(windows) {
            false
        } else {
            versions::value(&ctx, "uname", &["-s"], r"^(\S+)", None)
                .await?
                .map_err(TaskError::operation)?
                == "Darwin"
        };
        if matches!(args.scope, Scope::Full | Scope::Apple) {
            if apple {
                let mut found = versions::value(
                    &ctx,
                    "xcodebuild",
                    &["-version"],
                    r"^Xcode ([0-9.]+)[ \t]*$",
                    None,
                )
                .await?;
                let developer = "/Applications/Xcode.app/Contents/Developer";
                if found.is_err()
                    && host.get("DEVELOPER_DIR").is_none()
                    && Path::new(developer).is_dir()
                {
                    found = versions::value(
                        &ctx,
                        "xcodebuild",
                        &["-version"],
                        r"^Xcode ([0-9.]+)[ \t]*$",
                        Some(("DEVELOPER_DIR", developer)),
                    )
                    .await?;
                }
                report.entry(
                    "xcode",
                    versions::minimum(found, pins.setting("XCODE_MIN")?),
                );
            } else if args.scope == Scope::Apple {
                report.entry("xcode", Err("requires macOS".into()));
            } else {
                println!("SKIP xcode: requires macOS");
            }
        }
        if args.scope == Scope::Android || (args.scope == Scope::Full && apple) {
            android::sdk(
                &ctx,
                &host,
                &pins,
                &mut report,
                args.without_android_packages,
            )
            .await?;
        } else if args.scope == Scope::Full {
            println!("SKIP android packages: use --scope android to check");
        }
    }
    if report.failed {
        Err(TaskError::operation("Toolchain check failed"))
    } else {
        Ok(())
    }
}

#[derive(Args)]
struct Wire {}

pub(crate) async fn check_wire(ctx: TaskContext) -> TaskResult {
    check(
        ctx,
        Check {
            scope: Scope::Wire,
            without_android_packages: false,
        },
    )
    .await
}

async fn wire(ctx: TaskContext, _: Wire) -> TaskResult {
    check_wire(ctx.clone()).await?;
    let directory = tempfile::tempdir().map_err(TaskError::operation)?;
    let source = directory.path().join("probe.fbs");
    fs::write(
        &source,
        "namespace toolchain;\ntable Probe { sequence: uint; }\nroot_type Probe;\n",
    )
    .map_err(TaskError::operation)?;
    for language in ["rust", "ts", "swift", "kotlin"] {
        let output = directory.path().join(language);
        ctx.exec(
            ProcessCommand::new("flatc")
                .arg(format!("--{language}"))
                .arg("-o")
                .arg(&output)
                .arg(&source),
        )
        .await?;
        if !has_output(&output).map_err(TaskError::operation)? {
            return Err(TaskError::operation(format!("Missing {language} bindings")));
        }
        println!("Generated {language} bindings");
    }
    directory.close().map_err(TaskError::operation)
}

fn has_output(path: &Path) -> std::io::Result<bool> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if (ty.is_dir() && has_output(&entry.path())?)
            || (ty.is_file() && entry.metadata()?.len() > 0)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
