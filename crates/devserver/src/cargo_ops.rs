// Implementation of Cargo Project manager that handles loading and running a giving
// cargo application after building said application. This lets us control rebuilding and running
// of application in a different thread based on specifics.

use crate::types::{self, BoxedError};
use crate::{
    operators::{self, Operator},
    types::JoinHandle,
};
use crossbeam::channel;
use derive_more::From;
use std::{
    process::{self, Stdio},
    sync,
};

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
#[derive(Clone)]
pub struct CargoShellBuilder {
    pub project_dir: String,
    pub binary_name: String,
}

// constructors
impl CargoShellBuilder {
    pub fn shared<S>(project_dir: S, binary_name: S) -> sync::Arc<Self>
    where
        S: Into<String>,
    {
        sync::Arc::new(Self {
            project_dir: project_dir.into(),
            binary_name: binary_name.into(),
        })
    }
}

// -- Operator implementations

impl operators::Operator for sync::Arc<CargoShellBuilder> {
    fn run(&self, _signal: channel::Receiver<()>) -> JoinHandle<()> {
        let handle = self.clone();
        tokio::spawn(async move { handle.build().await })
    }
}

// builders
impl CargoShellBuilder {
    pub async fn build(&self) -> CargoShellResult<()> {
        self.run_checks().await?;
        self.run_build().await?;
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
}

// -- Constructor

impl CargoBinaryApp {
    pub fn shared(
        binary_path: String,
        project_directory: String,
        binary_args: Option<Vec<&'static str>>,
    ) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            project_directory,
            binary_path: binary_path.into(),
            binary_arguments: binary_args,
        })
    }
}

// -- Operator implementation

impl Operator for sync::Arc<CargoBinaryApp> {
    fn run(&self, signal: channel::Receiver<()>) -> JoinHandle<()> {
        let mut binary_runner = self.run_binary().expect("should have started binary");

        tokio::task::spawn_blocking(move || {
            _ = signal.recv().expect("should receive kill signal");
            binary_runner.kill().expect("should have being killed");
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
