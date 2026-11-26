use super::{output::query, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(super) struct BuildAndroid {}

/// One Cargo library and the Gradle modules that consume its Android artifacts.
pub struct Library<'a> {
    pub package: &'a str,
    pub library: &'a str,
    pub arguments: &'a [&'a str],
    pub destinations: Vec<PathBuf>,
}
impl Library<'_> {
    #[must_use]
    pub fn framework() -> Self {
        Self {
            package: "foundation_ui_native",
            library: "foundation_ui_native",
            arguments: &[],
            destinations: vec!["mobile/android/binding/src/main/jniLibs".into()],
        }
    }
}
pub(super) async fn build(ctx: TaskContext, _: BuildAndroid) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    build_at(&ctx, &root, &[Library::framework()]).await
}

/// Build each supplied library and stage its two supported Android architectures.
///
/// # Errors
/// Returns a tool or artifact staging error.
pub async fn build_at(ctx: &TaskContext, root: &Path, packages: &[Library<'_>]) -> TaskResult {
    let environment = read_environment(ctx, root).await?;
    ctx.exec(ProcessCommand::new("mise").current_dir(root).args([
        "exec",
        "--",
        "rustup",
        "target",
        "add",
        "aarch64-linux-android",
        "x86_64-linux-android",
    ]))
    .await?;
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    for (index, library) in packages.iter().enumerate() {
        let source = stage.path().join(index.to_string());
        compile(
            ctx,
            root,
            &source,
            library.package,
            library.arguments,
            &environment,
        )
        .await?;
        let destinations = library
            .destinations
            .iter()
            .map(|p| root.join(p))
            .collect::<Vec<_>>();
        let name = library.library.to_owned();
        blocking::unblock(move || {
            for target in destinations {
                install(&source, &target, &name)?;
            }
            Ok::<(), TaskError>(())
        })
        .await?;
    }
    blocking::unblock(move || stage.close())
        .await
        .map_err(TaskError::operation)
}

async fn read_environment(
    ctx: &TaskContext,
    root: &Path,
) -> Result<Vec<(OsString, OsString)>, TaskError> {
    let bytes = query(
        ctx,
        ProcessCommand::new("mise")
            .current_dir(root)
            .args(["env", "--json"]),
    )
    .await?;
    let variables: BTreeMap<String, String> =
        serde_json::from_slice(&bytes).map_err(TaskError::operation)?;
    let require = |key| {
        variables
            .get(key)
            .cloned()
            .ok_or_else(|| TaskError::operation(format!("mise did not provide {key}")))
    };
    let env = ctx.require::<NativeBuildEnv>()?;
    let home = env.home.clone().ok_or(TaskError::MissingService("HOME"))?;
    let path = require("PATH")?;
    let android = require("ANDROID_HOME")?;
    let ndk = Path::new(&android)
        .join("ndk")
        .join(require("ANDROID_NDK")?);
    let temp = env.temporary_dir.as_os_str().to_owned();
    Ok(vec![
        ("HOME".into(), home),
        ("PATH".into(), path.into()),
        ("TMPDIR".into(), temp),
        ("ANDROID_HOME".into(), android.into()),
        ("ANDROID_NDK_HOME".into(), ndk.into_os_string()),
    ])
}

async fn compile(
    ctx: &TaskContext,
    root: &Path,
    target: &Path,
    package: &str,
    flags: &[&str],
    environment: &[(OsString, OsString)],
) -> TaskResult {
    let mut command = ProcessCommand::new("cargo")
        .current_dir(root)
        .env_clear()
        .env_remove("CARGO_NDK_OUTPUT_PATH")
        .args(["ndk", "--platform", "26", "-t", "arm64-v8a", "-t", "x86_64"])
        .args([
            "build",
            "--locked",
            "-p",
            package,
            "--release",
            "--target-dir",
        ])
        .arg(target)
        .args(flags.iter().copied());
    for (name, value) in environment {
        command = command.env(name, value);
    }
    ctx.exec(command).await
}

fn install(source: &Path, output: &Path, library: &str) -> TaskResult {
    let name = format!("lib{library}.so");
    let libraries = [
        "foundation_ui_native",
        "foundation_nativeapis",
        "foundation_ui_connectrpc",
    ];
    for (abi, triple) in [
        ("arm64-v8a", "aarch64-linux-android"),
        ("x86_64", "x86_64-linux-android"),
    ] {
        let destination = output.join(abi);
        std::fs::create_dir_all(&destination).map_err(TaskError::operation)?;
        let artifact = source.join(triple).join("release").join(&name);
        let mut temporary =
            tempfile::NamedTempFile::new_in(&destination).map_err(TaskError::operation)?;
        let mut input = std::fs::File::open(&artifact).map_err(|error| {
            TaskError::operation(format!(
                "Missing Android library {}: {error}",
                artifact.display()
            ))
        })?;
        std::io::copy(&mut input, &mut temporary).map_err(TaskError::operation)?;
        temporary
            .persist(destination.join(&name))
            .map_err(TaskError::operation)?;
        for obsolete in libraries
            .into_iter()
            .filter(|candidate| *candidate != library)
        {
            let path = destination.join(format!("lib{obsolete}.so"));
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(TaskError::operation(error)),
            }
        }
    }
    Ok(())
}
