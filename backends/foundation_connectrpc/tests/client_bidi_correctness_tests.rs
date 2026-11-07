//! Bidirectional-stream client correctness (spec 73 feature 02, issue #261).
//!
//! WHY: `Client::bidi_stream` used to take a request stream and drop it. Every
//! caller therefore sent nothing through it. The fix removes the parameter: the
//! returned `BidiStream` facade is the one writer, and callers send through
//! `send`/`send_headers`/`close_request`/`split`.
//!
//! WHAT: three messages sent through the facade reach the server handler, the
//! handler observes all three, and `close_request` half-closes the request
//! direction so the response stream ends.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use futures::StreamExt;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    Client, ClientOptions, ConnectRpcServeH2, Ctx, H2Transport, HandlerOptions, JsonCodec,
    ProcedureCodecs, RequestStream, Router,
};
use foundation_core::valtron::valtron_test;
use foundation_http::native::serve::H2Serve;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::ShutdownSignal;

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

impl Msg {
    fn of(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

const BIDI: &str = "/t.Svc/Bidi";

fn codecs() -> ProcedureCodecs<Msg, Msg> {
    ProcedureCodecs::<Msg, Msg>::of((JsonCodec,))
}

/// Start an HTTP/2 server whose bidi handler records every message it receives.
fn start_server() -> (String, Arc<Mutex<Vec<String>>>, Arc<ShutdownSignal>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_in_handler = Arc::clone(&seen);

    let mut router = Router::new();
    router.bidi_stream(
        BIDI,
        codecs(),
        move |_ctx: Ctx, reqs: RequestStream<Msg>| {
            let seen = Arc::clone(&seen_in_handler);
            async move {
                let echoed = reqs.map(move |item| {
                    let msg = item?;
                    seen.lock().expect("record lock").push(msg.text.clone());
                    Ok(Msg::of(format!("re:{}", msg.text)))
                });
                Ok(echoed)
            }
        },
        HandlerOptions::new(),
    );

    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(BIDI, rpc);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(ShutdownSignal::new());
    let server_shutdown = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    thread::spawn(move || server.serve_with_listener(&listener, &server_shutdown));

    (addr.to_string(), seen, shutdown)
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(15))
}

/// Three facade sends reach the server, and `close_request` ends the stream.
///
/// The old code could not express this through its request-stream parameter:
/// `bidi_stream` dropped the stream, so the handler received nothing and the
/// response ended before any echo.
#[valtron_test]
async fn bidi_send_reaches_the_server() {
    let (addr, seen, shutdown) = start_server();

    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{BIDI}");
    let client = Client::new(
        transport,
        &url,
        codecs(),
        ClientOptions::new().with_grpc().with_codec("json"),
    )
    .expect("build client");

    let mut stream = client.bidi_stream(ctx()).await.expect("open bidi");

    for text in ["a", "b", "c"] {
        stream.send(&Msg::of(text)).await.expect("send");
    }
    stream.close_request().await.expect("close request");

    let mut got = Vec::new();
    while let Some(msg) = stream.receive().await.expect("receive") {
        got.push(msg.text);
    }

    assert_eq!(got, vec!["re:a", "re:b", "re:c"]);
    assert_eq!(
        *seen.lock().expect("record lock"),
        vec!["a", "b", "c"],
        "the server handler must receive every facade send"
    );

    shutdown.turn_on();
}
