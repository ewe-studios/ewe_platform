//! Developer tasks for native application packages.
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

mod android;
mod apple;
mod bindings;
mod output;
mod release;
mod sync;
mod todo;

#[derive(Clone)]
pub struct NativeBuildEnv {
    start: PathBuf,
    query_bytes: usize,
    cargo_message_bytes: usize,
    developer_dir: Option<OsString>,
    home: Option<OsString>,
    temporary_dir: PathBuf,
}

impl NativeBuildEnv {
    #[must_use]
    pub fn new(start: PathBuf) -> Self {
        Self {
            start,
            query_bytes: 8192,
            cargo_message_bytes: 1024 * 1024,
            developer_dir: None,
            home: None,
            temporary_dir: PathBuf::from("/tmp"),
        }
    }

    #[must_use]
    pub fn from_workspace() -> Self {
        let mut env = Self::new(PathBuf::from("."));
        env.developer_dir = std::env::var_os("DEVELOPER_DIR");
        env.home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        env.temporary_dir = std::env::temp_dir();
        if cfg!(target_os = "macos") && env.developer_dir.is_none() {
            env.developer_dir = Some("/Applications/Xcode.app/Contents/Developer".into());
        }
        env
    }

    /// Limit the output collected for short tool queries.
    #[must_use]
    pub fn with_query_bytes(mut self, capacity: std::num::NonZeroUsize) -> Self {
        self.query_bytes = capacity.get();
        self
    }

    /// Limit a single message from Cargo's JSON output.
    #[must_use]
    pub fn with_cargo_message_bytes(mut self, capacity: std::num::NonZeroUsize) -> Self {
        self.cargo_message_bytes = capacity.get();
        self
    }

    /// Set the home and temporary directories for a clean Android build environment.
    #[must_use]
    pub fn with_host_directories(mut self, home: OsString, temporary: PathBuf) -> Self {
        self.home = Some(home);
        self.temporary_dir = temporary;
        self
    }

    pub(super) fn apple_command(
        &self,
        root: &Path,
        program: impl Into<OsString>,
    ) -> ProcessCommand {
        let mut cmd = ProcessCommand::new(program).current_dir(root);
        if let Some(path) = &self.developer_dir {
            cmd = cmd.env("DEVELOPER_DIR", path);
        }
        cmd
    }

    async fn root(&self) -> Result<PathBuf, TaskError> {
        let start = self.start.clone();
        blocking::unblock(move || {
            let start = start.canonicalize().map_err(TaskError::operation)?;
            start
                .ancestors()
                .find(|root| {
                    root.join("scripts/ui_native").is_dir() && root.join("Cargo.toml").is_file()
                })
                .map(Path::to_path_buf)
                .ok_or_else(|| TaskError::operation("Run this task from a repository checkout"))
        })
        .await
    }
}

/// # Errors
/// Reports an invalid task definition.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_ui_native").with_namespace("native");
    set.add(
        "build-android",
        "Build the Android native libraries",
        android::build,
    )?;
    set.add("build-ios", "Build the Apple native archive", apple::build)?;
    set.add(
        "bindings:generate",
        "Generate Swift and Kotlin bindings",
        bindings::run,
    )?;
    set.add(
        "bindings:sync-android",
        "Update or check the Android bindings",
        sync::android,
    )?;
    set.add(
        "bindings:sync-ios",
        "Update or check the Apple bindings",
        sync::ios,
    )?;
    set.add(
        "todo:prepare-ios",
        "Prepare the Apple Todo applications",
        todo::prepare,
    )?;
    set.add(
        "todo:test-ios",
        "Test the Apple Todo applications",
        test_todo_ios,
    )?;
    set.add(
        "todo:test-android",
        "Test the Android Todo applications",
        test_todo_android,
    )?;
    set.add(
        "release:prepare",
        "Build and stage a release candidate",
        release::prepare,
    )?;
    set.add(
        "release:verify",
        "Verify a staged release candidate",
        release::verify,
    )?;
    set.add(
        "release:publish",
        "Publish a verified release candidate",
        release::publish,
    )?;
    set.add(
        "release:test-ios-starter",
        "Test the Apple starter application",
        test_ios,
    )?;
    set.add(
        "release:test-android-starter",
        "Test the Android starter application",
        test_android,
    )?;
    Ok(set)
}

#[derive(Args)]
struct NoArgs {}

async fn test_todo_ios(ctx: TaskContext, _: NoArgs) -> TaskResult {
    run_tool(&ctx, "test-ios-todo.sh", []).await
}

#[derive(Args)]
struct AndroidStarter {
    #[arg(long)]
    maven: Option<PathBuf>,
}

async fn test_todo_android(ctx: TaskContext, args: AndroidStarter) -> TaskResult {
    run_tool(
        &ctx,
        "test-android-todo.sh",
        path_option("--maven", args.maven),
    )
    .await
}

async fn test_android(ctx: TaskContext, args: AndroidStarter) -> TaskResult {
    run_tool(
        &ctx,
        "release/test-android-starter.sh",
        path_option("--maven", args.maven),
    )
    .await
}

#[derive(Args)]
struct AppleStarter {
    #[arg(long)]
    local_package: Option<PathBuf>,
}

async fn test_ios(ctx: TaskContext, args: AppleStarter) -> TaskResult {
    run_tool(
        &ctx,
        "release/test-ios-starter.sh",
        path_option("--local-package", args.local_package),
    )
    .await
}

#[derive(Clone, Copy, ValueEnum)]
enum Templates {
    Both,
    Ios,
    Android,
}

fn path_option(flag: &str, path: Option<PathBuf>) -> Vec<OsString> {
    path.map_or_else(Vec::new, |path| vec![flag.into(), path.into_os_string()])
}

fn add_flag(flags: &mut Vec<OsString>, include: bool, flag: &str) {
    if include {
        flags.push(flag.into());
    }
}

async fn run_tool(
    ctx: &TaskContext,
    relative: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> TaskResult {
    let root = ctx.require::<NativeBuildEnv>()?.root().await?;
    run_tool_at(ctx, &root, relative, arguments).await
}

async fn run_tool_at(
    ctx: &TaskContext,
    root: &Path,
    relative: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> TaskResult {
    let script = root.join("scripts/ui_native").join(relative);
    let program = if script.extension().is_some_and(|ext| ext == "py") {
        "python3"
    } else {
        "bash"
    };
    ctx.exec(
        ProcessCommand::new(program)
            .current_dir(root)
            .arg(script)
            .args(arguments),
    )
    .await
}
