//! Isolate the session FileSync server: serve it on a loopback h2 socket and
//! call DiffCopy with our own gRPC client (no buildkitd). Proves the FileSync
//! DiffCopy sender + serve path work end-to-end independent of the session
//! tunnel.
#![cfg(all(unix, feature = "buildkit"))]

use std::sync::Arc;

use foundation_connectrpc::{
    Client, ClientOptions, Ctx, H2Transport, ProcedureCodecs, Request, Transport,
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

#[valtron_test]
async fn health_check_returns_serving() {
    use foundation_connectrpc::{ConnectRpcServeH2, Router};
    use foundation_deployment_docker::buildkit::services::health::{self, register_health, Health};
    use foundation_deployment_docker::buildkit::session::HealthService;
    use foundation_deployment_docker::buildkit::generated::grpc::health::v1::health_check_response::ServingStatus;
    use foundation_deployment_docker::buildkit::generated::grpc::health::v1::{HealthCheckRequest, HealthCheckResponse};
    use foundation_core::synca::OnSignal;
    use foundation_http::native::serve::H2Serve;
    use foundation_http::native::server::HttpServer;
    use foundation_http::shared::app::{HttpApp, ServerApp};

    // Serve Health service on loopback h2.
    let mut router = Router::new();
    register_health(&mut router, Arc::new(HealthService));
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(health::procedure::CHECK, rpc.clone());
    app.route_any_h2(health::procedure::WATCH, rpc);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = Arc::new(OnSignal::new());
    let sd = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    std::thread::spawn(move || server.serve_with_listener(&listener, &sd));

    // Call Health/Check.
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let client = Client::<HealthCheckRequest, HealthCheckResponse>::new(
        transport,
        &format!("http://{addr}/grpc.health.v1.Health/Check"),
        ProcedureCodecs::defaults(),
        ClientOptions::new().with_grpc(),
    ).unwrap();

    let resp = client
        .unary(Ctx::background(), Request::new(HealthCheckRequest::default()))
        .await
        .expect("Health/Check should succeed");

    let status = resp.msg.status.to_i32();
    eprintln!("Health/Check response: status={status} (SERVING={})", ServingStatus::SERVING as i32);
    assert_eq!(status, ServingStatus::SERVING as i32, "should return SERVING");

    shutdown.turn_on();
}

#[valtron_test]
async fn ssh_agent_proxy_forwards_bytes() {
    use foundation_connectrpc::{ConnectRpcServeH2, Router};
    use foundation_deployment_docker::buildkit::generated::moby::sshforward::v1::{
        BytesMessage, CheckAgentRequest, CheckAgentResponse,
    };
    use foundation_deployment_docker::buildkit::services::ssh::{self, register_ssh};
    use foundation_deployment_docker::buildkit::session::SshAgentProxy;
    use foundation_core::synca::OnSignal;
    use foundation_http::native::serve::H2Serve;
    use foundation_http::native::server::HttpServer;
    use foundation_http::shared::app::{HttpApp, ServerApp};
    use std::io::{Read as _, Write as _};

    // Fake "agent": a unix socket that echoes each chunk back prefixed with
    // "agent:". Proves ForwardAgent pumps both directions through a real
    // socket, without needing ssh-agent.
    let sock_path = std::env::temp_dir().join(format!("ewe-fake-agent-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);
    let agent_listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
    std::thread::spawn(move || {
        for conn in agent_listener.incoming() {
            let Ok(mut conn) = conn else { break };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = conn.read(&mut buf) {
                    if n == 0 { break; }
                    let mut reply = b"agent:".to_vec();
                    reply.extend_from_slice(&buf[..n]);
                    if conn.write_all(&reply).is_err() { break; }
                }
            });
        }
    });

    // Serve the SSH service on loopback h2.
    let mut router = Router::new();
    register_ssh(&mut router, Arc::new(SshAgentProxy::new(&sock_path)));
    let rpc: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router.into_handler()));
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(ssh::procedure::CHECK_AGENT, rpc.clone());
    app.route_any_h2(ssh::procedure::FORWARD_AGENT, rpc);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = Arc::new(OnSignal::new());
    let sd = shutdown.clone();
    let server = HttpServer::from_app(ServerApp::http2(app), &addr.to_string());
    std::thread::spawn(move || server.serve_with_listener(&listener, &sd));

    // CheckAgent succeeds while the socket is up.
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    let check = Client::<CheckAgentRequest, CheckAgentResponse>::new(
        transport.clone(),
        &format!("http://{addr}/moby.sshforward.v1.SSH/CheckAgent"),
        ProcedureCodecs::defaults(),
        ClientOptions::new().with_grpc(),
    )
    .unwrap();
    check
        .unary(Ctx::background(), Request::new(CheckAgentRequest::default()))
        .await
        .expect("CheckAgent should succeed against a live socket");

    // ForwardAgent round-trips bytes through the fake agent.
    let forward = Client::<BytesMessage, BytesMessage>::new(
        transport,
        &format!("http://{addr}/moby.sshforward.v1.SSH/ForwardAgent"),
        ProcedureCodecs::defaults(),
        ClientOptions::new().with_grpc(),
    )
    .unwrap();
    let mut bidi = forward
        .bidi_stream(Ctx::background(), futures::stream::pending::<BytesMessage>())
        .await
        .expect("open ForwardAgent");
    bidi.send(&BytesMessage { data: b"hello".to_vec(), ..Default::default() })
        .await
        .expect("send to agent");
    let reply = bidi
        .receive()
        .await
        .expect("receive from agent")
        .expect("agent reply present");
    assert_eq!(reply.data, b"agent:hello", "agent echo mismatch");

    shutdown.turn_on();
    let _ = std::fs::remove_file(&sock_path);
}
