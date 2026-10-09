//! WHY: Feature 00's contract is exactly the three failure modes it fixes
//! (spec-42 features/00 §1) plus the three-lifetime model: fragments must
//! actually reach the DOM, recipes must be droppable/shareable, and both
//! `html!` forms must produce the same slot shape.
//!
//! WHAT: `<Fragment>` splicing (pure + reactive + owned values), the
//! `Render`/`Slot` surface with typed slot structs, span text slots on the
//! wire, `runtime_id`, `mount_fragment`'s registry semantics, and
//! `to_markup` parity.

use std::rc::Rc;

use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_ui_traits::{DomOp, Html, HtmlTag};
use foundation_wasm::MemoryAllocations;
use foundation_wasm_ui::{
    html, mount_fragment, MockProtocol, Render, Runtime, SharedInstructionReceiver, Slot,
};

struct Harness {
    signals: Rc<SignalsRuntime>,
    ctx: Context,
    sent: Rc<std::cell::RefCell<Vec<Vec<DomOp>>>>,
    receiver: SharedInstructionReceiver,
}

fn setup() -> Harness {
    let signals = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&signals));
    let mock = MockProtocol::new();
    let sent = mock.sent_batches();
    let runtime = Runtime::builder()
        .protocol(mock)
        .memory(MemoryAllocations::new())
        .build();
    runtime.attach(&signals);
    let receiver = runtime.receiver();
    Harness {
        signals,
        ctx,
        sent,
        receiver,
    }
}

fn all_ops(sent: &Rc<std::cell::RefCell<Vec<Vec<DomOp>>>>) -> Vec<DomOp> {
    sent.borrow().iter().flatten().cloned().collect()
}

fn created_tags(ops: &[DomOp]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match op {
            DomOp::CreateElement { tag, .. } => tag.name().map(String::from),
            _ => None,
        })
        .collect()
}

// ─── Failure mode 1: owned Html in a slot used to be E0507 ────────────────────

/// `<Fragment>` consumes owned values fine — it expands once at mount, not
/// into an `FnMut` effect.
#[test]
fn fragment_accepts_owned_html() {
    let h = setup();
    let fragment = html! { <b>"bold"</b> };
    let _tree = html! { h.ctx, h.receiver, <div><Fragment>{fragment}</Fragment></div> };
    h.signals.stabilize();

    let ops = all_ops(&h.sent);
    assert!(
        created_tags(&ops).contains(&String::from("b")),
        "owned fragment was BUILT, not dropped: {ops:?}"
    );
}

// ─── Failure mode 2: pure fragments were silently dropped (SetText("")) ──────

#[test]
fn pure_fragment_splices_with_fresh_ids() {
    let h = setup();
    let badge = html! { <span class="badge">"hi"</span> };
    let _tree = html! { h.ctx, h.receiver, <div class="card"><Fragment>{badge}</Fragment></div> };
    h.signals.stabilize();

    let ops = all_ops(&h.sent);
    // The badge's span + its text node were created, registered, appended.
    let span_id = ops
        .iter()
        .find_map(|op| match op {
            DomOp::CreateElement { node_id, tag, class }
                if tag.name() == Some("span") && class == "badge" => Some(*node_id),
            _ => None,
        })
        .expect("badge span created");
    assert!(ops
        .iter()
        .any(|op| matches!(op, DomOp::RegisterNode { node_id } if *node_id == span_id)));
    // Re-stamped primal-id matches the RUNTIME id, not the compile-time one.
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::SetAttribute { node_id, name, value }
            if *node_id == span_id
                && name.name() == Some("primal-id")
                && *value == span_id.to_string()
    )));
    // Appended under the card div (the parent), text under the span.
    let div_id = ops
        .iter()
        .find_map(|op| match op {
            DomOp::CreateElement { node_id, tag, .. } if tag.name() == Some("div") => {
                Some(*node_id)
            }
            _ => None,
        })
        .expect("parent div");
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::AppendChild { parent_id, child_id }
            if *parent_id == div_id && *child_id == span_id
    )));
}

// ─── Failure mode 3: reactive children were orphaned (never appended) ────────

#[test]
fn mounted_fragment_splices_by_reference() {
    let h = setup();
    let child = html! { h.ctx, h.receiver, <em>"child"</em> };
    let child_root = child.runtime_id.expect("reactive root carries its wire id");

    let _parent = html! { h.ctx, h.receiver, <section><Fragment>{child}</Fragment></section> };
    h.signals.stabilize();

    let ops = all_ops(&h.sent);
    // The child was built ONCE (no duplicate <em>)…
    assert_eq!(
        created_tags(&ops).iter().filter(|t| *t == "em").count(),
        1,
        "already-mounted fragment must not be rebuilt"
    );
    // …and spliced under the section by id.
    let section_id = ops
        .iter()
        .find_map(|op| match op {
            DomOp::CreateElement { node_id, tag, .. } if tag.name() == Some("section") => {
                Some(*node_id)
            }
            _ => None,
        })
        .expect("section");
    assert!(
        ops.iter().any(|op| matches!(
            op,
            DomOp::AppendChild { parent_id, child_id }
                if *parent_id == section_id && *child_id == child_root
        )),
        "child not orphaned — AppendChild(section, child_root) present"
    );
    // Its interior stays live: effects belong to ctx, not to the value.
}

/// Interior reactivity of a spliced fragment keeps updating after the splice.
#[test]
fn spliced_fragment_interior_stays_reactive() {
    let h = setup();
    let (count, set_count) = h.ctx.signal(1i64);
    let child = html! { h.ctx, h.receiver, <em>{count.get()}</em> };
    let _parent = html! { h.ctx, h.receiver, <section><Fragment>{child}</Fragment></section> };
    h.signals.stabilize();

    set_count.set(2);
    h.signals.stabilize();
    let ops = all_ops(&h.sent);
    let last_set_text = ops
        .iter()
        .rev()
        .find_map(|op| match op {
            DomOp::SetText { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .expect("slot SetText");
    assert_eq!(last_set_text, "2", "interior effect re-rendered after splice");
}

// ─── Span text slots on the wire (feature 00 §4) ─────────────────────────────

#[test]
fn reactive_text_slot_is_an_id_bearing_span() {
    let h = setup();
    let (count, _set) = h.ctx.signal(7i64);
    let _tree = html! { h.ctx, h.receiver, <div>{count.get()}</div> };
    h.signals.stabilize();

    let ops = all_ops(&h.sent);
    // div=base, slot span=base+1.
    let span_id = ops
        .iter()
        .find_map(|op| match op {
            DomOp::CreateElement { node_id, tag, .. } if tag.name() == Some("span") => {
                Some(*node_id)
            }
            _ => None,
        })
        .expect("slot renders as a span ELEMENT, not a bare text node");
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::SetAttribute { node_id, name, .. }
            if *node_id == span_id && name.name() == Some("primal-id")
    )));
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::SetText { node_id, text } if *node_id == span_id && text == "7"
    )));
}

// ─── Render / Slot / typed slot structs (feature 00 §2-3) ────────────────────

struct CardSlots {
    title: Slot,
    badge: Option<Slot>,
    children: Vec<Slot>,
}

fn card(ctx: &Context, rcv: &SharedInstructionReceiver, slots: &CardSlots) -> Html {
    let title = slots.title.render(ctx, rcv);
    let badge = slots
        .badge
        .as_ref()
        .map(|b| b.render(ctx, rcv))
        .unwrap_or_default();
    let children: Vec<Html> = slots.children.iter().map(|s| s.render(ctx, rcv)).collect();
    html! { ctx, rcv,
        <div class="card">
            <Fragment>{title}</Fragment>
            <Fragment>{badge}</Fragment>
            <ul><Fragment>{children}</Fragment></ul>
        </div>
    }
}

#[test]
fn typed_slot_struct_composes_required_optional_children() {
    let h = setup();
    let slots = CardSlots {
        title: html! { <h2>"Title"</h2> }.into(),
        badge: None,
        children: vec![
            html! { <li>"a"</li> }.into(),
            Slot::lazy(|ctx, rcv| html! { ctx, rcv, <li>"b"</li> }),
        ],
    };
    let _tree = card(&h.ctx, &h.receiver, &slots);
    h.signals.stabilize();

    let tags = created_tags(&all_ops(&h.sent));
    assert!(tags.contains(&String::from("h2")), "required slot mounted");
    assert_eq!(
        tags.iter().filter(|t| *t == "li").count(),
        2,
        "static AND lazy children both mounted: {tags:?}"
    );
}

/// The recipe/instance split: dropping the Slot after render leaves the
/// mounted instance fully functional; rendering twice mints two instances.
#[test]
fn slots_are_reusable_recipes() {
    let h = setup();
    let (count, set_count) = h.ctx.signal(0i64);
    let recipe = Slot::lazy({
        let count = count.clone();
        move |ctx, rcv| {
            let count = count.clone();
            html! { ctx, rcv, <p>{count.get()}</p> }
        }
    });

    let first = recipe.render(&h.ctx, &h.receiver);
    let second = recipe.render(&h.ctx, &h.receiver);
    assert_ne!(
        first.runtime_id, second.runtime_id,
        "each render is a fresh instance with its own id block"
    );
    drop(recipe); // the recipe's lifetime is independent of the instances

    set_count.set(5);
    h.signals.stabilize();
    let ops = all_ops(&h.sent);
    let renders = ops
        .iter()
        .filter(|op| matches!(op, DomOp::SetText { text, .. } if text == "5"))
        .count();
    assert_eq!(renders, 2, "both instances updated after the recipe died");
}

/// Struct components implement `Render` directly and erase via `into_slot`.
#[test]
fn struct_components_implement_render() {
    struct Badge {
        label: &'static str,
    }
    impl Render for Badge {
        fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
            let label = self.label;
            html! { ctx, rcv, <span class="badge">{label}</span> }
        }
    }

    let h = setup();
    let slot = Badge { label: "rust" }.into_slot();
    let mounted = slot.render(&h.ctx, &h.receiver);
    h.signals.stabilize();

    assert!(mounted.runtime_id.is_some());
    let ops = all_ops(&h.sent);
    assert!(ops
        .iter()
        .any(|op| matches!(op, DomOp::SetText { text, .. } if text == "rust")));
}

// ─── mount_fragment directly ──────────────────────────────────────────────────

#[test]
fn mount_fragment_handles_grouping_nodes() {
    let h = setup();
    let items = vec![html! { <li>"x"</li> }, html! { <li>"y"</li> }];
    let wrapper = Html {
        children: items,
        ..Html::new()
    };
    let root = mount_fragment(&h.ctx, &h.receiver, wrapper, 1 /* body */);
    assert_eq!(root, 1, "tagless multi-child fragments have no single root");
    h.signals.stabilize();
    let ops = all_ops(&h.sent);
    assert_eq!(
        created_tags(&ops).iter().filter(|t| *t == "li").count(),
        2,
        "grouping node children splice directly into the parent"
    );
}

// ─── to_markup parity (feature 00 §5) ─────────────────────────────────────────

#[test]
fn to_markup_serializes_with_escaping_and_voids() {
    let name = "a < b & \"c\"";
    let tree = html! {
        <div class="x">
            <img src="pic.png" />
            {name}
        </div>
    };
    let markup = tree.to_markup();
    assert_eq!(
        markup,
        "<div primal-id=\"0\" class=\"x\">\
         <img primal-id=\"1\" src=\"pic.png\">\
         <span>a &lt; b &amp; \"c\"</span>\
         </div>"
    );
}

#[test]
fn to_markup_matches_reactive_slot_shape() {
    // The morph contract: pure markup and the reactive op stream produce the
    // same STRUCTURE at slot positions — a span wrapping the text.
    let pure = html! { <div>{42u32}</div> };
    assert!(pure.to_markup().contains("<span>42</span>"));

    let h = setup();
    let (n, _setter) = h.ctx.signal(42u32);
    let _live = html! { h.ctx, h.receiver, <div>{n.get()}</div> };
    h.signals.stabilize();
    let ops = all_ops(&h.sent);
    assert!(
        created_tags(&ops).contains(&String::from("span")),
        "reactive slot is the same span element"
    );
}

#[test]
fn fragment_in_pure_form_inlines() {
    let badge = html! { <b>"hot"</b> };
    let tree = html! { <div><Fragment>{badge}</Fragment></div> };
    assert!(tree.to_markup().contains("<b primal-id=\"0\">hot</b>"));
    assert_eq!(
        tree.children[0].tag, None,
        "Fragment is a transparent grouping node in the pure tree"
    );
    assert_eq!(tree.children[0].children[0].tag, Some(HtmlTag::from_static("b")));
}
