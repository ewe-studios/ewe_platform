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

    /// Also serve `SandboxService` on this listener (feature 11).
    ///
    /// The inner control plane runs Firecracker micro virtual machines (VMs)
    /// in this process. A build without the `control` and `firecracker`
    /// features ignores this argument and reports `can_control: false`.
    #[arg(long)]
    pub control: bool,

    /// The directory under which file calls that name a plain root work.
    #[arg(long, default_value = "/")]
    pub plain_root: std::path::PathBuf,

    /// The mode of the Unix domain socket file, in octal.
    ///
    /// Default `600`. A container sets `666`: the slave runs as root there and
    /// the control plane on the host is a normal user. The run directory's own
    /// mode keeps other users out.
    #[arg(long, default_value = "600", value_parser = parse_octal_mode)]
    pub socket_mode: u32,
}

/// Parses an octal file mode, such as `666` or `0600`.
fn parse_octal_mode(value: &str) -> Result<u32, String> {
    let text = value.trim_start_matches("0o").trim_start_matches('0');
    let text = if text.is_empty() { "0" } else { text };
    u32::from_str_radix(text, 8).map_err(|error| format!("{value} is not an octal mode: {error}"))
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
            control: cli.control || control_from_cmdline(),
            plain_root: cli.plain_root,
            socket_mode: cli.socket_mode,
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

/// Whether the kernel command line asks for the control role (`ewe.control=1`).
///
/// A micro virtual machine (VM) gives the slave no arguments, so the key is
/// the only way to ask a guest slave for the control role.
#[cfg(target_os = "linux")]
fn control_from_cmdline() -> bool {
    std::fs::read_to_string("/proc/cmdline")
        .map(|text| {
            text.split_ascii_whitespace()
                .any(|word| word == "ewe.control=1")
        })
        .unwrap_or(false)
}

/// Another target has no kernel command line.
#[cfg(not(target_os = "linux"))]
fn control_from_cmdline() -> bool {
    false
}
