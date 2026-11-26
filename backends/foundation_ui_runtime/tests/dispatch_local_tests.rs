use std::future::poll_fn;
use std::sync::Arc;
use std::task::Poll;

use foundation_core::valtron::valtron_test;
use foundation_ui_runtime::action::{
    ActionManager, ActionRegistry, ActionResult, ActionTransport, Context, Interaction, Origin,
    Surface, TriggerEvent,
};
use foundation_ui_runtime::dispatch::{
    Connection, DispatcherBuilder, HelloOutcome, LocalDispatcher,
};
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, HmacStateSigner, StateConfig,
};
use foundation_ui_wire::writer::ClientCaps;
use foundation_ui_wire::{read_frame, ActionInputT, ClientHelloT, FrameRequestT, WireLimits};

use super::dispatch_support::{envelope, request};
use super::support::{test_ctx, WALL};

#[foundation_ui_runtime::action(host)]
async fn toggle(ctx: &Context, event: Interaction<bool>) -> ActionResult {
    let mut waiting = true;
    poll_fn(|task| {
        if waiting {
            waiting = false;
            task.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
    ctx.receiver().state(event.node_id, &!event.state)
}

fn registry() -> Arc<ActionRegistry> {
    let mut registry = ActionRegistry::new();
    registry.register(toggle).unwrap();
    Arc::new(registry)
}

#[valtron_test]
async fn manager_triggers_local_action() {
    let (ctx, _) = test_ctx(WALL);
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([8; 32], StateConfig::default()).unwrap()),
    );
    let registry = registry();
    let dispatcher = DispatcherBuilder::new(Origin::Host, registry.clone()).build();
    let connection = Connection::stateless(ClientCaps::web_current(), Surface::Web, 7);
    let local = LocalDispatcher::new(dispatcher, ctx.clone(), connection);
    let manager = ActionManager::new(registry, Surface::Web, 7, WireLimits::default())
        .with_host(Arc::new(local));
    let event = TriggerEvent {
        node_id: 42,
        input: ActionInputT::default(),
        state: sign_state(&ctx, b"false").unwrap(),
    };
    let reply = manager
        .trigger("host:toggle", event)
        .await
        .unwrap()
        .unwrap();
    let frame = read_frame(&reply, &WireLimits::default()).unwrap();
    assert_eq!(frame.stream_id(), 7);
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), 1);
    let messages = body.messages().unwrap();
    assert_eq!(messages.len(), 1);
    let node = messages
        .get(0)
        .operation()
        .unwrap()
        .ui_as_attach_state()
        .unwrap();
    assert_eq!(node.root_node(), 42);
    assert_eq!(
        verify_state(&ctx, node.state().unwrap().bytes()).unwrap(),
        b"true"
    );
}

#[valtron_test]
async fn local_transport_admits_requests_in_send_order_even_when_polled_in_reverse() {
    let (ctx, _) = test_ctx(WALL);
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([8; 32], StateConfig::default()).unwrap()),
    );
    let dispatcher = DispatcherBuilder::new(Origin::Host, registry()).build();
    let caps = ClientCaps::web_current();
    let mut hello = ClientHelloT::default();
    hello.vocabulary_version = caps.vocabulary_version;
    hello.platform = caps.platform;
    hello.mount = caps.mount;
    let mut handshake = FrameRequestT::default();
    handshake.hello = Some(Box::new(hello));
    let HelloOutcome::Accepted { connection, .. } =
        dispatcher.hello(&ctx, &envelope(handshake, 0)).unwrap()
    else {
        panic!("The host must accept its current versions");
    };
    let local = LocalDispatcher::new(dispatcher, ctx.clone(), connection);
    let signed = sign_state(&ctx, b"false").unwrap();
    let first = local.send(request("host:toggle", 42, Some(signed.clone()), 1));
    let second = local.send(request("host:toggle", 42, Some(signed), 2));
    for (seq, bytes) in [
        (2, second.await.unwrap().unwrap()),
        (1, first.await.unwrap().unwrap()),
    ] {
        let frame = read_frame(&bytes, &WireLimits::default()).unwrap();
        let body = frame.kind_as_frame_message().unwrap();
        assert_eq!(body.ack(), seq);
        let message = body.messages().unwrap().get(0);
        assert!(message.operation().unwrap().ui_as_attach_state().is_some());
    }
}
