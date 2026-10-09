//! WHY: `Context` clones are HANDLES to one scope (spec-42 feature 03 —
//! `App::context()` hands out an owned Context). Disposal must happen at the
//! LAST handle, and `Rc::strong_count` cannot decide it (a parent's children
//! list holds the inner `Rc` without being a handle) — so the explicit
//! handle count needs its own contract tests.
//!
//! WHAT: clone-keeps-alive, last-drop-disposes, and child disposal staying
//! independent of parent handle clones.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_signals::{Context, Runtime};

fn runs_counter(ctx: &Context, signal: &foundation_signals::SignalGetter<i64>) -> Rc<RefCell<u32>> {
    let runs = Rc::new(RefCell::new(0u32));
    let counter = Rc::clone(&runs);
    let getter = signal.clone();
    ctx.effect(move || {
        let _ = getter.get();
        *counter.borrow_mut() += 1;
    });
    runs
}

#[test]
fn clone_is_a_handle_not_a_new_scope() {
    let runtime = Rc::new(Runtime::new());
    let ctx = Context::new(Rc::clone(&runtime));
    let clone = ctx.clone();

    let (count, set_count) = ctx.signal(0i64);
    let runs = runs_counter(&clone, &count); // effect via the CLONE
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1);

    // Dropping ONE handle leaves the scope alive.
    drop(ctx);
    set_count.set(1);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 2, "scope survived the first handle's drop");

    // Dropping the LAST handle disposes it.
    drop(clone);
    set_count.set(2);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 2, "disposed at the last handle");
}

#[test]
fn child_disposal_is_unaffected_by_parent_clones() {
    let runtime = Rc::new(Runtime::new());
    let parent = Context::new(Rc::clone(&runtime));
    let _parent_clone = parent.clone();

    let child = parent.child();
    let (count, set_count) = child.signal(0i64);
    let runs = runs_counter(&child, &count);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1);

    // Child has ONE handle — dropping it disposes the child scope even
    // though the parent (and its clone) hold the inner Rc in `children`.
    drop(child);
    set_count.set(1);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1, "child disposed on its own last handle");
}

#[test]
fn parent_drop_still_disposes_children_recursively() {
    let runtime = Rc::new(Runtime::new());
    let parent = Context::new(Rc::clone(&runtime));
    let child = parent.child();

    let (count, set_count) = child.signal(0i64);
    let runs = runs_counter(&child, &count);
    runtime.stabilize();

    // Parent's LAST handle drops → child scope dies too, even while a child
    // handle is still alive (parent-owns-child, the documented nesting).
    drop(parent);
    set_count.set(1);
    runtime.stabilize();
    assert_eq!(*runs.borrow(), 1, "child effects died with the parent");
    drop(child); // late child-handle drop must be a safe no-op
}
