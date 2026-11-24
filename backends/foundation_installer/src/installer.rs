//! The run layer: order the entries, keep the record and lock, report each
//! outcome.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::config::InstallerConfig;
use crate::context::InstallContext;
use crate::deploy::{
    docker::DockerDeploy, none::NoneDeploy, systemd::SystemdDeploy, DeployTargetAdapter, Deployed,
};
use crate::error::InstallError;
use crate::manifest::{Entry, EntryName, Manifest};
use crate::record::{InstallRecord, InstallerLock, RecordedEntry};
use crate::source::{
    apt::AptSource, cargo::CargoSource, docker_image::DockerImageSource,
    github_release::GithubReleaseSource, mise::MiseSource, url::UrlSource, SourceAdapter,
};

/// Runs a manifest. Holds one adapter for each source kind and each deploy
/// kind.
pub struct Installer {
    sources: BTreeMap<&'static str, Arc<dyn SourceAdapter>>,
    targets: BTreeMap<&'static str, Arc<dyn DeployTargetAdapter>>,
    config: InstallerConfig,
}

impl Installer {
    /// An installer with every built-in adapter.
    #[must_use]
    pub fn with_defaults(config: InstallerConfig) -> Self {
        let mut installer = Self {
            sources: BTreeMap::new(),
            targets: BTreeMap::new(),
            config,
        };
        installer = installer
            .with_source(Arc::new(GithubReleaseSource))
            .with_source(Arc::new(UrlSource))
            .with_source(Arc::new(CargoSource))
            .with_source(Arc::new(AptSource))
            .with_source(Arc::new(MiseSource))
            .with_source(Arc::new(DockerImageSource))
            .with_target(Arc::new(SystemdDeploy))
            .with_target(Arc::new(DockerDeploy))
            .with_target(Arc::new(NoneDeploy));
        installer
    }

    /// Add or replace a source adapter. Adding a platform never changes
    /// [`Installer::run`].
    #[must_use]
    pub fn with_source(mut self, adapter: Arc<dyn SourceAdapter>) -> Self {
        self.sources.insert(adapter.kind(), adapter);
        self
    }

    /// Add or replace a deploy-target adapter.
    #[must_use]
    pub fn with_target(mut self, adapter: Arc<dyn DeployTargetAdapter>) -> Self {
        self.targets.insert(adapter.kind(), adapter);
        self
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &InstallerConfig {
        &self.config
    }

    /// Apply every entry in order. One failed entry does not stop the others.
    ///
    /// The run takes the installer lock first. A lock or record failure is a
    /// run-level error; an entry failure is in the report.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Locked`] when another run holds the lock,
    /// [`InstallError::Record`] when the record cannot be read, and
    /// [`InstallError::ManifestIo`] is not used here.
    pub async fn run(
        &self,
        manifest: &Manifest,
        ctx: &InstallContext,
    ) -> Result<RunReport, InstallError> {
        let _lock = InstallerLock::acquire(&self.config, ctx).await?;
        let mut record = InstallRecord::load(&self.config.record_path())?;
        let mut outcomes = Vec::with_capacity(manifest.entries.len());

        for entry in &manifest.entries {
            let (outcome, recorded) = self.apply_entry(entry, &record, ctx).await;
            if let Some(recorded) = recorded {
                record.insert(&entry.name, recorded);
                if let Err(err) = record.save(&self.config.record_path()) {
                    outcomes.push(EntryOutcome::Failed {
                        name: entry.name.clone(),
                        error: err,
                        kept: None,
                    });
                    continue;
                }
            }
            outcomes.push(outcome);
        }

        Ok(RunReport { entries: outcomes })
    }

    /// Say what [`Installer::run`] would do, with no change to the host.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Record`] when the record cannot be read, and
    /// [`InstallError::UnknownDeployKind`] for a deploy kind with no adapter.
    pub async fn plan(
        &self,
        manifest: &Manifest,
        ctx: &InstallContext,
    ) -> Result<Vec<PlannedStep>, InstallError> {
        let record = InstallRecord::load(&self.config.record_path())?;
        let mut steps = Vec::with_capacity(manifest.entries.len());
        for entry in &manifest.entries {
            let step = match record.get(&entry.name) {
                Some(previous) => {
                    let still_there = if previous.version == entry.version {
                        self.target_for(entry)?
                            .exists(entry, &previous.deployed, ctx)
                            .await
                            .unwrap_or(false)
                    } else {
                        false
                    };
                    if still_there {
                        PlannedStep::Unchanged {
                            name: entry.name.clone(),
                        }
                    } else {
                        PlannedStep::Upgrade {
                            name: entry.name.clone(),
                            from: previous.version.clone(),
                            to: entry.version.clone(),
                        }
                    }
                }
                None => PlannedStep::Install {
                    name: entry.name.clone(),
                    version: entry.version.clone(),
                },
            };
            steps.push(step);
        }
        Ok(steps)
    }

    async fn apply_entry(
        &self,
        entry: &Entry,
        record: &InstallRecord,
        ctx: &InstallContext,
    ) -> (EntryOutcome, Option<RecordedEntry>) {
        let previous = record.get(&entry.name).cloned();
        let kept = previous.as_ref().map(|value| value.version.clone());

        let target = match self.target_for(entry) {
            Ok(target) => target,
            Err(error) => {
                return (
                    EntryOutcome::Failed {
                        name: entry.name.clone(),
                        error,
                        kept,
                    },
                    None,
                );
            }
        };

        if let Some(previous) = &previous {
            let still_there = target
                .exists(entry, &previous.deployed, ctx)
                .await
                .unwrap_or(false);
            if still_there && previous.version == entry.version {
                return (
                    EntryOutcome::Unchanged {
                        name: entry.name.clone(),
                        version: entry.version.clone(),
                    },
                    None,
                );
            }
        }

        let source = match self.source_for(entry) {
            Ok(source) => source,
            Err(error) => {
                return (
                    EntryOutcome::Failed {
                        name: entry.name.clone(),
                        error,
                        kept,
                    },
                    None,
                );
            }
        };

        let artifact = match source.resolve(entry, ctx).await {
            Ok(artifact) => artifact,
            Err(error) => {
                return (
                    EntryOutcome::Failed {
                        name: entry.name.clone(),
                        error,
                        kept,
                    },
                    None,
                );
            }
        };

        if !entry.deploy.accepts(artifact.class()) {
            return (
                EntryOutcome::Failed {
                    name: entry.name.clone(),
                    error: InstallError::Incompatible {
                        entry: entry.name.to_string(),
                        deploy: entry.deploy.kind().to_string(),
                        artifact: artifact.class().to_string(),
                    },
                    kept,
                },
                None,
            );
        }

        let previous_deployed = previous.as_ref().map(|value| &value.deployed);
        match target.apply(entry, &artifact, previous_deployed, ctx).await {
            Ok(deployed) => {
                let recorded = RecordedEntry {
                    version: entry.version.clone(),
                    artifact,
                    deployed: deployed.clone(),
                    installed_at: ctx.unix_seconds(),
                };
                (
                    EntryOutcome::Applied {
                        name: entry.name.clone(),
                        version: entry.version.clone(),
                        deployed,
                    },
                    Some(recorded),
                )
            }
            Err(error) => (
                EntryOutcome::Failed {
                    name: entry.name.clone(),
                    error,
                    kept,
                },
                None,
            ),
        }
    }

    fn source_for(&self, entry: &Entry) -> Result<&Arc<dyn SourceAdapter>, InstallError> {
        self.sources
            .get(entry.source.kind())
            .ok_or_else(|| InstallError::UnknownSourceKind {
                entry: entry.name.to_string(),
                kind: entry.source.kind().to_string(),
            })
    }

    fn target_for(&self, entry: &Entry) -> Result<&Arc<dyn DeployTargetAdapter>, InstallError> {
        self.targets
            .get(entry.deploy.kind())
            .ok_or_else(|| InstallError::UnknownDeployKind {
                entry: entry.name.to_string(),
                kind: entry.deploy.kind().to_string(),
            })
    }
}

/// The result of one run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    /// One outcome for each entry, in manifest order.
    pub entries: Vec<EntryOutcome>,
}

impl RunReport {
    /// True when at least one entry failed.
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| matches!(entry, EntryOutcome::Failed { .. }))
    }
}

/// What one entry of a run did.
#[derive(Debug, Clone, PartialEq)]
pub enum EntryOutcome {
    /// Installed or upgraded now.
    Applied {
        /// The entry name.
        name: EntryName,
        /// The version that the installer applied.
        version: String,
        /// What the deploy target made.
        deployed: Deployed,
    },
    /// The install record already has this version, and the unit or container
    /// exists. Nothing changed.
    Unchanged {
        /// The entry name.
        name: EntryName,
        /// The version that still runs.
        version: String,
    },
    /// Failed. For an upgrade, `kept` is the version that still runs.
    Failed {
        /// The entry name.
        name: EntryName,
        /// Why the entry failed.
        error: InstallError,
        /// The version that still runs, when this was an upgrade.
        kept: Option<String>,
    },
}

/// One step that [`Installer::plan`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedStep {
    /// The entry is not in the record.
    Install {
        /// The entry name.
        name: EntryName,
        /// The version to install.
        version: String,
    },
    /// The record has a different version, or the old one is gone.
    Upgrade {
        /// The entry name.
        name: EntryName,
        /// The version in the record.
        from: String,
        /// The version in the manifest.
        to: String,
    },
    /// Nothing would change.
    Unchanged {
        /// The entry name.
        name: EntryName,
    },
}
