//! WHY: Something must OWN the `InstructionReceiver` and wire it into the
//! reactive loop (feature 04 sections 5/6): effects queue ops during
//! `stabilize()`, and when stabilize completes the batch must flush — exactly
//! once, automatically, with no global state (decision 030).
//!
//! WHAT: [`Runtime`] (built via [`RuntimeBuilder`]) owning a
//! [`SharedInstructionReceiver`] — the cloneable handle effects capture — plus
//! [`DomSignalBinding`], the signal→DOM bridge from decision 004, and the
//! flush-on-stabilize [`NotificationManager`] glue.
//!
//! HOW: The receiver sits in `Rc<RefCell<...>>`; clones are cheap and
//! single-threaded (the same G15 stance as `foundation_signals`).
//! `Runtime::attach` registers a notification manager on the signals runtime
//! that calls `flush()` after every stabilize — the full loop becomes:
//! `set() → stabilize() → effects queue DomOps → manager flushes → protocol
//! encodes → one host_apply → JS applies + ACKs`.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use foundation_signals::{Context, NotificationManager, SignalGetter};
use foundation_ui_traits::DomOp;
use foundation_wasm::{MemoryAllocations, MemoryId};

use crate::instruction::InstructionReceiver;
use crate::protocol::{ProtocolMethods, SendResult};

// ─── SharedInstructionReceiver ─────────────────────────────────────────────────

/// Cloneable handle to the runtime-owned receiver. Effects capture a clone and
/// `queue()` without knowing about protocols, arenas, or the FFI.
#[derive(Clone)]
pub struct SharedInstructionReceiver {
    inner: Rc<RefCell<InstructionReceiver>>,
}

impl SharedInstructionReceiver {
    /// Wrap a receiver for shared single-threaded access.
    #[must_use]
    pub fn new(receiver: InstructionReceiver) -> Self {
        Self {
            inner: Rc::new(RefCell::new(receiver)),
        }
    }

    /// Queue one op for the next flush.
    pub fn queue(&self, op: DomOp) {
        self.inner.borrow_mut().queue(op);
    }

    /// Encode and ship everything queued (no-op when empty). See
    /// [`InstructionReceiver::flush`].
    #[must_use = "the SendResult's memory_id is what the host must ACK"]
    pub fn flush(&self) -> Option<SendResult> {
        self.inner.borrow_mut().flush()
    }

    /// Release an arena slot (host-side ACK path).
    pub fn ack(&self, memory_id: MemoryId) {
        self.inner.borrow_mut().ack(memory_id);
    }

    /// Ops queued but not yet flushed.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.inner.borrow().pending_count()
    }

    /// Non-empty flushes since construction.
    #[must_use]
    pub fn flush_count(&self) -> u64 {
        self.inner.borrow().flush_count()
    }

    /// Inspect the receiver-OWNED arena (tests). `None` for global-arena
    /// receivers; the closure form keeps the `RefCell` borrow scoped.
    pub fn with_memory<R>(&self, f: impl FnOnce(Option<&MemoryAllocations>) -> R) -> R {
        f(self.inner.borrow().memory())
    }
}

// ─── Runtime + builder ─────────────────────────────────────────────────────────

/// The `foundation_wasm_ui` runtime: owns the receiver that bridges the signal
/// graph to the WASM-JS boundary. Construct via [`Runtime::builder`].
pub struct Runtime {
    receiver: SharedInstructionReceiver,
}

impl Runtime {
    #[must_use]
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder {
            protocol: None,
            memory: None,
            global_arena: false,
        }
    }

    /// The shared receiver handle (clone freely into effects/bindings).
    #[must_use]
    pub fn receiver(&self) -> SharedInstructionReceiver {
        self.receiver.clone()
    }

    /// Wire the stabilize→flush loop (feature 04 section 6): registers a
    /// [`NotificationManager`] on `signals` that flushes this runtime's
    /// receiver after every `stabilize()` completes.
    pub fn attach(&self, signals: &foundation_signals::Runtime) {
        signals.add_notification_manager(Box::new(FlushOnStabilize {
            receiver: self.receiver.clone(),
        }));
    }
}

/// Post-stabilize hook: drain the queued `DomOp`s in one protocol message.
struct FlushOnStabilize {
    receiver: SharedInstructionReceiver,
}

impl NotificationManager for FlushOnStabilize {
    fn on_stabilize_complete(&mut self) {
        let _ = self.receiver.flush();
    }
}

/// Builder for [`Runtime`] (feature 04 section 5).
pub struct RuntimeBuilder {
    protocol: Option<Box<dyn ProtocolMethods<Vec<DomOp>>>>,
    memory: Option<MemoryAllocations>,
    global_arena: bool,
}

impl RuntimeBuilder {
    /// The wire protocol (`ColumnarV1` / `BatchInstructionsV1` / `JsonV1` /
    /// `MockProtocol`).
    #[must_use]
    pub fn protocol(mut self, protocol: impl ProtocolMethods<Vec<DomOp>> + 'static) -> Self {
        self.protocol = Some(Box::new(protocol));
        self
    }

    /// A receiver-OWNED arena (native tests / custom hosts).
    #[must_use]
    pub fn memory(mut self, memory: MemoryAllocations) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Use the GLOBAL arena — required for the live JS loop, whose ACKs come
    /// through the `dispose_allocation` WASM export (feature-00 arena seam).
    #[must_use]
    pub fn global_arena(mut self) -> Self {
        self.global_arena = true;
        self
    }

    /// Build the runtime.
    ///
    /// # Panics
    /// Panics if no protocol was configured, or if neither
    /// [`memory`](Self::memory) nor [`global_arena`](Self::global_arena) was
    /// chosen (the spec's required-fields contract).
    #[must_use]
    pub fn build(self) -> Runtime {
        let protocol = self.protocol.expect("protocol is required");
        let receiver = if self.global_arena {
            assert!(
                self.memory.is_none(),
                "choose either memory(..) or global_arena(), not both"
            );
            InstructionReceiver::with_global_arena(protocol)
        } else {
            let memory = self.memory.expect("memory is required");
            InstructionReceiver::new(protocol, memory)
        };
        Runtime {
            receiver: SharedInstructionReceiver::new(receiver),
        }
    }
}

// ─── DomSignalBinding (decision 004 — bindings ARE effects) ───────────────────

/// An effect bridging a signal to a DOM text slot: reads the getter (tracked),
/// transforms, queues `DomOp::SetText`. The closure captures the receiver
/// handle directly — no bridge trait, no downcasts (decision 004 / F02 spec).
pub struct DomSignalBinding {
    node_id: u32,
}

impl DomSignalBinding {
    /// Bind `getter` to text content of `node_id`. The effect runs immediately
    /// (decision 008), queueing the initial render; every later change queues a
    /// new `SetText` during `stabilize()`, flushed by the attached runtime.
    pub fn bind<T, F>(
        ctx: &Context,
        getter: &SignalGetter<T>,
        receiver: &SharedInstructionReceiver,
        node_id: u32,
        transform: F,
    ) -> Self
    where
        T: Clone + 'static,
        F: Fn(&T) -> String + 'static,
    {
        let getter = getter.clone();
        let receiver = receiver.clone();
        ctx.effect(move || {
            let value = getter.get(); // dependency tracked
            receiver.queue(DomOp::SetText {
                node_id,
                text: transform(&value).into(),
            });
        });
        Self { node_id }
    }

    /// The bound element's primal id.
    #[must_use]
    pub fn node_id(&self) -> u32 {
        self.node_id
    }
}
