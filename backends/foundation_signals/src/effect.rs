//! WHY: Downstream crates build concrete effect TYPES (e.g.
//! `foundation_wasm_ui`'s `DomSignalBinding`) that wrap a graph effect plus
//! whatever resources it bridges to; they need a uniform lifecycle contract
//! without `foundation_signals` knowing anything about DOM or transports
//! (decision 004 — bindings ARE effects).
//!
//! WHAT: The [`Effect`] trait — `register` on creation, `unregister` on drop.
//!
//! HOW: Implementors call `Context::effect` (or hold node ids) inside
//! `register` and dispose in `unregister`. The signal system itself only runs
//! closures; this trait is the extension point, not a runtime requirement.

use std::rc::Rc;

use crate::runtime::Runtime;

/// Lifecycle contract for concrete effect types in downstream crates.
pub trait Effect {
    /// Called on creation — wire the effect into the graph (it runs
    /// immediately per decision 008).
    fn register(&mut self, runtime: &Rc<Runtime>);
    /// Called on drop — detach from the graph and release resources.
    fn unregister(&self, runtime: &Rc<Runtime>);
}
