//! Build a command for a fixed check inside the local guest.
use super::Guest;
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError};
use std::path::PathBuf;

#[derive(Args)]
pub(super) struct Connection {
    #[arg(long, value_enum)]
    pub(super) os: Guest,
    #[arg(long)]
    key: PathBuf,
    #[arg(long, default_value_t = 2222, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    /// Maximum time to establish a loopback connection.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    connect_seconds: u32,
}

pub(super) fn command(
    ctx: &TaskContext,
    connection: &Connection,
    probe: &str,
) -> Result<ProcessCommand, TaskError> {
    let host = ctx.require::<CiEnvironment>()?;
    let default_user = match connection.os {
        Guest::Macos => "ci",
        Guest::Windows => "Docker",
    };
    let user = host
        .text("PI_VM_USER")
        .unwrap_or_else(|| default_user.into());
    Ok(ProcessCommand::new("ssh")
        .arg("-i")
        .arg(host.root.join(&connection.key))
        .arg("-p")
        .arg(connection.port.to_string())
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "LogLevel=ERROR",
            "-o",
        ])
        .arg(format!("ConnectTimeout={}", connection.connect_seconds))
        .args(["-o", "ServerAliveInterval=30", "-l"])
        .arg(user)
        .args(["127.0.0.1", probe]))
}
