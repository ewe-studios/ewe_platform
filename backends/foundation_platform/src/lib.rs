//! `foundation_platform` — Tauri-based cross-platform coordination layer.
//!
//! Wraps `tauri::Builder` with a session backbone, route policy engine,
//! capability registry, cache manager, transport lanes, and WebView stack
//! manager. The platform extends Tauri's lifecycle — it doesn't replace it.

mod backend;
mod builder;
mod cache;
mod capability;
pub mod codegen;
mod mutation;
pub mod stack;
pub mod ewe;
pub mod pattern;
pub mod profiles;
mod route;
mod route_handler;
mod session;
mod types;

pub use profiles::{default_profile_for_source, Access, ProfileError, ProfileGate, Service};

pub use backend::{query_backend, BackendTransport, DefaultTransport};
pub use builder::PlatformBuilder;
pub use cache::{CacheManager, CacheStorage, CachedEntry, MemoryCacheStorage};
pub use capability::{Capability, CapabilityRegistry};
pub use mutation::{MemoryQueueStorage, Mutation, MutationQueue, MutationStatus, QueueStorage, ReplayError, ReplayResult};
pub use stack::{SlotState, StackConfig, WebViewOps, WebViewSlot, WebViewStack};
pub use ewe::{encode_protocol, register_ewe_protocol, select_protocol, transport_for_source, Transport};
pub use route::{ipc_shell, ipc_shell_with, remote_fetch, webview_app, RouteDecisionExt};
pub use route_handler::{FnRouteHandler, RouteHandler};
pub use session::PlatformSession;
pub use types::{CapabilityRequest, CapabilityResponse};

// Re-export all platform types from foundation_ui_traits so consumers
// only need one `use foundation_platform::*` import.
pub use foundation_ui_traits::*;


/// Build and launch a foundation_platform app.
///
/// Wraps `PlatformBuilder::build()` with `tauri::generate_context!()` called
/// internally. The user never touches Tauri directly.
///
/// ```ignore
/// platform_run!(PlatformBuilder::new()
///     .route("/app/*", webview_app())
///     .setup(|session| { session.capabilities().register(MyCap); }));
/// ```
#[macro_export]
macro_rules! platform_run {
    ($builder:expr) => {{
        let __builder: $crate::PlatformBuilder = $builder;
        __builder.build(::tauri::generate_context!())
            .expect("failed to build foundation_platform app")
            .run(|_handle, event| {
                if let ::tauri::RunEvent::Exit = event {
                    std::process::exit(0);
                }
            })
    }};
    ($builder:expr, |$handle:ident, $event:ident| $body:block) => {{
        let __builder: $crate::PlatformBuilder = $builder;
        __builder.build(::tauri::generate_context!())
            .expect("failed to build foundation_platform app")
            .run(|$handle, $event| $body)
    }};
}
