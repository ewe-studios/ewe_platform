//! The `mise` source: `mise install <tool>@<version>`.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{Entry, SourceSpec};
use crate::source::{Artifact, SourceAdapter};

/// The `mise` source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct MiseSource;

#[async_trait]
impl SourceAdapter for MiseSource {
    fn kind(&self) -> &'static str {
        "mise"
    }

    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError> {
        let SourceSpec::Mise { tool, program } = &entry.source else {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the entry source is not a mise tool".to_string(),
            });
        };
        let program = program.as_deref().unwrap_or(tool);
        let reference = format!("{tool}@{}", entry.version);
        let install = vec!["install".to_string(), reference.clone()];
        let output =
            ctx.commands()?
                .run("mise", &install)
                .await
                .map_err(|err| InstallError::Resolve {
                    entry: entry.name.to_string(),
                    reason: err.to_string(),
                })?;
        if !output.success() {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("mise install failed: {}", output.stderr.trim()),
            });
        }
        let which = vec!["which".to_string(), reference];
        let output =
            ctx.commands()?
                .run("mise", &which)
                .await
                .map_err(|err| InstallError::Resolve {
                    entry: entry.name.to_string(),
                    reason: err.to_string(),
                })?;
        let path = output.stdout.trim();
        if !output.success() || path.is_empty() {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("mise cannot name the program {program}"),
            });
        }
        Ok(Artifact::Tool {
            program: program.to_string(),
            path: std::path::PathBuf::from(path),
        })
    }
}
