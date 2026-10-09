//! WHY: Function composition needs a way for a component to accept content
//! it places itself — with the content's effects owned by the PLACING
//! component's scope, while the recipe stays shareable and droppable
//! (spec-42 feature 00 §2-3: the three-lifetime model).
//!
//! WHAT: [`Render`] ("given a context and receiver, I present my html"),
//! [`Slot`] (the type-erased field type for per-component slot structs), and
//! [`mount_fragment`] (the runtime behind `<Fragment>`: splice an
//! already-mounted fragment by reference, or build a pure fragment with a
//! fresh id block).
//!
//! HOW: `Render` takes `&self` — object-safe, re-invokable (each call mounts
//! a FRESH instance), implementations hand out clones. `Slot` erases through
//! `Box<dyn Render>`; closures enter via [`Slot::lazy`] (they cannot
//! blanket-implement `Render` — it would overlap `impl Render for Html`,
//! E0119). `mount_fragment` walks pure trees in document order with ids from
//! `Context::allocate_id_block`, mirroring the macro's own build ops.

use alloc::boxed::Box;

use foundation_signals::Context;
use foundation_ui_traits::{AttrName, DomOp, Html, IntoHtml};

use crate::runtime::SharedInstructionReceiver;

/// A nameable component: "given a context and receiver, I present my html;
/// what happens to me afterwards is my owner's business."
///
/// Each `render` call mounts a FRESH instance (own id block, own effects,
/// owned by the `ctx` passed in). Mount-once is a convention, not a
/// compiler guarantee — rendering twice is DEFINED behavior (two
/// independent instances), the same as calling a component function twice.
pub trait Render {
    fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html;

    /// Erase into a [`Slot`] (the slot-struct field type).
    fn into_slot(self) -> Slot
    where
        Self: Sized + 'static,
    {
        Slot(Box::new(self))
    }
}

/// Static content lifts free: rendering an `Html` hands out a clone and
/// ignores the context (no effects — nothing to own).
impl Render for Html {
    fn render(&self, _ctx: &Context, _rcv: &SharedInstructionReceiver) -> Html {
        self.clone()
    }
}

/// The type-erased slot value — the ONE type per-component slot structs
/// name: required = `Slot`, optional = `Option<Slot>`, children =
/// `Vec<Slot>` (spec-42 feature 00 §3).
pub struct Slot(Box<dyn Render>);

impl Slot {
    /// Closure entry — `Slot::lazy(|ctx, rcv| nav_menu(ctx, rcv))`.
    ///
    /// Closures wrap here instead of implementing [`Render`] directly
    /// (coherence: a blanket `Fn` impl would overlap `impl Render for
    /// Html`). `Fn`, not `FnOnce`: slots are re-invokable templates;
    /// closures clone from their captures into each output.
    pub fn lazy(
        f: impl Fn(&Context, &SharedInstructionReceiver) -> Html + 'static,
    ) -> Self {
        struct FnRender<F>(F);
        impl<F> Render for FnRender<F>
        where
            F: Fn(&Context, &SharedInstructionReceiver) -> Html,
        {
            fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
                (self.0)(ctx, rcv)
            }
        }
        Slot(Box::new(FnRender(f)))
    }

    /// Render this slot's content. Effects created inside register under
    /// `ctx` — content lifecycle follows PLACEMENT, not the recipe.
    #[must_use]
    pub fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
        self.0.render(ctx, rcv)
    }
}

impl From<Html> for Slot {
    fn from(html: Html) -> Self {
        html.into_slot()
    }
}

/// Delegation — a `Slot` is itself a `Render`, so slot groups nest.
impl Render for Slot {
    fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
        self.0.render(ctx, rcv)
    }
}

impl core::fmt::Debug for Slot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Slot(dyn Render)")
    }
}

/// Splice a fragment under `parent_id` — the runtime behind `<Fragment>`
/// (spec-42 feature 00 §6-7).
///
/// - A fragment mounted by the reactive `html!` form (`runtime_id` set) is
///   already built and registered, DETACHED — it splices with a single
///   `AppendChild`.
/// - A pure fragment is BUILT here: ids from a fresh
///   [`Context::allocate_id_block`] block, create/register/append ops in
///   document order — the same registry semantics the macro emits. Its
///   compile-time `primal-id` attributes are REPLACED by the runtime ids
///   (the registry is the source of truth). Its `parts` are inert (no
///   effects — reactivity needs the reactive form).
/// - Tagless grouping nodes (the `Vec<Html>`/`<Fragment>` wrapper shape)
///   contribute children only.
///
/// Returns the spliced subtree's root id — for tagless multi-child
/// fragments, `parent_id` (there is no single root).
// By-value `fragment` is deliberate API: splicing consumes the value's
// identity (an already-mounted tree must not be spliced twice), even though
// the walk only reads it.
#[allow(clippy::needless_pass_by_value)]
#[must_use = "the returned root id is how callers later detach the fragment"]
pub fn mount_fragment(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    fragment: Html,
    parent_id: u32,
) -> u32 {
    if let Some(root) = fragment.runtime_id {
        rcv.queue(DomOp::AppendChild {
            parent_id,
            child_id: root,
        });
        return root;
    }

    let count = count_nodes(&fragment);
    if count == 0 {
        return parent_id;
    }
    let base = ctx.allocate_id_block(count);
    let mut next = base;
    build_subtree(&fragment, parent_id, Attach::Append, rcv, &mut next, None);

    // A single-root fragment's root got the first id; tagless multi-child
    // wrappers have no single root.
    if fragment.tag.is_some() || fragment.is_text() {
        base
    } else {
        parent_id
    }
}

/// Splice `fragment` under `parent_id` BEFORE `ref_id`, returning EVERY
/// top-level spliced id (spec-42 feature 01: `<Show>`/`<For>` track them to
/// unmount multi-root content correctly). Same already-mounted /
/// pure-build / grouping semantics as [`mount_fragment`].
#[allow(clippy::needless_pass_by_value)] // same identity argument as mount_fragment
#[must_use = "the returned ids are how the fragment is later unmounted"]
pub fn mount_before(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    fragment: Html,
    parent_id: u32,
    ref_id: u32,
) -> alloc::vec::Vec<u32> {
    let mut top_level = alloc::vec::Vec::new();
    if fragment.runtime_id.is_none() {
        let count = count_nodes(&fragment);
        if count == 0 {
            return top_level;
        }
        let base = ctx.allocate_id_block(count);
        let mut next = base;
        build_subtree(
            &fragment,
            parent_id,
            Attach::Before(ref_id),
            rcv,
            &mut next,
            Some(&mut top_level),
        );
        return top_level;
    }
    build_subtree(
        &fragment,
        parent_id,
        Attach::Before(ref_id),
        rcv,
        &mut 0,
        Some(&mut top_level),
    );
    top_level
}

/// How a node joins its parent: appended at the end, or inserted before a
/// reference sibling (anchored regions).
#[derive(Clone, Copy)]
enum Attach {
    Append,
    Before(u32),
}

fn attach_op(parent_id: u32, child_id: u32, attach: Attach) -> DomOp {
    match attach {
        Attach::Append => DomOp::AppendChild {
            parent_id,
            child_id,
        },
        Attach::Before(ref_id) => DomOp::InsertBefore {
            parent_id,
            child_id,
            ref_id,
        },
    }
}

/// Ids needed to build `node` (elements + text nodes; grouping nodes are
/// transparent).
fn count_nodes(node: &Html) -> u32 {
    if node.runtime_id.is_some() {
        return 0; // splices by reference — builds nothing
    }
    let own = u32::from(node.tag.is_some() || node.is_text());
    node.children.iter().map(count_nodes).sum::<u32>() + own
}

fn build_subtree(
    node: &Html,
    parent_id: u32,
    attach: Attach,
    rcv: &SharedInstructionReceiver,
    next: &mut u32,
    mut top_level: Option<&mut alloc::vec::Vec<u32>>,
) {
    // Already-mounted fragments splice by reference at ANY depth (e.g.
    // inside a Vec<Html> grouping wrapper) — rebuilding would duplicate
    // their live nodes.
    if let Some(root) = node.runtime_id {
        rcv.queue(attach_op(parent_id, root, attach));
        if let Some(ids) = top_level.as_deref_mut() {
            ids.push(root);
        }
        return;
    }

    if let Some(tag) = &node.tag {
        let id = *next;
        *next += 1;

        // class rides CreateElement (op 0's value column); primal-id is
        // re-stamped with the runtime id.
        let class = node
            .attributes
            .iter()
            .find(|(name, _)| name.name() == Some("class"))
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        rcv.queue(DomOp::CreateElement {
            node_id: id,
            tag: tag.clone(),
            class,
        });
        rcv.queue(DomOp::RegisterNode { node_id: id });
        rcv.queue(DomOp::SetAttribute {
            node_id: id,
            name: AttrName::from_static("primal-id"),
            value: alloc::string::ToString::to_string(&id).into(),
        });
        for (name, value) in &node.attributes {
            if matches!(name.name(), Some("class" | "primal-id")) {
                continue;
            }
            rcv.queue(DomOp::SetAttribute {
                node_id: id,
                name: name.clone(),
                value: value.clone(),
            });
        }
        if let Some(text) = &node.text {
            rcv.queue(DomOp::SetText {
                node_id: id,
                text: text.clone(),
            });
        }
        for child in &node.children {
            build_subtree(child, id, Attach::Append, rcv, next, None);
        }
        rcv.queue(attach_op(parent_id, id, attach));
        if let Some(ids) = top_level.as_deref_mut() {
            ids.push(id);
        }
        return;
    }

    if let Some(text) = &node.text {
        let id = *next;
        *next += 1;
        rcv.queue(DomOp::CreateTextNode {
            node_id: id,
            content: text.clone(),
        });
        rcv.queue(DomOp::RegisterNode { node_id: id });
        rcv.queue(attach_op(parent_id, id, attach));
        if let Some(ids) = top_level.as_deref_mut() {
            ids.push(id);
        }
        return;
    }

    // Grouping node: children splice directly into the parent, each one a
    // TOP-LEVEL node of this fragment.
    for child in &node.children {
        build_subtree(child, parent_id, attach, rcv, next, top_level.as_deref_mut());
    }
}

/// Build a fragment from anything `IntoHtml` and splice it — convenience
/// over [`mount_fragment`] mirroring the macro's `<Fragment>` expansion.
pub fn mount_into(
    ctx: &Context,
    rcv: &SharedInstructionReceiver,
    value: impl IntoHtml,
    parent_id: u32,
) -> u32 {
    mount_fragment(ctx, rcv, value.into_html(), parent_id)
}
