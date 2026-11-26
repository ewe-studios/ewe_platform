//! The native platform wrapper.
//!
//! A caller that reaches the platform through a raw [`Ctx`] cannot create a
//! sandbox by itself (requirements section 6.0). The host builds this wrapper
//! once, holds the `Ctx` and the handles of the native part, and passes it
//! down. A function that serves several parts takes `&dyn Platform` and calls
//! [`as_native`](dyn Platform::as_native) when it needs this one.

use std::any::Any;
use std::sync::Arc;

use foundation_context::Ctx;

use crate::shared::container::ContainerControl;
use crate::shared::control::{SandboxManager, SandboxProvider};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::platform::{Platform, PlatformKind};

/// The native part of the platform: Firecracker and libkrun, in process.
pub struct NativePlatform {
    /// The context of this run.
    pub ctx: Ctx,
    /// The same object as `manager`.
    pub sandboxes: Arc<dyn SandboxProvider>,
    /// The control of the first driver in the order that has one.
    pub containers: Option<Arc<dyn ContainerControl>>,
    /// The local manager: the drivers, the record store and the configuration.
    pub manager: Arc<SandboxManager>,
}

impl Platform for NativePlatform {
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
        PlatformKind::Native
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl dyn Platform + '_ {
    /// The same object as [`NativePlatform`].
    ///
    /// # Errors
    /// [`SandboxError::PlatformMismatch`] when `kind()` is not
    /// [`PlatformKind::Native`].
    pub fn as_native(&self) -> SandboxResult<&NativePlatform> {
        let actual = self.kind();
        if actual != PlatformKind::Native {
            return Err(SandboxError::PlatformMismatch {
                expected: PlatformKind::Native,
                actual,
            }
            .into());
        }
        self.as_any()
            .downcast_ref::<NativePlatform>()
            .ok_or_else(|| {
                SandboxError::PlatformMismatch {
                    expected: PlatformKind::Native,
                    actual,
                }
                .into()
            })
    }
}
