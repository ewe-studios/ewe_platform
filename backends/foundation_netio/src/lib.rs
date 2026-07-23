// wire modules depend on netcap::rawstream, connection, sslconnector — only available on native targets.
// on wasm32, these are excluded unless you have alternative netcap bindings (use `wire-native` feature
// with your own netcap wasm implementation).

// event_source and the http client have native-only client code but shared types are wasm-compatible.
// their modules internally gate native parts behind cfg(not(target_family = "wasm")).

#![cfg_attr(feature = "nightly", feature(unix_socket_peek, tcp_linger))]

// ── Shared (wasm-compatible) modules — always compiled ──────────────
pub mod shared;

// ── Cross-platform client + builder (F51) ─────────────────────────────
pub mod network_client;

/// Platform-agnostic re-export of the cross-platform client builder.
pub use network_client::HttpClientBuilder;
/// Combined HTTP + WebSocket client handle (F52).
pub use network_client::{DynNetClient, NetClient};
/// Cross-platform request builder + data struct (F51).
pub use shared::client::request::PreparedRequest;
pub use shared::client::request_builder::PreparedRequestBuilder;

/// Platform-agnostic `default_http_client()` — resolves to the native
/// (`crate::http`) or wasm (`crate::wasm::client`) constructor at compile time.
/// Canonical replacement for the removed `simple_http::client::default_http_client`.
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use crate::http::default_http_client;
#[cfg(target_family = "wasm")]
pub use crate::wasm::client::default_http_client;

/// Unified HTTP client module (F51 Stage 4 canonical path).
///
/// Native client types live at `foundation_netio::http::*`; shared client
/// types at `foundation_netio::shared::client::*`; shared HTTP types at
/// `foundation_netio::shared::http::*`. (The old `simple_http` shim was removed.)
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod http;

// ── Native-only modules ─────────────────────────────────────────────
#[cfg(not(target_family = "wasm"))]
pub mod native;

// ── Wasm-only modules ───────────────────────────────────────────────
#[cfg(target_family = "wasm")]
pub mod wasm;

// ── Phase 0 compat: legacy module paths kept for downstream crates ──
// All implementation moved to shared/native/wasm; these are re-exports.

pub mod event_source;
pub mod netcap;
pub mod websocket;

#[cfg(not(target_family = "wasm"))]
pub mod http2;

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub mod quic;

/// HTTP/3 (RFC 9114) — framing, QPACK, and the mapping onto the QUIC traits.
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub mod http3;

/// WebTransport (draft-ietf-webtrans-http3) over sans-I/O QUIC + HTTP/3 (spec-55, F06).
/// Sessions, bidi/uni streams, and unreliable datagrams — no tokio.
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub mod webtransport;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod http_stream;

// ConnectRPC code generation moved to `foundation_connectrpc::codegen`
// (behind its `codegen` feature) — the prost toolchain now lives with the
// runtime crate rather than netio.

#[cfg(all(feature = "ssl-native-tls", not(target_family = "wasm")))]
extern crate native_tls;

#[cfg(feature = "cli")]
pub mod cli;
