//! Tauri plugin crate for native platform capabilities (F42).
//!
//! Each feature-gated module provides:
//!   - An IPC handler (Ipc + PlatformIpc) that the app registers at startup
//!   - A WASM wrapper (on wasm32) for typed IPC calls from WASM apps
//!   - Kotlin/Swift sources in `android/` and `ios/` (auto-injected by
//!     tauri_plugin::Builder at build time)
//!
//! Usage:
//! ```toml
//! [dependencies]
//! foundation_platform_native = { features = ["modal"] }
//! ```
//!
//! ```rust,ignore
//! // In src-tauri/src/lib.rs setup:
//! foundation_platform_native::modal::register(&session);
//! ```

#[cfg(feature = "modal")]
pub mod modal;
// #[cfg(feature = "camera")]    pub mod camera;
// #[cfg(feature = "biometric")] pub mod biometric;
// #[cfg(feature = "chrome")]    pub mod chrome;
