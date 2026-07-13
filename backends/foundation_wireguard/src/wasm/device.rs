//! In-memory FIFO device for bridging smoltcp to the JS relay transport (F07).
//!
//! WHY: In wasm, the "link layer" is a relay WebSocket — not a kernel socket.
//! This device queues IP packets between a smoltcp poll cycle and JS I/O.
//!
//! WHAT: [`JsDevice`] — two independent FIFO queues: rx (inbound from relay)
//! and tx (outbound to relay). The JS bridge calls [`inject`](Self::inject) when
//! relayed WG packets arrive, and [`drain_outbound`](Self::drain_outbound) to
//! get packets to send through the relay.
//!
//! HOW: Single-threaded (wasm has one thread), so no locking needed. The
//! smoltcp `phy::Device` impl lives in `foundation_nativeapis::NetStack`, which
//! this device feeds — this struct is the I/O side, not the trait side.

use std::collections::VecDeque;

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

    /// Pop one inbound IP packet for the netstack driver.
    pub fn pop_inbound(&mut self) -> Option<Vec<u8>> {
        self.rx.pop_front()
    }

    /// Queue an outbound IP packet from smoltcp → relay.
    pub fn push_outbound(&mut self, pkt: Vec<u8>) {
        self.tx.push_back(pkt);
    }

    /// Drain all outbound packets for the JS bridge to relay.
    pub fn drain_outbound(&mut self) -> Vec<Vec<u8>> {
        self.tx.drain(..).collect()
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

    #[must_use]
    pub fn mtu(&self) -> usize {
        self.mtu
    }
}
