//! The `cargo` source: `cargo install` with a lock file.
//!
//! The installer gives cargo an explicit install root under its artifact
//! directory. Then the path of the program is known and no environment
//! variable decides it.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{Entry, SourceSpec};
use crate::source::{Artifact, SourceAdapter};

/// The `cargo` source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct CargoSource;

#[async_trait]
impl SourceAdapter for CargoSource {
    fn kind(&self) -> &'static str {
        "cargo"
    }

    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError> {
        let SourceSpec::Cargo { krate, program } = &entry.source else {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the entry source is not a cargo crate".to_string(),
            });
        };
        let program = program.as_deref().unwrap_or(krate);
        let root = ctx
            .config()
            .artifact_dir
            .join(entry.name.as_str())
            .join(&entry.version);
        let root_text = root.display().to_string();
        let args = vec![
            "install".to_string(),
            krate.clone(),
            "--version".to_string(),
            entry.version.clone(),
            "--locked".to_string(),
            "--root".to_string(),
            root_text,
        ];
        let output =
            ctx.commands()?
                .run("cargo", &args)
                .await
                .map_err(|err| InstallError::Resolve {
                    entry: entry.name.to_string(),
                    reason: err.to_string(),
                })?;
        if !output.success() {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("cargo install failed: {}", output.stderr.trim()),
            });
        }
        let path = root.join("bin").join(program);
        if !path.exists() {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!(
                    "cargo did not install the program {} at {}",
                    program,
                    path.display()
                ),
            });
        }
        Ok(Artifact::Tool {
            program: program.to_string(),
            path,
        })
    }
}
