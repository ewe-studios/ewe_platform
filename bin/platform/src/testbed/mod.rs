mod cli;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    let testbed_cmd = clap::Command::new("testbed")
        .about("QEMU/KVM VM orchestration — build, test, and run Windows/Linux binaries")
        .subcommand_required(true)
        .subcommand(
            clap::Command::new("start")
                .about("Boot a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("headful").long("headful").action(clap::ArgAction::SetTrue)),
        )
        .subcommand(
            clap::Command::new("stop")
                .about("Gracefully shut down a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true)),
        )
        .subcommand(
            clap::Command::new("build")
                .about("Build a project inside a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("project").long("project").action(clap::ArgAction::Set)),
        )
        .subcommand(
            clap::Command::new("exec")
                .about("Run a command inside a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("cmd").required(true)),
        )
        .subcommand(
            clap::Command::new("shell")
                .about("Open an interactive SSH shell to a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true)),
        )
        .subcommand(
            clap::Command::new("run")
                .about("Launch a compiled binary inside a VM")
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("bin").long("bin").action(clap::ArgAction::Set)),
        )
        .subcommand(
            clap::Command::new("screenshot")
                .about("Capture a screenshot of a VM display")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("out").long("out").required(true).action(clap::ArgAction::Set)),
        )
        .subcommand(
            clap::Command::new("logs")
                .about("View build or run logs from a VM")
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("follow").long("follow").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("errors").long("errors").action(clap::ArgAction::SetTrue))
                .arg(clap::Arg::new("tail").long("tail").action(clap::ArgAction::Set).value_parser(clap::value_parser!(u32)))
                .arg(clap::Arg::new("kind").long("kind").default_value("build")),
        )
        .subcommand(
            clap::Command::new("doctor")
                .about("Health checks for host and/or VM")
                .arg(clap::Arg::new("profile")),
        )
        .subcommand(
            clap::Command::new("push")
                .about("Push a file or directory to a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("from").long("from").required(true).action(clap::ArgAction::Set))
                .arg(clap::Arg::new("to").long("to").required(true).action(clap::ArgAction::Set)),
        )
        .subcommand(
            clap::Command::new("pull")
                .about("Pull a file from a VM")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("from").long("from").required(true).action(clap::ArgAction::Set))
                .arg(clap::Arg::new("to").long("to").required(true).action(clap::ArgAction::Set)),
        )
        .subcommand(
            clap::Command::new("resize-disk")
                .about("Grow a VM disk image")
                .arg_required_else_help(true)
                .arg(clap::Arg::new("profile").required(true))
                .arg(clap::Arg::new("plus_gb").long("plus-gb").required(true).action(clap::ArgAction::Set).value_parser(clap::value_parser!(u32))),
        )
        .subcommand(
            clap::Command::new("ls")
                .about("List all known VMs"),
        )
        .subcommand(
            clap::Command::new("snapshot")
                .about("VM snapshot operations")
                .subcommand_required(true)
                .subcommand(
                    clap::Command::new("save")
                        .about("Save a VM state snapshot")
                        .arg(clap::Arg::new("profile").required(true))
                        .arg(clap::Arg::new("name").required(true)),
                )
                .subcommand(
                    clap::Command::new("load")
                        .about("Restore a VM state snapshot")
                        .arg(clap::Arg::new("profile").required(true))
                        .arg(clap::Arg::new("name").required(true)),
                )
                .subcommand(
                    clap::Command::new("delete")
                        .about("Delete a VM state snapshot")
                        .arg(clap::Arg::new("profile").required(true))
                        .arg(clap::Arg::new("name").required(true)),
                )
                .subcommand(
                    clap::Command::new("list")
                        .about("List VM snapshots")
                        .arg(clap::Arg::new("profile").required(true)),
                ),
        );

    command.subcommand(testbed_cmd)
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    match args.subcommand() {
        Some(("start", m)) => cli::cmd_start(m),
        Some(("stop", m)) => cli::cmd_stop(m),
        Some(("build", m)) => cli::cmd_build(m),
        Some(("exec", m)) => cli::cmd_exec(m),
        Some(("shell", m)) => cli::cmd_shell(m),
        Some(("run", m)) => cli::cmd_run(m),
        Some(("screenshot", m)) => cli::cmd_screenshot(m),
        Some(("logs", m)) => cli::cmd_logs(m),
        Some(("doctor", m)) => cli::cmd_doctor(m),
        Some(("push", m)) => cli::cmd_push(m),
        Some(("pull", m)) => cli::cmd_pull(m),
        Some(("resize-disk", m)) => cli::cmd_resize_disk(m),
        Some(("ls", _)) => cli::cmd_ls(),
        Some(("snapshot", sub)) => cli::cmd_snapshot(sub),
        _ => {
            eprintln!("unknown testbed subcommand");
            Ok(())
        }
    }
}
