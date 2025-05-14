// Implementation of Cargo Project manager that handles loading and running a giving
// cargo application after building said application. This lets us control rebuilding and running
// of application in a different thread based on specifics.

use crate::types::{self, BoxedError};
use crate::FileChange;
use crate::{
    operators::{self, Operator},
    types::JoinHandle,
    ProjectDefinition, SenderExt,
};
use derive_more::From;
use std::{
    process::{self, Stdio},
    sync,
};
use tokio::sync::broadcast;

#[derive(Debug, From)]
pub enum CargoShellError {
    CargoCheckFailed,
    CargoBinaryRunFailed,
    ShellError(BoxedError),
}

impl std::error::Error for CargoShellError {}

impl core::fmt::Display for CargoShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type CargoShellResult<T> = types::Result<T>;

/// `CargoShellApp` implements a cargo project builder and compiler that
/// runs shell commands to easily check, build and run a rust project
/// via cargo shell commands.
///
/// It specifically runs the relevant shell commands, validate the binary
/// was produced and run giving binary with a target command you provide.
pub struct CargoShellBuilder {
    pub skip_check: bool,
    pub stop_on_failure: bool,
    pub project: ProjectDefinition,
    pub trigger_notifier: broadcast::Sender<()>,
    pub build_notifier: broadcast::Sender<()>,
    pub file_notifications: broadcast::Sender<FileChange>,
}

// constructors
impl CargoShellBuilder {
    pub fn shared(
        skip_check: bool,
        stop_on_failure: bool,
        project: ProjectDefinition,
        build_notifier: broadcast::Sender<()>,
        trigger_notifier: broadcast::Sender<()>,
        file_notifications: broadcast::Sender<FileChange>,
    ) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            project,
            skip_check,
            stop_on_failure,
            trigger_notifier,
            build_notifier,
            file_notifications,
        })
    }
}

impl Clone for CargoShellBuilder {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            stop_on_failure: self.stop_on_failure,
            skip_check: self.skip_check,
            build_notifier: self.build_notifier.clone(),
            trigger_notifier: self.trigger_notifier.clone(),
            file_notifications: self.file_notifications.clone(),
        }
    }
}

// -- Operator implementations

impl operators::Operator for sync::Arc<CargoShellBuilder> {
    fn run(&self, mut signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let stop_on_failure = self.stop_on_failure;
        let handle = self.clone();
        let mut trigger = self.trigger_notifier.subscribe();
        let mut recver = self.file_notifications.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = trigger.recv() => {
                            match handle.build().await {
                                Ok(()) => {
                                    ewe_trace::info!("Finished rebuilding binary!");
                                },
                                Err(err) => {
                                    ewe_trace::error!("Failed rebuilding due to: {:?}!", err);
                                    if stop_on_failure {
                                        return Err(err);
                                    }
                                }
                            }
                    },
                    changed_file = recver.recv() => {
                        ewe_trace::info!("Received rebuilding signal for binary due to {:?}!", &changed_file);
                        if let Ok(FileChange::Rust(_)) = changed_file {
                            match handle.build().await {
                                Ok(()) => {
                                    ewe_trace::info!("Finished rebuilding binary!");
                                },
                                Err(err) => {
                                    ewe_trace::error!("Failed rebuilding due to: {:?}!", err);
                                    if stop_on_failure {
                                        return Err(err);
                                    }
                                }
                            }
                        };
                        continue;
                    },
                    _ = signal.recv() => {
                        ewe_trace::info!("Cancel signal received, shutting down!");
                        break;
                    }
                }
            }

            Ok(())
        })
    }
}

// builders
impl CargoShellBuilder {
    pub async fn build(&self) -> CargoShellResult<()> {
        // only run checks if allowed
        if !self.skip_check {
            self.run_checks().await?;
        } else {
            ewe_trace::info!("Skipping cargo checks")
        }
        self.run_build().await?;
        self.build_notifier.send(())?;
        Ok(())
    }

    async fn run_build(&self) -> CargoShellResult<()> {
        ewe_trace::info!(
            "Building project binary with cargo (project={}, binary={:?})",
            self.project.crate_name,
            self.project.run_arguments,
        );

        let mut binary_and_arguments = self.project.build_arguments.clone();
        let binary_arguments = binary_and_arguments.split_off(1);

        let mut command = tokio::process::Command::new(binary_and_arguments.pop().unwrap());
        match command
            .current_dir(self.project.workspace_root.clone())
            .args(binary_arguments)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .await
        {
            Ok(result) => {
                ewe_trace::info!(
                    "Running command `cargo build` (project={}, binary={:?})",
                    self.project.crate_name,
                    self.project.run_arguments,
                );
                if !result.status.success() {
                    ewe_trace::error!(
                        "Running command `cargo build` returned error (project={}, binary={:?})\n\t{:?}",
                        self.project.crate_name,
                        self.project.run_arguments,
                        String::from_utf8(result.stderr).expect("should correct decode error"),
                    );
                    return Err(Box::new(CargoShellError::CargoCheckFailed));
                }
                Ok(())
            }
            Err(err) => {
                ewe_trace::error!(
                    "Failed command execution: `cargo build` (project={}, binary={:?}): {:?}",
                    self.project.crate_name,
                    self.project.run_arguments,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }

    async fn run_checks(&self) -> CargoShellResult<()> {
        let mut command = tokio::process::Command::new("cargo");
        match command
            .current_dir(self.project.workspace_root.clone())
            .args(["check"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .await
        {
            Ok(result) => {
                ewe_trace::info!(
                    "Running command `cargo check` (project={}, binary={:?})",
                    self.project.crate_name,
                    self.project.run_arguments,
                );
                if !result.status.success() {
                    ewe_trace::error!(
                        "Running command `cargo check` returned error (project={}, binary={:?})\n\t{:?}",
                        self.project.crate_name,
                        self.project.run_arguments,
                        String::from_utf8(result.stderr).expect("should correct decode error"),
                    );
                    return Err(Box::new(CargoShellError::CargoCheckFailed));
                }
                Ok(())
            }
            Err(err) => {
                ewe_trace::error!(
                    "Failed command execution: `cargo check` (project={}, binary={:?}): {:?}",
                    self.project.crate_name,
                    self.project.run_arguments,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }
}

pub struct BinaryApp {
    project: ProjectDefinition,
    // we actually use the send to send notification that the app
    // is now running again.
    running_notifications: broadcast::Sender<()>,
    // using the sender so we can create as many subscriber/receivers as needed.
    build_notifications: broadcast::Sender<()>,
}

impl Clone for BinaryApp {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            build_notifications: self.build_notifications.clone(),
            running_notifications: self.build_notifications.clone(),
        }
    }
}

// -- Constructor
impl BinaryApp {
    pub fn shared(
        project: ProjectDefinition,
        build_notifications: broadcast::Sender<()>,
        running_notifications: broadcast::Sender<()>,
    ) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            project,
            running_notifications,
            build_notifications,
        })
    }
}

// -- Operator implementation

impl Operator for sync::Arc<BinaryApp> {
    fn run(&self, mut signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let handle = self.clone();

        let run_sender = self.running_notifications.clone();
        let mut build_notifier = self.build_notifications.subscribe();

        let wait_before_reload = self.project.wait_before_reload;

        tokio::spawn(async move {
            let mut binary_handle: Option<process::Child> = None;

            loop {
                tokio::select! {
                    _ = build_notifier.recv() => {
                        if let Some(mut binary) = binary_handle {
                            ewe_trace::info!("Killing current version of binary");
                            binary.kill().expect("kill binary and re-starts");
                        }

                        ewe_trace::info!("Restarting latest version of binary");
                        binary_handle = Some(handle.run_binary().expect("re-run binary"));

                        ewe_trace::info!("Restart done!");
                        if run_sender.send_in((), wait_before_reload).await.is_err() {
                            ewe_trace::warn!("No one is listening for re-running messages");
                        }
                        continue;
                    },
                    _ = signal.recv() => {
                        ewe_trace::info!("Cancel signal received, shutting down!");
                        if let Some(mut binary) = binary_handle {
                            match binary.kill() {
                                Ok(()) => break,
                                Err(err) => return Err(Box::new(err).into()),
                            }
                        }
                        break;
                    }
                }
            }

            Ok(())
        })
    }
}

// -- Binary starter
impl BinaryApp {
    fn run_binary(&self) -> types::Result<process::Child> {
        ewe_trace::info!("Running binary from project={}", self.project);

        let mut binary_and_arguments = self.project.run_arguments.clone();
        let run_arguments = binary_and_arguments.split_off(1);

        let mut command = process::Command::new(binary_and_arguments.pop().unwrap());
        match command
            .current_dir(self.project.workspace_root.clone())
            .args(run_arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => {
                ewe_trace::info!(
                    "Running command `cargo run` (binary={:?}, args={:?})",
                    self.project.crate_name,
                    self.project.run_arguments,
                );
                Ok(child)
            }
            Err(err) => {
                ewe_trace::error!(
                    "Running command `cargo check` returned error (binary={:?}, args={:?})\n\t{:?}",
                    self.project.crate_name,
                    self.project.run_arguments,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }
}
