//! The `agent-slave` command (feature 06).
//!
//! The arguments are the command line first and the kernel command line second
//! (feature 01, section 6.3). A microVM gives the slave no arguments and sets
//! `ewe.listen=` in `/proc/cmdline`.

use clap::Parser;

use crate::cli::parse_listen;
use crate::native::slave::{run as run_slave, ListenAddr, SlaveArgs};

/// The `agent-slave` program.
#[derive(Debug, Parser)]
#[command(name = "agent-slave", about = "Serve SlaveService in a sandbox")]
pub struct SlaveCli {
    /// The listen address: `vsock:<port>`, `unix:<path>` or `tcp:<host>:<port>`.
    #[arg(long)]
    pub listen: Option<String>,

    /// The directory under which file calls that name a plain root work.
    #[arg(long, default_value = "/")]
    pub plain_root: std::path::PathBuf,
}

/// Parses the arguments and runs the slave.
///
/// It returns the exit code when the slave is not process identifier 1, and
/// never returns when it is.
pub fn run(ctx: foundation_context::Ctx, cli: SlaveCli) -> i32 {
    let value = match cli.listen {
        Some(value) => value,
        None => match listen_from_cmdline() {
            Some(value) => value,
            None => {
                eprintln!("agent-slave: no listen address");
                return 64;
            }
        },
    };
    let address = match parse_listen(&value) {
        Ok(crate::shared::listen::ListenAddress::Unix { path }) => ListenAddr::Unix { path },
        Ok(crate::shared::listen::ListenAddress::Tcp { addr }) => ListenAddr::Tcp { addr },
        Ok(crate::shared::listen::ListenAddress::Vsock { port }) => ListenAddr::Vsock { port },
        Err(error) => {
            eprintln!("agent-slave: {error}");
            return 64;
        }
    };
    run_slave(
        ctx,
        SlaveArgs {
            listen: address,
            control: false,
            plain_root: cli.plain_root,
        },
    )
}

/// The `ewe.listen=` value of `/proc/cmdline`, when the file has one.
#[cfg(target_os = "linux")]
fn listen_from_cmdline() -> Option<String> {
    let text = std::fs::read_to_string("/proc/cmdline").ok()?;
    text.split_ascii_whitespace()
        .find_map(|word| word.strip_prefix("ewe.listen="))
        .map(ToString::to_string)
}

/// Another target has no kernel command line.
#[cfg(not(target_os = "linux"))]
fn listen_from_cmdline() -> Option<String> {
    None
}
