//! Browser peer: Tunn + smoltcp NetStack + wss join + WS relay (spec-55, F07).
//!
//! WHY: The browser peer does its own WG crypto + TCP/IP end-to-end — only the
//! transport differs from native. The smoltcp Device wire is a JS callback
//! channel that sends/receives through a relay WebSocket.
//!
//! WHAT: [`BrowserWgNode`] — wraps a `WgTunnel`, `NetStack` driven by
//! [`JsDevice`], and a [`WsRelayClient`] into one browser-ready unit.
//!
//! HOW: The same `shared` types (keys, tunnel, bootstrap, membership) are used
//! unchanged. Only the transport (`JsDevice` instead of native UDP) and join
//! path (`wss` connectrpc) differ from native.

use std::net::IpAddr;

use crate::shared::keys::{IdentityKeypair, NetworkId, WgSeed};
use crate::shared::membership::PeerId;
use crate::shared::tunnel::{WgOutcome, WgTunnel};

use super::device::JsDevice;

// ---------------------------------------------------------------------------
// WsRelayClient — browser-side relay transport
// ---------------------------------------------------------------------------

/// A sans-I/O WebSocket relay client for browser peers.
///
/// WHY: Browsers have no UDP — all WG packets go through a native relay node.
/// This client frames WG data as [`RelayFrame`]s and provides queues that the
/// JS bridge feeds/drains.
///
/// WHAT: `send_packet(dst, wg_bytes)` queues a relay frame. `drain_outbound()`
/// returns frames for the JS bridge to send. `on_frame(raw)` feeds inbound
/// relay frames from the JS bridge.
pub struct WsRelayClient {
    outbound: Vec<Vec<u8>>,
    inbound: Vec<(PeerId, Vec<u8>)>,
}

impl WsRelayClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            outbound: Vec::new(),
            inbound: Vec::new(),
        }
    }

    /// Queue a WG packet for relay forwarding to `dst`.
    pub fn send_packet(&mut self, dst: PeerId, wg_bytes: Vec<u8>) {
        use crate::shared::relay::RelayFrame;
        let frame = RelayFrame::new(dst, wg_bytes);
        self.outbound.push(frame.encode());
    }

    /// Drain all queued outbound relay frames for the JS bridge.
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        self.outbound.drain(..).collect()
    }

    /// Feed an inbound relay frame from the JS bridge.
    pub fn on_frame(&mut self, raw: &[u8]) {
        use crate::shared::relay::RelayFrame;
        if let Some(frame) = RelayFrame::decode(raw) {
            self.inbound.push((frame.dst_peer_id, frame.ciphertext));
        }
    }

    /// Drain all queued inbound WG packets.
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
/// no kernel module, no privileges, no UDP socket.
///
/// WHAT: Wraps a `WgTunnel`, `JsDevice`, and `WsRelayClient`. After `tick()`,
/// inbound WG packets are decapsulated → injected into the device; outbound
/// IP packets from the device are encapsulated and relayed.
pub struct BrowserWgNode {
    /// The WG tunnel for this peer.
    tunnel: WgTunnel,
    /// JS-bridged smoltcp device — doubles as the data plane.
    device: JsDevice,
    /// Relay client for sending/receiving WG packets.
    relay: WsRelayClient,
    /// This peer's identity (post-handoff).
    identity: Option<IdentityKeypair>,
    /// This peer's overlay IP.
    overlay_ip: IpAddr,
    /// Known peers: overlay IP → PeerId. Populated by the JS bridge
    /// from SWIM membership data received over wss.
    peers: std::collections::HashMap<IpAddr, PeerId>,
}

impl BrowserWgNode {
    /// Create a browser peer with the given tunnel and overlay IP.
    #[must_use]
    pub fn new(tunnel: WgTunnel, mtu: usize, overlay_ip: IpAddr) -> Self {
        Self {
            tunnel,
            device: JsDevice::new(mtu),
            relay: WsRelayClient::new(),
            identity: None,
            overlay_ip,
            peers: std::collections::HashMap::new(),
        }
    }

    /// WHY: Perform a local-only join — derive keys, set up the tunnel and
    /// device. The actual WebSocket connectrpc bootstrap is performed by the
    /// JS bridge; this method initializes the Rust side.
    ///
    /// WHAT: Derive bootstrap keys from `seed`, create a `WgTunnel`, set up
    /// the `JsDevice`, and set the overlay IP.
    pub fn join(seed: &WgSeed, network_id: &NetworkId, mtu: usize) -> Self {
        
        let boot = seed.derive_bootstrap(network_id);
        let overlay_ip = IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1));
        Self::new(
            WgTunnel::new(boot.static_secret, boot.public, Some(boot.psk), Some(25), 0),
            mtu,
            overlay_ip,
        )
    }

    /// WHY: Drive one tick of the tunnel + device pipeline.
    ///
    /// WHAT: Process inbound WG packets from the relay, inject decrypted IP
    /// into the device, drain outbound IP packets to be encrypted and relayed.
    ///
    /// HOW: Call this from a valtron task or JS requestAnimationFrame loop.
    pub fn tick(&mut self) {
        // Process inbound relayed WG packets → decapsulate → inject into device.
        for (_src, wg_bytes) in self.relay.drain_inbound() {
            let outcome = self.tunnel.decapsulate(
                Some(IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
                &wg_bytes,
            );
            if let WgOutcome::WriteToTunnel(ip_pkt) = outcome {
                self.device.inject(ip_pkt);
            }
        }

        // Tunnel timer upkeep.
        if let WgOutcome::WriteToNetwork(ct) = self.tunnel.update_timers() {
            self.relay.send_packet(self.resolve_peer(), ct);
        }

        // Drain WG-encrypted packets from the tunnel.
        while let Some(ct) = self.tunnel.flush_network() {
            self.relay.send_packet(self.resolve_peer(), ct);
        }

        // Drain outbound IP packets from the device → encapsulate → relay.
        for ip_pkt in self.device.drain_outbound() {
            let outcome = self.tunnel.encapsulate(&ip_pkt);
            if let WgOutcome::WriteToNetwork(ct) = outcome {
                self.relay.send_packet(self.resolve_peer(), ct);
            }
        }
    }

    /// Register a peer's overlay IP → PeerId mapping (from SWIM membership).
    pub fn add_peer(&mut self, ip: IpAddr, id: PeerId) {
        self.peers.insert(ip, id);
    }

    /// Resolve an overlay IP to a PeerId for relay routing.
    /// Returns the first known peer as a fallback for single-peer setups.
    fn resolve_peer(&self) -> PeerId {
        self.peers.values().next().copied().unwrap_or(PeerId([0; 32]))
    }

    /// Create a pre-joined node from an existing tunnel (for testing).
    pub fn from_tunnel(tunnel: WgTunnel, mtu: usize, overlay_ip: IpAddr) -> Self {
        Self::new(tunnel, mtu, overlay_ip)
    }

    /// Mut access to the relay client for JS bridge integration.
    #[must_use]
    pub fn relay_mut(&mut self) -> &mut WsRelayClient {
        &mut self.relay
    }

    /// Mut access to the JS device.
    #[must_use]
    pub fn device_mut(&mut self) -> &mut JsDevice {
        &mut self.device
    }

    /// This peer's identity keypair (after handoff).
    #[must_use]
    pub fn identity(&self) -> Option<&IdentityKeypair> {
        self.identity.as_ref()
    }

    /// Set the identity after handoff.
    pub fn set_identity(&mut self, kp: IdentityKeypair) {
        self.identity = Some(kp);
    }

    /// This peer's overlay IP.
    #[must_use]
    pub fn overlay_ip(&self) -> IpAddr {
        self.overlay_ip
    }
}
