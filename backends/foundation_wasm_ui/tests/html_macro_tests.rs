//! WHY: `html!` is the user-facing surface of the whole UI stack — its id
//! assignment, `Part` emission, setter detection, and reactive wiring are
//! contracts every component will lean on (feature 03 section 14).
//!
//! WHAT: Spec tests 1-34 (parsing/codegen, ids, parts, setter codegen,
//! `IntoHtml` integration) against the PURE form, and 40-42 (mount
//! integration) against the REACTIVE form with a `MockProtocol` receiver.
//! Compile-error diagnostics (35-39) are unit-tested inside the parser
//! (proc-macro crates can't export test hooks; see parser.rs).
//!
//! HOW: Pure-form trees are inspected structurally (typed `Html`); reactive
//! runs assert the exact `DomOp` stream the mock captured.

use std::rc::Rc;

use foundation_signals::{Context, EventData, Runtime as SignalsRuntime};
use foundation_ui_traits::{AttrName, DomOp, Html, HtmlTag, Part};
use foundation_wasm::MemoryAllocations;
use foundation_wasm_ui::{html, MaybeCallback, MockProtocol, Runtime};

/// Attribute value by name (resolving known-id wire forms).
fn attr<'a>(html: &'a Html, name: &str) -> Option<&'a str> {
    html.attributes
        .iter()
        .find(|(n, _)| n.name() == Some(name))
        .map(|(_, v)| v.as_ref())
}

fn tag_name(html: &Html) -> Option<&str> {
    html.tag.as_ref().and_then(HtmlTag::name)
}

// ─── Parsing & codegen (tests 1-12) ────────────────────────────────────────────

/// Test 1.
#[test]
fn empty_div() {
    let h = html! { <div></div> };
    assert_eq!(tag_name(&h), Some("div"));
    assert!(h.children.is_empty());
    assert_eq!(attr(&h, "primal-id"), Some("0"));
}

/// Tests 2 + 11.
#[test]
fn static_attributes_and_user_id_coexist() {
    let h = html! { <div id="user" class="x"></div> };
    assert_eq!(attr(&h, "class"), Some("x"));
    assert_eq!(attr(&h, "id"), Some("user"));
    assert_eq!(attr(&h, "primal-id"), Some("0"));
}

/// Test 3.
#[test]
fn void_element_without_slash() {
    let h = html! { <br> };
    assert_eq!(tag_name(&h), Some("br"));
    assert_eq!(attr(&h, "primal-id"), Some("0"));
}

/// Tests 4 + 13 + 14 + 15 + 16 — depth-first element ids.
#[test]
fn primal_ids_are_depth_first() {
    let h = html! { <div><span><a></a></span><p></p></div> };
    assert_eq!(attr(&h, "primal-id"), Some("0"));
    assert_eq!(attr(&h.children[0], "primal-id"), Some("1")); // span
    assert_eq!(attr(&h.children[0].children[0], "primal-id"), Some("2")); // a
    assert_eq!(attr(&h.children[1], "primal-id"), Some("3")); // p

    let list = html! { <ul><li></li><li></li><li></li></ul> };
    let ids: Vec<_> = list
        .children
        .iter()
        .map(|c| attr(c, "primal-id").unwrap().to_string())
        .collect();
    assert_eq!(ids, ["1", "2", "3"]);

    let voids = html! { <div><img /><br><hr></div> };
    let ids: Vec<_> = voids
        .children
        .iter()
        .map(|c| attr(c, "primal-id").unwrap().to_string())
        .collect();
    assert_eq!(ids, ["1", "2", "3"]);
}

/// Test 5.
#[test]
fn child_slot_emits_text_part() {
    let name = "world";
    let h = html! { <div>{name}</div> };
    assert_eq!(h.parts.len(), 1);
    assert!(matches!(&h.parts[0], Part::Text(t) if t.node_id == 0));
    // Text-shaped slot values render as the slot <span> (feature-00 §4,
    // morph parity with the reactive form) wrapping the text child.
    let slot_span = &h.children[0];
    assert_eq!(tag_name(slot_span), Some("span"));
    assert_eq!(slot_span.children[0].text.as_deref(), Some("world"));
}

/// Test 6.
#[test]
fn dynamic_attribute_emits_attr_part() {
    let cls = "highlight";
    let h = html! { <div class={cls}></div> };
    assert!(
        matches!(&h.parts[0], Part::Attribute(a) if a.node_id == 0 && a.attr_name == "class")
    );
    assert_eq!(attr(&h, "class"), Some("highlight"));
}

/// Test 7.
#[test]
fn event_attribute_emits_event_part() {
    let handler = |_: &EventData| {};
    let h = html! { <button primal:onclick={handler}></button> };
    assert!(
        matches!(&h.parts[0], Part::Event(e) if e.node_id == 0 && e.event_name == "click")
    );
    assert_eq!(attr(&h, "primal:onclick"), Some("true"));
}

/// Tests 8 + 26 + 29 — setter detection injects primal:setter.
#[test]
fn setter_handler_injects_primal_setter() {
    let runtime = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&runtime));
    let (_name, set_name) = ctx.signal(String::new());
    let expected = set_name.callback_id().to_string();

    let h = html! { <input primal:onchange={set_name} /> };
    assert_eq!(attr(&h, "primal:setter"), Some(expected.as_str()));
    assert_eq!(attr(&h, "primal:onchange"), Some("true"));
}

/// Test 9.
#[test]
fn two_slots_same_parent() {
    let (a, b) = ("x", "y");
    let h = html! { <div>{a}{b}</div> };
    let text_parts: Vec<u32> = h
        .parts
        .iter()
        .filter_map(|p| match p {
            Part::Text(t) => Some(t.node_id),
            _ => None,
        })
        .collect();
    assert_eq!(text_parts, [0, 0]);
}

/// Tests 10 + 25 — part node ids and document order.
#[test]
#[allow(clippy::many_single_char_names)] // slot vars mirror the spec table's a/b/c
fn parts_follow_document_order() {
    let (a, b, c) = (1, 2, 3);
    let h = html! { <div>{a}<span>{b}</span>{c}</div> };
    let ids: Vec<u32> = h
        .parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.node_id,
            Part::Attribute(at) => at.node_id,
            Part::Event(e) => e.node_id,
        })
        .collect();
    assert_eq!(ids, [0, 1, 0], "spec test 25: [Text(0), Text(1), Text(0)]");

    let (x, y) = ("x", "y");
    let h = html! { <div><span>{x}</span><p>{y}</p></div> };
    let ids: Vec<u32> = h
        .parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.node_id,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(ids, [1, 2]);
}

/// Test 12.
#[test]
fn bare_boolean_attribute() {
    let h = html! { <input disabled /> };
    assert_eq!(attr(&h, "disabled"), Some("true"));
}

// ─── Part collection (tests 22-24) ─────────────────────────────────────────────

/// Test 22 — attribute part before child text part.
#[test]
fn attr_part_precedes_child_slot() {
    let (c, x) = ("cls", "content");
    let h = html! { <div class={c}>{x}</div> };
    assert!(matches!(&h.parts[0], Part::Attribute(a) if a.node_id == 0));
    assert!(matches!(&h.parts[1], Part::Text(t) if t.node_id == 0));
}

/// Test 23 — event parts on nested elements.
#[test]
fn event_parts_on_nested_elements() {
    let (s, h2) = (|_: &EventData| {}, |_: &EventData| {});
    let h = html! { <form><input primal:onchange={s}/><button primal:onclick={h2}></button></form> };
    assert!(matches!(&h.parts[0], Part::Event(e) if e.node_id == 1 && e.event_name == "change"));
    assert!(matches!(&h.parts[1], Part::Event(e) if e.node_id == 2 && e.event_name == "click"));
}

/// Test 24 — static-only template has no parts.
#[test]
fn static_template_has_no_parts() {
    let h = html! { <div class="x">"hello"</div> };
    assert!(h.parts.is_empty());
    assert_eq!(h.children[0].text.as_deref(), Some("hello"));
}

// ─── Setter codegen (tests 27-28) ──────────────────────────────────────────────

/// Test 27 — closures yield no primal:setter.
#[test]
fn closure_handler_has_no_setter_attr() {
    let handler = |_: &EventData| {};
    assert_eq!(MaybeCallback::maybe_callback_id(&handler), None);
    let h = html! { <button primal:onclick={handler}></button> };
    assert_eq!(attr(&h, "primal:setter"), None);
}

/// Test 28 — two setters get distinct ids.
#[test]
fn two_setters_distinct_callback_ids() {
    let runtime = Rc::new(SignalsRuntime::new());
    let ctx = Context::new(Rc::clone(&runtime));
    let (_a, set_a) = ctx.signal(String::new());
    let (_b, set_b) = ctx.signal(String::new());
    let (id_a, id_b) = (set_a.callback_id(), set_b.callback_id());

    let h = html! { <form><input primal:onchange={set_a}/><input primal:oninput={set_b}/></form> };
    assert_eq!(
        attr(&h.children[0], "primal:setter"),
        Some(id_a.to_string().as_str())
    );
    assert_eq!(
        attr(&h.children[1], "primal:setter"),
        Some(id_b.to_string().as_str())
    );
}

// ─── IntoHtml integration (tests 30-34) ────────────────────────────────────────

/// Tests 30 + 31.
#[test]
fn primitive_slots_become_text_nodes() {
    let h = html! { <div>{42u32}</div> };
    assert_eq!(tag_name(&h.children[0]), Some("span"));
    assert_eq!(h.children[0].children[0].text.as_deref(), Some("42"));

    let h = html! { <div>{"hello"}</div> };
    assert_eq!(h.children[0].children[0].text.as_deref(), Some("hello"));
}

/// Tests 32 + 33.
#[test]
fn option_slots() {
    let some = Some(html! { <b></b> });
    let h = html! { <div>{some}</div> };
    assert_eq!(tag_name(&h.children[0]), Some("b"));

    // None renders the empty slot <span> (same shape the reactive form
    // mounts — feature-00 parity).
    let h = html! { <div>{None::<Html>}</div> };
    assert_eq!(tag_name(&h.children[0]), Some("span"));
    assert_eq!(h.children[0].children[0], Html::new());
}

/// Test 34.
#[test]
fn vec_slot_becomes_tagless_children() {
    let items = vec![html! { <li></li> }, html! { <li></li> }];
    let h = html! { <ul>{items}</ul> };
    let wrapper = &h.children[0];
    assert_eq!(wrapper.tag, None);
    assert_eq!(wrapper.children.len(), 2);
    assert_eq!(tag_name(&wrapper.children[0]), Some("li"));
}

// ─── Typed-Html specifics (beyond the spec tables) ─────────────────────────────

/// Known tags/attrs resolve to compact wire ids (F01 lineage).
#[test]
fn known_names_resolve_to_ids() {
    let h = html! { <div class="x"></div> };
    assert_eq!(h.tag, Some(HtmlTag::from_static("div")));
    assert!(matches!(h.tag, Some(HtmlTag::Id(_))));
    assert!(h
        .attributes
        .iter()
        .any(|(n, _)| *n == AttrName::from_static("class")
            && matches!(n, AttrName::Id(_))));
}

/// Custom elements stay name-form and require explicit closing.
#[test]
fn custom_elements_supported() {
    let h = html! { <my-widget data-kind="a"></my-widget> };
    assert_eq!(tag_name(&h), Some("my-widget"));
    assert!(matches!(h.tag, Some(HtmlTag::Name(_))));
    assert_eq!(attr(&h, "data-kind"), Some("a"));
}

/// Bare text with punctuation glue ("Count:" stays glued, spec §7 example).
#[test]
fn bare_text_runs() {
    let n = 5;
    let h = html! { <button>Count: {n}</button> };
    assert_eq!(h.children[0].text.as_deref(), Some("Count:"));
    assert_eq!(tag_name(&h.children[1]), Some("span"));
    assert_eq!(h.children[1].children[0].text.as_deref(), Some("5"));
}

// ─── Mount integration (tests 40-42, reactive form) ────────────────────────────

struct Mounted {
    signals: Rc<SignalsRuntime>,
    ctx: Context,
    sent: Rc<std::cell::RefCell<Vec<Vec<DomOp>>>>,
    receiver: foundation_wasm_ui::SharedInstructionReceiver,
}

fn reactive_setup() -> Mounted {
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
    Mounted {
        signals,
        ctx,
        sent,
        receiver,
    }
}

/// Flattened view of every op sent so far.
fn all_ops(sent: &Rc<std::cell::RefCell<Vec<Vec<DomOp>>>>) -> Vec<DomOp> {
    sent.borrow().iter().flatten().cloned().collect()
}

/// Tests 40 + 42 — slot effect queues the initial `SetText` and re-renders on
/// signal change, through the FULL loop (macro -> effect -> receiver ->
/// stabilize-flush).
#[test]
fn reactive_slot_renders_and_updates() {
    let m = reactive_setup();
    let (count, set_count) = m.ctx.signal(7i64);

    let tree = html! { m.ctx, m.receiver, <div><span>{count.get()}</span></div> };
    assert_eq!(tree.parts.len(), 1, "parts still recorded");

    m.signals.stabilize(); // flushes the build ops + initial effect renders
    let ops = all_ops(&m.sent);

    // The build created div + span + the slot's dedicated text node, with
    // explicit registration (F01 registry semantics).
    assert!(matches!(ops[0], DomOp::CreateElement { node_id, .. } if node_id == 16));
    assert!(ops
        .iter()
        .any(|op| matches!(op, DomOp::RegisterNode { node_id: 16 })));
    let initial = ops
        .iter()
        .filter_map(|op| match op {
            DomOp::SetText { node_id, text } => Some((*node_id, text.to_string())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(initial, vec![(18, "7".into())], "initial render targets the slot text node");

    // Signal change re-renders exactly the slot.
    set_count.set(8);
    m.signals.stabilize();
    let after = all_ops(&m.sent);
    let last = after.last().unwrap();
    assert!(
        matches!(last, DomOp::SetText { node_id: 18, text } if text == "8"),
        "got {last:?}"
    );
}

/// Test 41 — event handlers queue `AddEventListener`; setters also stamp
/// primal:setter on the wire.
#[test]
fn reactive_event_wiring() {
    let m = reactive_setup();
    let (_name, set_name) = m.ctx.signal(String::new());
    let expected_cb = set_name.callback_id().to_string();

    let _tree = html! { m.ctx, m.receiver, <input primal:onchange={set_name} /> };
    m.signals.stabilize();
    let ops = all_ops(&m.sent);

    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::AddEventListener { node_id: 16, event_name } if event_name.name() == Some("change")
    )));
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::SetAttribute { node_id: 16, name, value }
            if name.name() == Some("primal:setter") && *value == expected_cb
    )));
}

/// Two instances (G20 — e.g. loop iterations) get DISJOINT id blocks.
#[test]
fn instances_get_disjoint_id_blocks() {
    let m = reactive_setup();
    let first = html! { m.ctx, m.receiver, <div><span>"a"</span></div> };
    let second = html! { m.ctx, m.receiver, <div><span>"b"</span></div> };

    let first_id: u32 = attr(&first, "primal-id").unwrap().parse().unwrap();
    let second_id: u32 = attr(&second, "primal-id").unwrap().parse().unwrap();
    assert_ne!(first_id, second_id);
    // Each template has 3 nodes (div, span, text) — blocks are contiguous.
    assert_eq!(second_id, first_id + 3);
}

/// Reactive dynamic attribute: initial value arrives via the immediate effect
/// run and updates on change.
#[test]
fn reactive_dynamic_attribute() {
    let m = reactive_setup();
    let (cls, set_cls) = m.ctx.signal(String::from("on"));

    let _tree = html! { m.ctx, m.receiver, <div class={cls.get()}></div> };
    m.signals.stabilize();
    let ops = all_ops(&m.sent);
    assert!(ops.iter().any(|op| matches!(
        op,
        DomOp::SetAttribute { node_id: 16, name, value }
            if name.name() == Some("class") && *value == "on"
    )));

    set_cls.set(String::from("off"));
    m.signals.stabilize();
    let ops = all_ops(&m.sent);
    assert!(matches!(
        ops.last().unwrap(),
        DomOp::SetAttribute { node_id: 16, value, .. } if *value == "off"
    ));
}
