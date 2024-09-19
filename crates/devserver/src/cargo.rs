// Implementation of Cargo Project manager that handles loading and running a giving
// cargo application after building said application. This lets us control rebuilding and running
// of application in a different thread based on specifics.

use crate::types::{self, BoxedError};
use crate::{
    operators::{self, Operator},
    types::JoinHandle,
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

/// CargoShellApp implements a cargo project builder and compiler that
/// runs shell commands to easily check, build and run a rust project
/// via cargo shell commands.
///
/// It specifically runs the relevant shell commands, validate the binary
/// was produced and run giving binary with a target command you provide.
pub struct CargoShellBuilder {
    pub project_dir: String,
    pub binary_name: String,
    pub build_notifier: broadcast::Sender<()>,
    // because we want to re-subscribe as many times as we need
    pub file_notifications: broadcast::Sender<()>,
}

// constructors
impl CargoShellBuilder {
    pub fn shared<S>(
        project_dir: S,
        binary_name: S,
        build_notifier: broadcast::Sender<()>,
        file_notifications: broadcast::Sender<()>,
    ) -> sync::Arc<Self>
    where
        S: Into<String>,
    {
        sync::Arc::new(Self {
            project_dir: project_dir.into(),
            binary_name: binary_name.into(),
            file_notifications,
            build_notifier,
        })
    }
}

impl Clone for CargoShellBuilder {
    fn clone(&self) -> Self {
        Self {
            project_dir: self.project_dir.clone(),
            binary_name: self.binary_name.clone(),
            build_notifier: self.build_notifier.clone(),
            file_notifications: self.file_notifications.clone(),
        }
    }
}

// -- Operator implementations

impl operators::Operator for sync::Arc<CargoShellBuilder> {
    fn run(&self, mut signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let handle = self.clone();
        let mut recver = self.file_notifications.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = recver.recv() => {
                        ewe_logs::info!("Received rebuilding signal for binary!");
                        match handle.build().await {
                            Ok(_) => {
                                ewe_logs::info!("Finished rebuilding binary!");
                                continue;
                            },
                            Err(err) => {
                                return Err(err);
                            }
                        }
                    },
                    _ = signal.recv() => {
                        ewe_logs::info!("Cancel signal received, shutting down!");
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
        self.run_checks().await?;
        self.run_build().await?;
        self.build_notifier.send(())?;
        Ok(())
    }

    async fn run_build(&self) -> CargoShellResult<()> {
        ewe_logs::info!(
            "Building project binary with cargo (project={}, binary={})",
            self.project_dir,
            self.binary_name,
        );
        let mut command = tokio::process::Command::new("cargo");
        match command
            .current_dir(self.project_dir.clone())
            .args(["build", "--bin", self.binary_name.as_str()])
            .output()
            .await
        {
            Ok(result) => {
                ewe_logs::info!(
                    "Running command `cargo build` (project={}, binary={})",
                    self.project_dir,
                    self.binary_name,
                );
                if !result.status.success() {
                    ewe_logs::error!(
                        "Running command `cargo build` returned error (project={}, binary={})\n\t{:?}",
                        self.project_dir,
                        self.binary_name,
                        String::from_utf8(result.stderr).expect("should correct decode error"),
                    );
                    return Err(Box::new(CargoShellError::CargoCheckFailed));
                }
                Ok(())
            }
            Err(err) => {
                ewe_logs::error!(
                    "Failed command execution: `cargo build` (project={}, binary={}): {:?}",
                    self.project_dir,
                    self.binary_name,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }

    async fn run_checks(&self) -> CargoShellResult<()> {
        let mut command = tokio::process::Command::new("cargo");
        match command
            .current_dir(self.project_dir.clone())
            .args(["check"])
            .output()
            .await
        {
            Ok(result) => {
                ewe_logs::info!(
                    "Running command `cargo check` (project={}, binary={})",
                    self.project_dir,
                    self.binary_name,
                );
                if !result.status.success() {
                    ewe_logs::error!(
                        "Running command `cargo check` returned error (project={}, binary={})\n\t{:?}",
                        self.project_dir,
                        self.binary_name,
                        String::from_utf8(result.stderr).expect("should correct decode error"),
                    );
                    return Err(Box::new(CargoShellError::CargoCheckFailed));
                }
                Ok(())
            }
            Err(err) => {
                ewe_logs::error!(
                    "Failed command execution: `cargo check` (project={}, binary={}): {:?}",
                    self.project_dir,
                    self.binary_name,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }
}

pub struct CargoBinaryApp {
    binary_path: String,
    project_directory: String,
    binary_arguments: Option<Vec<&'static str>>,
    // we actually use the send to send notification that the app
    // is now running again.
    running_notifications: broadcast::Sender<()>,
    // using the sender so we can create as many subscriber/receivers as needed.
    build_notifications: broadcast::Sender<()>,
}

impl Clone for CargoBinaryApp {
    fn clone(&self) -> Self {
        Self {
            binary_path: self.binary_path.clone(),
            project_directory: self.project_directory.clone(),
            binary_arguments: self.binary_arguments.clone(),
            build_notifications: self.build_notifications.clone(),
            running_notifications: self.build_notifications.clone(),
        }
    }
}

// -- Constructor
impl CargoBinaryApp {
    pub fn shared(
        binary_path: String,
        project_directory: String,
        binary_args: Option<Vec<&'static str>>,
        build_notifications: broadcast::Sender<()>,
        running_notifications: broadcast::Sender<()>,
    ) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            project_directory,
            build_notifications,
            running_notifications,
            binary_path: binary_path.into(),
            binary_arguments: binary_args,
        })
    }
}

// -- Operator implementation

impl Operator for sync::Arc<CargoBinaryApp> {
    fn run(&self, mut signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let handle = self.clone();

        let run_sender = self.running_notifications.clone();
        let mut build_notifier = self.build_notifications.subscribe();

        tokio::spawn(async move {
            let mut binary_handle: Option<process::Child> = None;

            loop {
                tokio::select! {
                    _ = build_notifier.recv() => {
                        if let Some(mut binary) = binary_handle {
                            ewe_logs::info!("Killing current version of binary");
                            binary.kill().expect("kill binary and re-starts");
                        }

                        ewe_logs::info!("Restarting latest version of binary");
                        binary_handle = Some(handle.run_binary().expect("re-run binary"));

                        ewe_logs::info!("Restart done!");
                        if let Err(_) = run_sender.send(()) {
                            ewe_logs::warn!("No one is listening for re-running messages");
                        }
                        continue;
                    },
                    _ = signal.recv() => {
                        ewe_logs::info!("Cancel signal received, shutting down!");
                        if let Some(mut binary) = binary_handle {
                            match binary.kill() {
                                Ok(_) => break,
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
impl CargoBinaryApp {
    fn run_binary(&self) -> types::Result<process::Child> {
        ewe_logs::info!(
            "Running binary from package directory={}",
            self.project_directory
        );

        let mut command = process::Command::new(self.binary_path.clone());

        let mut args = vec![];
        if let Some(mut bin_args) = self.binary_arguments.clone() {
            args.append(&mut bin_args);
        }

        match command
            .current_dir(self.project_directory.clone())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => {
                ewe_logs::info!(
                    "Running command `cargo run` (binary={}, args={:?})",
                    self.binary_path,
                    self.binary_arguments,
                );
                Ok(child)
            }
            Err(err) => {
                ewe_logs::error!(
                    "Running command `cargo check` returned error (binary={}, args={:?})\n\t{:?}",
                    self.binary_path,
                    self.binary_arguments,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }
}
