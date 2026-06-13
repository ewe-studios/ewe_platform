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
    accordion, checkbox, collapsible, dialog, field, input, number_field, otp_field,
    parent_check_state, popover, radio_group, separator, switch, tabs, toast_viewport, toggle,
    tooltip, AccordionConfig, AccordionItem, CheckState, CheckboxConfig, CheckboxSlots,
    CollapsibleConfig, CollapsibleSlots, DialogConfig, DialogSlots, FieldConfig, FieldSlots,
    menu, navigation_menu, progress, scroll_area, select, skeleton, slider, toolbar, InputConfig,
    ItemConfig, MenuConfig, MenuEntry, NavItem, NavMenuConfig, NumberFieldConfig, OtpConfig,
    PickItem, PopoverConfig, PopoverSlots, ProgressConfig, RadioGroupConfig, RadioItem,
    ScrollAreaConfig, SelectConfig, SeparatorConfig, SkeletonShape, SliderConfig, SwitchConfig,
    TabDef, TabsConfig, Toast, ToastManager, ToggleConfig, ToggleSlots, ToolbarConfig,
    ValidationMode,
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

// ─── F4 overlays ──────────────────────────────────────────────────────────────────

#[test]
fn dialog_syncs_open_and_wires_aria_and_driver() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    let _h = dialog(
        &ctx, &rcv, DialogConfig::default(), &open, set_open.clone(),
        DialogSlots { title: Some(Html::text("Title").into()), ..DialogSlots::default() },
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-dialog-mode") && *value == "modal")), "modal driver mode");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, .. } if name.name() == Some("aria-labelledby"))),
        "title wires aria-labelledby");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, .. } if name.name() == Some("primal:script"))), "dialog driver emitted");

    set_open.set(true);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-open") && *value == "")), "open flips data-open");
}

#[test]
fn popover_wires_trigger_positioner_and_modal_machinery() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    let _h = popover(
        &ctx, &rcv,
        PopoverConfig { modal: true, ..PopoverConfig::default() },
        &open, set_open,
        PopoverSlots { trigger: vec![Html::text("Open").into()], children: vec![Html::text("Body").into()], ..PopoverSlots::default() },
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, .. } if name.name() == Some("aria-controls"))), "trigger controls popup");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-anchor") && value.starts_with("popover-trigger-"))), "positioner anchored");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-scroll-lock") && *value == "true")), "modal → scroll lock");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-focus-trap") && *value == "true")), "modal → focus trap");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-dismiss") && *value == "true")), "popup is light-dismissable");
}

#[test]
fn hover_behavior_ports_safe_polygon() {
    use foundation_ui_components::machinery::hover::HOVER_JS;
    // The full safePolygon geometry is present (not just a timer).
    assert!(HOVER_JS.contains("inQuad"), "quadrilateral containment test");
    assert!(HOVER_JS.contains("mousemove"), "tracks the cursor toward the popup");
    assert!(HOVER_JS.contains("trough"), "ignores the trough between trigger and popup");
    assert!(HOVER_JS.contains("data-hover-popup"), "resolves the floating element");
    assert!(HOVER_JS.contains("leaveRight") && HOVER_JS.contains("leaveBottom"), "cursor exit side");
}

#[test]
fn tooltip_is_hover_describe_popover() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    let _h = tooltip(
        &ctx, &rcv, &open, set_open,
        PopoverSlots { trigger: vec![Html::text("?").into()], children: vec![Html::text("Help").into()], ..PopoverSlots::default() },
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "tooltip")), "tooltip role");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, .. } if name.name() == Some("aria-describedby"))), "describe link");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-hover-delay") && *value == "600")), "600ms tooltip delay");
}

#[test]
fn toast_manager_adds_and_closes_over_the_list() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let manager = ToastManager::new(&ctx);
    let _h = toast_viewport(&ctx, &rcv, &manager);
    app.stabilize();

    let id = manager.add(Toast::new("", "Saved"));
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-toast") && *value == "true")), "a toast rendered");
    assert_eq!(manager.toasts().get().len(), 1, "one toast queued");

    manager.close(&id);
    app.stabilize();
    assert_eq!(manager.toasts().get().len(), 0, "close drops it from the list");
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
    // M5 typeahead now on the roving variant too (ported from useTypeahead).
    assert!(COMPOSITE_JS.contains("typeahead") && COMPOSITE_JS.contains("750"), "750ms typeahead buffer");
    assert!(COMPOSITE_JS.contains("allowRapid"), "same-letter cycling");
}

#[test]
fn position_behavior_has_full_collision_avoidance() {
    use foundation_ui_components::machinery::position::POSITION_JS;
    assert!(POSITION_JS.contains("data-fallback-axis"), "fallbackAxisSide knob");
    assert!(POSITION_JS.contains("perpNeed"), "perpendicular-axis fallback");
    assert!(POSITION_JS.contains("data-sticky"), "sticky keeps popup on-screen");
    assert!(POSITION_JS.contains("data-arrow") && POSITION_JS.contains("--arrow-x"), "arrow centering");
    assert!(POSITION_JS.contains("data-uncentered"), "uncentered arrow flag");
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

#[test]
fn radio_group_wires_roving_and_move_and_select() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal::<Option<String>>(None);
    let items = vec![
        RadioItem { value: alloc_string("a"), content: Html::text("A"), disabled: false },
        RadioItem { value: alloc_string("b"), content: Html::text("B"), disabled: false },
    ];
    let _h = radio_group(&ctx, &rcv, RadioGroupConfig::default(), &value, set_value, items);
    app.stabilize();
    let ops = all_ops(&sent);
    // Group opts into the radio "move AND select" composite pattern.
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-composite-select") && *value == "true")),
        "radiogroup opts into move-and-select");
    // Inputs are roving members.
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-composite-item") && *value == "true")),
        "radio inputs marked as composite items");
    // The composite roving script is embedded.
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, .. } if name.name() == Some("primal:script"))),
        "composite behavior script emitted into the radiogroup");
}

// ─── M3 / M1 / M7 machinery (scoped-script behaviors) ────────────────────────────

#[test]
fn dismiss_behavior_is_a_light_dismiss_script() {
    use foundation_ui_components::machinery::dismiss::{dismiss_behavior, DISMISS_JS};
    let s = dismiss_behavior();
    let body = s.children[0].text.as_deref().unwrap();
    assert!(body.contains("Escape"), "Escape closes");
    assert!(body.contains("pointerdown"), "outside pointer closes");
    assert!(body.contains("data-dismiss-action"), "clicks the close action");
    assert!(body.contains("data-anchor"), "ignores clicks on the trigger");
    assert!(DISMISS_JS.contains("data-dismiss-reason"), "stamps a dismiss reason");
}

#[test]
fn position_behavior_emits_placement_and_var_contract() {
    use foundation_ui_components::machinery::position::{position_behavior, POSITION_JS};
    let s = position_behavior();
    let body = s.children[0].text.as_deref().unwrap();
    assert!(body.contains("data-side") && body.contains("data-align"), "reflects final placement");
    assert!(body.contains("opposite"), "flips to the opposite side on overflow");
    assert!(body.contains("--anchor-width"), "emits the anchor-size vars");
    assert!(body.contains("--transform-origin"), "emits the origin var");
    assert!(body.contains("--popup-width"), "emits the popup-size vars");
    assert!(POSITION_JS.contains("data-anchor"), "resolves the anchor element");
}

#[test]
fn transition_behavior_manages_starting_and_ending_styles() {
    use foundation_ui_components::machinery::transition::{transition_behavior, TRANSITION_JS};
    let s = transition_behavior();
    let body = s.children[0].text.as_deref().unwrap();
    assert!(body.contains("data-starting-style"), "stamps enter baseline");
    assert!(body.contains("data-ending-style"), "holds exit style");
    assert!(body.contains("data-instant"), "honors instant suppression");
    assert!(body.contains("MutationObserver"), "observes the open state");
    assert!(TRANSITION_JS.contains("transitionend"), "completes on transitionend");
}

#[test]
fn scroll_lock_behavior_locks_and_restores() {
    use foundation_ui_components::machinery::scroll_lock::{scroll_lock_behavior, SCROLL_LOCK_JS};
    let s = scroll_lock_behavior();
    let body = s.children[0].text.as_deref().unwrap();
    assert!(body.contains("overflowY"), "toggles overflow-y");
    assert!(body.contains("scrollbar-gutter") || body.contains("scrollbarGutter"), "prefers gutter-stable");
    assert!(body.contains("paddingRight"), "pads the gutter on the fallback path");
    assert!(SCROLL_LOCK_JS.contains("data-open"), "engages while open");
}

// ─── F5 menus + F6 pickers ──────────────────────────────────────────────────────

#[test]
fn menu_renders_roled_items_and_listbox_machinery() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    let (checked, set_checked) = ctx.signal(false);
    let items = vec![
        MenuEntry::Item {
            cfg: ItemConfig::default(),
            label: Html::text("Cut").into(),
            on_select: Box::new(|| {}),
        },
        MenuEntry::Checkbox {
            cfg: ItemConfig { close_on_click: false, ..ItemConfig::default() },
            label: Html::text("Word wrap").into(),
            checked,
            set_checked,
        },
        MenuEntry::Separator,
    ];
    let _h = menu(
        &ctx, &rcv, MenuConfig::default(), &open, set_open,
        vec![Html::text("Edit").into()], items,
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("aria-haspopup") && *value == "menu")),
        "trigger announces a menu popup");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "menuitem")),
        "command item role");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "menuitemcheckbox")),
        "checkbox item role");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-list-item") && *value == "true")),
        "items are list members for virtual highlight");
}

#[test]
fn toolbar_marks_children_as_composite_members() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    // Toolbar children are element widgets (separators here) — the marker rides
    // each child's root element.
    let _h = toolbar(
        &ctx, &rcv, ToolbarConfig::default(),
        vec![
            separator(SeparatorConfig::default()).into(),
            separator(SeparatorConfig::default()).into(),
        ],
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "toolbar")));
    let marked = ops.iter().filter(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-composite-item") && *value == "true")).count();
    assert_eq!(marked, 2, "each child becomes a roving member");
}

#[test]
fn select_renders_listbox_options_and_hidden_input() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    let (value, set_value) = ctx.signal::<Option<String>>(Some(alloc_string("b")));
    let items = vec![PickItem::new("a"), PickItem::new("b")];
    let _h = select(&ctx, &rcv, SelectConfig::default(), &open, set_open, &value, set_value, items);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("aria-haspopup") && *value == "listbox")),
        "select trigger opens a listbox");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "option")),
        "options rendered");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("type") && *value == "hidden")),
        "hidden input serializes the value");
}

#[test]
fn navigation_menu_is_nav_with_linked_panels() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (active, set_active) = ctx.signal::<Option<String>>(None);
    let items = vec![
        NavItem { value: alloc_string("products"), trigger: Html::text("Products").into(),
                  content: Some(Html::text("panel").into()), href: None, current: false },
        NavItem { value: alloc_string("home"), trigger: Html::text("Home").into(),
                  content: None, href: Some("/".into()), current: true },
    ];
    let _h = navigation_menu(&ctx, &rcv, NavMenuConfig::default(), &active, set_active, items);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("aria-controls") && value.starts_with("nav-panel-"))),
        "trigger controls its panel");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("aria-current") && *value == "page")),
        "current link marked");
}

// ─── F8 indicators & surfaces ───────────────────────────────────────────────────

#[test]
fn progress_reflects_value_and_indeterminate() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal::<Option<f64>>(Some(50.0));
    let _h = progress(&ctx, &rcv, ProgressConfig::default(), &value, None);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "progressbar")));
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("aria-valuenow") && *value == "50")),
        "valuenow reflects the value");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("style") && value.contains("--progress-value:50"))), "var published");

    set_value.set(None);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-indeterminate") && *value == "")),
        "None → indeterminate");
}

#[test]
fn slider_publishes_percent_and_hidden_range_input() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal(25.0_f64);
    let _h = slider(&ctx, &rcv, SliderConfig::default(), &value, set_value);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("style") && value.contains("--slider-percent:25"))), "percent var published");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("type") && *value == "range")),
        "hidden range input carries focus/form/AT");
}

#[test]
fn scroll_area_and_skeleton_are_pure_markup() {
    let area = scroll_area(ScrollAreaConfig::default(), vec![Html::text("body")]);
    assert_eq!(attr(&area, "data-scroll-area"), Some("true"), "keyed for the JS module");

    let line = skeleton(SkeletonShape::Line { width: "60%".into() });
    assert_eq!(attr(&line, "aria-hidden"), Some("true"), "decorative");
    assert_eq!(attr(&line, "data-loading"), Some("true"));
    assert!(attr(&line, "style").unwrap().contains("inline-size:60%"), "sized");
}

// ─── F7 form ──────────────────────────────────────────────────────────────────────

#[test]
fn input_reflects_value_and_is_field_aware() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal(alloc_string("hi"));
    // Build a field to source a FieldState.
    let (_f, state) = field(&ctx, &rcv, FieldConfig::default(), FieldSlots::default(), |c, r, _b| {
        html! { c, r, <span /> }
    });
    let _h = input(&ctx, &rcv, InputConfig::default(), &value, set_value, Some(&state));
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("value") && *value == "hi")),
        "input renders its value");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("type") && *value == "text")),
        "default text type");
}

#[test]
fn number_field_emits_spinbutton_structure() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal(3.0_f64);
    let _h = number_field(
        &ctx, &rcv, NumberFieldConfig { min: Some(0.0), max: Some(10.0), step: 1.0, ..NumberFieldConfig::default() },
        &value, set_value,
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "spinbutton")),
        "input is a spinbutton");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-nf-increment") && *value == "true")),
        "increment affordance present");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("aria-valuenow") && *value == "3")),
        "aria-valuenow reflects the value");
}

#[test]
fn number_field_drives_full_m8_behavior() {
    use foundation_ui_components::number_field::NUMBER_FIELD_JS;
    // The JS carries the base-ui constants + gestures 1:1.
    assert!(NUMBER_FIELD_JS.contains("400") && NUMBER_FIELD_JS.contains("60"), "hold-repeat 400/60");
    assert!(NUMBER_FIELD_JS.contains("requestPointerLock"), "scrub pointer lock");
    assert!(NUMBER_FIELD_JS.contains("altKey") && NUMBER_FIELD_JS.contains("shiftKey"), "small/large step modifiers");
    assert!(NUMBER_FIELD_JS.contains("snapTo"), "snap-on-step");

    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal(3.0_f64);
    let _h = number_field(
        &ctx, &rcv,
        NumberFieldConfig { min: Some(0.0), max: Some(10.0), scrub: true, allow_wheel_scrub: true, ..NumberFieldConfig::default() },
        &value, set_value,
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-nf-scrub") && *value == "true")),
        "scrub area rendered when configured");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-allow-wheel") && *value == "true")),
        "wheel scrub enabled");
}

#[test]
fn slider_wires_drag_machinery() {
    use foundation_ui_components::machinery::gestures::SLIDER_DRAG_JS;
    assert!(SLIDER_DRAG_JS.contains("getBoundingClientRect"), "maps finger → value");
    assert!(SLIDER_DRAG_JS.contains("PageUp"), "largeStep keys");

    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (value, set_value) = ctx.signal(25.0_f64);
    let _h = slider(&ctx, &rcv, SliderConfig::default(), &value, set_value);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-slider-control") && *value == "true")),
        "control is the drag surface");
}

#[test]
fn toast_swipe_to_dismiss_wired() {
    use foundation_ui_components::machinery::gestures::SWIPE_JS;
    assert!(SWIPE_JS.contains("data-swipe-dismiss"), "swipe past threshold dismisses");
    assert!(SWIPE_JS.contains("movement-x") && SWIPE_JS.contains("movement-y"), "publishes movement vars");

    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let manager = ToastManager::new(&ctx);
    let _h = toast_viewport(&ctx, &rcv, &manager);
    manager.add(Toast::new("", "Saved"));
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-swipe-direction") && *value == "right")),
        "toast swipes right to dismiss");
}

#[test]
fn otp_field_emits_cells_and_hidden_aggregate() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (_code, set_code) = ctx.signal(String::new());
    let _h = otp_field(&ctx, &rcv, OtpConfig { length: 4, ..OtpConfig::default() }, set_code);
    app.stabilize();
    let ops = all_ops(&sent);
    let cell_count = ops.iter().filter(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-otp-cell") && *value == "true")).count();
    assert_eq!(cell_count, 4, "one input per cell");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("data-otp-value") && *value == "true")),
        "hidden aggregate present");
}

// ─── F3 disclosure ──────────────────────────────────────────────────────────────

#[test]
fn collapsible_toggles_expanded_and_hidden() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    let _h = collapsible(
        &ctx, &rcv, CollapsibleConfig::default(), &open, set_open.clone(), CollapsibleSlots::default(),
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-expanded") && *value == "false")), "closed → aria-expanded false");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, .. } if name.name() == Some("hidden"))), "closed panel is hidden");
    // Embeds M7 + measurement.
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-size-var") && *value == "--collapsible-panel")), "panel measured");

    set_open.set(true);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-expanded") && *value == "true")), "open → aria-expanded true");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::RemoveAttribute { name, .. } if name.name() == Some("hidden"))), "open panel un-hidden");
}

#[test]
fn accordion_reflects_open_item_and_ties_aria() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (open_items, set_open) = ctx.signal::<Vec<String>>(Vec::new());
    let items = vec![
        AccordionItem { value: alloc_string("a"), header: Html::text("A"), panel: Html::text("pa"), disabled: false },
        AccordionItem { value: alloc_string("b"), header: Html::text("B"), panel: Html::text("pb"), disabled: false },
    ];
    let _h = accordion(&ctx, &rcv, AccordionConfig::default(), &open_items, set_open.clone(), items);
    app.stabilize();
    let ops = all_ops(&sent);
    // Panel↔trigger wiring present.
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-controls") && value.starts_with("accordion-panel-"))),
        "trigger controls its panel");

    set_open.set(vec![alloc_string("a")]);
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-expanded") && *value == "true")), "opened item is expanded");
}

#[test]
fn tabs_selection_drives_aria_and_automatic_mode_selects() {
    let (app, sent) = App::mock();
    let (ctx, rcv) = app.context();
    let (selected, set_selected) = ctx.signal::<Option<String>>(Some(alloc_string("a")));
    let defs = vec![
        TabDef { value: alloc_string("a"), label: Html::text("A"), panel: Html::text("PA"), disabled: false },
        TabDef { value: alloc_string("b"), label: Html::text("B"), panel: Html::text("PB"), disabled: false },
    ];
    let _h = tabs(
        &ctx, &rcv,
        TabsConfig { activate_on_focus: true, ..TabsConfig::default() },
        &selected, set_selected.clone(), defs,
    );
    app.stabilize();
    let ops = all_ops(&sent);
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("aria-selected") && *value == "true")), "selected tab is aria-selected");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. }
            if name.name() == Some("data-composite-select") && *value == "true")),
        "automatic mode → arrows move-and-select");
    assert!(ops.iter().any(|op| matches!(op,
        DomOp::SetAttribute { name, value, .. } if name.name() == Some("role") && *value == "tabpanel")),
        "panels carry the tabpanel role");
}

#[test]
fn focus_trap_behavior_traps_and_restores() {
    use foundation_ui_components::machinery::focus_trap::{focus_trap_behavior, FOCUS_TRAP_JS};
    let s = focus_trap_behavior();
    let body = s.children[0].text.as_deref().unwrap();
    assert!(body.contains("Tab"), "wraps Tab");
    assert!(body.contains("data-initial-focus"), "honors initial-focus target");
    assert!(body.contains("data-anchor"), "restores to the trigger");
    assert!(FOCUS_TRAP_JS.contains("activeElement"), "records/restores focus");
}
