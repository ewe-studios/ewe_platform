//! WHY: Feature 00 fixed fragment PLACEMENT at mount; real UIs also need
//! the SET of mounted fragments to follow signals — conditionals and keyed
//! lists (spec-42 feature 01; the F5/F6 catalog families block on this).
//!
//! WHAT: [`mount_show`] and [`mount_for`] — the runtime behind the `<Show>`
//! and `<For>` built-ins. Each owns an anchored region (an empty
//! id-bearing `<span>` marking the region's END; content inserts before
//! it), mounts instances under fresh child scopes, and unmounts with
//! `RemoveNode` + scope drop.
//!
//! HOW: One watcher effect per primitive. The condition/items expression is
//! the TRACKED read; all mounting runs inside `Runtime::untracked` (tracked
//! evaluations never nest, and content signals must not become watcher
//! dependencies — toggling a counter inside `<Show>` must re-render the
//! counter's slot, not remount the whole region).

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use foundation_signals::Context;
use foundation_ui_traits::{AttrName, DomOp, Html, HtmlTag};

use crate::runtime::SharedInstructionReceiver;
use crate::slots::{mount_before, Render};

/// A mounted `<Show>` instance: its tracked top-level ids + owning scope.
type ShowInstance = Option<(Vec<u32>, Context)>;

/// Create the region's end anchor: an empty registered `<span primal-id>`.
fn create_anchor(ctx: &Context, rcv: &SharedInstructionReceiver, parent_id: u32) -> u32 {
    let id = ctx.allocate_id_block(1);
    rcv.queue(DomOp::CreateElement {
        node_id: id,
        tag: HtmlTag::from_static("span"),
        class: "".into(),
    });
    rcv.queue(DomOp::RegisterNode { node_id: id });
    rcv.queue(DomOp::SetAttribute {
        node_id: id,
        name: AttrName::from_static("primal-id"),
        value: alloc::string::ToString::to_string(&id).into(),
    });
    rcv.queue(DomOp::AppendChild {
        parent_id,
        child_id: id,
    });
    id
}

fn unmount(rcv: &SharedInstructionReceiver, ids: &[u32]) {
    for id in ids {
        rcv.queue(DomOp::RemoveNode { node_id: *id });
    }
}

/// The `<Show>` runtime: mount `content` while `when()` holds, unmount when
/// it stops. Each rising edge mounts a FRESH instance (recipe semantics)
/// under its own child scope; the falling edge removes every tracked
/// top-level node and drops the scope (disposing the instance's effects).
pub fn mount_show(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    parent_id: u32,
    when: impl Fn() -> bool + 'static,
    content: impl Render + 'static,
) {
    let anchor = create_anchor(ctx, rcv, parent_id);
    let state: Rc<RefCell<ShowInstance>> = Rc::new(RefCell::new(None));

    let scope_parent = ctx.clone();
    let rcv = rcv.clone();
    ctx.effect(move || {
        let want = when(); // the watcher's ONLY tracked read
        scope_parent.untracked(|| {
            let mut state = state.borrow_mut();
            if want && state.is_none() {
                let scope = scope_parent.child();
                let html = content.render(&scope, &rcv);
                let ids = mount_before(&scope, &rcv, html, parent_id, anchor);
                *state = Some((ids, scope));
            } else if !want {
                if let Some((ids, scope)) = state.take() {
                    unmount(&rcv, &ids);
                    drop(scope); // disposes the instance's effects
                }
            }
        });
    });
}

/// One mounted list item.
struct Entry<K> {
    key: K,
    ids: Vec<u32>,
    _scope: Context, // held for ownership; dropping unmounts the effects
}

/// The `<For>` runtime: a keyed list over `each()`.
///
/// Reconciliation per run: removed keys unmount (ops + scope drop); when
/// the KEPT keys' relative order is unchanged (append/remove/update — the
/// common case) NO move ops are emitted; otherwise an end→start
/// `InsertBefore` pass restores order (correct, occasionally over-moves —
/// an LIS minimal-move pass is documented future work). New keys mount at
/// their position. Keys identify INSTANCES: a changed item with the same
/// key does NOT re-render — changing data belongs in signals the item's
/// content reads.
pub fn mount_for<T, K>(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    parent_id: u32,
    each: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> K + 'static,
    render: impl Fn(&Context, &SharedInstructionReceiver, &T) -> Html + 'static,
) where
    T: 'static,
    K: PartialEq + Clone + core::fmt::Debug + 'static,
{
    let anchor = create_anchor(ctx, rcv, parent_id);
    let entries: Rc<RefCell<Vec<Entry<K>>>> = Rc::new(RefCell::new(Vec::new()));

    let scope_parent = ctx.clone();
    let rcv = rcv.clone();
    ctx.effect(move || {
        let items = each(); // the watcher's ONLY tracked read
        scope_parent.untracked(|| {
            let mut entries = entries.borrow_mut();
            let new_keys: Vec<K> = items.iter().map(&key).collect();
            for (i, k) in new_keys.iter().enumerate() {
                if new_keys[..i].contains(k) {
                    tracing::error!("<For>: duplicate key {k:?} — last instance wins");
                }
            }

            // 1. Unmount removed keys.
            entries.retain(|entry| {
                let keep = new_keys.contains(&entry.key);
                if !keep {
                    unmount(&rcv, &entry.ids);
                }
                keep
            });

            // 2. Detect whether the kept keys' relative order changed.
            let kept_old_order: Vec<&K> = entries.iter().map(|e| &e.key).collect();
            let kept_new_order: Vec<&K> = new_keys
                .iter()
                .filter(|k| kept_old_order.contains(k))
                .collect();
            let order_changed = kept_old_order != kept_new_order;

            // 3. Mount NEW items in FORWARD order (renders and their initial
            //    effects run in document order): each inserts before the
            //    next KEPT item's first id, else the anchor — consecutive
            //    new items before one ref keep their relative order.
            let mut new_entries: Vec<Option<Entry<K>>> =
                items.iter().map(|_| None).collect();
            for (i, item) in items.iter().enumerate() {
                let k = &new_keys[i];
                if let Some(at) = entries.iter().position(|e| &e.key == k) {
                    new_entries[i] = Some(entries.remove(at));
                    continue;
                }
                let ref_id = new_keys[i + 1..]
                    .iter()
                    .find_map(|later| {
                        entries
                            .iter()
                            .find(|e| &e.key == later)
                            .and_then(|e| e.ids.first().copied())
                    })
                    .unwrap_or(anchor);
                let scope = scope_parent.child();
                let html = render(&scope, &rcv, item);
                let ids = mount_before(&scope, &rcv, html, parent_id, ref_id);
                new_entries[i] = Some(Entry {
                    key: k.clone(),
                    ids,
                    _scope: scope,
                });
            }
            let new_entries: Vec<Entry<K>> = new_entries.into_iter().flatten().collect();

            // 4. Positioning pass (ONLY when kept order changed): end→start
            //    InsertBefore restores document order; the common
            //    append/remove/update path emits zero moves.
            if order_changed {
                let mut next_ref = anchor;
                for entry in new_entries.iter().rev() {
                    for id in entry.ids.iter().rev() {
                        rcv.queue(DomOp::InsertBefore {
                            parent_id,
                            child_id: *id,
                            ref_id: next_ref,
                        });
                    }
                    if let Some(first) = entry.ids.first() {
                        next_ref = *first;
                    }
                }
            }

            *entries = new_entries;
        });
    });
}
