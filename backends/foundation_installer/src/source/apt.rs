//! The `apt` source: `apt-get install <package>=<version>`.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{Entry, SourceSpec};
use crate::source::{Artifact, SourceAdapter};

/// The `apt` source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct AptSource;

#[async_trait]
impl SourceAdapter for AptSource {
    fn kind(&self) -> &'static str {
        "apt"
    }

    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError> {
        let SourceSpec::Apt { package, program } = &entry.source else {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the entry source is not an apt package".to_string(),
            });
        };
        let program = program.as_deref().unwrap_or(package);
        let args = vec![
            "install".to_string(),
            "-y".to_string(),
            format!("{package}={}", entry.version),
        ];
        let output =
            ctx.commands()?
                .run("apt-get", &args)
                .await
                .map_err(|err| InstallError::Resolve {
                    entry: entry.name.to_string(),
                    reason: err.to_string(),
                })?;
        if !output.success() {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("apt-get install failed: {}", output.stderr.trim()),
            });
        }
        let path = find_program(program, &ctx.config().program_search_dirs).ok_or_else(|| {
            InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("the program {program} is not in the search directories"),
            }
        })?;
        Ok(Artifact::Tool {
            program: program.to_string(),
            path,
        })
    }
}

/// Find a program in the search directories.
#[must_use]
pub fn find_program(
    program: &str,
    directories: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    directories
        .iter()
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}
