// Implementation of Cargo Project manager that handles loading and running a giving
// cargo application after building said application. This lets us control rebuilding and running
// of application in a different thread based on specifics.

use crate::types::BoxedError;
use std::{process, result};

#[derive(Debug)]
pub enum CargoShellError {
    CargoCheckFailed,
    ShellError(BoxedError),
}

type Result<T> = result::Result<T, CargoShellError>;

/// CargoShellApp implements a cargo project builder and compiler that
/// runs shell commands to easily check, build and run a rust project
/// via cargo shell commands.
///
/// It specifically runs the relevant shell commands, validate the binary
/// was produced and run giving binary with a target command you provide.
pub struct CargoShellApp {
    pub project_dir: String,
    pub binary_name: String,
}

// constructors
impl CargoShellApp {
    pub fn new<S>(project_dir: S, binary_name: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            project_dir: project_dir.into(),
            binary_name: binary_name.into(),
        }
    }
}

// builders
impl CargoShellApp {
    pub fn build() -> Result<()> {
        Ok(())
    }

    pub fn do_checks(&self) {
        self.run_checks()
            .expect("should succeed with `cargo check`");
        self.run_build().expect("should succeed with `cargo check`");
    }

    fn run_build(&self) -> Result<()> {
        let mut command = process::Command::new("cargo");
        match command
            .args(["build", &format!("--bin {}", self.binary_name)])
            .output()
        {
            Ok(result) => {
                ewe_logs::info!(
                    "Running command `cargo build` (project={}, binary={})",
                    self.project_dir,
                    self.binary_name
                );
                if !result.status.success() {
                    ewe_logs::error!(
                        "Running command `cargo build` returned error (project={}, binary={})\n\t{:?}",
                        self.project_dir,
                        self.binary_name,
                        String::from_utf8(result.stderr).expect("should correct decode error"),
                    );
                    return Err(CargoShellError::CargoCheckFailed);
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
                Err(CargoShellError::ShellError(Box::new(err)))
            }
        }
    }

    fn run_checks(&self) -> Result<()> {
        let mut command = process::Command::new("cargo");
        match command
            .args(["check", &format!("--bin {}", self.binary_name)])
            .output()
        {
            Ok(result) => {
                ewe_logs::info!(
                    "Running command `cargo check` (project={}, binary={})",
                    self.project_dir,
                    self.binary_name
                );
                if !result.status.success() {
                    ewe_logs::error!(
                        "Running command `cargo check` returned error (project={}, binary={})\n\t{:?}",
                        self.project_dir,
                        self.binary_name,
                        String::from_utf8(result.stderr).expect("should correct decode error"),
                    );
                    return Err(CargoShellError::CargoCheckFailed);
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
                Err(CargoShellError::ShellError(Box::new(err)))
            }
        }
    }
}
