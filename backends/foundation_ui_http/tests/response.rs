use foundation_ui_http::{
    get, http, BodyWriterConfig, FrameBody, Response, ViewError, ViewRequest,
};
use foundation_ui_runtime::{html, html_remove};
use foundation_ui_wire::{read_frame, PlacementKind, WireLimits};
use futures_lite::future::block_on;
use serde::{Serialize, Serializer};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct Settings(String);

#[get("/initial")]
fn initial(request: ViewRequest) -> Result<Response, ViewError> {
    let settings = request.require::<Settings>()?;
    Ok(Response::frame(
        html! { <Text>{settings.0.as_str()}</Text> },
    ))
}

#[get("/updates")]
async fn updates(req: ViewRequest) -> Result<Response, ViewError> {
    let ctx = std::future::ready(req.render_context()).await;
    html_remove!(7_u32, &ctx, ctx.receiver()).map_err(|e| ViewError::Render(e.to_string()))?;
    Ok(Response::frame(ctx))
}

fn request(ctx: &http::Ctx) -> ViewRequest {
    ViewRequest::new(
        ctx,
        http::SimpleIncomingRequest::builder()
            .with_plain_url("/")
            .build()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn handler_keeps_its_original_tree_and_encodes_a_full_view() {
    let ctx = http::Ctx::host();
    ctx.store(Settings("Buy milk".into()));
    let request = request(&ctx);
    let session = request.session();
    let response = block_on(initial.respond(request)).unwrap();
    assert!(
        matches!(&response, Response::Frame(FrameBody::View(tree)) if tree.children[0].text.as_deref() == Some("Buy milk"))
    );
    assert_eq!(response.media_type(), "application/vnd.ewe.frame");
    let mut body = Vec::new();
    session.body(response).write_body(&mut body).unwrap();
    let frame = read_frame(&body, &WireLimits::default()).unwrap();
    let messages = frame.kind_as_frame_message().unwrap().messages().unwrap();
    let place = messages.get(0).operation().unwrap().ui_as_place().unwrap();
    assert_eq!(place.kind(), PlacementKind::FullView);
    assert!(place.target().is_none());
}

#[test]
fn queued_output_keeps_the_declared_target() {
    let req = request(&http::Ctx::host());
    let session = req.session();
    let response = block_on(updates.respond(req)).unwrap();
    assert!(matches!(&response, Response::Frame(FrameBody::Updates(_))));
    let mut bytes = Vec::new();
    session.body(response).write_body(&mut bytes).unwrap();
    let messages = read_frame(&bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    let place = messages.get(0).operation().unwrap().ui_as_place().unwrap();
    assert_eq!(place.kind(), PlacementKind::Remove);
    assert_eq!(place.target().unwrap().node_id(), 7);
}

struct CountedJson(Arc<AtomicUsize>);

impl Serialize for CountedJson {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        serializer.serialize_str("Buy milk")
    }
}

#[test]
fn json_is_typed_and_signals_run_at_write_time() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(AtomicUsize::new(0));
    let req = request(&http::Ctx::host());
    let (value, set) = req.signals().signal(0);
    let record = observed.clone();
    req.signals()
        .effect(move || record.store(value.get(), Ordering::SeqCst));
    set.set(4);
    let body = req
        .session()
        .body(Response::json(CountedJson(calls.clone())));
    drop(req);
    let Response::Json(json) = body.response() else {
        panic!("expected JSON")
    };
    assert!(json.value().downcast_ref::<CountedJson>().is_some());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(observed.load(Ordering::SeqCst), 0);
    let mut bytes = Vec::new();
    body.write_body(&mut bytes).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(observed.load(Ordering::SeqCst), 4);
    assert_eq!(bytes, br#""Buy milk""#);
}

struct ShortWriter {
    bytes: Vec<u8>,
    fail_after: usize,
}
impl Write for ShortWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() >= self.fail_after {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = bytes.len().min(1);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn short_writes_finish_and_writer_failures_reach_the_caller() {
    let session = request(&http::Ctx::host()).session();
    let mut out = ShortWriter {
        bytes: vec![],
        fail_after: usize::MAX,
    };
    session
        .clone()
        .body(Response::json("abc"))
        .write_body(&mut out)
        .unwrap();
    assert_eq!(out.bytes, br#""abc""#);
    let mut sink = ShortWriter {
        bytes: vec![],
        fail_after: 2,
    };
    let error = session
        .body(Response::html(html! { <Text>Hello</Text> }))
        .write_body(&mut sink as &mut dyn Write)
        .unwrap_err();
    assert!(matches!(error, ViewError::Write(e) if e.kind() == io::ErrorKind::BrokenPipe));
}

#[get("/json")]
fn json_route() -> Response {
    Response::json("hello")
}

#[test]
fn mounted_route_sends_only_one_body_through_the_socket_writer() {
    use http::shared::router::Router;
    let mut router = Router::new();
    router.mount(json_route).unwrap();
    let ctx = http::Ctx::host();
    ctx.store(BodyWriterConfig::default().with_chunk_bytes(2.try_into().unwrap()));
    let req = http::SimpleIncomingRequest::builder()
        .with_plain_url("/json")
        .build()
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let conn = http::SharedByteBufferStream::rwrite(
        http::RawStream::from_tcp(listener.accept().unwrap().0).unwrap(),
    );
    let result = router
        .dispatch(&http::SimpleMethod::GET, "/json")
        .unwrap()
        .serve(ctx, req, conn);
    assert!(matches!(
        block_on(result.split().ok().unwrap()),
        http::ServeOutcome::Keep
    ));
    let mut bytes = Vec::new();
    client.read_to_end(&mut bytes).unwrap();
    let output = std::str::from_utf8(&bytes).unwrap();
    let (head, body) = output.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"));
    assert!(head
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert!(head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked"));
    assert!(!head.to_ascii_lowercase().contains("content-length"));
    let mut decoded = Vec::new();
    let mut input = body;
    loop {
        let (length, rest) = input.split_once("\r\n").unwrap();
        let length = usize::from_str_radix(length, 16).unwrap();
        if length == 0 {
            assert_eq!(rest, "\r\n");
            break;
        }
        decoded.extend_from_slice(&rest.as_bytes()[..length]);
        input = rest[length..].strip_prefix("\r\n").unwrap();
    }
    assert_eq!(decoded, br#""hello""#);
}

#[test]
fn verified_frame_is_inspected_and_forwarded_without_repacking() {
    let bytes = include_bytes!("../../foundation_ui_wire/fixtures/frame_ops_counter.bin");
    let original =
        foundation_ui_wire::OwnedFrame::new(bytes.to_vec(), &WireLimits::default()).unwrap();
    let body = request(&http::Ctx::host())
        .session()
        .body(Response::frame(original));
    let Response::Frame(FrameBody::Encoded(frame)) = body.response() else {
        panic!("expected original frame")
    };
    assert_eq!(frame.as_bytes(), bytes);
    let mut out = ShortWriter {
        bytes: Vec::new(),
        fail_after: usize::MAX,
    };
    body.write_body(&mut out).unwrap();
    assert_eq!(&out.bytes, bytes);
}
