use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::task::{Context as TaskContext, Poll, Waker};
use std::{future::Future, pin::pin};

use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{
    ActionError, ActionRegistry, ActionResult, Context, Interaction, Origin, Surface,
};
use foundation_ui_runtime::dispatch::{Connection, Dispatcher, DispatcherBuilder, HelloOutcome};
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, with_state_binding, HmacStateSigner, StateConfig,
};
use foundation_ui_wire::vocab::VOCABULARY_VERSION;
use foundation_ui_wire::writer::{ClientCaps, WriterConfig};
use foundation_ui_wire::{
    finish_frame, read_frame, ClientHelloT, ControlT, ErrorCode, FrameKindT, FrameRequestT,
    MountSurface, Platform, WireLimits, SCHEMA_VERSION,
};

use super::dispatch_support::{envelope, request};
use super::support::{test_ctx, TTL, WALL};

#[action(server, web)]
async fn count(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    ctx.inner()
        .get::<AtomicUsize>()
        .unwrap()
        .fetch_add(1, Ordering::Relaxed);
    ctx.receiver().state(event.node_id, &event.state)
}

#[action(server)]
async fn ping(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    ctx.inner()
        .get::<AtomicUsize>()
        .unwrap()
        .fetch_add(1, Ordering::Relaxed);
    Ok(())
}

#[action(server)]
async fn fail_after_queue(ctx: &Context, event: Interaction<()>) -> ActionResult {
    ctx.receiver().data(event.node_id, vec![1]);
    ctx.receiver().state(event.node_id, &2)?;
    Err(
        ErrorTrace::new(ActionError::new(ErrorCode::ActionFailed, "title required"))
            .attach("host-only detail"),
    )
}

#[action(server)]
async fn oversized(ctx: &Context, event: Interaction<()>) -> ActionResult {
    ctx.receiver().data(event.node_id, vec![1]);
    ctx.receiver()
        .state(event.node_id, &"longer than the configured state limit")
}

fn dispatcher(config: WriterConfig) -> Dispatcher {
    let mut registry = ActionRegistry::new();
    registry.register(count).unwrap();
    registry.register(ping).unwrap();
    registry.register(fail_after_queue).unwrap();
    registry.register(oversized).unwrap();
    DispatcherBuilder::new(Origin::Server, Arc::new(registry))
        .writer_config(config)
        .build()
}

fn stateless() -> Connection {
    Connection::stateless(ClientCaps::web_current(), Surface::Web, 7)
}

fn control(bytes: &[u8], ack: u64) -> ControlT {
    let frame = read_frame(bytes, &WireLimits::default()).unwrap();
    let reply = frame.kind_as_frame_message().unwrap();
    assert_eq!(reply.ack(), ack);
    let messages = reply.messages().unwrap();
    assert_eq!(messages.len(), 1);
    let message = messages.get(0);
    assert!(message.data().is_none() && message.operation().is_none());
    message.control().unwrap().unpack()
}

#[valtron_test]
async fn lookup_schema_and_input_failures_stop_before_the_handler() {
    let (ctx, _) = test_ctx(WALL);
    ctx.store(AtomicUsize::new(0));
    let dispatcher = dispatcher(WriterConfig::default());
    let connection = stateless();
    let mobile = Connection::stateless(
        ClientCaps::native_current(Platform::Ios),
        Surface::Native,
        7,
    );
    for (reference, conn, expected) in [
        ("count", &connection, ErrorCode::BadRequest),
        ("host:count", &connection, ErrorCode::UnavailableOrigin),
        ("server:missing", &connection, ErrorCode::UnknownAction),
        ("server:count", &mobile, ErrorCode::UnsupportedAction),
        ("server:count", &connection, ErrorCode::BadRequest),
    ] {
        let reply = dispatcher
            .dispatch(&ctx, conn, &request(reference, 42, None, 1))
            .await;
        assert_eq!(control(&reply, 1).error.unwrap().code, expected);
    }
    let valid = request("server:ping", 42, None, 5);
    let mut frame = read_frame(&valid, &WireLimits::default()).unwrap().unpack();
    frame.schema_version = SCHEMA_VERSION + 1;
    let reply = dispatcher
        .dispatch(&ctx, &connection, &finish_frame(&frame).unwrap())
        .await;
    let update = control(&reply, 5).update_required.unwrap();
    assert_eq!(
        (update.client_schema_version, update.server_schema_version),
        (SCHEMA_VERSION + 1, SCHEMA_VERSION)
    );
    let wrong_kind = dispatcher.dispatch(&ctx, &connection, &reply).await;
    let known_ack = read_frame(&reply, &WireLimits::default())
        .unwrap()
        .sequence_id();
    assert_eq!(
        control(&wrong_kind, known_ack).error.unwrap().code,
        ErrorCode::BadRequest
    );
    assert_eq!(ctx.get::<AtomicUsize>().unwrap().load(Ordering::Relaxed), 0);
}

#[valtron_test]
async fn state_failures_keep_the_action_and_its_output_private() {
    let (ctx, clock) = test_ctx(WALL);
    ctx.store(AtomicUsize::new(0));
    let dispatcher = dispatcher(WriterConfig::default());
    let conn = stateless();
    let missing = dispatcher
        .dispatch(&ctx, &conn, &request("server:count", 42, Some(vec![1]), 7))
        .await;
    assert_eq!(
        control(&missing, 7).error.unwrap().code,
        ErrorCode::ActionFailed
    );
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([3; 32], StateConfig::default().max_body_bytes(10)).unwrap()),
    );
    let alice = with_state_binding(&ctx, b"Alice").unwrap();
    let bob = with_state_binding(&ctx, b"Bob").unwrap();
    let signed = sign_state(&alice, b"1").unwrap();
    let mut forged = signed.clone();
    *forged.last_mut().unwrap() ^= 1;
    for (caller, blob) in [
        (&bob, signed.clone()),
        (&alice, forged),
        (&alice, sign_state(&alice, b"false").unwrap()),
    ] {
        let reply = dispatcher
            .dispatch(caller, &conn, &request("server:count", 42, Some(blob), 8))
            .await;
        assert_eq!(
            control(&reply, 8).error.unwrap().code,
            ErrorCode::StateRejected
        );
    }
    clock.set_elapsed(TTL + 1);
    let expired = dispatcher
        .dispatch(&alice, &conn, &request("server:count", 42, Some(signed), 9))
        .await;
    let reply = control(&expired, 9);
    assert!(reply.error.is_none());
    assert_eq!(reply.state_expired.unwrap().token_issued_at, 0);
    assert_eq!(ctx.get::<AtomicUsize>().unwrap().load(Ordering::Relaxed), 0);

    for (name, text) in [
        ("server:fail_after_queue", "title required"),
        ("server:oversized", "action failed"),
    ] {
        let reply = dispatcher
            .dispatch(&alice, &conn, &request(name, 42, None, 10))
            .await;
        let error = control(&reply, 10).error.unwrap();
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.message.as_deref(), Some(text));
    }
}

#[valtron_test]
async fn limits_always_leave_room_for_a_protocol_error() {
    let (ctx, _) = test_ctx(WALL);
    for limit in [1, 64] {
        let config = WriterConfig::default().limits(WireLimits::default().max_frame_bytes(limit));
        let reply = dispatcher(config)
            .dispatch(&ctx, &stateless(), &request("server:ping", 42, None, 5))
            .await;
        assert_eq!(
            control(&reply, 0).error.unwrap().code,
            ErrorCode::BadRequest
        );
    }
    let config = WriterConfig::default().limits(WireLimits::default().max_action_request_bytes(1));
    let reply = dispatcher(config)
        .dispatch(&ctx, &stateless(), &request("server:ping", 42, None, 6))
        .await;
    assert_eq!(
        control(&reply, 0).error.unwrap().code,
        ErrorCode::BadRequest
    );
}

fn connected(dispatcher: &Dispatcher, ctx: &Ctx) -> Connection {
    let mut hello = ClientHelloT::default();
    hello.platform = Platform::Web;
    hello.mount = MountSurface::Web;
    hello.vocabulary_version = VOCABULARY_VERSION;
    let mut req = FrameRequestT::default();
    req.hello = Some(Box::new(hello));
    match dispatcher.hello(ctx, &envelope(req, 0)).unwrap() {
        HelloOutcome::Accepted { connection, .. } => connection,
        HelloOutcome::UpdateRequired { .. } => panic!("the current client must negotiate"),
    }
}

#[test]
fn concurrent_duplicate_runs_once_and_stateless_requests_run_independently() {
    let (ctx, _) = test_ctx(WALL);
    ctx.store(AtomicUsize::new(0));
    let dispatcher = dispatcher(WriterConfig::default());
    for (conn, expected_calls) in [(connected(&dispatcher, &ctx), 1), (stateless(), 2)] {
        let barrier = Barrier::new(2);
        let bytes = request("server:ping", 42, None, 1);
        let run = || {
            barrier.wait();
            // This handler completes on its first poll.
            let mut future = pin!(dispatcher.dispatch(&ctx, &conn, &bytes));
            let Poll::Ready(reply) = future
                .as_mut()
                .poll(&mut TaskContext::from_waker(Waker::noop()))
            else {
                panic!("the handler unexpectedly suspended");
            };
            let frame = read_frame(&reply, &WireLimits::default()).unwrap();
            let FrameKindT::FrameMessage(reply) = frame.unpack().kind else {
                panic!("expected reply")
            };
            assert_eq!(reply.ack, 1);
            if let Some(message) = reply.messages.unwrap().into_iter().next() {
                assert_eq!(
                    message.control.unwrap().error.unwrap().code,
                    ErrorCode::BadRequest
                );
            }
            frame.sequence_id()
        };
        std::thread::scope(|scope| {
            let other = scope.spawn(run);
            let first = run();
            assert_ne!(first, other.join().unwrap());
        });
        assert_eq!(
            ctx.get::<AtomicUsize>().unwrap().swap(0, Ordering::Relaxed),
            expected_calls
        );
    }
}

#[action(server)]
async fn render_view(ctx: &Context, event: Interaction<()>) -> ActionResult {
    ctx.receiver().data(event.node_id, vec![1]);
    ctx.receiver().render_op(
        ctx.inner()
            .get_cloned::<foundation_ui_wire::OperationT>()
            .unwrap(),
    );
    Ok(())
}

#[valtron_test]
async fn raw_view_operations_cannot_supply_state_attachments() {
    use foundation_ui_wire::writer::columnar_batch;
    use foundation_ui_wire::{AttachStateT, OperationT, ReplaceSubtreeT, SetTextT, UiOperationT};

    fn simple(ui: UiOperationT) -> OperationT {
        let mut op = OperationT::default();
        op.ui = ui;
        op
    }

    fn packed(row: UiOperationT) -> OperationT {
        let mut op = OperationT::default();
        op.columnar = Some(Box::new(columnar_batch(&[row]).unwrap()));
        op
    }

    fn nested(op: OperationT) -> OperationT {
        let mut tree = ReplaceSubtreeT::default();
        tree.node_id = 42;
        tree.operations = Some(vec![op]);
        simple(UiOperationT::ReplaceSubtree(Box::new(tree)))
    }

    let (ctx, _) = test_ctx(WALL);
    let mut registry = ActionRegistry::new();
    registry.register(render_view).unwrap();
    let dispatcher = DispatcherBuilder::new(Origin::Server, Arc::new(registry)).build();
    let connection = stateless();
    let mut attachment = AttachStateT::default();
    attachment.root_node = 42;
    attachment.state = Some(vec![1, 2, 3]);
    let row = UiOperationT::AttachState(Box::new(attachment));
    let forged = [
        simple(row.clone()),
        nested(nested(simple(row.clone()))),
        packed(row.clone()),
        nested(packed(row)),
    ];
    let request = request("server:render_view", 42, None, 5);
    for op in forged {
        ctx.store(op);
        let reply = dispatcher.dispatch(&ctx, &connection, &request).await;
        assert_eq!(
            control(&reply, 5).error.unwrap().code,
            ErrorCode::ActionFailed
        );
    }
    let mut text = SetTextT::default();
    text.node_id = 42;
    text.text = Some("new title".to_owned());
    let ordinary = nested(packed(UiOperationT::SetText(Box::new(text))));
    ctx.store(ordinary.clone());
    let reply = dispatcher.dispatch(&ctx, &connection, &request).await;
    let frame = read_frame(&reply, &WireLimits::default()).unwrap();
    let messages = frame.kind_as_frame_message().unwrap().messages().unwrap();
    assert_eq!(messages.len(), 2);
    assert!(messages.get(0).data().is_some());
    assert_eq!(messages.get(1).operation().unwrap().unpack(), ordinary);
}
