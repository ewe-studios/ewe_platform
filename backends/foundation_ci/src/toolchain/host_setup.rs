//! Install the tools for a Linux testbed or Quickemu host.
use crate::CiEnvironment;
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::path::PathBuf;

#[derive(Clone, Copy, ValueEnum)]
enum Distribution {
    Debian,
    Ubuntu,
    Arch,
}

#[derive(Args)]
struct Setup {
    /// Select the Linux distribution. The default reads the host release file.
    #[arg(long, value_enum)]
    distribution: Option<Distribution>,
    /// Path to the operating system release file.
    #[arg(long, default_value = "/etc/os-release")]
    os_release: PathBuf,
}

#[derive(Clone, Copy)]
enum Tools {
    Testbed,
    Quickemu,
}

fn distro(value: &str) -> Option<Distribution> {
    match value {
        "debian" | "debian-like" => Some(Distribution::Debian),
        "ubuntu" => Some(Distribution::Ubuntu),
        "arch" => Some(Distribution::Arch),
        _ => None,
    }
}

fn detect(setup: &Setup) -> Result<Distribution, TaskError> {
    if let Some(distribution) = setup.distribution {
        return Ok(distribution);
    }
    let file = std::fs::read_to_string(&setup.os_release).map_err(TaskError::operation)?;
    let mut id = "";
    let mut parents = "";
    for line in file.lines() {
        if let Some((key, value)) = line.trim().split_once('=') {
            let value = value.trim().trim_matches(['\'', '"']);
            match key {
                "ID" => id = value,
                "ID_LIKE" => parents = value,
                _ => {}
            }
        }
    }
    distro(id)
        .or_else(|| parents.split_whitespace().find_map(distro))
        .ok_or_else(|| TaskError::operation(format!("Unsupported Linux distribution {id}")))
}

fn names(distribution: Distribution, tools: Tools) -> &'static [&'static str] {
    match (distribution, tools) {
        (_, Tools::Quickemu) => &["quickemu", "jq", "python3"],
        (Distribution::Debian | Distribution::Ubuntu, Tools::Testbed) => &[
            "qemu-system-x86",
            "qemu-system-gui",
            "qemu-utils",
            "ovmf",
            "gvncviewer",
            "openssh-client",
            "curl",
            "spice-client-gtk",
            "virt-viewer",
        ],
        (Distribution::Arch, Tools::Testbed) => &[
            "qemu-base",
            "qemu-system-x86",
            "qemu-ui-spice-app",
            "qemu-ui-spice-core",
            "qemu-chardev-spice",
            "qemu-ui-gtk",
            "qemu-ui-opengl",
            "edk2-ovmf",
            "gvncviewer",
            "openssh",
            "curl",
        ],
    }
}

async fn install(ctx: TaskContext, setup: Setup, tools: Tools) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let distribution = detect(&setup)?;
    let packages = names(distribution, tools);
    let tool = match distribution {
        Distribution::Debian | Distribution::Ubuntu => {
            let apt = ProcessCommand::new("sudo")
                .arg("apt-get")
                .current_dir(&host.root);
            ctx.exec(apt.clone().arg("update")).await?;
            apt.args(["install", "-y", "--no-install-recommends"])
        }
        Distribution::Arch => {
            let tool = if super::android::find(&host, "yay").is_some() {
                ProcessCommand::new("yay")
            } else if super::android::find(&host, "pacman").is_some() {
                ProcessCommand::new("sudo").arg("pacman")
            } else {
                return Err(TaskError::operation(
                    "Install yay or pacman on the Arch host",
                ));
            };
            tool.current_dir(&host.root)
                .args(["-Syu", "--needed", "--noconfirm"])
        }
    };
    ctx.exec(tool.args(packages.iter().copied())).await
}

pub(super) fn register(tasks: &mut TaskSet) -> TaskResult {
    tasks.add(
        "install-testbed",
        "Install the Linux testbed packages",
        |ctx, args| install(ctx, args, Tools::Testbed),
    )?;
    tasks.add(
        "install-quickemu",
        "Install the Linux Quickemu packages",
        |ctx, args| install(ctx, args, Tools::Quickemu),
    )?;
    Ok(())
}
