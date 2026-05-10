//! Standalone CLI for foundation_testbed.
//!
//! Provides clap subcommands for VM lifecycle via the Provider trait,
//! plus provider-specific extras (QEMU snapshots, etc.).
//!
//! Usage:
//! - Standalone binary: see `src/bin/testbed.rs`
//! - As part of ewe_platform: `foundation_testbed::cli::command()`

use clap::{ArgMatches, Command};

mod bootstrap;
mod debloat;
mod provider;
mod qemu;
mod ssh_cmds;
mod winrm_test;

/// Build the complete `testbed` command tree.
///
/// WHY: Returns a ready-made Command so callers can integrate it directly
/// into their own CLI hierarchy. Both the standalone binary and ewe_platform
/// use this same definition.
pub fn command() -> Command {
    Command::new("testbed")
        .about("VM testbed — build, test, and run binaries on virtual machines")
        .subcommand_required(true)
        .subcommand(
            Command::new("start")
                .about("Boot a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("headful").long("headful").action(clap::ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("bootstrap")
                .about("Install development tools on a running VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("force-vsbuild").long("force-vsbuild-install").action(clap::ArgAction::SetTrue)
                    .help("Skip VS Build Tools precheck and force reinstallation (Windows only)")),
        )
        .subcommand(
            Command::new("debloat")
                .about("Remove pre-installed bloatware from a Windows VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name")),
        )
        .subcommand(
            Command::new("stop")
                .about("Gracefully shut down a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name")),
        )
        .subcommand(
            Command::new("import")
                .about("Download a VM image for the given profile")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name")),
        )
        .subcommand(
            Command::new("doctor")
                .about("Health checks for host and/or VM")
                .arg(clap::Arg::new("profile").help("VM profile name")),
        )
        .subcommand(
            Command::new("ls")
                .about("List all known VMs"),
        )
        .subcommand(
            Command::new("winrm-test")
                .about("Test WinRM script write and elevated execution (debug tool)")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("script").long("script").help("Path to a local .ps1 file to write and execute"))
                .arg(clap::Arg::new("timeout").long("timeout").default_value("60").help("Timeout in seconds")),
        )
        .subcommand(
            Command::new("exec")
                .about("Run a command inside a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("cmd").required(true).help("Command to run"))
                .arg(clap::Arg::new("method").long("method").value_parser(["ssh", "winrm"]).default_value("ssh").help("Connection method (ssh or winrm)")),
        )
        .subcommand(
            Command::new("shell")
                .about("Open an interactive SSH shell to a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name")),
        )
        .subcommand(
            Command::new("run")
                .about("Launch a compiled binary inside a VM")
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("bin").long("bin").help("Binary name to run")),
        )
        .subcommand(
            Command::new("screenshot")
                .about("Capture a screenshot of a VM display")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("out").long("out").required(true).help("Output path")),
        )
        .subcommand(
            Command::new("logs")
                .about("View build or run logs from a VM")
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("follow").long("follow").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("errors").long("errors").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("tail").long("tail").action(clap::ArgAction::Set).value_parser(clap::value_parser!(u32)))
                .arg(clap::Arg::new("kind").long("kind").default_value("build")),
        )
        .subcommand(
            Command::new("push")
                .about("Push a file or directory to a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("from").long("from").required(true).help("Local path"))
                .arg(clap::Arg::new("to").long("to").required(true).help("Guest path")),
        )
        .subcommand(
            Command::new("pull")
                .about("Pull a file from a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("from").long("from").required(true).help("Guest path"))
                .arg(clap::Arg::new("to").long("to").required(true).help("Local path")),
        )
        .subcommand(
            Command::new("init")
                .about("Scaffold project with testbed.toml, scripts, and .gitignore")
                .arg(clap::Arg::new("vms").long("vm").short('m').action(clap::ArgAction::Append).help("VM profile name (default: windows-build, linux-build, macos-build)")),
        )
        // QEMU-specific commands
        .subcommand(
            Command::new("snapshot")
                .about("VM snapshot operations (QEMU only)")
                .subcommand_required(true)
                .subcommand(
                    Command::new("save")
                        .about("Save a VM state snapshot")
                        .arg(clap::Arg::new("profile").required(true))
                        .arg(clap::Arg::new("name").required(true)),
                )
                .subcommand(
                    Command::new("load")
                        .about("Restore a VM state snapshot")
                        .arg(clap::Arg::new("profile").required(true))
                        .arg(clap::Arg::new("name").required(true)),
                )
                .subcommand(
                    Command::new("delete")
                        .about("Delete a VM state snapshot")
                        .arg(clap::Arg::new("profile").required(true))
                        .arg(clap::Arg::new("name").required(true)),
                )
                .subcommand(
                    Command::new("list")
                        .about("List VM snapshots")
                        .arg(clap::Arg::new("profile").required(true)),
                ),
        )
        .subcommand(
            Command::new("resize-disk")
                .about("Grow a VM disk image (QEMU only)")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("plus_gb").long("plus-gb").required(true).help("GB to add").value_parser(clap::value_parser!(u32))),
        )
        .subcommand(
            Command::new("mount")
                .about("Project mount diagnostics (QEMU only)")
                .subcommand_required(true)
                .subcommand(
                    Command::new("status")
                        .about("Show current mount configuration")
                        .arg(clap::Arg::new("profile").required(true)),
                )
                .subcommand(
                    Command::new("verify")
                        .about("Verify the project mount is active inside a VM")
                        .arg(clap::Arg::new("profile").required(true)),
                ),
        )
        .subcommand(
            Command::new("network")
                .about("Start a VM with specific network/share configuration for testing (QEMU only)")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name (e.g., windows-build)"))
                .arg(
                    clap::Arg::new("type")
                        .long("type")
                        .value_parser(["virtiofs", "smb", "9p", "none"])
                        .default_value("virtiofs")
                        .help("Mount type to use for project sharing"),
                )
                .arg(
                    clap::Arg::new("host-dir")
                        .long("host-dir")
                        .help("Host directory to share (default: current directory)")
                        .default_value("."),
                )
                .arg(
                    clap::Arg::new("guest-dir")
                        .long("guest-dir")
                        .help("Guest mount point (default: /mnt/project for Linux, C:\\Users\\vagrant\\project for Windows)")
                )
                .arg(
                    clap::Arg::new("headful")
                        .long("headful")
                        .action(clap::ArgAction::SetTrue)
                        .help("Run with graphical display (VNC viewer auto-launches)"),
                )
                .arg(
                    clap::Arg::new("daemonize")
                        .long("daemonize")
                        .action(clap::ArgAction::SetTrue)
                        .help("Run QEMU in background after boot completes"),
                )
                .arg(
                    clap::Arg::new("test-only")
                        .long("test-only")
                        .action(clap::ArgAction::SetTrue)
                        .help("Exit after verifying mount (for automated testing)"),
                ),
        .subcommand(
            Command::new("adopt")
                .about("Register an existing qcow2 image as a managed VM (QEMU only)")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("disk").long("disk").required(true).help("Path to qcow2 disk")),
        )
        .subcommand(
            Command::new("refresh-network")
                .about("Reallocate ports and relaunch a VM with fresh port forwarding (QEMU only)")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name")),
        )
        .subcommand(
            Command::new("export")
                .about("Export a VM as a pre-built qcow2 image (QEMU only)")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true).help("VM profile name"))
                .arg(clap::Arg::new("out").long("out").help("Output path"))
                .arg(clap::Arg::new("store").long("store").help("Store path"))
                .arg(clap::Arg::new("version").long("version").help("Version string"))
                .arg(clap::Arg::new("notes").long("notes").help("Release notes"))
                .arg(clap::Arg::new("include-bootstrap").long("include-bootstrap").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("clean").long("clean").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("shrink").long("shrink").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("compress").long("compress").default_value("none").help("Post-factum compression: none (default), gzip, xz")),
        )
}

/// Dispatch to the appropriate command handler.
pub fn run(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match args.subcommand() {
        // Provider-agnostic commands
        Some(("start", m)) => provider::cmd_start(m),
        Some(("stop", m)) => provider::cmd_stop(m),
        Some(("import", m)) => provider::cmd_import(m),
        Some(("doctor", m)) => provider::cmd_doctor(m),
        Some(("ls", _)) => provider::cmd_ls(),
        Some(("winrm-test", m)) => winrm_test::cmd_winrm_test(m),
        Some(("bootstrap", m)) => bootstrap::cmd_bootstrap(m),
        Some(("debloat", m)) => debloat::cmd_debloat(m),
        // SSH-based commands
        Some(("exec", m)) => ssh_cmds::cmd_exec(m),
        Some(("shell", m)) => ssh_cmds::cmd_shell(m),
        Some(("run", m)) => ssh_cmds::cmd_run(m),
        Some(("screenshot", m)) => ssh_cmds::cmd_screenshot(m),
        Some(("logs", m)) => ssh_cmds::cmd_logs(m),
        Some(("push", m)) => ssh_cmds::cmd_push(m),
        Some(("pull", m)) => ssh_cmds::cmd_pull(m),
        // Init
        Some(("init", m)) => provider::cmd_init(m),
        // QEMU-specific
        Some(("snapshot", sub)) => qemu::cmd_snapshot(sub),
        Some(("resize-disk", m)) => qemu::cmd_resize_disk(m),
        Some(("mount", m)) => qemu::cmd_mount(m),
        Some(("network", m)) => qemu::cmd_network(m),
        Some(("adopt", m)) => qemu::cmd_adopt(m),
        Some(("refresh-network", m)) => qemu::cmd_refresh_network(m),
        Some(("export", m)) => qemu::cmd_export(m),
        _ => {
            eprintln!("unknown testbed subcommand");
            Ok(())
        }
    }
}
