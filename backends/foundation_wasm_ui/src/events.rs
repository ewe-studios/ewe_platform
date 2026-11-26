//! WHY: Two-way binding's return leg (feature 08 / decision 029): a DOM event
//! in the browser must land on the `SignalSetter` whose `callback_id` the
//! `html!` macro stamped into `primal:setter`. Signal callback ids live in the
//! `foundation_signals` registry (G17) — a SEPARATE namespace from
//! `foundation_wasm`'s function-call registry — so they get their own export
//! instead of piggybacking on `invoke_callback`.
//!
//! WHAT: [`install_event_bridge`] (wire a signals runtime in once at app init)
//! and the [`invoke_signal_callback`] WASM export: JS serializes `EventData`
//! into a global-arena slot and calls `invoke_signal_callback(callback_id,
//! memory_id)`; this reads the JSON, dispatches through
//! `Runtime::invoke_callback_json`, runs `stabilize()` (so the resulting
//! effects flush in the same synchronous call chain), and frees the slot —
//! Rust owns the cleanup, no JS-side dispose.
//!
//! HOW: The bridge is a process-global slot guarded by the same
//! single-threaded-wasm pattern the ABI registries use. The export is also
//! callable natively, so the whole leg is testable without a JS host.

use alloc::rc::Rc;
use alloc::string::String;

extern crate std;
use core::cell::RefCell;
use foundation_signals::Runtime as SignalsRuntime;
use foundation_wasm::{internal_api, MemoryId};

#[derive(Clone)]
struct Bridge(Rc<SignalsRuntime>, Option<crate::SharedInstructionReceiver>);

std::thread_local! {
    static EVENT_BRIDGE: RefCell<Option<Bridge>> = const { RefCell::new(None) };
}

/// Install the signals runtime that [`invoke_signal_callback`] dispatches
/// into. Call once at app init (alongside `Runtime::attach`); installing
/// again replaces the previous runtime.
pub fn install_event_bridge(runtime: Rc<SignalsRuntime>) {
    EVENT_BRIDGE.replace(Some(Bridge(runtime, None)));
}

/// Install one local app as the owner of signal events and mounted scopes.
pub fn install_app_bridge(app: &crate::App) {
    EVENT_BRIDGE.replace(Some(Bridge(Rc::clone(app.signals()), Some(app.receiver()))));
}

/// Dispose the scopes that the local app registered for this node.
#[no_mangle]
pub extern "C" fn ewe_ui_dispose_mount(node_id: u32) {
    let receiver =
        EVENT_BRIDGE.with_borrow(|slot| slot.as_ref().and_then(|bridge| bridge.1.clone()));
    if let Some(receiver) = receiver {
        receiver.dispose_mount(node_id);
    }
}

/// Remove the installed runtime (teardown / tests).
pub fn uninstall_event_bridge() {
    EVENT_BRIDGE.take();
}

/// WASM export: deliver a serialized `EventData` to signal callback
/// `callback_id`.
///
/// `allocation_id` names a GLOBAL-arena slot holding UTF-8 JSON in the F02
/// `EventData` shape (the JS `buildEventData` object). The slot is freed here
/// after reading — JS never disposes it. Every failure mode is
/// logged-and-dropped (feature 08 §13): stale ids, missing bridge, missing
/// slot, bad UTF-8/JSON all leave signals unchanged.
#[no_mangle]
pub extern "C" fn invoke_signal_callback(callback_id: u64, allocation_id: u64) {
    let memory_id = MemoryId::from_u64(allocation_id);

    // Read + free the slot first — the slot must not leak even when dispatch
    // bails (the JS contract says Rust owns this cleanup).
    let json: Option<String> = {
        let bytes = internal_api::with_global_allocations(|memory| {
            let bytes = memory
                .get(memory_id)
                .ok()
                .and_then(|slot| slot.clone_memory().ok());
            let _ = memory.deallocate(memory_id);
            bytes
        });
        bytes.and_then(|b| String::from_utf8(b).ok())
    };

    let Some(bridge) = EVENT_BRIDGE.with_borrow(Clone::clone) else {
        tracing::warn!(
            callback_id,
            "invoke_signal_callback: no event bridge installed"
        );
        return;
    };
    let Some(json) = json else {
        tracing::warn!(
            callback_id,
            allocation_id,
            "invoke_signal_callback: event slot missing or not UTF-8"
        );
        return;
    };

    // Dispatch (stale ids return false — silently fine) and flush the effects
    // the setter dirtied in the SAME synchronous chain (feature 08 §12).
    let delivered = bridge.0.invoke_callback_json(callback_id, &json);
    if delivered {
        bridge.0.stabilize();
    }
}
