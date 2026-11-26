//! Copy the selected image's provisioning files to a local macOS guest.
use super::{
    Guest,
    connection::{Connection, command},
    state,
};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};

#[derive(Args)]
struct Provision {
    #[command(flatten)]
    connection: Connection,
    #[arg(long, default_value = "pi-vm")]
    name: String,
}

// The guest runs the provisions from the archive in a directory owned by this session.
// Bash preserves the installer exit status when tee writes its diagnostic log.
const INSTALL: &str = r#"bash -c 'set -euo pipefail
work=$(mktemp -d /tmp/platform-images-guest.XXXXXXXX)
trap "rm -rf -- \"\$work\"" EXIT
tar -xzf - -C "$work"
bash "$work/guest/provision.sh" 2>&1 | tee /tmp/platform-images-provision.log
'"#;

async fn run(ctx: TaskContext, args: Provision) -> TaskResult {
    if !matches!(args.connection.os, Guest::Macos) {
        return Err(TaskError::operation(
            "Windows provisions itself during installation",
        ));
    }
    let (id, status) = state(&ctx, &args.name)
        .await?
        .ok_or_else(|| TaskError::operation(format!("Missing guest container {}", args.name)))?;
    if status != "running" {
        return Err(TaskError::operation(format!(
            "The guest container {} is {status}",
            args.name
        )));
    }
    let stage = tempfile::tempdir().map_err(TaskError::operation)?;
    let guest = stage.path().join("guest");
    ctx.exec(
        ProcessCommand::new("docker")
            .arg("cp")
            .arg(format!("{id}:/opt/platform-images/guest"))
            .arg(&guest),
    )
    .await?;
    let check = ProcessCommand::new("docker").args([
        "exec",
        &id,
        "sh",
        "-c",
        "if test -d /opt/platform-images/layers; then exit 0; else exit 44; fi",
    ]);
    match ctx.exec(check).await {
        Ok(()) => {
            ctx.exec(
                ProcessCommand::new("docker")
                    .arg("cp")
                    .arg(format!("{id}:/opt/platform-images/layers"))
                    .arg(guest.join("layers")),
            )
            .await?;
        }
        Err(TaskError::Exit { code: 44, .. }) => {}
        Err(error) => return Err(error),
    }
    let archive = stage.path().join("guest.tar.gz");
    ctx.exec(
        ProcessCommand::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(stage.path())
            .arg("guest"),
    )
    .await?;
    ctx.exec(command(&ctx, &args.connection, INSTALL)?.file_input(archive))
        .await
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "vm-provision",
        "Install the image provisions in a macOS guest",
        run,
    )
}
