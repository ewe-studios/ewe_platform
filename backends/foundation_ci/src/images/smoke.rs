//! Run the image test program inside the selected Linux environment.
use crate::CiEnvironment;
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskResult, TaskSet};
use std::path::{Path, PathBuf};

#[derive(ValueEnum, Clone, Copy)]
enum Scope {
    Base,
    Image,
}
impl Scope {
    fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Image => "image",
        }
    }
}
#[derive(Args)]
struct Smoke {
    #[arg(long, default_value = "base")]
    layer: Scope,
    #[arg(long, default_value = "")]
    browsers: String,
    #[arg(long)]
    cuda: bool,
    /// Run inside this active container.
    #[arg(long, conflicts_with = "image")]
    container: Option<String>,
    /// Start this image for the test and remove the container on exit.
    #[arg(long)]
    image: Option<String>,
    /// Location of the test program inside the selected environment.
    #[arg(long)]
    script: Option<PathBuf>,
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "smoke",
        "Test the tools and libraries inside a Linux image",
        run,
    )?;
    set.expose("smoke", &["image-smoke"])
}
async fn run(ctx: TaskContext, args: Smoke) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let installed = Path::new("/opt/platform-images/scripts/smoke-test.nu");
    let (mut cmd, default_script) = if let Some(container) = &args.container {
        (
            ProcessCommand::new("docker").args(["exec", "--", container, "nu"]),
            installed.to_owned(),
        )
    } else if let Some(image) = &args.image {
        (
            ProcessCommand::new("docker").args(["run", "--rm", "--", image, "nu"]),
            installed.to_owned(),
        )
    } else {
        let source = host.root.join("ci/platform-images/scripts/smoke-test.nu");
        let path = if source.is_file() {
            source
        } else {
            host.get("PLATFORM_IMAGES_HOME").map_or_else(
                || installed.to_owned(),
                |p| PathBuf::from(p).join("scripts/smoke-test.nu"),
            )
        };
        (ProcessCommand::new("nu"), path)
    };
    cmd = cmd.arg(args.script.unwrap_or(default_script)).args([
        "--layer",
        args.layer.name(),
        "--browsers",
        &args.browsers,
    ]);
    if args.cuda {
        cmd = cmd.arg("--cuda");
    }
    ctx.exec(cmd).await
}
