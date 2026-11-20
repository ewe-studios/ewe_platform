use super::support::{test_ctx, WALL};
use foundation_ui_runtime::action::{prepare_messages, Context};
use foundation_ui_runtime::state::{
    install_state_signer, verify_state, HmacStateSigner, StateConfig,
};
use foundation_ui_runtime::view::{
    lower, mount, render,
    template_types::{AttrName, Html, HtmlTag, Part},
    LowerError, LowerOptions, TokenTable,
};
use foundation_ui_wire::{MountSurface, PlacementKind, UiOperationT, WebOperationT};
use std::sync::Arc;

fn element(tag: &'static str, id: u32) -> Html {
    let mut html = Html::new();
    html.tag = Some(HtmlTag::from_static(tag));
    html.attributes
        .push((AttrName::from_static("primal-id"), id.to_string().into()));
    html
}

#[test]
fn mixed_trees_allocate_distinct_ids_for_helpers_and_text() {
    let mut root = element("Stack", 0);
    let mut row = element("div", 0);
    row.children.push(Html::text("Milk"));
    root.children.extend([row.clone(), row]);
    let ops = lower(
        &root,
        &LowerOptions {
            id_base: 10,
            tokens: &TokenTable::default(),
        },
    )
    .unwrap();
    let edges: Vec<_> = ops
        .iter()
        .filter_map(|op| match &op.ui {
            UiOperationT::AppendChild(edge) => Some((edge.parent_id, edge.child_id)),
            _ => None,
        })
        .collect();
    assert_eq!(edges, [(11, 12), (10, 11), (13, 14), (10, 13)]);
    let registrations: Vec<_> = ops
        .iter()
        .filter_map(|op| match &op.web {
            WebOperationT::WebRegisterNode(node) => Some(node.node_id),
            _ => None,
        })
        .collect();
    assert_eq!(registrations, [11, 12, 13, 14]);
}

#[test]
fn rejects_invalid_properties_tokens_ids_and_state() {
    let tokens = TokenTable::default();
    let options = LowerOptions {
        id_base: 0,
        tokens: &tokens,
    };
    let mut text = element("Text", 0);
    text.attributes
        .push((AttrName::from_static("width"), "NaN".into()));
    assert!(matches!(
        lower(&text, &options),
        Err(LowerError::BadProp { .. })
    ));
    text.attributes.pop();
    text.attributes
        .push((AttrName::from_static("color"), "unknown".into()));
    assert!(matches!(
        lower(&text, &options),
        Err(LowerError::UnknownToken { .. })
    ));
    text.attributes.pop();
    text.parts.push(Part::state_marker(5));
    assert!(lower(&text, &options).is_ok());
    text.parts.push(Part::state_marker(0));
    assert_eq!(lower(&text, &options), Err(LowerError::MissingState));
    text.parts.clear();
    text.attributes
        .push((AttrName::from_static("primal::state"), "user bytes".into()));
    assert_eq!(lower(&text, &options), Err(LowerError::ReservedAttribute));
    assert_eq!(
        lower(&Html::text("loose"), &options),
        Err(LowerError::NoRootElement)
    );
}

#[test]
fn atomic_swap_keeps_root_and_signs_only_explicit_targets() {
    let (base, _) = test_ctx(WALL);
    install_state_signer(
        &base,
        Arc::new(HmacStateSigner::new([4; 32], StateConfig::default()).unwrap()),
    );
    let ctx = Context::for_action(base.clone(), MountSurface::Web, Some(40));
    let mut page = element("Stack", 2);
    let mut input = element("TextInput", 0);
    input.parts.push(Part::state_marker(0));
    page.children.push(input);
    let html = render(&ctx, ctx.receiver(), page.clone(), Some(&"a")).unwrap();
    assert_eq!(html.runtime_id, Some(40));
    let messages = prepare_messages(&base, ctx.into_receiver().drain()).unwrap();
    assert_eq!(messages.len(), 1);
    let UiOperationT::Place(place) = &messages[0].operation.as_ref().unwrap().ui else {
        panic!("expected placement")
    };
    assert_eq!(place.kind, PlacementKind::Swap);
    assert_eq!(place.target.as_ref().unwrap().node_id, 40);
    let ops = place.operations.as_ref().unwrap();
    let states: Vec<_> = ops
        .iter()
        .filter_map(|op| match &op.ui {
            UiOperationT::AttachState(state) => {
                Some((state.root_node, state.state.as_ref().unwrap()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].0, 40);
    assert_ne!(states[1].0, 40);
    assert_eq!(states[0].1, states[1].1);
    assert_eq!(verify_state(&base, states[0].1).unwrap(), br#""a""#);
    let orphan = Context::for_action(base.clone(), MountSurface::Web, None);
    assert!(render(&orphan, orphan.receiver(), page.clone(), Some(&"b")).is_err());
    assert!(orphan.into_receiver().drain().is_empty());
    let initial = Context::for_render(base);
    assert!(mount(
        &page,
        &LowerOptions {
            id_base: 10,
            tokens: &TokenTable::default()
        },
        1,
        initial.receiver(),
        None::<&()>
    )
    .is_err());
    assert!(initial.into_receiver().drain().is_empty());
}

#[test]
fn portable_vocabulary_lowers_each_node_and_property() {
    use foundation_ui_wire::{NodeKind, PropId, PropValueT};
    let mut tokens = TokenTable::new();
    let color = tokens.color("--accent");
    let font = tokens.font("--body");
    let options = LowerOptions {
        id_base: 10,
        tokens: &tokens,
    };
    for (name, kind) in [
        ("Stack", NodeKind::STACK),
        ("Text", NodeKind::TEXT),
        ("Image", NodeKind::IMAGE),
        ("Button", NodeKind::BUTTON),
        ("TextInput", NodeKind::TEXT_INPUT),
        ("Toggle", NodeKind::TOGGLE),
        ("List", NodeKind::LIST),
        ("Scroll", NodeKind::SCROLL),
        ("Spacer", NodeKind::SPACER),
        ("Overlay", NodeKind::OVERLAY),
    ] {
        let ops = lower(&element(name, 2), &options).unwrap();
        assert!(
            matches!(&ops[0].ui,UiOperationT::CreateNode(op) if op.kind==kind && op.node_id==12)
        );
    }
    let mut node = element("Stack", 0);
    for (name, value) in [
        ("padding", "2"),
        ("spacing", "5"),
        ("align", "stretch"),
        ("color", "--accent"),
        ("font", "--body"),
        ("enabled", "true"),
        ("a11y_label", "List"),
        ("axis", "vertical"),
        ("value", "12"),
        ("checked", "false"),
        ("source", "icon"),
        ("placeholder", "Type"),
        ("background", "--accent"),
        ("width", "50%"),
        ("height", "auto"),
        ("hidden", "false"),
    ] {
        node.attributes
            .push((AttrName::from_static(name), value.into()));
    }
    let ops = lower(&node, &options).unwrap();
    let props: Vec<_> = ops
        .iter()
        .filter_map(|op| match &op.ui {
            UiOperationT::SetProp(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(props.len(), 16);
    assert!(props
        .iter()
        .any(|p| p.prop == PropId::COLOR
            && matches!(&p.value,PropValueT::ColorToken(t) if t.id==color)));
    assert!(props.iter().any(
        |p| p.prop == PropId::FONT && matches!(&p.value,PropValueT::FontToken(t) if t.id==font)
    ));
}

#[test]
fn scoped_script_and_behaviours_belong_to_the_parent() {
    let mut root = element("Stack", 1);
    for name in ["dialog", "focus_trap"] {
        let mut marker = element("div", 1);
        marker
            .attributes
            .push((AttrName::from_static("data-ewe-behaviour"), name.into()));
        root.children.push(marker);
    }
    let mut script = Html::new();
    script.tag = Some(HtmlTag::from_static("script"));
    script
        .attributes
        .push((AttrName::from_static("primal:script"), "".into()));
    script
        .children
        .push(Html::text("scope.action('notify', notify)"));
    root.children.push(script);
    let ops = lower(
        &root,
        &LowerOptions {
            id_base: 40,
            tokens: &TokenTable::new(),
        },
    )
    .unwrap();
    assert_eq!(ops.len(), 4);
    assert!(matches!(&ops[1].ui,UiOperationT::UiBehaviour(b) if b.node_id==41));
    assert!(
        matches!(&ops[2].web,WebOperationT::WebBehaviour(b) if b.node_id==41 && b.custom.as_deref()==Some("focus_trap"))
    );
    assert!(
        matches!(&ops[3].web,WebOperationT::ScriptNode(s) if s.parent_id==41 && s.node_id==42 && s.body.as_deref()==Some("scope.action('notify', notify)"))
    );
}
