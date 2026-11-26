use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_ui_http::{
    http, ActionRegistryRouteHandler, ActionRequestPolicy, Response, ViewRequest,
};
use foundation_ui_runtime::{
    action,
    action::{ActionError, ActionRegistry, ActionResult, Context, Interaction},
};
use foundation_ui_wire::{
    finish_frame, ActionRequest, ActionRequestT, ClientHelloT, ErrorCode, FrameKindT, FrameReader,
    FrameRequestT, FrameT, MountSurface, OwnedFrame, Platform, WireLimits, SCHEMA_VERSION,
};
use futures_lite::future::block_on;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[action(server)]
async fn count(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    ctx.get::<AtomicUsize>()
        .unwrap()
        .fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn envelope(request: FrameRequestT) -> Vec<u8> {
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = 7;
    frame.sequence_id = 9;
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    finish_frame(&frame).unwrap()
}
fn hello(version: u32) -> Vec<u8> {
    let mut hello = ClientHelloT::default();
    hello.vocabulary_version = version;
    hello.platform = Platform::Android;
    hello.mount = MountSurface::Native;
    let mut req = FrameRequestT::default();
    req.hello = Some(Box::new(hello));
    envelope(req)
}
fn action_bytes() -> Vec<u8> {
    let mut a = ActionRequestT::default();
    a.action = Some("server:count".into());
    let mut req = FrameRequestT::default();
    req.action = Some(Box::new(a));
    envelope(req)
}
fn post(path: &str, bytes: Vec<u8>, hello: Option<&[u8]>) -> http::SimpleIncomingRequest {
    let mut b = http::SimpleIncomingRequest::builder()
        .with_plain_url(path)
        .with_method(http::SimpleMethod::POST)
        .with_body(http::SendSafeBody::Bytes(bytes));
    if let Some(hello) = hello {
        b = b.add_header("x-ewe-hello".to_owned(), URL_SAFE_NO_PAD.encode(hello));
    }
    b.build().unwrap()
}
fn frame(ctx: &http::Ctx, response: Response) -> OwnedFrame {
    assert_eq!(response.media_type(), "application/primal-frame");
    let session = ViewRequest::new(
        ctx,
        http::SimpleIncomingRequest::builder()
            .with_plain_url("/")
            .build()
            .unwrap(),
    )
    .unwrap()
    .session();
    let mut body = Vec::new();
    session.body(response).write_body(&mut body).unwrap();
    let mut reader = FrameReader::length_prefixed(body.as_slice(), WireLimits::default());
    let frame = reader.next().unwrap().unwrap();
    assert!(reader.next().is_none());
    frame
}

struct Deny;
impl ActionRequestPolicy for Deny {
    fn check(&self, _: &http::Ctx, _: ActionRequest<'_>) -> Result<(), ActionError> {
        Err(ActionError::new(
            ErrorCode::StateRejected,
            "Rejected by host",
        ))
    }
}

#[test]
fn aliases_dispatch_only_the_named_action_and_honor_request_policy() {
    let ctx = http::Ctx::host();
    ctx.store(AtomicUsize::new(0));
    let mut registry = ActionRegistry::new();
    registry.register(count).unwrap();
    let route = ActionRegistryRouteHandler::new(registry);
    let hello = hello(foundation_ui_wire::vocab::VOCABULARY_VERSION);
    for path in ["/action", "/action/count"] {
        let result =
            block_on(route.respond(&ctx, post(path, action_bytes(), Some(&hello)))).unwrap();
        assert_eq!(
            frame(&ctx, result)
                .view()
                .kind_as_frame_message()
                .unwrap()
                .ack(),
            9
        );
    }
    for (path, expected) in [
        ("/action/other", ErrorCode::BadRequest),
        ("/action", ErrorCode::StateRejected),
    ] {
        if expected == ErrorCode::StateRejected {
            ctx.store(Arc::new(Deny) as Arc<dyn ActionRequestPolicy>);
        }
        let result =
            block_on(route.respond(&ctx, post(path, action_bytes(), Some(&hello)))).unwrap();
        let result = frame(&ctx, result);
        assert_eq!(
            result
                .view()
                .kind_as_frame_message()
                .unwrap()
                .messages()
                .unwrap()
                .get(0)
                .control()
                .unwrap()
                .error()
                .unwrap()
                .code(),
            expected
        );
    }
    assert_eq!(ctx.get::<AtomicUsize>().unwrap().load(Ordering::SeqCst), 2);
    assert!(block_on(route.respond(&ctx, post("/action", vec![0; 5], Some(&hello)))).is_err());
    assert!(block_on(route.respond(&ctx, post("/action", action_bytes(), None))).is_err());
}

#[test]
fn handshake_reports_current_or_outdated_capabilities() {
    let ctx = http::Ctx::host();
    let route = ActionRegistryRouteHandler::new(ActionRegistry::new());
    for (version, status) in [
        (
            foundation_ui_wire::vocab::VOCABULARY_VERSION,
            foundation_ui_wire::CapsStatus::Ok,
        ),
        (999, foundation_ui_wire::CapsStatus::UpdateRequired),
    ] {
        let reply = block_on(route.respond(&ctx, post("/action", hello(version), None))).unwrap();
        let result = frame(&ctx, reply);
        let messages = result
            .view()
            .kind_as_frame_message()
            .unwrap()
            .messages()
            .unwrap();
        assert_eq!(
            messages.get(0).control().unwrap().caps().unwrap().status(),
            status
        );
    }
}
