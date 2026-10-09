//! WHY: The `html!` macro cannot resolve types — `primal:onX={h}` must accept
//! BOTH a `SignalSetter` (two-way binding, decision 029) and a plain closure
//! without the macro knowing which it got. Trait dispatch at Rust-compile time
//! answers it (G21): setters expose their interop `callback_id`, closures
//! don't.
//!
//! WHAT: [`MaybeCallback`] and its two impl families. The generated code calls
//! `MaybeCallback::maybe_callback_id(&handler)`; `Some(id)` becomes a
//! `primal:setter` attribute the JS event runtime (F08) wires back through
//! `invoke_callback`.
//!
//! HOW: This module lives HERE (not the proc-macro crate) because proc macros
//! can't depend on their host crates — the generated code names this trait
//! through the `foundation_wasm_ui` path (G21).

use foundation_signals::{Callback, EventData, SignalSetter};

/// Compile-time classification of `primal:on*` handler expressions (G21).
pub trait MaybeCallback {
    /// `Some(callback_id)` for setter-backed two-way bindings; `None` for
    /// plain handlers (their wiring is the F08 event runtime's concern).
    fn maybe_callback_id(&self) -> Option<u64>;
}

impl<T: Clone + PartialEq + 'static> MaybeCallback for SignalSetter<T> {
    fn maybe_callback_id(&self) -> Option<u64> {
        Some(self.callback_id())
    }
}

// A `Context::callback` handle: arbitrary event logic wired through the same
// `primal:setter` signal bridge as a setter (the escape hatch for events that
// carry no convertible value — clicks, image load/error, dismiss).
impl MaybeCallback for Callback {
    fn maybe_callback_id(&self) -> Option<u64> {
        Some(self.callback_id())
    }
}

// Closures over our owned event payload (the spec sketch used `web_sys::Event`,
// which the owned runtime does not depend on — `EventData` is the F02/F08
// event shape).
impl<F: Fn(&EventData)> MaybeCallback for F {
    fn maybe_callback_id(&self) -> Option<u64> {
        None
    }
}
