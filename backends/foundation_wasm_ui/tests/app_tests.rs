//! Exercise signal updates, scope disposal and server frame delivery.

use foundation_ui_wire::{OperationT, UiOperationT as Ui};

use foundation_wasm_ui::{html, App, BODY_NODE_ID};

#[test]
fn mock_records_the_signal_update() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (count, set_count) = ctx.signal(1i64);
    let _tree = html! { ctx, receiver, <div>{count.get()}</div> };
    app.stabilize().unwrap();

    set_count.set(2);
    app.stabilize().unwrap();

    let ops: Vec<OperationT> = sent.borrow().iter().flatten().cloned().collect();
    assert!(ops
        .iter()
        .any(|op| matches!(op, OperationT { ui: Ui::SetText(ref row), .. } if row.text.as_deref().unwrap_or_default() == "2")));
}

#[test]
fn presets_construct() {
    let _frame_default = App::new();

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
    app.stabilize().unwrap();
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
    app.stabilize().unwrap();
    let before_ops: usize = sent.borrow().iter().map(Vec::len).sum();

    drop(scope);
    set_count.set(5);
    app.stabilize().unwrap();
    let after_ops: usize = sent.borrow().iter().map(Vec::len).sum();
    assert_eq!(after_ops, before_ops, "scoped effect died with the scope");

    // The ROOT still works.
    let (ctx, receiver) = app.context();
    let (n, _set) = ctx.signal(1i64);
    let _alive = html! { ctx, receiver, <b>{n.get()}</b> };
    app.stabilize().unwrap();
    assert!(sent.borrow().iter().map(Vec::len).sum::<usize>() > after_ops);
}

// ─── The server presets (frames, not FFI) ─────────────────────────────────────

#[test]
fn server_preset_collects_ewf1_frames() {
    let (app, frames) = App::server();
    let (ctx, receiver) = app.context();
    let (count, _set) = ctx.signal(7i64);
    let _tree = html! { ctx, receiver, <div>{count.get()}</div> };
    app.stabilize().unwrap();

    let mut queue = frames.borrow_mut();
    assert!(!queue.is_empty(), "flushes landed as frames");
    let frame = queue.pop_front().expect("first frame");
    let frame =
        foundation_ui_wire::read_frame(&frame, &foundation_ui_wire::WireLimits::default()).unwrap();
    assert_eq!(frame.sequence_id(), 1);
    assert!(!frame
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap()
        .is_empty());
}

/// `App::mount` is the body half of startup delivery (mirrors `theme` →
/// `<head>`): it queues a single `AppendChild` of the built root onto the
/// reserved body node, landing at the bottom of `<body>`. Without it a
/// reactive `html!` tree is built but DETACHED — nothing reaches the page.
#[test]
fn mount_appends_root_to_the_reserved_body_node() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let root = app.mount(html! { ctx, receiver, <div id="root">"hi"</div> });
    app.stabilize().unwrap();

    let ops: Vec<OperationT> = sent.borrow().iter().flatten().cloned().collect();
    assert!(
        ops.iter().any(|op| matches!(
            op,
            OperationT { ui: Ui::AppendChild(ref row), .. }
                if row.parent_id == BODY_NODE_ID && row.child_id == root
        )),
        "mount splices the root under the body node: {ops:?}"
    );
}

/// Two mounts append in call order — the second lands after the first, so a
/// late `<script>` mounted after content sits at the bottom of `<body>`.
#[test]
fn successive_mounts_append_in_order() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let first = app.mount(html! { ctx, receiver, <main>"content"</main> });
    let second = app.mount(html! { ctx, receiver, <script>"boot()"</script> });
    app.stabilize().unwrap();

    let body_appends: Vec<u32> = sent
        .borrow()
        .iter()
        .flatten()
        .cloned()
        .filter_map(|op| match &op {
            OperationT {
                ui: Ui::AppendChild(ref row),
                ..
            } if row.parent_id == BODY_NODE_ID => Some(row.child_id),
            _ => None,
        })
        .collect();
    assert_eq!(
        body_appends,
        vec![first, second],
        "content first, then the late script — both at body bottom in order"
    );
}
