use super::{versions, Pins, Report};
use crate::CiEnvironment;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn find(host: &CiEnvironment, name: &str) -> Option<PathBuf> {
    let path = host.get("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let file = dir.join(name);
        if file.is_file() {
            return Some(file);
        }
        if cfg!(windows) {
            for ext in ["exe", "cmd", "bat"] {
                let file = file.with_extension(ext);
                if file.is_file() {
                    return Some(file);
                }
            }
        }
        None
    })
}

pub(super) async fn tools(
    ctx: &TaskContext,
    host: &CiEnvironment,
    pins: &Pins,
    report: &mut Report,
) -> TaskResult {
    let (vendor, version) = pins
        .tool("java")?
        .split_once('-')
        .ok_or_else(|| TaskError::operation("Expected a Java vendor and version"))?;
    let build = version.strip_suffix(".0.LTS").unwrap_or(version);
    let java_path = |name: &str| {
        host.get("JAVA_HOME").map_or_else(
            || PathBuf::from(name),
            |p| PathBuf::from(p).join("bin").join(name),
        )
    };
    versions::exact(
        ctx,
        report,
        "java",
        build,
        java_path("java"),
        &["-version"],
        &format!(
            r"(?i)^OpenJDK Runtime Environment {}-(\S+)",
            regex::escape(vendor)
        ),
    )
    .await?;
    versions::exact(
        ctx,
        report,
        "javac",
        build.split('+').next().unwrap_or(build),
        java_path("javac"),
        &["-version"],
        r"^javac (\S+)",
    )
    .await?;
    versions::exact(
        ctx,
        report,
        "gradle",
        pins.tool("gradle")?,
        "gradle",
        &["--version"],
        r"^Gradle (\S+)",
    )
    .await?;
    versions::exact(
        ctx,
        report,
        "kotlin",
        pins.tool("kotlin")?,
        "kotlinc",
        &["-version"],
        r"\bkotlinc-jvm (\S+)",
    )
    .await?;
    if find(host, "cargo-ndk").is_some() {
        versions::exact(
            ctx,
            report,
            "cargo-ndk",
            pins.tool("github:bbqsrc/cargo-ndk")?,
            "cargo",
            &["ndk", "--version"],
            r"^cargo-ndk (\S+)",
        )
        .await?;
    } else {
        report.entry("cargo-ndk", Err("not found".into()));
    }
    Ok(())
}

async fn revision(
    ctx: &TaskContext,
    host: &CiEnvironment,
) -> Result<Result<String, String>, TaskError> {
    let Some(mut manager) = find(host, "sdkmanager") else {
        return Ok(Err("sdkmanager not found".into()));
    };
    if manager
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|v| v == "shims")
        && find(host, "mise").is_some()
    {
        let resolved =
            versions::value(ctx, "mise", &["which", "sdkmanager"], r"^(.+)$", None).await?;
        let Ok(path) = resolved else {
            return Ok(Err("mise could not resolve sdkmanager".into()));
        };
        manager = PathBuf::from(path);
    }
    let manager = manager.canonicalize().map_err(TaskError::operation)?;
    let parent = manager
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| TaskError::operation("Invalid sdkmanager directory"))?;
    let metadata = parent.join("source.properties");
    let text = match fs::read_to_string(&metadata) {
        Ok(text) => text,
        Err(error) => {
            return Ok(Err(format!(
                "could not read the revision in {}: {error}",
                metadata.display()
            )))
        }
    };
    Ok(text
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "Pkg.Revision")
        .map(|(_, v)| v.trim().to_owned())
        .ok_or_else(|| format!("could not read the revision in {}", metadata.display())))
}

pub(super) async fn sdk(
    ctx: &TaskContext,
    host: &CiEnvironment,
    pins: &Pins,
    report: &mut Report,
    skip: bool,
) -> TaskResult {
    report.entry(
        "android-sdk",
        versions::equal(revision(ctx, host).await?, pins.tool("http:android-sdk")?),
    );
    if skip {
        println!("SKIP android packages: check disabled");
        return Ok(());
    }
    let mut packages = vec![
        "platform-tools".into(),
        format!("platforms/android-{}", pins.setting("ANDROID_COMPILE_SDK")?),
        format!("build-tools/{}", pins.setting("ANDROID_BUILD_TOOLS")?),
        format!("ndk/{}", pins.setting("ANDROID_NDK")?),
    ];
    if let Some(ui) = pins
        .env
        .get("ANDROID_UI_COMPILE_SDK")
        .and_then(toml::Value::as_str)
        .filter(|v| !v.is_empty())
    {
        packages.push(format!("platforms/android-{ui}"));
    }
    let result = host
        .get("ANDROID_HOME")
        .ok_or_else(|| "ANDROID_HOME is absent; run: mise run toolchain:android".to_owned())
        .and_then(|home| {
            let missing: Vec<_> = packages
                .iter()
                .filter(|p| !Path::new(&home).join(p).join("source.properties").is_file())
                .cloned()
                .collect();
            if missing.is_empty() {
                Ok(packages.join(", "))
            } else {
                Err(format!(
                    "missing {}; run: mise run toolchain:android",
                    missing.join(", ")
                ))
            }
        });
    report.entry("android packages", result);
    Ok(())
}

#[derive(clap::Args)]
pub(super) struct Install {}

pub(super) async fn install(ctx: TaskContext, _: Install) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let required = |key: &str| {
        host.text(key)
            .ok_or_else(|| TaskError::operation(format!("Missing {key}. Run this task with mise.")))
    };
    let home = PathBuf::from(required("ANDROID_HOME")?);
    let platform = required("ANDROID_COMPILE_SDK")?;
    let mut packages = vec![
        "platform-tools".into(),
        format!("platforms;android-{platform}"),
        format!("build-tools;{}", required("ANDROID_BUILD_TOOLS")?),
        format!("ndk;{}", required("ANDROID_NDK")?),
    ];
    if let Some(ui) = host
        .text("ANDROID_UI_COMPILE_SDK")
        .filter(|ui| ui != &platform)
    {
        packages.push(format!("platforms;android-{ui}"));
    }
    let installed = |pkg: &str| {
        home.join(pkg.replace(';', "/"))
            .join("source.properties")
            .is_file()
    };
    packages.retain(|pkg| !installed(pkg));
    if packages.is_empty() {
        println!("All Android SDK packages are present.");
        return Ok(());
    }
    let mut argument = std::ffi::OsString::from("--sdk_root=");
    argument.push(&home);
    let manager = ProcessCommand::new("sdkmanager").arg(argument);
    ctx.exec(
        manager
            .clone()
            .arg("--licenses")
            .repeated_input(b"y\n".to_vec()),
    )
    .await?;
    ctx.exec(manager.arg("--install").args(&packages)).await?;
    for pkg in packages {
        if !installed(&pkg) {
            return Err(TaskError::operation(format!(
                "sdkmanager did not install {pkg}"
            )));
        }
    }
    Ok(())
}
