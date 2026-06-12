//! WHY: The signal event bridge is the return leg of two-way binding — a JS
//! event must reach the `SignalSetter` behind a `primal:setter` id, flush its
//! effects in the same synchronous chain, and free the event slot, with every
//! failure mode dropped silently (feature 08 §12/§13).
//!
//! WHAT: The `invoke_signal_callback` export driven NATIVELY (no JS host):
//! happy path (set + stabilize + slot freed), stale ids, bad payloads, and
//! the no-bridge guard.
//!
//! HOW: One test fn — the bridge and the global arena are process-wide state,
//! and cargo runs tests on multiple threads; a single fn keeps the sequence
//! deterministic without a serial-test dependency.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_wasm::{exposed_runtime, internal_api, MemoryId};
use foundation_wasm_ui::{install_event_bridge, invoke_signal_callback, uninstall_event_bridge};

/// Write `json` into a fresh GLOBAL-arena slot (what JS's `signalDeliver`
/// does) and return its id.
fn stage_event(json: &str) -> u64 {
    let id = exposed_runtime::create_allocation(json.len() as u64);
    let slot = internal_api::get_memory(MemoryId::from_u64(id));
    slot.apply(|m| {
        m.clear();
        m.extend_from_slice(json.as_bytes());
    });
    id
}

fn slot_is_live(id: u64) -> bool {
    internal_api::with_global_allocations(|memory| memory.get(MemoryId::from_u64(id)).is_ok())
}

#[test]
fn signal_event_bridge_end_to_end() {
    // ── No bridge installed: slot still freed, nothing panics. ──────────────
    let orphan = stage_event(r#"{"type":"change","value":"ignored"}"#);
    invoke_signal_callback(0, orphan);
    assert!(!slot_is_live(orphan), "slot freed even without a bridge");

    // ── Install: a String signal + an effect observing it. ──────────────────
    let signals = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&signals));
    let (name, set_name) = ctx.signal(String::new());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let reader = name.clone();
    ctx.effect(move || log.borrow_mut().push(reader.get()));
    install_event_bridge(Rc::clone(&signals));

    // Happy path: the default G17 String conversion takes `value`; the bridge
    // must stabilize in the same call, so the effect has ALREADY re-run.
    let slot = stage_event(r#"{"type":"change","value":"hello","primalId":"7"}"#);
    invoke_signal_callback(set_name.callback_id(), slot);
    assert_eq!(name.get(), "hello");
    assert_eq!(
        *seen.borrow(),
        vec![String::new(), String::from("hello")],
        "stabilize ran inside the export — effects flushed synchronously"
    );
    assert!(!slot_is_live(slot), "event slot freed by Rust after reading");

    // Stale/unknown id: dropped silently, slot still freed.
    let stale = stage_event(r#"{"type":"change","value":"nope"}"#);
    invoke_signal_callback(9_999_999, stale);
    assert_eq!(name.get(), "hello", "unknown id changed nothing");
    assert!(!slot_is_live(stale));

    // Malformed JSON: logged + dropped, signal retains its value.
    let bad = stage_event("not json at all");
    invoke_signal_callback(set_name.callback_id(), bad);
    assert_eq!(name.get(), "hello", "bad payload skipped the set");
    assert!(!slot_is_live(bad));

    // Missing slot id: nothing to read, nothing to break.
    invoke_signal_callback(set_name.callback_id(), 88_888_888);
    assert_eq!(name.get(), "hello");

    // ── Uninstall: events stop reaching the runtime. ─────────────────────────
    uninstall_event_bridge();
    let after = stage_event(r#"{"type":"change","value":"late"}"#);
    invoke_signal_callback(set_name.callback_id(), after);
    assert_eq!(name.get(), "hello", "uninstalled bridge drops events");
    assert!(!slot_is_live(after));
}
