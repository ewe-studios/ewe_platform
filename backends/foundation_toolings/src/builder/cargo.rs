// CargoBuilder — standard Rust cargo build.

use std::process::{Command, Stdio};

use super::{BuildOutput, BuildResult, ProjectBuilder};
use crate::watcher::FileChange;
use crate::ToolingError;

#[derive(Debug)]
pub struct CargoBuilder {
    pub workspace_root: String,
    pub crate_name: String,
    pub build_args: Vec<String>,
    pub skip_check: bool,
}

impl ProjectBuilder for CargoBuilder {
    fn name(&self) -> &str {
        "cargo"
    }

    fn should_build(&self, change: &FileChange) -> bool {
        matches!(change, FileChange::Rust(_))
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        if !self.skip_check {
            run_cargo_check(&self.workspace_root)?;
        }
        run_cargo_build(&self.workspace_root, &self.crate_name, &self.build_args)?;
        Ok(BuildOutput::BuildComplete {
            binary: self.crate_name.clone(),
        })
    }
}

fn run_cargo_check(workspace: &str) -> BuildResult {
    let output = Command::new("cargo")
        .current_dir(workspace)
        .arg("check")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| ToolingError::Io(e))?;

    if output.status.success() {
        Ok(BuildOutput::CheckPassed)
    } else {
        Err(ToolingError::Build("cargo check failed".into()))
    }
}

fn run_cargo_build(workspace: &str, crate_name: &str, args: &[String]) -> BuildResult {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(workspace)
        .arg("build")
        .arg("--bin")
        .arg(crate_name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for arg in args.iter().skip(2) {
        // Skip "cargo" and "build" from args
        cmd.arg(arg);
    }

    let output = cmd.output().map_err(|e| ToolingError::Io(e))?;

    if output.status.success() {
        Ok(BuildOutput::BuildComplete {
            binary: crate_name.into(),
        })
    } else {
        Err(ToolingError::Build("cargo build failed".into()))
    }
}
