// Implementation of Cargo Project manager that handles loading and running a giving
// cargo application after building said application. This lets us control rebuilding and running
// of application in a different thread based on specifics.

use crate::types::{self, BoxedError};
use crate::{
    operators::{self, Operator},
    tcp_proxy, ProxyRemote,
};
use crossbeam::channel;
use derive_more::From;
use std::thread;
use std::{
    process::{self, Stdio},
    result, sync, time,
};
use tokio::task::JoinHandle;

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
pub struct CargoShellApp {
    pub project_dir: String,
    pub binary_name: String,
}

// constructors
impl CargoShellApp {
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

impl operators::Operator for sync::Arc<CargoShellApp> {
    fn run(&self, _signal: channel::Receiver<()>) -> JoinHandle<result::Result<(), BoxedError>> {
        let handle = self.clone();
        tokio::spawn(async move { handle.build().await })
    }
}

// builders
impl CargoShellApp {
    pub async fn build(&self) -> CargoShellResult<()> {
        self.run_checks()?;
        self.run_build()?;
        Ok(())
    }

    fn run_build(&self) -> CargoShellResult<()> {
        ewe_logs::info!(
            "Building project binary with cargo (project={}, binary={})",
            self.project_dir,
            self.binary_name
        );
        let mut command = process::Command::new("cargo");
        match command
            .current_dir(self.project_dir.clone())
            .args([&format!("build --bin {}", self.binary_name)])
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

    fn run_checks(&self) -> CargoShellResult<()> {
        let mut command = process::Command::new("cargo");
        match command
            .current_dir(self.project_dir.clone())
            .args(["check"])
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

pub struct CargoTcpApp {
    binary_name: String,
    project_directory: String,
    wait_for_binary_secs: time::Duration,
    binary_arguments: Option<Vec<&'static str>>,
    source_config: tcp_proxy::ProxyRemoteConfig,
    destination_config: tcp_proxy::ProxyRemoteConfig,
}

// -- Constructor

impl CargoTcpApp {
    pub fn shared(
        binary_name: String,
        project_directory: String,
        wait_for_binary_secs: time::Duration,
        binary_args: Option<Vec<&'static str>>,
        source: tcp_proxy::ProxyRemoteConfig,
        destination: tcp_proxy::ProxyRemoteConfig,
    ) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            project_directory,
            wait_for_binary_secs,
            binary_name: binary_name.into(),
            binary_arguments: binary_args,
            source_config: source,
            destination_config: destination,
        })
    }
}

// -- Operator implementation

impl Operator for sync::Arc<CargoTcpApp> {
    fn run(&self, signal: channel::Receiver<()>) -> JoinHandle<result::Result<(), BoxedError>> {
        let wait_for = self.wait_for_binary_secs.clone();
        let source = self.source_config.clone();
        let destination = self.destination_config.clone();

        let handler = self.clone();

        tokio::spawn(async move {
            let mut binary_process_handle =
                handler.run_binary().expect("should have started binary");

            tokio::time::sleep(wait_for).await;
            let proxy_handler = handler.run_proxy(signal);

            ewe_logs::info!(
                "Booting up proxy server for source={:?} through destination={:?}",
                source,
                destination
            );

            match proxy_handler.await? {
                Ok(_) => {
                    binary_process_handle
                        .kill()
                        .expect("should have killed binary");
                    Ok(())
                }
                Err(err) => {
                    ewe_logs::error!("Failed to properly end tcp proxy: {:?}", err);
                    binary_process_handle
                        .kill()
                        .expect("should have killed binary");
                    Ok(())
                }
            }
        })
    }
}

// -- Binary starter
impl CargoTcpApp {
    fn run_binary(&self) -> types::Result<process::Child> {
        ewe_logs::info!(
            "Running binary from package directory={}",
            self.project_directory
        );

        let bin_code = format!(
            "{}/target/debug/{}",
            self.project_directory, self.binary_name
        );
        let mut command = process::Command::new(bin_code);

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
                    self.binary_name,
                    self.binary_arguments,
                );
                Ok(child)
            }
            Err(err) => {
                ewe_logs::error!(
                    "Running command `cargo check` returned error (binary={}, args={:?})\n\t{:?}",
                    self.binary_name,
                    self.binary_arguments,
                    err,
                );
                Err(Box::new(CargoShellError::ShellError(Box::new(err))))
            }
        }
    }
}

// -- TCP Server

impl CargoTcpApp {
    fn run_proxy(&self, sig: channel::Receiver<()>) -> JoinHandle<types::Result<()>> {
        let proxy_server =
            ProxyRemote::shared(self.source_config.clone(), self.destination_config.clone());
        proxy_server.run(sig)
    }
}
