//! Developer tasks for native application packages.
use clap::ValueEnum;
use foundation_tasks::{ProcessCommand, TaskError, TaskResult, TaskSet};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub mod android;
mod apple;
mod archive;
mod bindings;
pub mod device;
pub mod output;
pub mod project;
mod publish;
mod release;
mod sign;
pub mod stage;
mod staging;
pub mod swift;
pub use swift::release as swift_release_manifest;
pub mod sync;

#[derive(Clone)]
pub struct NativeBuildEnv {
    start: PathBuf,
    query_bytes: usize,
    device_query_bytes: usize,
    cargo_message_bytes: usize,
    release_credentials: Option<publish::Credentials>,
    signing_job: Option<sign::JobEnv>,
    pub devices: device::Options,
    server_wait: std::time::Duration,
    signing_key: Option<String>,
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
            device_query_bytes: 1024 * 1024,
            cargo_message_bytes: 1024 * 1024,
            release_credentials: None,
            signing_job: None,
            devices: device::Options::default(),
            server_wait: std::time::Duration::from_secs(30),
            signing_key: None,
            developer_dir: None,
            home: None,
            temporary_dir: PathBuf::from("/tmp"),
        }
    }

    #[must_use]
    pub fn from_workspace() -> Self {
        let mut env = Self::new(PathBuf::from("."));
        env.developer_dir = std::env::var_os("DEVELOPER_DIR");
        env.signing_job = sign::JobEnv::from_host();
        env.devices = device::Options::from_host();
        env.release_credentials = publish::Credentials::from_host();
        env.signing_key = std::env::var("NATIVE_SIGNING_KEY")
            .ok()
            .filter(|key| !key.is_empty());
        env.home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        env.temporary_dir = std::env::temp_dir();
        if cfg!(target_os = "macos") && env.developer_dir.is_none() {
            env.developer_dir = Some("/Applications/Xcode.app/Contents/Developer".into());
        }
        env
    }

    /// Supply release credentials from the task host.
    #[must_use]
    pub fn with_publication_credentials(
        mut self,
        fingerprint: String,
        username: &str,
        password: &str,
    ) -> Self {
        self.release_credentials = Some(publish::Credentials::new(fingerprint, username, password));
        self
    }

    /// Limit the output collected for short tool queries.
    #[must_use]
    pub fn with_query_bytes(mut self, capacity: std::num::NonZeroUsize) -> Self {
        self.query_bytes = capacity.get();
        self
    }

    /// Limit a simulator inventory or accessibility tree. The default permits one mebibyte.
    #[must_use]
    pub fn with_device_query_bytes(mut self, capacity: std::num::NonZeroUsize) -> Self {
        self.device_query_bytes = capacity.get();
        self
    }

    #[must_use]
    pub fn query_bytes(&self) -> usize {
        self.query_bytes
    }

    #[must_use]
    pub fn server_wait(&self) -> std::time::Duration {
        self.server_wait
    }

    /// Set the deadline for a local server to report its port.
    #[must_use]
    pub fn with_server_wait(mut self, duration: std::time::Duration) -> Self {
        self.server_wait = duration;
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

    pub fn apple_command(&self, root: &Path, program: impl Into<OsString>) -> ProcessCommand {
        let mut cmd = ProcessCommand::new(program).current_dir(root);
        if let Some(path) = &self.developer_dir {
            cmd = cmd.env("DEVELOPER_DIR", path);
        }
        cmd
    }

    /// Find the enclosing source workspace.
    ///
    /// # Errors
    /// Returns an error when the start directory or workspace is unavailable.
    pub async fn root(&self) -> Result<PathBuf, TaskError> {
        let start = self.start.clone();
        blocking::unblock(move || {
            let start = start.canonicalize().map_err(TaskError::operation)?;
            start
                .ancestors()
                .find(|root| {
                    root.join("release/native-ui.toml").is_file()
                        && root.join("Cargo.toml").is_file()
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
    let mut set = release_set()?;
    add_apple_build(&mut set)?;
    set.add(
        "build-android",
        "Build the Android native libraries",
        android::build,
    )?;
    set.add(
        "bindings:sync-android",
        "Update or check the Android bindings",
        sync::android,
    )?;
    Ok(set)
}

/// Register the original repository command for an Apple framework build.
///
/// # Errors
/// Returns a conflict if the caller already registers that command.
pub fn add_apple_build(set: &mut TaskSet) -> TaskResult {
    set.add("build-ios", "Build the Apple native archive", apple::build)
}

/// Return shared release operations for a runner with its own application build tasks.
///
/// # Errors
/// Returns task registration errors.
pub fn release_set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_ui_native").with_namespace("native");
    set.add(
        "release:check-version",
        "Check the release tag against its configuration",
        stage::check_tag,
    )?;
    set.add(
        "release:sign-job",
        "Import the workflow key and sign the verified candidate",
        sign::job,
    )?;
    set.add(
        "release:archive-ios",
        "Check framework slices and create the Apple archive",
        archive::run,
    )?;
    set.add(
        "release:stage",
        "Check and stage built Apple and Android packages",
        stage::run,
    )?;
    set.add(
        "release:maven-local",
        "Test Android packages and install them in a local repository",
        release::maven,
    )?;
    set.add(
        "release:build-ios",
        "Build the Apple framework archive",
        apple::build,
    )?;
    set.add(
        "bindings:generate",
        "Generate Swift and Kotlin bindings",
        bindings::run,
    )?;
    set.add(
        "bindings:sync-ios",
        "Update or check the Apple bindings",
        sync::ios,
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
        "release:sign",
        "Sign the verified Maven artifacts",
        sign::run,
    )?;
    set.add(
        "release:publish",
        "Publish a verified release candidate",
        release::publish,
    )?;
    Ok(set)
}

#[derive(Clone, Copy, ValueEnum)]
enum Templates {
    Both,
    Ios,
    Android,
}
