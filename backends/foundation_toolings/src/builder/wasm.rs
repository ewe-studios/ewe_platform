// WasmBuilder — raw cargo build for WASM targets.
// No wasm-pack, no wasm-bindgen — just a .wasm binary.

use std::process::{Command, Stdio};

use super::{BuildOutput, BuildResult, ProjectBuilder};
use crate::watcher::FileChange;
use crate::ToolingError;

#[derive(Debug)]
pub struct WasmBuilder {
    pub crate_path: String,
    pub target: String,
    pub release: bool,
    pub extra_args: Vec<String>,
}

impl ProjectBuilder for WasmBuilder {
    fn name(&self) -> &str {
        "wasm"
    }

    fn should_build(&self, change: &FileChange) -> bool {
        matches!(change, FileChange::Rust(path) if path.starts_with(&self.crate_path))
    }

    fn build(&self, _change: &FileChange) -> BuildResult {
        let mut args = vec!["build".into(), "--target".into(), self.target.clone()];
        if self.release {
            args.push("--release".into());
        }
        args.extend_from_slice(&self.extra_args);

        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.crate_path)
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let output = cmd.output().map_err(|e| ToolingError::Io(e))?;

        if output.status.success() {
            Ok(BuildOutput::BuildComplete {
                binary: format!("{}.wasm", self.crate_path),
            })
        } else {
            Err(ToolingError::Build("wasm cargo build failed".into()))
        }
    }
}
