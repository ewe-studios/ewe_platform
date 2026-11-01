//! Starts the `wello` app on a free port and checks `/` through the netio client.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::initialize_pool;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::ShutdownSignal;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::SendSafeBody;
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};

/// Read body text from every `SendSafeBody` variant.
fn read_body(body: SendSafeBody) -> Result<String, Box<dyn std::error::Error>> {
    match body {
        SendSafeBody::Text(s) => Ok(s),
        SendSafeBody::Bytes(b) => Ok(String::from_utf8_lossy(&b).to_string()),
        body => {
            let bytes = try_collect_bytes(body)?;
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
    }
}

#[test]
fn root_route_serves_embedded_index() {
    let _guard = initialize_pool(42, Some(5));

    let shutdown = Arc::new(ShutdownSignal::new());

    // Bind first so the port is ours before the server thread starts.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let server = HttpServer::with_config(
        wello::app(),
        &format!("127.0.0.1:{}", addr.port()),
        ServerConfig::defaults(),
    );

    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });

    let client = HttpClientBuilder::new()
        .resolve_to(addr)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(10))
        .build();

    let response = client
        .send(PreparedRequestBuilder::get("http://testserver/").unwrap().build())
        .expect("request /");

    assert_eq!(response.get_status().into_usize(), 200);
    let (_, _, body) = response.into_parts();
    let text = read_body(body).expect("read body");
    assert!(
        text.contains("Wello DevServers Live!"),
        "body should contain the embedded page"
    );

    shutdown.turn_on();
}
