//! WASM Fetch transport capabilities (Decision 11 §Transport).
//!
//! WHY: Platform code needs a compile-time constant for WASM Fetch capability
//! checking. `H1Transport` is the transport on wasm too; this constant states
//! what the underlying fetch client can do.
//!
//! WHAT: `WASM_FETCH_CAPABILITIES` — a `TransportCapabilities` constant usable
//! by the router and client dispatch.

use super::capabilities::TransportCapabilities;

/// WASM Fetch transport capabilities.
///
/// Fetch streams a request body: `SendSafeBody::Stream` becomes a JS
/// `ReadableStream`. Fetch has no full duplex, no HTTP/2 trailers and no
/// multiplexing. Valid for request types: unary, client-streaming and
/// server-streaming.
pub const WASM_FETCH_CAPABILITIES: TransportCapabilities = TransportCapabilities {
    request_streaming: true,
    full_duplex: false,
    h2_trailers: false,
    http_versions: &[],
    multiplexed: false,
};
