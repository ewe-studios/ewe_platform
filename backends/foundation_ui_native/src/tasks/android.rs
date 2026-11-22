use super::{output::query, NativeBuildEnv};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub(super) struct BuildAndroid {
    #[arg(long)]
    with_todo: bool,
}

pub(super) async fn build(ctx: TaskContext, args: BuildAndroid) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    build_at(&ctx, &root, args.with_todo).await
}

pub(super) async fn build_at(ctx: &TaskContext, root: &Path, with_todo: bool) -> TaskResult {
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
    let binding = stage.path().join("binding");
    compile(
        ctx,
        root,
        &binding,
        "foundation_ui_native",
        &[],
        &environment,
    )
    .await?;
    let todo = stage.path().join("todo");
    if with_todo {
        compile(
            ctx,
            root,
            &todo,
            "ui_todo",
            &["--lib", "--no-default-features", "--features", "mobile"],
            &environment,
        )
        .await?;
    }
    let root = root.to_path_buf();
    blocking::unblock(move || {
        install(
            &binding,
            &root.join("mobile/android/binding/src/main/jniLibs"),
            "foundation_ui_native",
        )?;
        if with_todo {
            for app in ["todo_server", "todo_on_device"] {
                let destination = root
                    .join("examples/ui_todo/android")
                    .join(app)
                    .join("app/src/main/jniLibs");
                install(&todo, &destination, "ui_todo")?;
            }
        }
        stage.close().map_err(TaskError::operation)
    })
    .await
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
    output: &Path,
    package: &str,
    flags: &[&str],
    environment: &[(OsString, OsString)],
) -> TaskResult {
    let mut command = ProcessCommand::new("cargo")
        .current_dir(root)
        .env_clear()
        .args([
            "ndk",
            "--platform",
            "26",
            "-t",
            "arm64-v8a",
            "-t",
            "x86_64",
            "-o",
        ])
        .arg(output)
        .args(["build", "--locked", "-p", package, "--release"])
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
    for abi in ["arm64-v8a", "x86_64"] {
        let target = output.join(abi);
        std::fs::create_dir_all(&target).map_err(TaskError::operation)?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(&target).map_err(TaskError::operation)?;
        let mut input =
            std::fs::File::open(source.join(abi).join(&name)).map_err(TaskError::operation)?;
        std::io::copy(&mut input, &mut temporary).map_err(TaskError::operation)?;
        temporary
            .persist(target.join(&name))
            .map_err(TaskError::operation)?;
        for obsolete in libraries
            .into_iter()
            .filter(|candidate| *candidate != library)
        {
            let path = target.join(PathBuf::from(format!("lib{obsolete}.so")));
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(TaskError::operation(error)),
            }
        }
    }
    Ok(())
}
