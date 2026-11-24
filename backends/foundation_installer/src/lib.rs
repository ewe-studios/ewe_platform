//! `foundation_installer` puts software on a machine and starts it.
//!
//! It reads one manifest (the JSON contract of spec 78), gets each entry
//! through a source adapter, and applies the result through a deploy-target
//! adapter. It records what it did, so a second run is safe and an upgrade can
//! roll back.
//!
//! The crate holds no global side effect. The HTTP client, Docker API, process
//! runner, clock and target triple come from [`InstallContext`]. Only the
//! `ewe-installer` binary builds the real ones.

pub mod bootstrap;
pub mod config;
pub mod context;
pub mod deploy;
pub mod error;
pub mod installer;
pub mod manifest;
pub mod record;
pub mod source;

pub use bootstrap::{BootstrapInput, BootstrapScript, InstallerBuild};
pub use config::InstallerConfig;
pub use context::{
    CommandError, CommandOutput, CommandRunner, DockerApi, DockerEngine, InstallContext,
    SystemCommandRunner,
};
pub use deploy::{DeployTargetAdapter, Deployed};
pub use error::{FieldError, InstallError};
pub use installer::{EntryOutcome, Installer, PlannedStep, RunReport};
pub use manifest::{
    DeploySpec, Entry, EntryName, Manifest, SourceSpec, SUPPORTED_MANIFEST_VERSION,
};
pub use record::{InstallRecord, InstallerLock, RecordedEntry};
pub use source::{Artifact, ImageRef, SourceAdapter};
