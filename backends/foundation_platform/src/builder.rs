//! PlatformBuilder — wraps `tauri::Builder<R>`, extends Tauri's lifecycle.
//!
//! The platform never bypasses Tauri. It wraps Tauri's primitives in the
//! session coordination model. On `build()`, the builder creates an
//! `Arc<PlatformSession>` and stores it in Tauri's state manager.

use tauri::{App, Context, Manager, Runtime};

use crate::session::PlatformSession;

/// Wraps `tauri::Builder<R>`. The user calls `PlatformBuilder::new()` instead
/// of `tauri::Builder::<R>::new()`. On `build()`, creates the session
/// backbone and wires it into Tauri's state manager.
pub struct PlatformBuilder<R: Runtime> {
    inner: tauri::Builder<R>,
}

impl<R: Runtime> PlatformBuilder<R> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::<R>::new(),
        }
    }

    /// Consume the builder and produce a Tauri `App`.
    /// Creates the `Arc<PlatformSession>` and stores it via `app.manage()`.
    pub fn build(self, context: Context<R>) -> tauri::Result<App<R>> {
        self.inner
            .setup(|app| {
                let session = PlatformSession::new();
                app.manage(session);
                Ok(())
            })
            .build(context)
    }

    /// Access the inner `tauri::Builder<R>` for advanced customization.
    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> {
        &mut self.inner
    }
}

impl<R: Runtime> Default for PlatformBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
