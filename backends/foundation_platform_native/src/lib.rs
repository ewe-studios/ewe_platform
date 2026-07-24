//! Tauri plugin crate for native platform capabilities (F42).
//!
//! ## Structure
//!
//! ```text
//! foundation_platform_native/
//! ├── src/
//! │   ├── shared/           ← wire-format types (ALL targets)
//! │   │   ├── modal_types.rs
//! │   │   └── dialog_types.rs
//! │   ├── native/           ← IPC handlers (non-WASM only)
//! │   │   ├── modal.rs      ←   ModalIpc: WebViewStack + WindowManager
//! │   │   └── dialog.rs     ←   DialogIpc: native AlertDialog
//! │   └── wasm/             ← typed WASM wrappers (wasm32 only)
//! │       ├── modal.rs      ←   Modal::present() / dismiss()
//! │       └── dialog.rs     ←   Dialog::show()
//! ├── android/              ← Kotlin plugin sources
//! ├── ios/                  ← Swift plugin sources
//! └── permissions/          ← ACL permissions
//! ```
//!
//! ## Usage
//!
//! ### Native side (desktop / mobile binary):
//!
//! ```toml
//! [dependencies]
//! foundation_platform_native = { features = ["modal"] }
//! ```
//!
//! ```rust,ignore
//! foundation_platform_native::native::modal::register(&session);
//! ```
//!
//! ### WASM side (compiled to wasm32):
//!
//! ```toml
//! [dependencies]
//! foundation_platform_native = { features = ["modal"] }
//! ```
//!
//! ```rust,ignore
//! use foundation_platform_native::shared::modal_types::PresentArgs;
//! use foundation_platform_native::wasm::modal::Modal;
//!
//! let result = Modal::present(PresentArgs {
//!     route: "/app/settings".into(),
//!     style: Some("bottom_sheet".into()),
//!     title: Some("Settings".into()),
//! })?;
//! ```

// Shared wire-format types — always available, all targets.
pub mod shared;

// Target-gated: native IPC handlers OR typed WASM wrappers.
#[cfg(not(target_family = "wasm"))]
pub mod native;
#[cfg(target_family = "wasm")]
pub mod wasm;
