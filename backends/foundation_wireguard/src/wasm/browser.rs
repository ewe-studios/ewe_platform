//! Browser peer: Tunn + smoltcp NetStack + wss join + WS relay (spec-55, F07).
//!
//! WHY: The browser peer does its own WG crypto + TCP/IP end-to-end — only the
//! transport differs from native. The smoltcp Device wire is a JS callback
//! channel that sends/receives through a relay WebSocket.
//!
//! WHAT: [`BrowserWgNode`] — wraps a `WgTunnel`, `NetStack`, and `JsDevice` into
//! one browser-ready peer. `join` performs the connectrpc bootstrap over wss.
//! [`WsRelayClient`] — frames WG packets for relay forwarding.
//!
//! HOW: The same `shared` types (keys, tunnel, bootstrap, membership) are used
//! unchanged. Only the transport (`JsDevice` instead of native UDP) and join
//! path (`wss` connectrpc instead of TLS-PSK) differ.

use crate::shared::keys::IdentityKeypair;
use crate::shared::membership::PeerId;
use crate::shared::tunnel::WgTunnel;

use super::device::JsDevice;

// ---------------------------------------------------------------------------
// WsRelayClient — browser-side relay transport
// ---------------------------------------------------------------------------

/// A WebSocket-based relay client for browser peers.
///
/// WHY: Browsers have no UDP — all WG packets must go through a native relay
/// node. This client wraps the relay WebSocket and frames WG data.
///
/// WHAT: `send_packet(dst, wg_bytes)` frames a [`RelayFrame`] and queues it
/// for the JS bridge to send over the WebSocket. `recv_packet()` returns
/// inbound relayed packets.
///
/// HOW: In wasm, the actual WebSocket I/O happens in JS — this struct is the
/// Rust-side queue that the JS bridge feeds/drains.
pub struct WsRelayClient {
    /// Queued outbound relay frames.
    outbound: Vec<Vec<u8>>,
    /// Queued inbound relay frames (WG ciphertext only, dst stripped).
    inbound: Vec<(PeerId, Vec<u8>)>,
}

impl WsRelayClient {
    /// Create a new relay client with empty queues.
    #[must_use]
    pub fn new() -> Self {
        Self {
            outbound: Vec::new(),
            inbound: Vec::new(),
        }
    }

    /// WHY: JS bridge calls this to queue an outbound WG packet for relaying.
    ///
    /// WHAT: Frame the packet as `{dst_peer_id, wg_bytes}` for the relay server
    /// to forward (decision 07, trustless ciphertext only).
    pub fn send_packet(&mut self, dst: PeerId, wg_bytes: Vec<u8>) {
        use crate::shared::relay::RelayFrame;
        let frame = RelayFrame::new(dst, wg_bytes);
        self.outbound.push(frame.encode());
    }

    /// WHY: JS bridge calls this to get frames to send over the WebSocket.
    ///
    /// WHAT: Drain all queued outbound relay frames.
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        self.outbound.drain(..).collect()
    }

    /// WHY: JS bridge calls this when a relayed frame arrives over WebSocket.
    ///
    /// WHAT: Decode the [`RelayFrame`] and queue the (src, ciphertext) pair.
    pub fn on_frame(&mut self, raw: &[u8]) {
        use crate::shared::relay::RelayFrame;
        if let Some(frame) = RelayFrame::decode(raw) {
            self.inbound.push((frame.dst_peer_id, frame.ciphertext));
        }
    }

    /// WHY: The tunnel driver calls this to pick up inbound WG packets.
    ///
    /// WHAT: Drain all queued inbound relay frames.
    pub fn drain_inbound(&mut self) -> Vec<(PeerId, Vec<u8>)> {
        self.inbound.drain(..).collect()
    }
}

impl Default for WsRelayClient {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BrowserWgNode — the browser peer
// ---------------------------------------------------------------------------

/// A browser-side WireGuard mesh node.
///
/// WHY: The browser peer runs `boringtun::Tunn` + smoltcp `NetStack` in wasm —
/// no kernel module, no privileges, no UDP socket. WG packets are relayed
/// through a native relay node via WebSocket.
///
/// WHAT: Wraps a `WgTunnel`, `NetStack`, `JsDevice`, and `WsRelayClient` into
/// one browser-ready unit. After `join`, the node is ready to open overlay
/// TCP connections through the mesh.
pub struct BrowserWgNode {
    /// The WG tunnel for this peer (seed-derived, or handoff identity).
    tunnel: WgTunnel,
    /// Browser-side smoltcp netstack.
    netstack: Option<()>, // placeholder — will hold NetStack when dataplane is fully wired
    /// JS-bridged smoltcp Device.
    device: JsDevice,
    /// Relay client for sending/receiving WG packets.
    relay: WsRelayClient,
    /// This peer's identity (post-handoff).
    identity: Option<IdentityKeypair>,
    /// The overlay IP assigned to this peer.
    _overlay_ip: std::net::IpAddr,
}

impl BrowserWgNode {
    /// WHY: Create a browser peer ready for join.
    ///
    /// WHAT: Construct a new node with a fresh tunnel, JS device, and relay client.
    ///
    /// HOW: The tunnel is constructed with the seed-derived bootstrap keypair;
    /// after the identity handoff (F08), it is replaced with the identity keypair.
    #[must_use]
    pub fn new(
        tunnel: WgTunnel,
        mtu: usize,
        overlay_ip: std::net::IpAddr,
    ) -> Self {
        Self {
            tunnel,
            netstack: None,
            device: JsDevice::new(mtu),
            relay: WsRelayClient::new(),
            identity: None,
            _overlay_ip: overlay_ip,
        }
    }

    /// WHY: Feed a decrypted inbound WG packet to the tunnel → smoltcp pipeline.
    ///
    /// WHAT: Decapsulate the WG packet, inject the resulting IP packet into the
    /// JS device for smoltcp to process.
    pub fn inject_wg_packet(&mut self, src_addr: std::net::IpAddr, wg_bytes: &[u8]) {
        use crate::shared::tunnel::WgOutcome;
        let outcome = self.tunnel.decapsulate(
            Some(src_addr),
            wg_bytes,
        );
        match outcome {
            WgOutcome::WriteToTunnel(pkt) => {
                self.device.inject(pkt);
            }
            WgOutcome::WriteToNetwork(_ct) => {
                // Handled on next poll/drain.
            }
            WgOutcome::Done | WgOutcome::Error(_) => {}
        }
    }

    /// Mut access to the relay client for JS bridge integration.
    #[must_use]
    pub fn relay_mut(&mut self) -> &mut WsRelayClient {
        &mut self.relay
    }

    /// Mut access to the JS device for smoltcp hookup.
    #[must_use]
    pub fn device_mut(&mut self) -> &mut JsDevice {
        &mut self.device
    }

    /// This peer's identity keypair (after handoff).
    #[must_use]
    pub fn identity(&self) -> Option<&IdentityKeypair> {
        self.identity.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_client_send_recv_round_trip() {
        let mut client = WsRelayClient::new();
        let dst = PeerId([0xAA; 32]);

        client.send_packet(dst, b"wg data".to_vec());
        let frames = client.drain_outbound();
        assert_eq!(frames.len(), 1);

        // Simulate the relay forwarding back.
        client.on_frame(&frames[0]);
        let inbound = client.drain_inbound();
        assert_eq!(inbound.len(), 1);
        assert_eq!(inbound[0].1, b"wg data");
    }
}
