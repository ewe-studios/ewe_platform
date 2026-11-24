//! The platform wrapper trait and its kinds.
//!
//! A host builds one wrapper and passes it down. The wrapper holds the
//! [`Ctx`](foundation_context::Ctx) and the handles of one part of the platform.
//! A wrapper never goes into the context: the context holds an entry and the
//! wrapper holds the context, so the pair would make an `Arc` cycle.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use crate::shared::container::ContainerControl;
use crate::shared::control::SandboxProvider;

/// What every platform wrapper gives a caller that does not name the part.
pub trait Platform: Send + Sync + 'static {
    /// The context of this run: the clock, the timers, the entropy and the
    /// environment entry.
    fn ctx(&self) -> &Ctx;

    /// The sandbox control plane, local or remote.
    fn sandboxes(&self) -> &Arc<dyn SandboxProvider>;

    /// The container control, when the host has one.
    fn containers(&self) -> Option<&Arc<dyn ContainerControl>>;

    /// Which part this is.
    fn kind(&self) -> PlatformKind;

    /// The same object as `dyn Any`, for the downcast helpers of each part.
    fn as_any(&self) -> &dyn Any;
}

/// The parts that ship a wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    /// Firecracker and libkrun, in process (features 05 and 09).
    Native,
    /// The Docker Engine (feature 10).
    Docker,
    /// A Cloudflare Worker and its container (features 07 and 08).
    Cloudflare,
}

impl fmt::Display for PlatformKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Native => "native",
            Self::Docker => "docker",
            Self::Cloudflare => "cloudflare",
        };
        formatter.write_str(name)
    }
}
