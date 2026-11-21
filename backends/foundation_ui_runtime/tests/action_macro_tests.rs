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
