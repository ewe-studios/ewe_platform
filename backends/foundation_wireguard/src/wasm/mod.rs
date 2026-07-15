//! Browser/WASM build of the WireGuard mesh client (spec-55, F07).
//!
//! WHY: The browser is the only platform with no UDP socket API — it must relay
//! WG traffic through a native relay node. But the Tunn crypto + smoltcp TCP/IP
//! stack run unchanged in wasm — only the transport layer differs.
//!
//! WHAT: [`BrowserWgNode`] — a browser peer: Tunn + smoltcp netstack bridged to
//! JS via [`JsDevice`], wss connectrpc join, WebSocket relay client.
//!
//! HOW: All crypto and protocol logic is the same as native; the smoltcp `Device`
//! wire is a JS callback that sends/receives through the relay WebSocket. Browsers
//! are relay clients only, never relay servers.

/// Browser peer: Tunn + smoltcp NetStack + wss join + WS relay.
pub mod browser;

/// JS-bridged smoltcp Device: tunnels IP packets through WebSocket/WebRTC.
pub mod device;

/// WebRTC offerer (browser side).
pub mod webrtc;

/// WebTransport browser bridge — NoIoSession-based sans-I/O bridge
/// driven by JS callbacks. Uses `foundation_netio::webtransport` types
/// (always compiled, no QUIC feature needed).
///
/// The browser's `web_sys::WebTransport` API creates the QUIC connection;
/// this Rust module provides the data-plane session state machine.
#[cfg(target_family = "wasm")]
pub mod webtransport;
#[cfg(not(target_family = "wasm"))]
pub mod webtransport {
    //! WebTransport is unavailable on native targets — native peers use
    //! the full `WtSession<QuinnConnection>` (foundation_netio + quic feature).
    pub struct WasmWtBridge;
    impl WasmWtBridge {
        pub fn new(_datagrams_enabled: bool) -> Self { Self }
        pub fn on_connected(&mut self) {}
        pub fn is_ready(&self) -> bool { false }
        pub fn is_open(&self) -> bool { false }
        pub fn is_closed(&self) -> bool { false }
        pub fn tick(&mut self) -> Vec<Vec<u8>> { Vec::new() }
        pub fn inject_datagram(&mut self, _data: Vec<u8>) {}
        pub fn drain_inbound(&mut self) -> Vec<Vec<u8>> { Vec::new() }
        pub fn queue_datagram(&mut self, _data: Vec<u8>) -> Result<(), String> { Err("not wasm".into()) }
        pub fn close(&mut self, _code: u32, _reason: String) {}
        pub fn on_peer_close(&mut self, _code: u32, _reason: String) {}
    }
}
