//! Isolate the session FileSync server: serve it on a loopback h2 socket and
//! call DiffCopy with our own gRPC client (no buildkitd). Proves the FileSync
//! DiffCopy sender + serve path work end-to-end independent of the session
//! tunnel.
#![cfg(all(unix, feature = "buildkit"))]

use std::sync::Arc;

use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H2Transport, ProcedureCodecs, Transport,
};
use foundation_core::valtron::valtron_test;

use foundation_deployment_docker::buildkit::generated::fsutil::types::packet::PacketType;
use foundation_deployment_docker::buildkit::generated::fsutil::types::Packet;

#[valtron_test]
async fn filesync_diffcopy_serves_context_dir() {
    use foundation_connectrpc::{ConnectRpcServeH2, Router};
    use foundation_deployment_docker::buildkit::services::filesync::{self, register_file_sync};
    use foundation_deployment_docker::buildkit::session::DirFileSync;
    use foundation_core::synca::OnSignal;
    use foundation_http::native::serve::H2Serve;
    use foundation_http::native::server::HttpServer;
    use foundation_http::shared::app::{HttpApp, ServerApp};

    // Context dir with a single Dockerfile.
    let dir = std::env::temp_dir().join(format!("ewe-fs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Dockerfile"), b"FROM alpine\n").unwrap();

    // Serve FileSync on loopback h2.
    let mut router = Router::new();
    register_file_sync(&mut router, Arc::new(DirFileSync::new(&dir)));
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(filesync::procedure::DIFF_COPY, rpc.clone());
    app.route_any_h2(filesync::procedure::TAR_STREAM, rpc);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = Arc::new(OnSignal::new());
    let sd = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    std::thread::spawn(move || server.serve_with_listener(&listener, &sd));

    // gRPC client → DiffCopy. Send a PACKET_REQ for id 0 after the stats.
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let client = Client::<Packet, Packet>::new(
        transport,
        &format!("http://{addr}/moby.filesync.v1.FileSync/DiffCopy"),
        ProcedureCodecs::defaults(),
        ClientOptions::new().with_grpc(),
    )
    .unwrap();

    let mut bidi = client
        .bidi_stream(Ctx::background(), futures::stream::empty::<Packet>())
        .await
        .expect("open DiffCopy");

    // Read stat packets until the empty-stat terminator, then request id 0.
    let mut got_dockerfile = false;
    let mut stats = 0;
    // Request file 0 up-front (buildkit would after seeing stats).
    let mut req = Packet::default();
    req.r#type = PacketType::PACKET_REQ.into();
    req.ID = 0;
    bidi.send(&req).await.expect("send REQ");

    let mut data = Vec::new();
    for _ in 0..50 {
        match bidi.receive().await.expect("recv") {
            Some(pkt) => {
                let t = pkt.r#type.to_i32();
                if t == PacketType::PACKET_STAT as i32 {
                    if pkt.stat.into_option().is_some_and(|s| !s.path.is_empty()) { stats += 1; }
                } else if t == PacketType::PACKET_DATA as i32 {
                    if pkt.data.is_empty() { break; } else { data.extend_from_slice(&pkt.data); got_dockerfile = true; }
                }
            }
            None => break,
        }
    }

    shutdown.turn_on();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(stats >= 1, "should have received >=1 stat packet, got {stats}");
    assert!(got_dockerfile, "should have received Dockerfile data");
    assert_eq!(data, b"FROM alpine\n", "Dockerfile content mismatch");
}
