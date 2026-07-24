//! `foundation_platform` — Tauri-based cross-platform coordination layer.
//!
//! Wraps `tauri::Builder` with a session backbone, route policy engine,
//! capability registry, cache manager, transport lanes, and `WebView` stack
//! manager. The platform extends Tauri's lifecycle — it doesn't replace it.

pub mod assets;
pub mod handle;
pub mod backend;
/// Typed WASM wrappers for platform IPC handlers (F41 Part D).
/// `#[cfg(target_family = "wasm")]` — uses `ipc_ffi::ipc_dispatch` + serde.
#[cfg(target_family = "wasm")]
pub mod wasm;
mod builder;
mod cache;
pub mod capability;
/// `ewe-manifest` CLI implementation (F40).
///
/// Native-only: it is a thin wrapper over [`manifest`], which has no meaning
/// on wasm32.
#[cfg(not(target_family = "wasm"))]
pub mod cli;
pub mod codegen;
mod injector;
pub mod ipc;
/// Manifest generation, signing, and verification (F40).
///
/// Native-only: build scripts and the `ewe-manifest` CLI call this to mint
/// keys and describe bundles. It has no meaning on wasm32, where there is no
/// filesystem to scan and no key material to hold.
#[cfg(not(target_family = "wasm"))]
pub mod manifest;
pub mod multi_app;
mod mutation;
pub mod ota;
pub mod overlay;
pub mod stack;
pub mod window;
pub mod worker;
pub mod ewe;
pub mod pattern;
pub mod profiles;
mod responder;
mod route;
pub mod route_handler;
mod session;
mod types;
mod wasmtime_responder;

pub use profiles::{default_profile_for_source, Access, ProfileError, ProfileGate, Service};

pub use assets::{
    parse_semver, AssetLayout, AssetResolverFs, AssetSource, OtaDownload, OtaPlan,
    PlatformAssetManager, ReadOnlyVfsDirectory, ReadOnlyVfsFile, ReadOnlyVfsSeekableFile,
};
pub use backend::{query_backend, BackendTransport, ClosureTransport, DefaultTransport};
pub use backend::http::HttpBackend;
pub use backend::ipc_dispatch::{dispatch_ipc, SessionTransport};
pub use builder::PlatformBuilder;
pub use cache::{CacheManager, CacheStorage, CachedEntry, MemoryCacheStorage};
pub use capability::{PlatformIpc, PlatformIpcRegistry};
pub use multi_app::{AppConfig, AppIsolation};
pub use mutation::{MemoryQueueStorage, Mutation, MutationQueue, MutationStatus, QueueStorage, ReplayError, ReplayResult};
pub use manifest::{Manifest, ManifestApp, ManifestFile, ManifestSource, KeyPair};
pub use ota::{OtaAppEntry, OtaFileEntry, OtaManifest, PackageDirectorate, RollbackState};
pub use stack::{PooledWebView, PreloadEntry, SlotState, StackConfig, WebViewOps, WebViewPool, WebViewSlot, WebViewStack, WebViewState};
pub use ewe::{encode_protocol, register_ewe_protocol, select_protocol, transport_for_source, Transport};
pub use overlay::{OverlayCapability, OverlayConfig, OverlayPosition};
pub use responder::{MobileApp, MobileDisk, RemoteProxy, WebviewApp};
pub use wasmtime_responder::{wasmtime_app, WasmtimeResponder, WasmtimeShell};
pub use window::{NoopWindowOps, WindowManager, WindowOps};
pub use worker::{WorkerChannel, WorkerReceiver, WorkerRegistry};
pub use route::{ipc_shell, ipc_shell_with, remote_fetch, webview_app, RouteDecisionExt};
pub use route_handler::{FnRouteHandler, RouteHandler, RouteResponder};
pub use session::{PlatformSession, SessionEvent};
pub use foundation_wasm::ipc::{IpcRequest, IpcResponse};

// Re-export all platform types from foundation_ui_traits so consumers
// only need one `use foundation_platform::*` import.
pub use foundation_ui_traits::*;


/// Build and launch a `foundation_platform` app.
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
