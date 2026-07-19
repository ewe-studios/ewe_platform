//! Server entry points — platform-gated (spec-57, F009/F010).

#[cfg(not(target_family = "wasm"))]
pub mod native;
#[cfg(target_family = "wasm")]
pub mod cloudflare;
// SignalR WebSocket hub DO (wasm-only, uses worker-rs + Durable Objects).
// Built on the general machinery in foundation_deployment_cloudflare::workers.
#[cfg(target_family = "wasm")]
pub mod signalr_do;

