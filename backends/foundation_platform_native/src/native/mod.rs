//! Native IPC handler modules — compile only on non-WASM targets.
//!
//! Each module provides:
//! - A `register(session)` function that registers the handler on a
//!   [`PlatformSession`].
//! - An `Ipc` + `PlatformIpc` impl that drives the native platform
//!   (WebViewStack, WindowManager, Kotlin/Swift bridge).

pub mod plugin;

#[cfg(feature = "modal")]
pub mod modal;
#[cfg(feature = "dialog")]
pub mod dialog;
// #[cfg(feature = "camera")]    pub mod camera;
// #[cfg(feature = "biometric")] pub mod biometric;
// #[cfg(feature = "chrome")]    pub mod chrome;
