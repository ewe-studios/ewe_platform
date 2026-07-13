//! JS-bridged smoltcp Device for browser peers (spec-55, F07).
//!
//! WHY: In wasm, the "link layer" is a relay WebSocket — not a kernel socket.
//! This device implements `smoltcp::phy::Device` so it can drive a
//! `smoltcp::Interface` buffer-to-buffer, bridging JS I/O to the netstack.
//!
//! WHAT: [`JsDevice`] — a FIFO-backed `smoltcp::phy::Device`. The JS bridge
//! calls `inject` when relayed WG packets arrive; `drain_outbound` delivers
//! packets to the JS relay transport.
//!
//! HOW: Single-threaded (wasm has one thread), no locking. Follows the exact
//! same smoltcp Device pattern as `TunnDevice` in foundation_nativeapis.

use std::collections::VecDeque;

use smoltcp::phy::{Device, DeviceCapabilities, Medium};
use smoltcp::time::Instant;

pub struct JsDevice {
    mtu: usize,
    rx: VecDeque<Vec<u8>>,
    tx: VecDeque<Vec<u8>>,
}

impl JsDevice {
    #[must_use]
    pub fn new(mtu: usize) -> Self {
        Self { mtu, rx: VecDeque::new(), tx: VecDeque::new() }
    }

    /// Push a decrypted inbound IP packet from the relay → smoltcp.
    pub fn inject(&mut self, packet: Vec<u8>) {
        self.rx.push_back(packet);
    }

    /// Drain all outbound IP packets for the JS bridge to relay.
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        self.tx.drain(..).collect()
    }

    #[must_use]
    pub fn mtu(&self) -> usize { self.mtu }
}

// ── smoltcp phy::Device impl (matches TunnDevice pattern in nativeapis) ──

pub struct RxToken { buffer: Vec<u8> }

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.buffer)
    }
}

pub struct TxToken<'a> {
    tx: &'a mut VecDeque<Vec<u8>>,
}

impl smoltcp::phy::TxToken for TxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);
        self.tx.push_back(buffer);
        result
    }
}

impl Device for JsDevice {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn receive(&mut self, _now: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let buffer = self.rx.pop_front()?;
        let rx = RxToken { buffer };
        let tx = TxToken { tx: &mut self.tx };
        Some((rx, tx))
    }

    fn transmit(&mut self, _now: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken { tx: &mut self.tx })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = self.mtu;
        caps
    }
}
