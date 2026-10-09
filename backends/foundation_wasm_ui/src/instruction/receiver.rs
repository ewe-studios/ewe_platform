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
use foundation_wasm::{internal_api, MemoryAllocations, MemoryId, ProtocolHandler};

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
    /// Successful (non-empty) flushes since construction — diagnostics only.
    flush_count: u64,
}

/// Typical batch is 10-200 ops; start the queue at 64 so the common case never
/// reallocates (spec section 3).
const INITIAL_OPS_CAPACITY: usize = 64;

impl InstructionReceiver {
    /// Build a receiver with its OWN arena (native tests / custom hosts). For the
    /// live JS loop use [`InstructionReceiver::with_global_arena`] instead.
    #[must_use]
    pub fn new(protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>, memory: MemoryAllocations) -> Self {
        Self {
            ops: Vec::with_capacity(INITIAL_OPS_CAPACITY),
            protocol,
            memory: Arena::Owned(memory),
            flush_count: 0,
        }
    }

    /// Build a receiver that allocates message slots in the GLOBAL arena — the one
    /// JS's `dispose_allocation` export ACKs into. This is the constructor for any
    /// runtime actually talking to the JS host.
    #[must_use]
    pub fn with_global_arena(protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>) -> Self {
        Self {
            ops: Vec::with_capacity(INITIAL_OPS_CAPACITY),
            protocol,
            memory: Arena::Global,
            flush_count: 0,
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

    /// Spec-named alias of [`pending`](Self::pending).
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.ops.len()
    }

    /// Successful (non-empty) flushes since construction. Empty flushes do not
    /// increment.
    #[must_use]
    pub fn flush_count(&self) -> u64 {
        self.flush_count
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
        // G22: keep the queue's capacity across flush cycles — a bare
        // `mem::take` would hand the protocol our allocation and leave a
        // zero-capacity Vec, forcing a reallocation on the next queue().
        let ops = core::mem::replace(&mut self.ops, Vec::with_capacity(INITIAL_OPS_CAPACITY));
        self.flush_count += 1;
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

// ─── ColumnarReceiver (feature 19 — columnar-native accumulation) ──────────────

use foundation_ui_traits::ColumnarBatch;

use crate::protocol::ColumnarV1;

/// The columnar-native receiver: `queue` pushes ops STRAIGHT into column
/// buffers (no `Vec<DomOp>` exists anywhere in the path); `flush` frames the
/// finished columns through [`ColumnarV1`]'s absolute-aligned framing.
///
/// Only the columnar protocol can take this route — generic protocols (JSON,
/// byte-0, mocks) need row-shaped `DomOp`s and use [`InstructionReceiver`].
pub struct ColumnarReceiver {
    batch: ColumnarBatch,
    protocol: ColumnarV1,
    memory: Arena,
    flush_count: u64,
}

impl ColumnarReceiver {
    /// Receiver-owned arena (native tests / custom hosts).
    #[must_use]
    pub fn new(protocol: ColumnarV1, memory: MemoryAllocations) -> Self {
        Self {
            batch: ColumnarBatch::new(),
            protocol,
            memory: Arena::Owned(memory),
            flush_count: 0,
        }
    }

    /// GLOBAL-arena receiver — the live JS loop (see
    /// [`InstructionReceiver::with_global_arena`]).
    #[must_use]
    pub fn with_global_arena(protocol: ColumnarV1) -> Self {
        Self {
            batch: ColumnarBatch::new(),
            protocol,
            memory: Arena::Global,
            flush_count: 0,
        }
    }

    /// Append one op to the column buffers (string bytes copied exactly once).
    pub fn queue(&mut self, op: &DomOp) {
        self.batch.push(op);
    }

    /// Ops accumulated but not yet flushed.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.batch.len()
    }

    /// Non-empty flushes since construction.
    #[must_use]
    pub fn flush_count(&self) -> u64 {
        self.flush_count
    }

    /// Frame and ship the accumulated columns (no-op when empty). Column
    /// buffers keep their capacity across cycles (G22).
    pub fn flush(&mut self) -> Option<SendResult> {
        if self.batch.is_empty() {
            return None;
        }
        self.flush_count += 1;
        let result = match &mut self.memory {
            Arena::Owned(memory) => {
                let (result, ptr, len) = self.protocol.write_batch(&self.batch, memory);
                self.protocol.send_to_js(result.memory_id, ptr, len);
                result
            }
            Arena::Global => {
                // Write under the lock, SHIP after releasing it (host_apply
                // re-enters WASM — same discipline as the row receiver).
                let protocol = self.protocol;
                let batch = &self.batch;
                let (result, ptr, len) = internal_api::with_global_allocations(|memory| {
                    protocol.write_batch(batch, memory)
                });
                self.protocol.send_to_js(result.memory_id, ptr, len);
                result
            }
        };
        self.batch.clear();
        Some(result)
    }

    /// Release an arena slot (host-side ACK path).
    pub fn ack(&mut self, memory_id: MemoryId) {
        match &mut self.memory {
            Arena::Owned(memory) => self.protocol.ack(memory_id, memory),
            Arena::Global => {
                internal_api::with_global_allocations(|memory| {
                    self.protocol.ack(memory_id, memory);
                });
            }
        }
    }

    /// Borrow the receiver-OWNED arena (tests). `None` for global-arena mode.
    #[must_use]
    pub fn memory(&self) -> Option<&MemoryAllocations> {
        match &self.memory {
            Arena::Owned(memory) => Some(memory),
            Arena::Global => None,
        }
    }
}

