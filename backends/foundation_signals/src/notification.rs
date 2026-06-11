//! WHY: External listeners (telemetry, JS interop) sometimes need a "the graph
//! just settled" hook WITHOUT being nodes in it — DOM bindings don't use this
//! (they're effects, decision 004).
//!
//! WHAT: [`NotificationManager`] — registered on the `Runtime`, fired once
//! after each `stabilize()` completes, in registration order (G16).
//!
//! HOW: If a manager sets signals during its callback, the newly dirtied nodes
//! wait for the NEXT `stabilize()` — the dirty loop has already drained, and
//! `stabilize` never recurses, so no loop guard is needed.

/// Post-stabilize listener.
pub trait NotificationManager {
    fn on_stabilize_complete(&mut self);
}

impl<F: FnMut()> NotificationManager for F {
    fn on_stabilize_complete(&mut self) {
        self();
    }
}
