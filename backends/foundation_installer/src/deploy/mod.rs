//! Deploy targets: the trait that makes an artifact run and what it made.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::Entry;
use crate::source::{Artifact, ImageRef};

pub mod docker;
pub mod none;
pub mod systemd;

/// What a deploy-target adapter made. Written to the install record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Deployed {
    /// A systemd unit that is enabled and started.
    Systemd {
        /// Example: `ewe-agent-rpc.service`.
        unit: String,
        /// The versioned program path. It lets a failed upgrade point the
        /// `current` link back at the old version.
        exec_path: PathBuf,
        /// The entry version that the unit runs.
        version: String,
    },
    /// A started container.
    Container {
        /// Example: `vector-db`.
        name: String,
        /// The Docker container ID, from `create_container`.
        id: String,
        /// The image that the container runs.
        image: ImageRef,
        /// The entry version that the container runs.
        version: String,
    },
    /// Installed, not started (`DeploySpec::None`).
    Installed {
        /// What the source gave.
        artifact: Artifact,
    },
}

/// Makes an artifact run on the host, or only records it.
///
/// One implementation for each [`DeploySpec`](crate::manifest::DeploySpec)
/// variant. `async_trait` makes the trait object-safe, so the installer can
/// hold `Arc<dyn DeployTargetAdapter>`.
#[async_trait]
pub trait DeployTargetAdapter: Send + Sync {
    /// The `kind` value that this adapter handles, for example `systemd`.
    fn kind(&self) -> &'static str;

    /// Make the artifact run, or only record it for `none`. For an upgrade,
    /// `previous` is what the last run made. The adapter keeps the previous
    /// version until the new one has started.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Apply`], or [`InstallError::Incompatible`] for
    /// an artifact that this target cannot run.
    async fn apply(
        &self,
        entry: &Entry,
        artifact: &Artifact,
        previous: Option<&Deployed>,
        ctx: &InstallContext,
    ) -> Result<Deployed, InstallError>;

    /// True when what `deployed` names is still on the host.
    ///
    /// The default says yes. Each built-in adapter checks its own kind.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Apply`] when the check cannot be made.
    async fn exists(
        &self,
        _entry: &Entry,
        _deployed: &Deployed,
        _ctx: &InstallContext,
    ) -> Result<bool, InstallError> {
        Ok(true)
    }
}
