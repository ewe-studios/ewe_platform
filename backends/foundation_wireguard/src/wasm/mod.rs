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
    //! WebTransport is only available on wasm32 targets. Native peers use
    //! `foundation_netio::webtransport::WtSession<QuinnConnection>`.
    //! This type exists solely so the `wasm` module compiles on all targets;
    //! constructing it on native panics.
    pub struct WasmWtBridge;

    impl WasmWtBridge {
        /// # Panics
        /// Always panics on native. `WasmWtBridge` is wasm32-only.
        pub fn new(_datagrams_enabled: bool) -> Self {
            panic!("WasmWtBridge is wasm32-only; native peers use foundation_netio::webtransport::WtSession")
        }
        /// Always panics on native.
        pub fn on_connected(&mut self) { Self::unreachable() }
        /// Always `false` on native.
        pub fn is_ready(&self) -> bool { false }
        /// Always `false` on native.
        pub fn is_open(&self) -> bool { false }
        /// Always `false` on native.
        pub fn is_closed(&self) -> bool { false }
        /// Panics on native.
        pub fn tick(&mut self) -> Vec<Vec<u8>> { Self::unreachable() }
        /// Panics on native.
        pub fn inject_datagram(&mut self, _data: Vec<u8>) { Self::unreachable() }
        /// Panics on native.
        pub fn drain_inbound(&mut self) -> Vec<Vec<u8>> { Self::unreachable() }
        /// Panics on native.
        pub fn queue_datagram(&mut self, _data: Vec<u8>) -> Result<(), String> { Self::unreachable() }
        /// Panics on native.
        pub fn close(&mut self, _code: u32, _reason: String) { Self::unreachable() }
        /// Panics on native.
        pub fn on_peer_close(&mut self, _code: u32, _reason: String) { Self::unreachable() }

        fn unreachable() -> ! {
            panic!("WasmWtBridge is wasm32-only")
        }
    }
}
