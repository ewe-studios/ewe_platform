//! ConnectRPC over HTTP/2 cleartext (h2c) — same Router/Client API as unary_echo.
//!
//! Run with: `cargo run -p foundation_connectrpc --example h2_echo`
//!
//! Server: OS thread, h2 accept loop, `Router::unary()`, `ConnectRpcHandler::dispatch()`.
//! Client: `H2Transport` + typed `Client::unary()` under `#[valtron]`.
//! Both over a real loopback TCP socket.

use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::valtron;
use foundation_http::shared::context::ContextBag;
use foundation_netio::http2::connection::{H2Connection, H2Response};
use foundation_netio::http2::frame::*;
use foundation_netio::http2::hpack;
use foundation_netio::simple_http::shared::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, SimpleUrl, Status,
};
use foundation_netio::netcap::ConnectionContext;

use foundation_connectrpc::router::ConnectRpcHandler;
use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::{
    block_on, Client, ClientOptions, Ctx, H2Transport, HandlerOptions,
    JsonCodec, ProcedureCodecs, Request, Response, Router,
};

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct EchoMsg { text: String }

const ECHO_PATH: &str = "/echo.EchoService/Echo";

fn serve_h2_conn(stream: impl io::Read + io::Write, handler: Arc<ConnectRpcHandler>) -> io::Result<()> {
    let mut c = H2Connection::new(stream, true);
    c.server_handshake()?;
    let mut dec = hpack::Decoder::new();

    loop {
        c.flush()?;
        let (head, payload) = match c.read_frame() {
            Ok(v) => v,
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };
        if head.kind != Kind::Headers { continue; }

        let hf = HeadersFrame::parse(&head, &payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let decoded = dec.decode(&hf.header_block)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let req = hpack_to_request(&decoded);
        let bag = Arc::new(ContextBag::default());
        let response = block_on(handler.dispatch(bag, req));

        let body: Option<Bytes> = match response.body {
            Some(SendSafeBody::Bytes(b)) => Some(b),
            Some(SendSafeBody::Vec(v)) => Some(Bytes::from(v)),
            _ => None,
        };
        let has_body = body.as_ref().map_or(false, |b| !b.is_empty());
        c.send_response(head.stream_id, H2Response {
            status: status_code(response.status),
            headers: vec![],
            body,
            end_stream: !has_body,
        })?;
        c.flush()?;
    }
    Ok(())
}

fn hpack_to_request(headers: &[(Bytes, Bytes)]) -> SimpleIncomingRequest {
    let mut method = SimpleMethod::POST;
    let mut path = String::from("/");
    let mut req_headers = SimpleHeaders::new();
    for (name, value) in headers {
        match name.as_ref() {
            b":method" => {
                if String::from_utf8_lossy(value).to_uppercase() == "GET" {
                    method = SimpleMethod::GET;
                }
            }
            b":path" => path = String::from_utf8_lossy(value).into_owned(),
            b":authority" | b":scheme" | b":status" => {}
            _ => {
                req_headers
                    .entry(SimpleHeader::from(String::from_utf8_lossy(name).to_string()))
                    .or_default()
                    .push(String::from_utf8_lossy(value).to_string());
            }
        }
    }
    SimpleIncomingRequest {
        proto: Proto::HTTP20, request_uri: Default::default(), request_url: SimpleUrl::new(&path),
        body: None, headers: req_headers, method, extensions: None,
        connection: Arc::new(ConnectionContext::default()),
    }
}

fn status_code(s: Status) -> u16 {
    match s { Status::OK => 200, Status::NoContent => 204, Status::BadRequest => 400, Status::NotFound => 404, Status::InternalServerError => 500, _ => 500 }
}

#[valtron(seed = 1, threads = 4)]
async fn main() {
    let mut router = Router::new();
    router.unary(
        ECHO_PATH,
        ProcedureCodecs::<EchoMsg, EchoMsg>::of((JsonCodec,)),
        |_ctx: Ctx, req: Request<EchoMsg>| async move {
            Ok(Response::new(EchoMsg { text: format!("ECHO: {}", req.msg.text) }))
        },
        HandlerOptions::new(),
    );
    let handler = Arc::new(router.into_handler());

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let shutdown = Arc::new(OnSignal::new());
    let sd = shutdown.clone();

    thread::spawn(move || {
        for stream in listener.incoming() {
            if sd.probe() { break; }
            if let Ok(s) = stream {
                s.set_read_timeout(Some(Duration::from_secs(30))).ok();
                let h = handler.clone();
                thread::spawn(move || { if let Err(e) = serve_h2_conn(s, h) { eprintln!("[server] error: {e}"); } });
            }
        }
    });

    while TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err() {}
    println!("h2c server on {addr}");

    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let url = format!("http://{addr}{ECHO_PATH}");
    let client: Client<EchoMsg, EchoMsg> = Client::new(
        transport, &url,
        ProcedureCodecs::<EchoMsg, EchoMsg>::of((JsonCodec,)),
        ClientOptions::new(),
    ).expect("build client");

    let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
    let resp = client.unary(ctx, Request::new(EchoMsg { text: "hello-h2".into() })).await.expect("unary");
    println!("[client] response: {:?}", resp.msg.text);
    assert_eq!(resp.msg.text, "ECHO: hello-h2");
    println!("h2 round-trip verified ✓");
    shutdown.turn_on();
}
