//! JS-bridged smoltcp Device for browser peers (spec-55, F07).
//!
//! WHY: smoltcp drives all I/O through a phy::Device; in the browser, the
//! "device" is not a UDP socket but a JS callback that sends WG-encrypted IP
//! packets through the relay WebSocket (or WebRTC data channel).
//!
//! WHAT: [`JsDevice`] — an in-memory FIFO device: inbound packets are pushed via
//! [`inject`](Self::inject), outbound packets are drained via
//! [`drain_outbound`](Self::drain_outbound).
//!
//! HOW: The JS bridge calls `inject` when relayed WG packets arrive, and
//! `drain_outbound` to get packets to send through the relay. Single-threaded
//! (wasm has one thread), so no locking needed.

use std::collections::VecDeque;

/// A bidirectional FIFO bridge between the smoltcp netstack and the JS relay transport.
///
/// WHY: In wasm, the "link layer" is the relay WebSocket — not a kernel socket.
/// This device queues IP packets between the smoltcp poll cycle and the JS I/O.
///
/// WHAT: Two independent queues — rx (inbound from relay) and tx (outbound to relay).
pub struct JsDevice {
    /// Maximum transmission unit (inner L3 MTU, after WG overhead).
    mtu: usize,
    /// Inbound IP packets (pushed from JS relay → smoltcp).
    rx: VecDeque<Vec<u8>>,
    /// Outbound IP packets (smoltcp → JS relay).
    tx: VecDeque<Vec<u8>>,
}

impl JsDevice {
    /// Create a device with the given overlay MTU.
    #[must_use]
    pub fn new(mtu: usize) -> Self {
        Self {
            mtu,
            rx: VecDeque::new(),
            tx: VecDeque::new(),
        }
    }

    /// WHY: JS bridge calls this when a relayed WG packet arrives.
    ///
    /// WHAT: Push a decrypted inbound IP packet onto the rx queue for smoltcp to process.
    pub fn inject(&mut self, packet: Vec<u8>) {
        self.rx.push_back(packet);
    }

    /// WHY: The JS bridge calls this to get packets to send through the relay.
    ///
    /// WHAT: Drain all queued outbound IP packets (to be WG-encrypted and relayed).
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        self.tx.drain(..).collect()
    }

    /// Queue an outbound IP packet for delivery (called by the netstack layer).
    pub fn push_outbound(&mut self, pkt: Vec<u8>) {
        self.tx.push_back(pkt);
    }

    /// Pop one inbound IP packet (called by the netstack driver).
    pub fn pop_inbound(&mut self) -> Option<Vec<u8>> {
        self.rx.pop_front()
    }

    /// Number of pending inbound packets.
    #[must_use]
    pub fn pending_inbound(&self) -> usize {
        self.rx.len()
    }

    /// Number of pending outbound packets.
    #[must_use]
    pub fn pending_outbound(&self) -> usize {
        self.tx.len()
    }

    /// The overlay MTU.
    #[must_use]
    pub fn mtu(&self) -> usize {
        self.mtu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_and_drain() {
        let mut dev = JsDevice::new(1420);
        dev.inject(vec![0x45, 0x00, 0x00, 0x14]);
        assert_eq!(dev.pending_inbound(), 1);

        let pkt = dev.pop_inbound().expect("pop");
        assert_eq!(pkt[0], 0x45);
        assert_eq!(dev.pending_inbound(), 0);
    }

    #[test]
    fn outbound_flow() {
        let mut dev = JsDevice::new(1420);
        dev.push_outbound(b"out1".to_vec());
        dev.push_outbound(b"out2".to_vec());
        assert_eq!(dev.pending_outbound(), 2);

        let drained = dev.drain_outbound();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], b"out1");
    }

    #[test]
    fn mtu_is_stored() {
        let dev = JsDevice::new(1380);
        assert_eq!(dev.mtu(), 1380);
    }
}
