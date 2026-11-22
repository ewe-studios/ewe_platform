use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{
    encode_updates, invoke_action, Action, ActionError, ActionRegistry, ActionResult, Context,
    Interaction, InvokeError, Origin, Scope, Surface,
};
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, HmacStateSigner, StateConfig,
};
use foundation_ui_wire::writer::{ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{read_frame, ActionRequestT, ErrorCode, WireLimits};
use serde::{Deserialize, Serialize};

use super::support::{test_ctx, WALL};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Counter {
    current: u32,
}

#[action]
async fn increment_counter(ctx: &Context, event: Interaction<Counter>) -> ActionResult {
    let current = event.state.current.checked_add(1).ok_or_else(|| {
        ErrorTrace::new(ActionError::new(ErrorCode::ActionFailed, "Counter is full"))
    })?;
    ctx.receiver().state(event.node_id, &Counter { current })
}

#[action(server)]
async fn save(_ctx: &Context, _event: Interaction<()>) -> Result<(), ErrorTrace<ActionError>> {
    Ok(())
}

#[action(host)]
async fn r#type(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

mod web {
    use super::{action, ActionResult, Context, Interaction};

    /// Opens the web picker.
    #[action(host, web)]
    pub(super) async fn picker(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
        Ok(())
    }
}

mod mobile {
    use super::{action, ActionResult, Context, Interaction};

    #[action(host, mobile)]
    pub(super) async fn picker(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
        Ok(())
    }
}

#[action(server, web)]
async fn on_web(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[action(server, mobile)]
async fn on_mobile(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[test]
fn declarations_select_the_expected_name_origin_scope_and_state() {
    fn has_state<A: Action<State = S>, S>(_action: A) {}

    has_state::<_, Counter>(increment_counter);
    has_state::<_, ()>(save);
    assert_eq!(increment_counter::origin(), None);
    assert_eq!(increment_counter::scope(), Scope::All);
    assert_eq!(increment_counter::NAME, "increment_counter");
    assert_eq!(save::origin(), Some(Origin::Server));
    assert_eq!(save::scope(), Scope::All);
    assert_eq!(r#type::NAME, "type");
    assert_eq!(r#type::origin(), Some(Origin::Host));
    assert_eq!(r#type::scope(), Scope::All);

    let mut registry = ActionRegistry::new();
    registry.register(web::picker).unwrap();
    registry.register(mobile::picker).unwrap();
    for (surface, scope) in [(Surface::Web, Scope::Web), (Surface::Native, Scope::Mobile)] {
        let action = registry.get(Origin::Host, "picker", surface).unwrap();
        assert_eq!(action.scope(), scope);
        assert_eq!(action.origin(), Some(Origin::Host));
    }
    assert_eq!(
        (on_web::origin(), on_web::scope()),
        (Some(Origin::Server), Scope::Web)
    );
    assert_eq!(
        (on_mobile::origin(), on_mobile::scope()),
        (Some(Origin::Server), Scope::Mobile)
    );
}

#[valtron_test]
async fn registered_action_returns_signed_state_and_preserves_handler_errors() {
    let (ctx, _) = test_ctx(WALL);
    let signer = HmacStateSigner::new([1; 32], StateConfig::default()).unwrap();
    install_state_signer(&ctx, Arc::new(signer));
    let mut registry = ActionRegistry::new();
    registry.register(increment_counter).unwrap();
    let mut request = ActionRequestT::default();
    request.node_id = 42;
    request.state = Some(sign_state(&ctx, br#"{"current":1}"#).unwrap());
    let entry = registry
        .get(Origin::Host, "increment_counter", Surface::Web)
        .unwrap();
    let items = invoke_action(&ctx, entry, Surface::Web, Some(40), request.clone())
        .await
        .unwrap();
    let frame = encode_updates(
        &ctx,
        items,
        &ClientCaps::web_current(),
        FrameHeader {
            stream_id: 1,
            sequence_id: 2,
            ack: 1,
        },
        &WriterConfig::default(),
    )
    .unwrap();
    let messages = read_frame(&frame, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    assert_eq!(messages.len(), 1);
    let attachment = messages
        .get(0)
        .operation()
        .unwrap()
        .ui_as_attach_state()
        .unwrap();
    assert_eq!(attachment.root_node(), 42);
    let signed = attachment.state().unwrap().bytes();
    let body = verify_state(&ctx, signed).unwrap();
    assert_eq!(
        serde_json::from_slice::<Counter>(&body).unwrap(),
        Counter { current: 2 }
    );

    request.state = Some(sign_state(&ctx, br#"{"current":4294967295}"#).unwrap());
    let entry = registry
        .get(Origin::Server, "increment_counter", Surface::Web)
        .unwrap();
    let result = invoke_action(&ctx, entry, Surface::Web, Some(40), request).await;
    assert!(matches!(result, Err(InvokeError::Action(error))
        if error.current_context().code == ErrorCode::ActionFailed
        && error.current_context().message == "Counter is full"));
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct TodoInput {
    title: String,
    counter: Counter,
}

#[action]
async fn save_input(ctx: &Context, event: Interaction<Counter, TodoInput>) -> ActionResult {
    ctx.store((event.state.current, event.input_value));
    Ok(())
}

type PlainText = String;

#[action]
async fn save_text(ctx: &Context, event: Interaction<(), PlainText>) -> ActionResult {
    ctx.store(event.input_value);
    Ok(())
}

#[action]
async fn save_unit(ctx: &Context, event: Interaction<(), TodoInput>) -> ActionResult {
    ctx.store(event.input_value);
    Ok(())
}

#[valtron_test]
async fn typed_action_decodes_the_declared_input_after_verifying_state() {
    use foundation_ui_wire::ActionInputT;
    let (ctx, _) = test_ctx(WALL);
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([9; 32], StateConfig::default()).unwrap()),
    );
    let mut registry = ActionRegistry::new();
    registry.register(save_input).unwrap();
    registry.register(save_unit).unwrap();
    registry.register(save_text).unwrap();
    let signed = sign_state(&ctx, br#"{"current":42}"#).unwrap();
    let payload = br#"{"title":"Buy milk","counter":{"current":7}}"#;
    let make = |value: Option<&str>, json: Option<&[u8]>, state: Option<Vec<u8>>| {
        let mut input = ActionInputT::default();
        input.value = value.map(str::to_owned);
        input.json = json.map(<[u8]>::to_vec);
        let mut req = ActionRequestT::default();
        req.input = Some(Box::new(input));
        req.state = state;
        req
    };
    for origin in [Origin::Host, Origin::Server] {
        let entry = registry.get(origin, "save_input", Surface::Native).unwrap();
        let req = make(None, Some(payload), Some(signed.clone()));
        invoke_action(&ctx, entry, Surface::Native, None, req)
            .await
            .unwrap();
        let received = ctx.get::<(u32, TodoInput)>().unwrap();
        assert_eq!(received.0, 42);
        assert_eq!(received.1, serde_json::from_slice(payload).unwrap());
    }
    let entry = registry
        .get(Origin::Host, "save_unit", Surface::Web)
        .unwrap();
    for (text, json) in [
        (None, None),
        (Some("text"), None),
        (Some("both"), Some(payload.as_slice())),
        (None, Some(b"".as_slice())),
        (None, Some(b"{".as_slice())),
        (None, Some(b"{}".as_slice())),
    ] {
        let result = invoke_action(&ctx, entry, Surface::Web, None, make(text, json, None)).await;
        assert!(
            matches!(result, Err(InvokeError::Action(e)) if e.current_context().code == ErrorCode::InvalidInput)
        );
        assert!(ctx.get::<TodoInput>().is_none());
    }
    invoke_action(
        &ctx,
        entry,
        Surface::Web,
        None,
        make(None, Some(payload), None),
    )
    .await
    .unwrap();
    assert_eq!(ctx.get::<TodoInput>().unwrap().title, "Buy milk");
    let text = registry
        .get(Origin::Host, "save_text", Surface::Web)
        .unwrap();
    for value in [None, Some(""), Some("plain"), Some("\"quoted\"")] {
        invoke_action(&ctx, text, Surface::Web, None, make(value, None, None))
            .await
            .unwrap();
        assert_eq!(ctx.get::<String>().unwrap().as_str(), value.unwrap_or(""));
    }
    assert!(
        matches!(invoke_action(&ctx, text, Surface::Web, None, make(None, Some(payload), None)).await,
        Err(InvokeError::Action(e)) if e.current_context().code == ErrorCode::InvalidInput)
    );
}

#[test]
fn only_one_callable_can_fill_an_exact_declaration() {
    use foundation_ui_runtime::action::RegistrationError;
    let mut registry = ActionRegistry::new();
    registry
        .declare(Some(Origin::Host), "picker", Scope::Web)
        .unwrap();
    registry.register(web::picker).unwrap();
    registry.register(mobile::picker).unwrap();
    assert_eq!(
        registry.register(web::picker),
        Err(RegistrationError::Duplicate)
    );
    assert_eq!(
        registry.declare(Some(Origin::Host), "picker", Scope::All),
        Err(RegistrationError::Duplicate)
    );
    registry
        .declare(Some(Origin::Server), "picker", Scope::All)
        .unwrap();
    assert!(
        registry
            .lookup(Origin::Host, "picker", Surface::Web)
            .unwrap()
            .local
    );
    assert!(
        !registry
            .lookup(Origin::Server, "picker", Surface::Web)
            .unwrap()
            .local
    );
}
