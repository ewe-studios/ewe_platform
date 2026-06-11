//! WHY: Replaces the old global `FRAME_BATCH` with a Runtime-owned batching
//! component, so there is no `static mut` and protocols are per-message, not
//! per-session (decision 030).
//!
//! WHAT: [`InstructionReceiver`] — collects `DomOp`s via [`queue`](InstructionReceiver::queue),
//! and on [`flush`](InstructionReceiver::flush) encodes + ships them once through the
//! configured protocol.
//!
//! HOW: Holds a `Box<dyn ProtocolMethods<Vec<DomOp>>>` and an [`Arena`] — either an
//! OWNED `MemoryAllocations` (tests, custom hosts) or the GLOBAL arena that the JS
//! host's `dispose_allocation` export frees (the live loop). `flush` drains the
//! queued ops with `core::mem::take` and hands them to the protocol's
//! `encode_and_send`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use foundation_ui_traits::DomOp;
use foundation_wasm::{internal_api, MemoryAllocations, MemoryId};

use crate::protocol::{ProtocolMethods, SendResult};

/// Where the receiver allocates outgoing message slots. The live JS loop MUST use
/// [`Arena::Global`]: JS ACKs by calling the `dispose_allocation` WASM export, which
/// frees the GLOBAL arena — a slot id from a private arena would fail its generation
/// check there (the feature-00 arena seam).
enum Arena {
    /// A receiver-owned arena — for native tests and custom (non-JS) hosts that
    /// route ACKs back through [`InstructionReceiver::ack`].
    Owned(MemoryAllocations),
    /// The process-wide arena shared with the `exposed_runtime` WASM exports.
    Global,
}

/// WHY: One place owns the pending DOM ops, the protocol, and the arena, so effects
/// can `queue` without knowing anything about encoding or the FFI.
///
/// WHAT: A per-Runtime batching receiver for `DomOp`s.
///
/// HOW: `queue` appends; `flush` drains + encodes + ships once; `ack` releases a
/// slot after the host confirms it has applied the batch.
pub struct InstructionReceiver {
    ops: Vec<DomOp>,
    protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>,
    memory: Arena,
}

impl InstructionReceiver {
    /// Build a receiver with its OWN arena (native tests / custom hosts). For the
    /// live JS loop use [`InstructionReceiver::with_global_arena`] instead.
    #[must_use]
    pub fn new(protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>, memory: MemoryAllocations) -> Self {
        Self {
            ops: Vec::new(),
            protocol,
            memory: Arena::Owned(memory),
        }
    }

    /// Build a receiver that allocates message slots in the GLOBAL arena — the one
    /// JS's `dispose_allocation` export ACKs into. This is the constructor for any
    /// runtime actually talking to the JS host.
    #[must_use]
    pub fn with_global_arena(protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>) -> Self {
        Self {
            ops: Vec::new(),
            protocol,
            memory: Arena::Global,
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
        let protocol = &mut self.protocol;
        Some(match &mut self.memory {
            Arena::Owned(memory) => protocol.encode_and_send(ops, memory),
            Arena::Global => {
                // Write under the lock; SHIP after releasing it. `host_apply`
                // synchronously re-enters WASM (JS ACKs via the dispose_allocation
                // export, which locks the global arena) — shipping under the lock
                // would deadlock/panic on that re-entry.
                let (result, ptr, len) = internal_api::with_global_allocations(|memory| {
                    protocol.encode_and_write(ops, memory)
                });
                protocol.send_to_js(result.memory_id, ptr, len);
                result
            }
        })
    }

    /// Release an arena slot (the WASM-side ACK that mirrors JS's
    /// `dispose_allocation`). For a global-arena receiver JS normally ACKs directly
    /// via the export; this covers host-side error paths.
    pub fn ack(&mut self, memory_id: MemoryId) {
        let protocol = &mut self.protocol;
        match &mut self.memory {
            Arena::Owned(memory) => protocol.ack(memory_id, memory),
            Arena::Global => {
                internal_api::with_global_allocations(|memory| protocol.ack(memory_id, memory));
            }
        }
    }

    /// Borrow the receiver-OWNED arena (e.g. to inspect slot bytes in tests).
    /// `None` for a global-arena receiver — inspect via `internal_api` instead.
    #[must_use]
    pub fn memory(&self) -> Option<&MemoryAllocations> {
        match &self.memory {
            Arena::Owned(memory) => Some(memory),
            Arena::Global => None,
        }
    }
}
