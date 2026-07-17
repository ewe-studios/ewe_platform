//! PlatformBuilder — wraps `tauri::Builder<R>`, extends Tauri's lifecycle.
//!
//! The platform never bypasses Tauri. It wraps Tauri's primitives in the
//! session coordination model. User code talks to the session. The session
//! talks to Tauri. Tauri's primitives remain the authoritative source for
//! window/webview/plugin/event state — the session just coordinates them.

use tauri::{App, Context, Runtime};

/// Wraps `tauri::Builder<R>`. The user calls `PlatformBuilder::new()` instead
/// of `tauri::Builder::<R>::new()`. The platform extends Tauri's lifecycle —
/// it doesn't replace or contain it.
pub struct PlatformBuilder<R: Runtime> {
    inner: tauri::Builder<R>,
}

impl<R: Runtime> PlatformBuilder<R> {
    /// Create a new platform builder. Internally wraps
    /// `tauri::Builder::<R>::new()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: tauri::Builder::<R>::new(),
        }
    }

    /// Consume the builder and produce a Tauri `App`. Takes a
    /// `tauri::Context` (build configuration, assets, plugins).
    ///
    /// In future features, this will register setup hooks, custom protocol
    /// handlers, and navigation interception before calling build.
    pub fn build(self, context: Context<R>) -> tauri::Result<App<R>> {
        self.inner.build(context)
    }

    /// Access the inner `tauri::Builder<R>` for advanced customization.
    /// Callers can register Tauri plugins, commands, menus, etc. directly.
    pub fn inner_mut(&mut self) -> &mut tauri::Builder<R> {
        &mut self.inner
    }
}

impl<R: Runtime> Default for PlatformBuilder<R> {
    fn default() -> Self {
        Self::new()
    }
}
