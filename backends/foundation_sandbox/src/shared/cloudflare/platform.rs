//! The Cloudflare platform wrapper.
//!
//! The wrapper holds the context of this run, this DO's sandbox manager and its
//! container control. A caller takes `&CloudflarePlatform`; it never names the
//! `worker` crate.
//!
//! The type serves both targets: the container control is generic over
//! [`ContainerApi`](super::ContainerApi), so only the `worker` adapters in
//! `crate::cloudflare` are wasm-only.

use std::any::Any;
use std::sync::Arc;

use foundation_context::Ctx;

use crate::shared::container::ContainerControl;
use crate::shared::control::SandboxProvider;
use crate::shared::platform::{Platform, PlatformKind};

use super::container::CloudflareContainerControl;

/// The Cloudflare part of one run.
pub struct CloudflarePlatform {
    /// The context of this run.
    pub ctx: Ctx,
    /// This DO's sandbox manager: one sandbox, `SandboxIdPolicy::Fixed`.
    pub sandboxes: Arc<dyn SandboxProvider>,
    /// The same object as `control`.
    pub containers: Option<Arc<dyn ContainerControl>>,
    /// This DO's container control. It implements [`ContainerControl`].
    pub control: Arc<CloudflareContainerControl>,
}

impl Platform for CloudflarePlatform {
    fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    fn sandboxes(&self) -> &Arc<dyn SandboxProvider> {
        &self.sandboxes
    }

    fn containers(&self) -> Option<&Arc<dyn ContainerControl>> {
        self.containers.as_ref()
    }

    fn kind(&self) -> PlatformKind {
        PlatformKind::Cloudflare
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The Cloudflare wrapper behind a `dyn Platform`, or `None` for another part.
#[must_use]
pub fn as_cloudflare(platform: &dyn Platform) -> Option<&CloudflarePlatform> {
    platform.as_any().downcast_ref::<CloudflarePlatform>()
}
