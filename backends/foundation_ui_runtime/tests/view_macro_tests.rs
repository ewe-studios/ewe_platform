//! Render templates through their public macro entry points.

use foundation_ui_runtime::action::{ActionResult, Context, Interaction};
use foundation_ui_runtime::view::template_types::Html;
use foundation_ui_runtime::{
    action, html, html_add, html_add_after, html_add_before, html_remove, html_swap,
};
use foundation_ui_wire::MountSurface;

use super::support::{test_ctx, WALL};

#[action(server)]
async fn save(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[test]
fn core_and_web_bindings_follow_the_action_declaration() {
    let core = html! { <Button on:tap={save}>Save</Button> };
    let web = html! { <button on:tap="host:focus_field">Focus</button> };
    assert!(core.to_markup().contains("ewe:on:tap=\"server:save\""));
    assert!(web
        .to_markup()
        .contains("primal:onclick=\"host:focus_field\""));
}

#[test]
fn pure_templates_preserve_void_elements_and_bare_text_spacing() {
    let n = 2;
    let page = html! { <div><br><img /><hr><Text>Count: {n}</Text></div> };
    assert_eq!(page.children.len(), 4);
    assert_eq!(page.children[3].children[0].text.as_deref(), Some("Count:"));
    assert_eq!(page.children[3].children[1].text.as_deref(), Some("2"));
}

fn todo_row(text: &str) -> Html {
    html! { <Text>{ text }</Text> }
}

fn render_list(ctx: &Context, list: &[&str]) -> ActionResult {
    let out = ctx.receiver();
    let html = html! { ctx, out, Some(&list),
        <Stack primal::state>
            <List>{list.iter().copied().map(todo_row)}</List>
            <TextInput primal::state on:submit={save} />
        </Stack>
    };
    assert_eq!(html.children.len(), 2);
    assert!(!html.to_markup().contains("primal::state"));
    Ok(())
}

#[test]
fn state_template_queues_mapped_helpers_and_signed_state_targets() {
    let (base, _) = test_ctx(WALL);
    let ctx = Context::for_action(base, MountSurface::Web, Some(40));
    render_list(&ctx, &["Milk", "Tea"]).unwrap();
    assert!(!ctx.into_receiver().drain().is_empty());
}

fn place_views(ctx: &Context) -> ActionResult {
    let out = ctx.receiver();
    html! { ctx, out, <div>Ready</div> };
    html_add! { 4u32, ctx, out, <Text>More</Text> }?;
    html_add_before! { 4u32, None, ctx, out, <Text>First</Text> }?;
    html_add_after! { 4u32, Some(7u32), ctx, out, <Text>Next</Text> }?;
    html_swap! { "#card", ctx, out, Some(&7), <Stack primal::state/> }?;
    html_remove! { 8u32, ctx, out }?;
    Ok(())
}

#[test]
fn target_macros_queue_their_views_and_state_on_one_receiver() {
    let (base, _) = test_ctx(WALL);
    let ctx = Context::for_action(base, MountSurface::Web, Some(40));
    place_views(&ctx).unwrap();
    assert!(!ctx.into_receiver().drain().is_empty());
}

#[test]
fn pure_core_templates_lower_in_creation_order_and_resolve_property_tokens() {
    use foundation_ui_runtime::view::{lower, LowerError, LowerOptions, TokenTable};
    use foundation_ui_wire::{LengthUnit, PropValueT, UiOperationT, WebOperationT};
    use std::collections::BTreeSet;

    let mut tokens = TokenTable::new();
    let color = tokens.color("--accent");
    let font = tokens.font("--body");
    let options = LowerOptions {
        id_base: 100,
        tokens: &tokens,
    };
    let width = "25%";
    let template = html! { <Stack width={width} axis="vertical" padding="4">
        <Text color="--accent" font="--body">Header</Text><Image/><Button/><TextInput/>
        <Toggle/><List>{["A","B"].into_iter().map(todo_row)}</List><Scroll/><Spacer/><Overlay><Modal><BottomSheet/></Modal></Overlay>
    </Stack> };
    let ops = lower(&template, &options).unwrap();
    assert_eq!(ops, lower(&template, &options).unwrap());
    let mut created = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut props = Vec::new();
    for op in &ops {
        assert_eq!(op.web, WebOperationT::NONE);
        match &op.ui {
            UiOperationT::CreateNode(n) => {
                assert!(created.insert(n.node_id));
                kinds.insert(n.kind.0);
            }
            UiOperationT::SetProp(p) => {
                assert!(created.contains(&p.node_id));
                props.push(&p.value);
            }
            UiOperationT::SetText(text) => assert!(created.contains(&text.node_id)),
            UiOperationT::AppendChild(edge) => {
                assert!(created.contains(&edge.parent_id));
                assert!(created.contains(&edge.child_id));
            }
            unit => panic!("unexpected core unit {unit:?}"),
        }
    }
    assert_eq!(created.len(), 14);
    assert_eq!(kinds.len(), 12);
    assert!(props
        .iter()
        .any(|v| matches!(v, PropValueT::ColorToken(v) if v.id == color)));
    assert!(props
        .iter()
        .any(|v| matches!(v, PropValueT::FontToken(v) if v.id == font)));
    assert!(props.iter().any(|v| matches!(v, PropValueT::Length(v) if v.v.as_ref().is_some_and(|n| n.unit == LengthUnit::Percent && n.value.to_bits() == 25.0_f32.to_bits()))));
    let bad = "a lot";
    let invalid = html! { <Stack width={bad}/> };
    assert!(matches!(
        lower(&invalid, &options),
        Err(LowerError::BadProp { .. })
    ));
}

#[action(server)]
async fn add_and_replace(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    let next = event.state + 1;
    let out = ctx.receiver();
    html_add! { 18u32, ctx, out, Some(&next), <Text>{next}</Text> }?;
    html_swap! { 22u32, ctx, out, Some(&next), <Stack><Button primal::state on:tap={add_and_replace}/></Stack> }?;
    Ok(())
}

#[foundation_core::valtron::valtron_test]
async fn dispatcher_returns_add_and_swap_with_their_signed_state_in_one_frame() {
    use super::dispatch_support::request;
    use foundation_ui_runtime::action::{ActionRegistry, Origin};
    use foundation_ui_runtime::dispatch::{Connection, DispatcherBuilder};
    use foundation_ui_runtime::state::{
        install_state_signer, sign_state, verify_state, HmacStateSigner, StateConfig,
    };
    use foundation_ui_wire::{read_frame, writer::ClientCaps, PlacementKind, WireLimits};
    use std::sync::Arc;

    let (ctx, _) = test_ctx(WALL);
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([9; 32], StateConfig::default()).unwrap()),
    );
    let mut registry = ActionRegistry::new();
    registry.register(add_and_replace).unwrap();
    let dispatcher = DispatcherBuilder::new(Origin::Server, Arc::new(registry)).build();
    let connection = Connection::stateless(ClientCaps::web_current(), MountSurface::Web, 7);
    let req = request(
        "server:add_and_replace",
        22,
        Some(sign_state(&ctx, b"1").unwrap()),
        4,
    );
    let bytes = dispatcher.dispatch(&ctx, &connection, &req).await;
    let frame = read_frame(&bytes, &WireLimits::default()).unwrap();
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), 4);
    let messages = body.messages().unwrap();
    assert_eq!(messages.len(), 2);
    for (message, (kind, target, count)) in messages
        .iter()
        .zip([(PlacementKind::Append, 18, 1), (PlacementKind::Swap, 22, 2)])
    {
        let place = message.operation().unwrap().ui_as_place().unwrap();
        assert_eq!(place.kind(), kind);
        assert_eq!(place.target().unwrap().node_id(), target);
        let ops = place.operations().unwrap();
        let targets = ops
            .iter()
            .filter_map(|op| op.ui_as_attach_state())
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), count);
        for state in targets {
            assert!(ops.iter().any(|op| op
                .ui_as_create_node()
                .is_some_and(|node| node.node_id() == state.root_node())));
            assert_eq!(
                verify_state(&ctx, state.state().unwrap().bytes()).unwrap(),
                b"2"
            );
        }
    }
}

#[test]
fn behaviour_markers_apply_to_the_parent_and_script_has_one_operation() {
    use foundation_ui_runtime::view::{lower, LowerOptions, TokenTable};
    use foundation_ui_wire::{BehaviourId, UiOperationT, WebOperationT};

    let tree = html! { <div>
        <span data-ewe-behaviour="dialog"/>
        <span data-ewe-behaviour="focus_trap"/>
        <script primal:script>"setup()"</script>
    </div> };
    let ops = lower(
        &tree,
        &LowerOptions {
            id_base: 10,
            tokens: &TokenTable::new(),
        },
    )
    .unwrap();
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(&op.web, WebOperationT::WebElement(_)))
            .count(),
        1
    );
    assert_eq!(ops.iter().filter(|op| matches!(&op.ui, UiOperationT::UiBehaviour(b) if b.node_id == 10 && b.behaviour == BehaviourId::DIALOG)).count(),1);
    assert_eq!(ops.iter().filter(|op| matches!(&op.web, WebOperationT::WebBehaviour(b) if b.node_id == 10 && b.custom.as_deref() == Some("focus_trap"))).count(),1);
    assert_eq!(ops.iter().filter(|op| matches!(&op.web, WebOperationT::ScriptNode(s) if s.parent_id == 10 && s.body.as_deref() == Some("setup()"))).count(),1);
}

#[test]
fn missing_root_is_an_action_error_and_none_matches_the_two_field_form() {
    use foundation_ui_runtime::action::prepare_messages;
    use foundation_ui_runtime::view::LowerError;
    use foundation_ui_wire::ErrorCode;

    fn emit(ctx: &Context, explicit: bool) -> ActionResult {
        if explicit {
            html! {ctx, ctx.receiver(), None, <Stack/>};
        } else {
            html! {ctx, ctx.receiver(), <Stack/>};
        }
        Ok(())
    }

    let (base, _) = test_ctx(WALL);
    let bad = Context::for_action(base, MountSurface::Web, None);
    let error = emit(&bad, true).unwrap_err();
    assert_eq!(error.current_context().code, ErrorCode::ActionFailed);
    assert_eq!(
        error.current_context().message,
        LowerError::NoComponentRoot.to_string()
    );
    assert!(bad.into_receiver().drain().is_empty());

    let render = |explicit| {
        let (base, _) = test_ctx(WALL);
        let ctx = Context::for_render(base.clone());
        emit(&ctx, explicit).unwrap();
        prepare_messages(&base, ctx.into_receiver().drain()).unwrap()
    };
    assert_eq!(render(false), render(true));
}

#[test]
fn mount_appends_the_template_and_attaches_verifiable_state() {
    use foundation_ui_runtime::action::prepare_messages;
    use foundation_ui_runtime::state::{
        install_state_signer, verify_state, HmacStateSigner, StateConfig,
    };
    use foundation_ui_runtime::view::{mount, LowerOptions, TokenTable};
    use foundation_ui_wire::UiOperationT;
    use std::sync::Arc;
    let (base, _) = test_ctx(WALL);
    install_state_signer(
        &base,
        Arc::new(HmacStateSigner::new([5; 32], StateConfig::default()).unwrap()),
    );
    let ctx = Context::for_render(base.clone());
    let options = LowerOptions {
        id_base: 40,
        tokens: &TokenTable::new(),
    };
    mount(
        &html! {<Text>hi</Text>},
        &options,
        3,
        ctx.receiver(),
        Some(&8u32),
    )
    .unwrap();
    let messages = prepare_messages(&base, ctx.into_receiver().drain()).unwrap();
    let units = messages
        .iter()
        .filter_map(|m| m.operation.as_ref())
        .map(|op| &op.ui)
        .collect::<Vec<_>>();
    assert!(units
        .iter()
        .any(|u| matches!(u,UiOperationT::AppendChild(p) if p.parent_id == 3 && p.child_id == 40)));
    assert!(
        matches!(units.last(), Some(UiOperationT::AttachState(s)) if s.root_node == 40 && verify_state(&base,s.state.as_ref().unwrap()).unwrap() == b"8")
    );
}

#[test]
fn web_and_core_nodes_are_created_before_every_reference_across_tree_shapes() {
    use foundation_ui_runtime::view::{lower, LowerOptions, TokenTable};
    use foundation_ui_wire::{EventId, UiOperationT, WebOperationT};
    use std::collections::BTreeSet;
    let trees = [
        html! {<Stack><Text>A</Text></Stack>},
        html! {<button on:tap="host:open_picker">Pick</button>},
        html! {<div><Stack><span>text</span></Stack></div>},
        html! {<Stack><List><div><Text>Deep</Text></div></List></Stack>},
        html! {<List>{["one","two","three","four"].into_iter().map(todo_row)}</List>},
    ];
    for (shape, tree) in trees.into_iter().enumerate() {
        let mut known = BTreeSet::new();
        let ops = lower(
            &tree,
            &LowerOptions {
                id_base: 40,
                tokens: &TokenTable::default(),
            },
        )
        .unwrap();
        let mut binds = 0;
        for op in ops {
            match op.web {
                WebOperationT::WebElement(n) => assert!(known.insert(n.node_id)),
                WebOperationT::WebText(n) => assert!(known.insert(n.node_id)),
                WebOperationT::WebRegisterNode(n) => assert!(known.contains(&n.node_id)),
                WebOperationT::WebSetAttribute(n) => assert!(known.contains(&n.node_id)),
                WebOperationT::NONE => (),
                n => panic!("unexpected web operation {n:?}"),
            }
            match op.ui {
                UiOperationT::CreateNode(n) => assert!(known.insert(n.node_id)),
                UiOperationT::SetText(n) => assert!(known.contains(&n.node_id)),
                UiOperationT::AppendChild(edge) => {
                    assert!(known.contains(&edge.parent_id));
                    assert!(known.contains(&edge.child_id));
                }
                UiOperationT::BindAction(b) => {
                    assert!(known.contains(&b.node_id));
                    assert_eq!(
                        (b.node_id, b.event, b.action.as_deref()),
                        (40, EventId::TAP, Some("host:open_picker"))
                    );
                    binds += 1;
                }
                UiOperationT::NONE => (),
                n => panic!("unexpected shared operation {n:?}"),
            }
        }
        assert_eq!(binds, usize::from(shape == 1));
    }
}

#[test]
fn either_builds_only_the_selected_children_and_preserves_their_bindings() -> ActionResult {
    use foundation_ui_runtime::action::prepare_messages;
    use foundation_ui_runtime::state::{
        install_state_signer, verify_state, HmacStateSigner, StateConfig,
    };
    use foundation_ui_wire::UiOperationT;
    use std::cell::Cell;
    use std::sync::Arc;

    for selected in [false, true] {
        let (base, _) = test_ctx(WALL);
        install_state_signer(
            &base,
            Arc::new(HmacStateSigner::new([9; 32], StateConfig::default()).unwrap()),
        );
        let ctx = Context::for_action(base.clone(), MountSurface::Native, Some(40));
        let calls = Cell::new(0);
        let mut visits = Vec::new();
        let tree = html! { ctx, ctx.receiver(), Some(&7),
            <Stack>
                <Text>Before</Text>
                <Either condition={calls.set(calls.get() + 1); selected}>
                    <Left>
                        <Button primal::state on:tap={save}>{{visits.push(false); "Save"}}</Button>
                        <Text>Left</Text>
                    </Left>
                    <Right>
                        <Button primal::state on:tap="host:save">{{visits.push(true); "Save"}}</Button>
                        <Text>Right</Text>
                    </Right>
                </Either>
                <Text>After</Text>
            </Stack>
        };
        assert_eq!(calls.get(), 1);
        assert_eq!(visits, [selected]);
        assert_eq!(tree.children.len(), 4);
        let messages = prepare_messages(&base, ctx.into_receiver().drain()).unwrap();
        let UiOperationT::Place(place) = &messages[0].operation.as_ref().unwrap().ui else {
            panic!("missing placement")
        };
        let ops = place.operations.as_ref().unwrap();
        let labels: Vec<_> = ops
            .iter()
            .filter_map(|op| match &op.ui {
                UiOperationT::SetText(text) => text.text.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            [
                "Before",
                "Save",
                if selected { "Right" } else { "Left" },
                "After"
            ]
        );
        let buttons: Vec<_> = ops
            .iter()
            .filter_map(|op| match &op.ui {
                UiOperationT::BindAction(binding) => Some(binding),
                _ => None,
            })
            .collect();
        assert_eq!(buttons.len(), 1);
        let button = buttons[0];
        assert_eq!(
            button.action.as_deref(),
            Some(if selected { "host:save" } else { "server:save" })
        );
        let signed = ops
            .iter()
            .find_map(|op| match &op.ui {
                UiOperationT::AttachState(state) if state.root_node == button.node_id => {
                    Some(state)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            verify_state(&base, signed.state.as_deref().unwrap()).unwrap(),
            b"7"
        );
    }
    Ok(())
}

#[test]
fn either_supports_a_root_or_empty_branch_and_nested_conditions() {
    let root = html! { <Either condition={true}><Right><Button/></Right><Left/></Either> };
    assert_eq!(root.tag.unwrap().name(), Some("Button"));
    let nested = html! { <Stack>
        <Either condition={true}><Left/><Right>
            <Either condition={false}><Right/><Left><Text>Selected</Text><Text>Second</Text></Left></Either>
        </Right></Either>
        <Either condition={false}><Left/><Right><Text>Skipped</Text></Right></Either>
    </Stack> };
    assert_eq!(nested.children.len(), 2);
    assert_eq!(
        nested.children[0].children[0].text.as_deref(),
        Some("Selected")
    );
}

#[test]
fn shared_control_keeps_utility_names_and_unsupported_styles() {
    use foundation_ui_runtime::view::{lower, LowerOptions, TokenTable};
    use foundation_ui_wire::{PropId, PropValueT, UiOperationT};
    let view =
        html! { <Text class="text-primary p-md ios-p-lg android-p-sm web-only">Buy milk</Text> };
    let ops = lower(
        &view,
        &LowerOptions {
            id_base: 10,
            tokens: &TokenTable::default(),
        },
    )
    .unwrap();
    assert!(ops
        .iter()
        .any(|op| matches!(&op.ui, UiOperationT::SetProp(prop)
        if prop.prop == PropId::UTILITY_CLASSES && matches!(&prop.value, PropValueT::Text(text)
            if text.v.as_deref() == Some("text-primary p-md ios-p-lg android-p-sm web-only")))));
}
