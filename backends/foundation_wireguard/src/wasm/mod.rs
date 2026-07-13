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
