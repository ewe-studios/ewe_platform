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
    let (ssh, user) = client(ctx, connection, "ssh", "-p")?;
    Ok(ssh.arg("-l").arg(user).args(["127.0.0.1", probe]))
}

pub(super) fn copy(
    ctx: &TaskContext,
    connection: &Connection,
) -> Result<ProcessCommand, TaskError> {
    let (scp, user) = client(ctx, connection, "scp", "-P")?;
    Ok(scp.arg("-o").arg(format!("User={user}")))
}

fn client(
    ctx: &TaskContext,
    connection: &Connection,
    tool: &str,
    port_flag: &str,
) -> Result<(ProcessCommand, String), TaskError> {
    let host = ctx.require::<CiEnvironment>()?;
    let default_user = match connection.os {
        Guest::Macos => "ci",
        Guest::Windows => "Docker",
    };
    let user = host
        .text("PI_VM_USER")
        .unwrap_or_else(|| default_user.into());
    let client = ProcessCommand::new(tool)
        .arg("-i")
        .arg(host.root.join(&connection.key))
        .arg(port_flag)
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
        .args(["-o", "ServerAliveInterval=30"]);
    Ok((client, user))
}
