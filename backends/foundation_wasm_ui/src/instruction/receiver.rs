//! WHY: Replaces the old global `FRAME_BATCH` with a Runtime-owned batching
//! component, so there is no `static mut` and protocols are per-message, not
//! per-session (decision 030).
//!
//! WHAT: [`InstructionReceiver`] — collects `DomOp`s via [`queue`](InstructionReceiver::queue),
//! and on [`flush`](InstructionReceiver::flush) encodes + ships them once through the
//! configured protocol.
//!
//! HOW: Holds a `Box<dyn ProtocolMethods<Vec<DomOp>>>` and its own
//! `MemoryAllocations` arena. `flush` drains the queued ops with `core::mem::take`
//! and hands them to the protocol's `encode_and_send`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use foundation_ui_traits::DomOp;
use foundation_wasm::{MemoryAllocations, MemoryId};

use crate::protocol::{ProtocolMethods, SendResult};

/// WHY: One place owns the pending DOM ops, the protocol, and the arena, so effects
/// can `queue` without knowing anything about encoding or the FFI.
///
/// WHAT: A per-Runtime batching receiver for `DomOp`s.
///
/// HOW: `queue` appends; `flush` drains + encodes + ships once; `ack` releases a
/// slot after JS confirms it has applied the batch.
pub struct InstructionReceiver {
    ops: Vec<DomOp>,
    protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>,
    memory: MemoryAllocations,
}

impl InstructionReceiver {
    /// Build a receiver from the default protocol and an arena.
    #[must_use]
    pub fn new(protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>, memory: MemoryAllocations) -> Self {
        Self {
            ops: Vec::new(),
            protocol,
            memory,
        }
    }

    /// Queue a single `DomOp` without encoding — called by effects during
    /// `stabilize()`.
    pub fn queue(&mut self, op: DomOp) {
        self.ops.push(op);
    }

    /// Number of ops queued but not yet flushed.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.ops.len()
    }

    /// Encode and ship all queued ops in one batch.
    ///
    /// Returns `None` (and does nothing) when there is nothing queued, so a no-op
    /// `stabilize()` makes no encoding or FFI calls. Otherwise returns the
    /// [`SendResult`] whose `memory_id` JS will `dispose_allocation` after applying.
    pub fn flush(&mut self) -> Option<SendResult> {
        if self.ops.is_empty() {
            return None;
        }
        let ops = core::mem::take(&mut self.ops);
        Some(self.protocol.encode_and_send(ops, &mut self.memory))
    }

    /// Release an arena slot back to the receiver's arena (the WASM-side ACK that
    /// mirrors JS's `dispose_allocation`).
    pub fn ack(&mut self, memory_id: MemoryId) {
        self.protocol.ack(memory_id, &mut self.memory);
    }

    /// Borrow the underlying arena (e.g. to inspect slot bytes in tests).
    #[must_use]
    pub fn memory(&self) -> &MemoryAllocations {
        &self.memory
    }
}
