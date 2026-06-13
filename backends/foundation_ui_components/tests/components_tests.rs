//! WHY: F1 primitives + M6 field state are the catalog's foundation; their
//! ARIA/data-attribute and event contracts (spec-42 feature 05) must hold on
//! the real op stream, not just compile.
//!
//! WHAT: separator's pure markup, toggle's signal→`data-pressed` reflection,
//! and the M6 field binding actually driving the six state signals (the
//! FieldBinding callbacks invoked through the runtime).
//!
//! HOW: pure components are inspected as `Html`; reactive ones mount through
//! `App::mock`, `stabilize`, and assert the captured `DomOp`s / signal values.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_signals::EventData;
use foundation_ui_components::{
    checkbox, field, parent_check_state, radio_group, separator, switch, toggle, CheckState,
    CheckboxConfig, CheckboxSlots, FieldConfig, FieldSlots, RadioGroupConfig, RadioItem,
    SeparatorConfig, SwitchConfig, ToggleConfig, ToggleSlots, ValidationMode,
};
use foundation_ui_traits::{DomOp, Html};
use foundation_wasm_ui::{html, App};

fn attr<'a>(h: &'a Html, name: &str) -> Option<&'a str> {
    h.attributes
        .iter()
        .find(|(n, _)| n.name() == Some(name))
        .map(|(_, v)| v.as_ref())
}

fn all_ops(sent: &Rc<RefCell<Vec<Vec<DomOp>>>>) -> Vec<DomOp> {
    sent.borrow().iter().flatten().cloned().collect()
}

// ─── separator (pure) ──────────────────────────────────────────────────────────

#[test]
fn separator_is_pure_accessible_markup() {
    let h = separator(SeparatorConfig::default());
    assert_eq!(attr(&h, "role"), Some("separator"));
    assert_eq!(attr(&h, "aria-orientation"), Some("horizontal"));
    assert_eq!(attr(&h, "data-orientation"), Some("horizontal"));
    // Decorative by default → aria-hidden present.
    assert_eq!(attr(&h, "aria-hidden"), Some("true"));

    let non_decorative = separator(SeparatorConfig {
        decorative: false,
        ..SeparatorConfig::default()
    });
    assert_eq!(attr(&non_decorative, "aria-hidden"), None, "presence-only");
}

// ─── toggle (signal → data-pressed presence) ────────────────────────────────────

#[test]
fn toggle_reflects_pressed_signal_as_presence_attr() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (pressed, set_pressed) = ctx.signal(false);

    let _h = toggle(
        &ctx,
        &rcv,
        ToggleConfig::default(),
        &pressed,
        set_pressed.clone(),
        ToggleSlots::default(),
    );
    app.stabilize();

    // pressed=false → aria-pressed "false", data-pressed REMOVED (presence).
    let ops = all_ops(&sent);
    assert!(
        ops.iter().any(|op| matches!(op,
            DomOp::SetAttribute { name, value, .. }
                if name.name() == Some("aria-pressed") && *value == "false")),
        "aria-pressed=false initially"
    );
    assert!(
        ops.iter().any(|op| matches!(op,
            DomOp::RemoveAttribute { name, .. } if name.name() == Some("data-pressed"))),
        "data-pressed absent when unpressed"
    );

    // Flip on: data-pressed SET to presence (empty string).
    set_pressed.set(true);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(
        matches!(ops.last().unwrap(),
            DomOp::SetAttribute { name, value, .. }
                if name.name() == Some("data-pressed") && *value == ""),
        "data-pressed present when pressed, got {:?}",
        ops.last()
    );
}

// ─── M6 field: the binding drives the six signals ───────────────────────────────

#[test]
fn field_binding_input_and_blur_drive_state() {
    let (app, _sent) = App::mock();
    let (ctx, rcv) = app.context();

    let validator = |v: &str| {
        if v.is_empty() {
            vec![alloc_string("required")]
        } else {
            Vec::new()
        }
    };

    // Capture the binding's callback ids so the test can fire DOM events.
    let ids = Rc::new(RefCell::new((0u64, 0u64, 0u64)));
    let (_html, state) = field(
        &ctx,
        &rcv,
        FieldConfig {
            validation_mode: ValidationMode::OnChange,
            validator: Some(validator),
            ..FieldConfig::default()
        },
        FieldSlots::default(),
        {
            let ids = ids.clone();
            move |c: &_, r: &_, b: &foundation_ui_components::FieldBinding| {
                *ids.borrow_mut() = (
                    b.on_focus.callback_id(),
                    b.on_blur.callback_id(),
                    b.on_input.callback_id(),
                );
                html! { c, r, <input id=[b.control_id.clone()] /> }
            }
        },
    );

    // Initial state.
    assert_eq!(state.filled.get(), false);
    assert_eq!(state.touched.get(), false);
    assert_eq!(state.valid.get(), None);

    let (focus_id, blur_id, input_id) = *ids.borrow();

    // focus → focused.
    app.signals().invoke_callback(focus_id, EventData::default());
    app.stabilize();
    assert!(state.focused.get(), "focus set focused");

    // input "hi" → filled + dirty + (OnChange) valid.
    app.signals()
        .invoke_callback(input_id, EventData::with_value("input", "hi"));
    app.stabilize();
    assert!(state.filled.get(), "non-empty → filled");
    assert!(state.dirty.get(), "diverged from initial \"\" → dirty");
    assert_eq!(state.valid.get(), Some(true), "validator passed");

    // input "" → invalid (validator returns an error).
    app.signals()
        .invoke_callback(input_id, EventData::with_value("input", ""));
    app.stabilize();
    assert_eq!(state.valid.get(), Some(false), "empty → invalid");
    assert_eq!(state.errors.get(), vec![alloc_string("required")]);

    // blur → touched, not focused.
    app.signals().invoke_callback(blur_id, EventData::default());
    app.stabilize();
    assert!(state.touched.get(), "blur set touched");
    assert!(!state.focused.get(), "blur cleared focused");
}

fn alloc_string(s: &str) -> String {
    s.to_string()
}

// ─── F2 selection controls ──────────────────────────────────────────────────────

#[test]
fn switch_reflects_checked_as_presence_pair() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (checked, set_checked) = ctx.signal(false);

    let _h = switch(&ctx, &rcv, SwitchConfig::default(), &checked, set_checked.clone());
    app.stabilize();
    let ops = all_ops(&sent);
    // unchecked → data-unchecked present, data-checked removed.
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-unchecked") && *value == "")));

    set_checked.set(true);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-checked") && *value == "")),
        "data-checked present when on");
}

#[test]
fn checkbox_three_state_drives_aria_and_data() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (state, set_state) = ctx.signal(CheckState::Indeterminate);

    let _h = checkbox(
        &ctx, &rcv, CheckboxConfig::default(), &state, set_state.clone(), CheckboxSlots::default(),
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-checked") && *value == "mixed")), "indeterminate → mixed");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-indeterminate") && *value == "")));

    set_state.set(CheckState::Checked);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-checked") && *value == "true")));
}

#[test]
fn parent_check_state_computes_tristate() {
    let (app, _sent) = App::mock();
    let (ctx, _rcv) = app.context();
    let (values, set_values) = ctx.signal::<Vec<String>>(Vec::new());
    let all = vec![alloc_string("a"), alloc_string("b")];

    let parent = parent_check_state(&ctx, &values, all);
    app.stabilize();
    assert_eq!(parent.get(), CheckState::Unchecked, "none selected");

    set_values.set(vec![alloc_string("a")]);
    app.stabilize();
    assert_eq!(parent.get(), CheckState::Indeterminate, "some selected");

    set_values.set(vec![alloc_string("a"), alloc_string("b")]);
    app.stabilize();
    assert_eq!(parent.get(), CheckState::Checked, "all selected");
}

#[test]
fn radio_group_marks_selected_item_checked() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal::<Option<String>>(None);
    let items = vec![
        RadioItem { value: alloc_string("a"), content: Html::text("A"), disabled: false },
        RadioItem { value: alloc_string("b"), content: Html::text("B"), disabled: false },
    ];

    let _h = radio_group(&ctx, &rcv, RadioGroupConfig::default(), &value, set_value.clone(), items);
    app.stabilize();

    set_value.set(Some(alloc_string("b")));
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-checked") && *value == "")),
        "selecting an item sets data-checked");
}

// ─── M5 machinery (roving focus, scoped-script delivery) ─────────────────────────

#[test]
fn scoped_script_builds_primal_script_node() {
    let s = foundation_ui_components::machinery::scoped_script("function(scope){}");
    assert_eq!(s.tag.as_ref().and_then(|t| t.name()), Some("script"));
    assert!(s.attributes.iter().any(|(n, _)| n.name() == Some("primal:script")));
    assert_eq!(s.children[0].text.as_deref(), Some("function(scope){}"));
}

#[test]
fn composite_behavior_is_a_roving_script() {
    use foundation_ui_components::machinery::composite::{composite_behavior, COMPOSITE_JS};
    let s = composite_behavior();
    let body = s.children[0].text.as_deref().unwrap();
    assert!(body.contains("keydown"), "wires keydown");
    assert!(body.contains("data-composite-item"), "finds items");
    assert!(body.contains("ArrowRight") && body.contains("ArrowDown"), "orientation-aware");
    assert!(COMPOSITE_JS.contains("data-composite-select"), "radio move+select opt-in");
}

#[test]
fn toggle_group_embeds_composite_machinery_and_marks_items() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (values, set_values) = ctx.signal::<Vec<String>>(Vec::new());
    let items = vec![foundation_ui_components::ToggleGroupItem {
        value: alloc_string("a"),
        content: Html::text("A"),
        disabled: false,
    }];
    let _h = foundation_ui_components::toggle_group(
        &ctx,
        &rcv,
        foundation_ui_components::ToggleGroupConfig::default(),
        &values,
        set_values,
        items,
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(
        ops.iter().any(|op| matches!(op,
            DomOp::SetAttribute { name, value, .. }
                if name.name() == Some("data-composite-item") && *value == "true")),
        "items marked for roving"
    );
    assert!(
        ops.iter().any(|op| matches!(op,
            DomOp::SetAttribute { name, .. } if name.name() == Some("primal:script"))),
        "composite behavior script emitted"
    );
}
