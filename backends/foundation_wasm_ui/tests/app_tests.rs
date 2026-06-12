//! WHY: `App` exists to make the five-object ritual one line — its contract
//! is that everything reachable from it behaves exactly like the hand-wired
//! version (spec-42 feature 03), and that the SERVER preset actually
//! captures wire frames (native `host_apply` is a no-op, so without
//! `FrameSink` server flushes vanish).
//!
//! WHAT: Preset construction, `context()` driving the reactive loop
//! end-to-end, drop-disposal, child scopes, and the server frame queue
//! (arrow-family v1 default + encoder override incl. JSON; Arrow IPC v2
//! under the `arrow` feature).

use foundation_ui_traits::DomOp;
use foundation_wasm_ui::{html, App};

#[test]
fn default_is_arrow_family_and_drives_the_loop() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (count, set_count) = ctx.signal(1i64);
    let _tree = html! { ctx, receiver, <div>{count.get()}</div> };
    app.stabilize();

    set_count.set(2);
    app.stabilize();

    let ops: Vec<DomOp> = sent.borrow().iter().flatten().cloned().collect();
    assert!(ops
        .iter()
        .any(|op| matches!(op, DomOp::SetText { text, .. } if text == "2")));
}

#[test]
fn presets_construct() {
    let _arrow_default = App::new();
    let _alias = App::columnar();
    let _json = App::json();
    let _default_trait = App::default();
}

/// Dropping the App disposes the root scope: effects die with it.
#[test]
fn drop_disposes_effects() {
    let (app, sent) = App::mock();
    let signals = std::rc::Rc::clone(app.signals());
    let (ctx, receiver) = app.context();
    let (count, set_count) = ctx.signal(0i64);
    let _tree = html! { ctx, receiver, <p>{count.get()}</p> };
    app.stabilize();
    let before = sent.borrow().len();

    drop(ctx);
    drop(app); // last Context handle → scope disposed

    set_count.set(9);
    signals.stabilize();
    assert_eq!(
        sent.borrow().len(),
        before,
        "no effect ran after the App died"
    );
}

/// `scope()` children tear down independently of the root.
#[test]
fn child_scope_disposes_independently() {
    let (app, sent) = App::mock();
    let scope = app.scope();
    let receiver = app.receiver();
    let (count, set_count) = scope.signal(0i64);
    let _tree = html! { scope, receiver, <p>{count.get()}</p> };
    app.stabilize();
    let before_ops: usize = sent.borrow().iter().map(Vec::len).sum();

    drop(scope);
    set_count.set(5);
    app.stabilize();
    let after_ops: usize = sent.borrow().iter().map(Vec::len).sum();
    assert_eq!(after_ops, before_ops, "scoped effect died with the scope");

    // The ROOT still works.
    let (ctx, receiver) = app.context();
    let (n, _set) = ctx.signal(1i64);
    let _alive = html! { ctx, receiver, <b>{n.get()}</b> };
    app.stabilize();
    assert!(sent.borrow().iter().map(Vec::len).sum::<usize>() > after_ops);
}

// ─── The server presets (frames, not FFI) ─────────────────────────────────────

#[test]
fn server_preset_collects_arrow_v1_frames() {
    let (app, frames) = App::server();
    let (ctx, receiver) = app.context();
    let (count, _set) = ctx.signal(7i64);
    let _tree = html! { ctx, receiver, <div>{count.get()}</div> };
    app.stabilize();

    let mut queue = frames.borrow_mut();
    assert!(!queue.is_empty(), "flushes landed as frames");
    let frame = queue.pop_front().expect("first frame");
    assert_eq!(frame[0], 1, "protocol byte: arrow family");
    assert_eq!(frame[1], 1, "wire VERSION 1 (compact columnar)");
    assert!(frame.len() > 6, "envelope + payload");
}

#[test]
fn server_with_swaps_the_encoder() {
    let (app, frames) = App::server_with(foundation_ui_traits::JsonEncoder);
    let (ctx, receiver) = app.context();
    let _tree = html! { ctx, receiver, <div>"x"</div> };
    app.stabilize();

    let frame = frames.borrow_mut().pop_front().expect("frame");
    assert_eq!(frame[0], 2, "protocol byte: json");
    let payload = &frame[6..];
    assert!(
        payload.starts_with(b"[") || payload.starts_with(b"{"),
        "JSON payload is readable bytes"
    );
}

/// Wire VERSION 2 — real Arrow IPC — over the server path.
#[cfg(feature = "arrow")]
#[test]
fn server_with_arrow_ipc_emits_version_2() {
    let (app, frames) = App::server_with(foundation_arrow::ArrowIpcEncoder);
    let (ctx, receiver) = app.context();
    let _tree = html! { ctx, receiver, <div>"x"</div> };
    app.stabilize();

    let frame = frames.borrow_mut().pop_front().expect("frame");
    assert_eq!(frame[0], 1, "protocol byte: arrow family");
    assert_eq!(frame[1], 2, "wire VERSION 2 (Arrow IPC)");
}

#[cfg(feature = "arrow")]
#[test]
fn arrow_ipc_preset_constructs() {
    let _app = App::arrow_ipc();
}
