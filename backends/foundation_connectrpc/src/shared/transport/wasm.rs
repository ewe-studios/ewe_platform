//! WASM Fetch transport capabilities stub (Decision 11 §Transport).
//!
//! WHY: Platform code needs a compile-time constant for WASM Fetch capability
//! checking (`request_streaming: false`, `full_duplex: false`, etc.).
//! The full `Transport` impl wrapping `WasmHttpExchangeTask` lands in F24
//! (client-core); this provides the capabilities constant now.
//!
//! WHAT: `WASM_FETCH_CAPABILITIES` — a `TransportCapabilities` constant usable
//! by the router and client dispatch.

use super::capabilities::TransportCapabilities;

/// WASM Fetch transport capabilities — the minimum set.
///
/// Fetch has no chunked upload, no full duplex, no HTTP/2 trailers, no
/// multiplexing. Valid for request types: unary, server-streaming.
pub const WASM_FETCH_CAPABILITIES: TransportCapabilities = TransportCapabilities {
    request_streaming: false,
    full_duplex: false,
    h2_trailers: false,
    http_versions: &[],
    multiplexed: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_fetch_capabilities_are_correct() {
        let caps = WASM_FETCH_CAPABILITIES;
        assert!(!caps.request_streaming);
        assert!(!caps.full_duplex);
        assert!(!caps.h2_trailers);
        assert!(!caps.multiplexed);
    }
}
