//! WHY: `<Show>`/`<For>` make the SET of mounted fragments signal-driven —
//! the contracts that matter are mount/unmount edges, per-instance scope
//! disposal, anchored ordering, and the no-move fast path (spec-42
//! feature 01 §6).
//!
//! WHAT: The acceptance suite over `App::mock`, asserting exact frame
//! streams — including the §5 risk (effect creation inside a running
//! watcher effect, made legal by `Runtime::untracked`).

use foundation_ui_wire::{OperationT, UiOperationT as Ui, WebOperationT as Web};
#[path = "support/names.rs"]
mod names;
use names::wire_name;

use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, App, SentBatches, SharedInstructionReceiver, Slot};

fn ops_of(sent: &SentBatches) -> Vec<OperationT> {
    sent.borrow().iter().flatten().cloned().collect()
}

fn created_tags(ops: &[OperationT]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match op {
            OperationT {
                web: Web::WebElement(ref row),
                ..
            } => wire_name(
                row.tag_id,
                row.custom_tag.as_deref(),
                foundation_ui_traits::TAG_NAMES,
            )
            .map(String::from),
            _ => None,
        })
        .collect()
}

fn removed_ids(ops: &[OperationT]) -> Vec<u32> {
    ops.iter()
        .filter_map(|op| match op {
            OperationT {
                ui: Ui::Remove(ref row),
                ..
            } => Some(row.node_id),
            _ => None,
        })
        .collect()
}

// ─── <Show> ───────────────────────────────────────────────────────────────────

#[test]
fn show_mounts_on_rising_edge_and_unmounts_on_falling() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (open, set_open) = ctx.signal(false);

    let _tree = html! { ctx, receiver,
        <section>
            <Show when={open.get()}>{ Slot::lazy(|c, r| html! { c, r, <em>"shown"</em> }) }</Show>
        </section>
    };
    app.stabilize().unwrap();
    assert!(
        !created_tags(&ops_of(&sent)).contains(&String::from("em")),
        "nothing mounted while false"
    );

    set_open.set(true);
    app.stabilize().unwrap();
    let ops = ops_of(&sent);
    assert!(
        created_tags(&ops).contains(&String::from("em")),
        "mounted on true"
    );
    // Inserted BEFORE the region anchor, not appended blind.
    assert!(ops.iter().any(|op| matches!(
        op,
        OperationT {
            ui: Ui::InsertBefore(_),
            ..
        }
    )));

    let em_id = ops
        .iter()
        .find_map(|op| match op {
            OperationT {
                web: Web::WebElement(ref row),
                ..
            } if wire_name(
                row.tag_id,
                row.custom_tag.as_deref(),
                foundation_ui_traits::TAG_NAMES,
            ) == Some("em") =>
            {
                Some(row.node_id)
            }
            _ => None,
        })
        .expect("em id");

    set_open.set(false);
    app.stabilize().unwrap();
    assert!(
        removed_ids(&ops_of(&sent)).contains(&em_id),
        "unmounted on false"
    );
}

/// The instance's own effects DIE with the instance (scope disposal).
#[test]
fn hide_disposes_the_interior_effects() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (open, set_open) = ctx.signal(true);
    let (count, set_count) = ctx.signal(0i64);

    let count_for_content = count.clone();
    let _tree = html! { ctx, receiver,
        <div>
            <Show when={open.get()}>{ Slot::lazy(move |c, r| {
                let count = count_for_content.clone();
                html! { c, r, <p>{count.get()}</p> }
            }) }</Show>
        </div>
    };
    app.stabilize().unwrap();

    set_count.set(1);
    app.stabilize().unwrap();
    let mid = ops_of(&sent).len();

    set_open.set(false);
    app.stabilize().unwrap();
    let after_hide = ops_of(&sent).len();

    set_count.set(2);
    app.stabilize().unwrap();
    assert_eq!(
        ops_of(&sent).len(),
        after_hide,
        "no SetText after hide — interior effect disposed with the scope"
    );
    assert!(mid < after_hide, "hide itself queued the removals");
}

/// Toggling content signals must NOT remount the region (untracked mount).
#[test]
fn content_signals_do_not_become_watcher_dependencies() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (open, _set_open) = ctx.signal(true);
    let (count, set_count) = ctx.signal(0i64);

    let count_for_content = count.clone();
    let _tree = html! { ctx, receiver,
        <div>
            <Show when={open.get()}>{ Slot::lazy(move |c, r| {
                let count = count_for_content.clone();
                html! { c, r, <p>{count.get()}</p> }
            }) }</Show>
        </div>
    };
    app.stabilize().unwrap();
    let mounts_before = created_tags(&ops_of(&sent))
        .iter()
        .filter(|t| *t == "p")
        .count();

    set_count.set(5);
    app.stabilize().unwrap();
    let mounts_after = created_tags(&ops_of(&sent))
        .iter()
        .filter(|t| *t == "p")
        .count();
    assert_eq!(mounts_before, mounts_after, "content change ≠ remount");
    assert!(ops_of(&sent)
        .iter()
        .any(|op| matches!(op, OperationT { ui: Ui::SetText(ref row), .. } if row.text.as_deref().unwrap_or_default() == "5")));
}

/// Re-show mounts a FRESH instance; static Html content works (Render).
#[test]
fn reshow_is_a_fresh_instance_and_static_content_renders() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (open, set_open) = ctx.signal(true);

    let _tree = html! { ctx, receiver,
        <div><Show when={open.get()}>{ html! { <b>"static"</b> } }</Show></div>
    };
    app.stabilize().unwrap();
    set_open.set(false);
    app.stabilize().unwrap();
    set_open.set(true);
    app.stabilize().unwrap();

    let mounts = created_tags(&ops_of(&sent))
        .iter()
        .filter(|t| *t == "b")
        .count();
    assert_eq!(mounts, 2, "two rising edges, two fresh instances");
}

// ─── <For> ────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct Todo {
    id: u32,
    label: &'static str,
}

fn todo_item(c: &Context, r: &SharedInstructionReceiver, t: &Todo) -> Html {
    let label = t.label;
    html! { c, r, <li>{label}</li> }
}

fn texts_in_order(ops: &[OperationT]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match op {
            OperationT {
                ui: Ui::SetText(ref row),
                ..
            } => Some(row.text.as_deref().unwrap_or_default().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn for_renders_appends_and_removes_without_moves() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (todos, set_todos) =
        ctx.signal(vec![Todo { id: 1, label: "a" }, Todo { id: 2, label: "b" }]);

    let _tree = html! { ctx, receiver,
        <ul>
            <For each={todos.get()} key={|t: &Todo| t.id} render={todo_item} />
        </ul>
    };
    app.stabilize().unwrap();
    let ops = ops_of(&sent);
    assert_eq!(
        created_tags(&ops).iter().filter(|t| *t == "li").count(),
        2,
        "initial items mounted"
    );
    assert_eq!(texts_in_order(&ops), vec!["a", "b"], "document order");

    // APPEND: no InsertBefore on EXISTING items (the no-move fast path) —
    // count the ops before and after.
    let moves_before = ops
        .iter()
        .filter(|op| {
            matches!(
                op,
                OperationT {
                    ui: Ui::InsertBefore(_),
                    ..
                }
            )
        })
        .count();
    set_todos.set(vec![
        Todo { id: 1, label: "a" },
        Todo { id: 2, label: "b" },
        Todo { id: 3, label: "c" },
    ]);
    app.stabilize().unwrap();
    let ops = ops_of(&sent);
    let new_moves = ops
        .iter()
        .filter(|op| {
            matches!(
                op,
                OperationT {
                    ui: Ui::InsertBefore(_),
                    ..
                }
            )
        })
        .count()
        - moves_before;
    assert_eq!(new_moves, 1, "ONE insert for the new item, zero moves");

    // REMOVE: one RemoveNode (the li), zero moves.
    set_todos.set(vec![Todo { id: 1, label: "a" }, Todo { id: 3, label: "c" }]);
    app.stabilize().unwrap();
    let ops = ops_of(&sent);
    assert!(!removed_ids(&ops).is_empty(), "removed item unmounted");
}

#[test]
fn for_reorders_with_insert_before() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (todos, set_todos) = ctx.signal(vec![
        Todo { id: 1, label: "a" },
        Todo { id: 2, label: "b" },
        Todo { id: 3, label: "c" },
    ]);

    let _tree = html! { ctx, receiver,
        <ul><For each={todos.get()} key={|t: &Todo| t.id} render={todo_item} /></ul>
    };
    app.stabilize().unwrap();
    let creates_before = created_tags(&ops_of(&sent)).len();

    set_todos.set(vec![
        Todo { id: 3, label: "c" },
        Todo { id: 1, label: "a" },
        Todo { id: 2, label: "b" },
    ]);
    app.stabilize().unwrap();
    let ops = ops_of(&sent);
    assert_eq!(
        created_tags(&ops).len(),
        creates_before,
        "reorder creates NOTHING (instances preserved)"
    );
    assert!(
        ops.iter()
            .filter(|op| matches!(
                op,
                OperationT {
                    ui: Ui::InsertBefore(_),
                    ..
                }
            ))
            .count()
            > 3,
        "order restored via InsertBefore moves"
    );
}

#[test]
fn for_empty_filled_empty_round_trip() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (todos, set_todos) = ctx.signal(Vec::<Todo>::new());

    let _tree = html! { ctx, receiver,
        <ul><For each={todos.get()} key={|t: &Todo| t.id} render={todo_item} /></ul>
    };
    app.stabilize().unwrap();

    set_todos.set(vec![Todo { id: 7, label: "x" }]);
    app.stabilize().unwrap();
    assert_eq!(
        created_tags(&ops_of(&sent))
            .iter()
            .filter(|t| *t == "li")
            .count(),
        1
    );

    set_todos.set(Vec::new());
    app.stabilize().unwrap();
    assert!(!removed_ids(&ops_of(&sent)).is_empty(), "emptied");

    set_todos.set(vec![Todo { id: 7, label: "x" }]);
    app.stabilize().unwrap();
    assert_eq!(
        created_tags(&ops_of(&sent))
            .iter()
            .filter(|t| *t == "li")
            .count(),
        2,
        "refill mounts a fresh instance"
    );
}

#[test]
fn remote_unmount_disposes_a_whole_fragment_once() {
    use std::{cell::Cell, rc::Rc};
    let (app, _) = App::mock();
    let receiver = app.receiver();
    let scope = app.ctx().child();
    let (value, set_value) = app.ctx().signal(0u32);
    let runs = Rc::new(Cell::new(0));
    let disposed = Rc::new(Cell::new(0));
    scope.effect({
        let runs = Rc::clone(&runs);
        move || {
            let _tracked = value.get();
            runs.set(runs.get() + 1);
        }
    });
    scope.on_dispose({
        let disposed = Rc::clone(&disposed);
        move || disposed.set(disposed.get() + 1)
    });
    receiver.register_scope(&[40, 41], &scope);
    receiver.dispose_mount(40);
    assert_eq!(disposed.get(), 0);
    receiver.dispose_mount(41);
    receiver.dispose_mount(41);
    set_value.set(3);
    app.stabilize().unwrap();
    drop(scope);
    assert_eq!(runs.get(), 1);
    assert_eq!(disposed.get(), 1);
}

#[test]
fn local_bridge_teardown_is_thread_local() {
    use foundation_wasm_ui::{
        events::ewe_ui_dispose_mount, install_app_bridge, uninstall_event_bridge,
    };
    use std::{cell::Cell, rc::Rc};
    let (app, _) = App::mock();
    install_app_bridge(&app);
    let scope = app.ctx().child();
    let removed = Rc::new(Cell::new(false));
    scope.on_dispose({
        let removed = Rc::clone(&removed);
        move || removed.set(true)
    });
    app.receiver().register_scope(&[42], &scope);
    std::thread::spawn(|| {
        let (other, _) = App::mock();
        install_app_bridge(&other);
        ewe_ui_dispose_mount(42);
        uninstall_event_bridge();
    })
    .join()
    .unwrap();
    assert!(!removed.get());
    ewe_ui_dispose_mount(42);
    assert!(removed.get());
    uninstall_event_bridge();
}

#[test]
fn either_selects_once_and_mounts_only_the_selected_branch() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let calls = std::cell::Cell::new(0);
    let (enabled, change) = ctx.signal(true);
    let _tree = html! { ctx, receiver,
        <section><Either condition={calls.set(calls.get() + 1); enabled.get()}>
            <Left><b/></Left>
            <Right><em/><strong/></Right>
        </Either><footer/></section>
    };
    app.stabilize().unwrap();
    assert_eq!(
        created_tags(&ops_of(&sent)),
        ["section", "em", "strong", "footer"]
    );
    change.set(false);
    app.stabilize().unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        created_tags(&ops_of(&sent)),
        ["section", "em", "strong", "footer"]
    );
}

#[test]
fn either_preserves_a_selected_show_subscription() {
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    let (visible, set_visible) = ctx.signal(false);
    let _tree = html! { ctx, receiver,
        <section><Either condition={true}><Left/><Right>
            <Show when={visible.get()}>{ Slot::lazy(|c, r| html! { c, r, <em>Ready</em> }) }</Show>
        </Right></Either></section>
    };
    app.stabilize().unwrap();
    assert!(!created_tags(&ops_of(&sent)).contains(&String::from("em")));
    set_visible.set(true);
    app.stabilize().unwrap();
    assert!(created_tags(&ops_of(&sent)).contains(&String::from("em")));
}
