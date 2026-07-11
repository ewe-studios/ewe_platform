// wire modules depend on netcap::rawstream, connection, sslconnector — only available on native targets.
// on wasm32, these are excluded unless you have alternative netcap bindings (use `wire-native` feature
// with your own netcap wasm implementation).

// event_source and simple_http have native-only client code but shared types are wasm-compatible.
// their modules internally gate native parts behind cfg(not(target_family = "wasm")).

#![cfg_attr(feature = "nightly", feature(unix_socket_peek, tcp_linger))]

// ── Shared (wasm-compatible) modules — always compiled ──────────────
pub mod shared;

// ── Cross-platform client builder (F51) ──────────────────────────────
pub mod http_client_builder;

// ── Native-only modules ─────────────────────────────────────────────
#[cfg(not(target_family = "wasm"))]
pub mod native;

// ── Wasm-only modules ───────────────────────────────────────────────
#[cfg(target_family = "wasm")]
pub mod wasm;

// ── Phase 0 compat: legacy module paths kept for downstream crates ──
// All implementation moved to shared/native/wasm; these are re-exports.

pub mod event_source;
pub mod simple_http;
pub mod websocket;
pub mod netcap;

#[cfg(not(target_family = "wasm"))]
pub mod http2;

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub mod quic;

/// HTTP/3 (RFC 9114) — framing, QPACK, and the mapping onto the QUIC traits.
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub mod http3;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod http_stream;

// ConnectRPC code generation moved to `foundation_connectrpc::codegen`
// (behind its `codegen` feature) — the prost toolchain now lives with the
// runtime crate rather than netio.

#[cfg(all(feature = "ssl-native-tls", not(target_family = "wasm")))]
extern crate native_tls;
