//! WHY: Fine-grained reactive state for any Rust target — native, WASM, server
//! — with no DOM or transport coupling (decision 002). The UI layers build on
//! top: effects queue `DomOp`s, the `html!` macro reads setter callback ids,
//! but this crate knows nothing about either.
//!
//! WHAT: An R3-style reactive graph: height-ordered topological processing
//! (bucket queue), version-based stale detection, three-state dirty flags with
//! Check short-circuit, diamond safety by construction, explicit scoped
//! disposal, and batch coalescing through a single [`Runtime::stabilize`]
//! entry point. Signals are `(getter, setter)` tuples (decision 029).
//!
//! HOW:
//!
//! ```
//! use std::rc::Rc;
//! use foundation_signals::{Context, Runtime};
//!
//! let runtime = Rc::new(Runtime::new());
//! let ctx = Context::new(Rc::clone(&runtime));
//!
//! let (count, set_count) = ctx.signal(0);
//! let doubled = ctx.computed(move || count.get() * 2);
//!
//! let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
//! let log = Rc::clone(&seen);
//! ctx.effect(move || log.borrow_mut().push(doubled.get())); // runs NOW (decision 008)
//!
//! set_count.set(5);
//! set_count.set(21);          // coalesced — nothing propagated yet
//! runtime.stabilize();        // one flush: effect re-runs once, sees 42
//! assert_eq!(*seen.borrow(), vec![0, 42]);
//! ```

mod arena;
mod callback;
mod computed;
mod context;
mod effect;
#[cfg(not(target_arch = "wasm32"))]
pub mod hub;
mod node;
mod notification;
mod runtime;
mod signal;

pub use arena::NodeId;
pub use callback::{Callback, EventData, Modifiers};
pub use computed::ComputedGetter;
pub use context::Context;
pub use effect::Effect;
#[cfg(not(target_arch = "wasm32"))]
pub use hub::{HubGone, HubHandle, PumpReport, RemoteGetter, RemoteSetter, SignalHub, SignalStream};
#[cfg(all(not(target_arch = "wasm32"), feature = "valtron"))]
pub use hub::HubDriver;
pub use node::ThreeState;
pub use notification::NotificationManager;
pub use runtime::Runtime;
pub use signal::{SignalGetter, SignalSetter};

// ─── IntoHtml bridge (decision 007 / F01 integration) ─────────────────────────
//
// Signals appear directly in `html!` output: `{count}` reads the getter —
// inside an effect context the read auto-subscribes, so the surrounding
// binding re-renders when the signal changes.

use foundation_ui_traits::{Html, IntoHtml};

impl<T: IntoHtml + Clone + 'static> IntoHtml for &SignalGetter<T> {
    fn into_html(self) -> Html {
        self.get().into_html()
    }
}

impl<T: IntoHtml + Clone + 'static> IntoHtml for SignalGetter<T> {
    fn into_html(self) -> Html {
        self.get().into_html()
    }
}

impl<T: IntoHtml + Clone + 'static> IntoHtml for &ComputedGetter<T> {
    fn into_html(self) -> Html {
        self.get().into_html()
    }
}

impl<T: IntoHtml + Clone + 'static> IntoHtml for ComputedGetter<T> {
    fn into_html(self) -> Html {
        self.get().into_html()
    }
}
