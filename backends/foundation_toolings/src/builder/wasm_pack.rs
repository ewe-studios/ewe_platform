// WasmPackBuilder — wasm-pack build for web/browser targeting.
// Produces .wasm + JS bindings + pkg directory.

use std::process::{Command, Stdio};

use super::{BuildOutput, BuildResult, ProjectBuilder};
use crate::watcher::FileChange;
use crate::ToolingError;

#[derive(Debug)]
pub struct WasmPackBuilder {
    pub crate_path: String,
    pub target: String,
    pub release: bool,
    pub extra_args: Vec<String>,
}

impl ProjectBuilder for WasmPackBuilder {
    fn name(&self) -> &str {
        "wasm-pack"
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

        let output = Command::new("wasm-pack")
            .current_dir(&self.crate_path)
            .args(&args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| ToolingError::Io(e))?;

        if output.status.success() {
            Ok(BuildOutput::BuildComplete {
                binary: format!("{}/pkg", self.crate_path),
            })
        } else {
            Err(ToolingError::Build("wasm-pack build failed".into()))
        }
    }
}
