//! Native module crate (F42).
//!
//! Each native IPC module lives here as a feature-gated subdirectory with:
//!   - `android/*.kt` — Kotlin sources (injected into Tauri gen/android/)
//!   - `ios/*.swift` — Swift sources (injected into Tauri gen/apple/)
//!   - `handler.rs` — IPC handler (Ipc + PlatformIpc + AndroidIpc)
//!   - `wasm.rs` — typed WASM wrapper
//!
//! Users add this as a build dependency and runtime dependency:
//! ```toml
//! [dependencies]
//! foundation_platform_native = { features = ["modal"] }
//!
//! [build-dependencies]
//! foundation_platform_native = { features = ["modal"] }
//! ```

pub mod native_module;
pub mod pipeline;

// Feature-gated modules:
#[cfg(feature = "modal")]
pub mod modal;
// #[cfg(feature = "camera")]    pub mod camera;
// #[cfg(feature = "biometric")] pub mod biometric;
// #[cfg(feature = "chrome")]    pub mod chrome;
// #[cfg(feature = "filesystem")] pub mod filesystem;
