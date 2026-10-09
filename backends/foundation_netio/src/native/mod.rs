//! Native-only modules (TCP, TLS, `io_uring` — not compiled on wasm).

pub mod connection;
pub mod raw_stream;
pub mod ssl;

/// QUIC listener (F35 — D12 §9): bind a UDP socket and accept QUIC connections
/// for HTTP/3 serving. Lives here rather than in the `Listener` enum because
/// QUIC's non-blocking accept model doesn't fit the blocking `accept()` API.
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub mod quic_listener;

// Dead code: tls_verification was never wired in.
// pub mod tls_verification;
