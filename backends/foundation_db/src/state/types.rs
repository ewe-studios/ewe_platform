//! Deployment state types for the state store.
//!
//! WHY: The deployment engine needs a uniform way to track what's deployed,
//! detect config changes, and decide whether to redeploy.
//!
//! WHAT: `ResourceState` captures identity, lifecycle status, config hash,
//! and provider output for each deployed resource. `StateStatus` is the
//! lifecycle enum.
//!
//! HOW: Plain data structs with `Serialize`/`Deserialize` for persistence
//! across all backends (JSON files, `SQLite`, HTTP APIs).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lifecycle status of a deployed resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateStatus {
    /// Resource is being created for the first time.
    Creating,
    /// Resource exists and is healthy.
    Created,
    /// Resource is being updated (config changed).
    Updating,
    /// Resource is being torn down.
    Deleting,
    /// Resource has been successfully deleted.
    Deleted,
    /// Last operation failed.
    Failed {
        /// Human-readable error description.
        error: String,
    },
}

/// Full state snapshot of a deployed resource.
///
/// This is the source of truth for what's deployed — not config files.
/// Stored in whichever `StateStore` backend is active.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceState {
    /// Unique resource identifier (e.g. "my-worker", "my-cloud-run-service").
    pub id: String,

    /// Resource kind (e.g. `cloudflare::worker`, `gcp::cloud-run-service`).
    pub kind: String,

    /// Provider name ("cloudflare", "gcp", "aws").
    pub provider: String,

    /// Current lifecycle status.
    pub status: StateStatus,

    /// Deployment environment (staging, production, etc.).
    pub environment: Option<String>,

    /// SHA-256 hash of the serialized input config at time of deploy.
    /// Used for change detection: if hash matches, skip deployment.
    pub config_hash: String,

    /// Provider-specific output data (deployment ID, URL, etc.).
    pub output: serde_json::Value,

    /// Serialized input config (for inspection/debugging).
    pub config_snapshot: serde_json::Value,

    /// When this resource was first created.
    pub created_at: DateTime<Utc>,

    /// When this resource was last updated.
    pub updated_at: DateTime<Utc>,
}

impl ResourceState {
    /// Check if config has changed by comparing hashes.
    #[must_use]
    pub fn config_changed(&self, new_config_hash: &str) -> bool {
        self.config_hash != new_config_hash
    }

    /// Check if this resource needs deployment.
    ///
    /// A resource needs deployment if:
    /// - It is in `Created` status and the config hash differs
    /// - It is in `Failed` status (retry)
    /// - It is in any transitional status (`Creating`, `Updating`, `Deleting`)
    #[must_use]
    pub fn needs_deploy(&self, new_config_hash: &str) -> bool {
        match &self.status {
            StateStatus::Created => self.config_changed(new_config_hash),
            StateStatus::Failed { .. }
            | StateStatus::Creating
            | StateStatus::Updating
            | StateStatus::Deleting
            | StateStatus::Deleted => true,
        }
    }
}

impl core::fmt::Display for StateStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Creating => write!(f, "creating"),
            Self::Created => write!(f, "created"),
            Self::Updating => write!(f, "updating"),
            Self::Deleting => write!(f, "deleting"),
            Self::Deleted => write!(f, "deleted"),
            Self::Failed { error } => write!(f, "failed: {error}"),
        }
    }
}
