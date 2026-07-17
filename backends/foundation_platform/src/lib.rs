//! `foundation_platform` — Tauri-based cross-platform coordination layer.
//!
//! Wraps `tauri::Builder` with a session backbone, route policy engine,
//! capability registry, cache manager, transport lanes, and WebView stack
//! manager. The platform extends Tauri's lifecycle — it doesn't replace it.

mod builder;
mod route;
mod types;

pub use builder::PlatformBuilder;
pub use route::{ipc_shell, ipc_shell_with, remote_fetch, webview_app, RouteDecisionExt};
pub use types::{CapabilityRequest, CapabilityResponse};

// Re-export all platform types from foundation_ui_traits so consumers
// only need one `use foundation_platform::*` import:
//   RouteDecision, NavigationIntent, RouteSource, Presentation, ViewKind,
//   ProtocolHint, CachePolicy, Profile, CapabilityId, Protocol,
//   SessionId, PageIdentity, IntentSource, Method
pub use foundation_ui_traits::*;
