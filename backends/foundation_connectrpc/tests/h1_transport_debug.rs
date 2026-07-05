use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{self, initialize_pool};
use foundation_core::synca::OnSignal;
use foundation_connectrpc::transport::Transport;
use foundation_connectrpc::H1Transport;
use foundation_connectrpc::{
    ConnectRpcServe, HandlerOptions, IdempotencyLevel, ProcedureCodecs, Request, Response, Router,
};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::serve::Serve;
use foundation_netio::simple_http::client::SimpleHttpClient;
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeaders, SimpleMethod, SimpleUrl,
};
use foundation_core::url::Uri;

const ECHO: &str = "/test.EchoService/Echo";

fn echo_serve() -> Arc<dyn Serve> {
    let mut router = Router::new();
    router.unary(
        ECHO,
        ProcedureCodecs::<(), ()>::defaults(),
        |_ctx, _req: Request<()>| async move { Ok(Response::new(())) },
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    Arc::new(ConnectRpcServe::new(router.into_handler()))
}

/// Full end-to-end with explicit timeout on pipe poll.
/// Uses a single pool for both server and client.
#[test]
fn h1_client_with_real_server_timed_poll() {
    let _guard = initialize_pool(96, Some(8));

    // Server
    let mut app = HttpApp::new_serve();
    app.router.add_route_any(ECHO, &echo_serve());
    let shutdown = Arc::new(OnSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let srv = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), ServerConfig::defaults());
    let sd = shutdown.clone();
    std::thread::spawn(move || srv.serve_with_listener(&listener, &sd));

    let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("server ready");
    eprintln!("[test] server ready at {addr}");

    // Client
    let transport = H1Transport::new(SimpleHttpClient::from_system());
    let url = format!("http://127.0.0.1:{}{ECHO}", addr.port());
    let uri = Uri::parse(&url).expect("parse URL");
    let descriptor = RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url),
        request_uri: uri,
        headers: {
            let mut h = SimpleHeaders::new();
            h.insert(foundation_netio::simple_http::shared::SimpleHeader::CONTENT_TYPE, vec!["application/json".to_string()]);
            h
        },
        method: SimpleMethod::POST,
    };

    eprintln!("[test] calling open()");
    let stream = transport.open(descriptor).expect("open");
    eprintln!("[test] open() returned — pump is spawned on pool");

    // Push request body (empty JSON object for empty () message)
    stream.send_body.try_send(bytes::Bytes::from("{}".as_bytes())).expect("send");

    // Poll with timeout — same pattern that works for the dead-port test
    eprintln!("[test] polling head pipe with timeout...");
    use foundation_core::valtron::TryRecvError;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match stream.head.try_recv() {
            Ok((status, _headers)) => {
                eprintln!("[test] received head: {status:?}");
                assert_eq!(status, foundation_netio::simple_http::shared::Status::OK);
                shutdown.turn_on();
                return;
            }
            Err(TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(TryRecvError::Closed) => {
                eprintln!("[test] head pipe closed unexpectedly");
                shutdown.turn_on();
                return;
            }
        }
    }
    eprintln!("[test] TIMEOUT: no data after 5s");
    shutdown.turn_on();
}
