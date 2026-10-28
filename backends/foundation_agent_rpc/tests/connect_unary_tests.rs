//! T6 — unary round-trip over Connect on real loopback TCP, with the error →
//! status mapping asserted on the wire.
//!
//! Pool size: an HTTP/1.1 call holds a valtron worker for its duration (the
//! server's connection driver blocks in `poll(2)`), and a turn needs workers
//! for the client tasks and the agent loop too — 4 workers starve a `Chat`,
//! 8 do not. See LEARNINGS (01 T6).

use std::sync::Arc;
use std::time::Duration;

use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::{ChatRequest, CreateInstanceRequest, HealthRequest};
use foundation_agent_rpc::{AgentRpcServer, ModelHandle};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_connectrpc::{ClientOptions, Code, Ctx, H1Transport, Request, Transport};
use foundation_core::valtron::valtron_test;
use foundation_netio::http::NativeHttpClient;

fn mock_model(reply: &str) -> ModelHandle {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text(reply)]);
    ModelHandle::from_router("mock", "mock", mock.into_router())
}

fn client(base_url: &str) -> AgentServiceClient {
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(Arc::new(NativeHttpClient::from_system())));
    AgentServiceClient::new(transport, base_url, ClientOptions::new().with_connect())
        .expect("client")
}

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(20))
}

fn create(name: &str) -> CreateInstanceRequest {
    CreateInstanceRequest {
        name: name.into(),
        model: "mock".into(),
        ..Default::default()
    }
}

#[valtron_test(threads = 8)]
async fn create_instance_and_chat_round_trip_over_connect() {
    let server = AgentRpcServer::builder()
        .with_model(mock_model("hello over the wire"))
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = client(&server.base_url());

    let health = client
        .health(ctx(), Request::new(HealthRequest::default()))
        .await
        .expect("health")
        .msg;
    assert_eq!(health.status, "serving");

    let info = client
        .create_instance(ctx(), Request::new(create("alpha")))
        .await
        .expect("create")
        .msg;
    assert_eq!(info.name, "alpha");

    let reply = client
        .chat(
            ctx(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("chat")
        .msg;
    assert_eq!(reply.text, "hello over the wire");

    server.graceful_shutdown().expect("clean shutdown");
}

#[valtron_test(threads = 8)]
async fn errors_reach_the_client_as_typed_status_codes() {
    let server = AgentRpcServer::builder()
        .with_model(mock_model("x"))
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = client(&server.base_url());

    let missing = client
        .chat(
            ctx(),
            Request::new(ChatRequest {
                instance: "nobody".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect_err("no such instance");
    assert_eq!(missing.current_context().code(), Code::NotFound);
    assert!(
        missing.current_context().is_wire_error(),
        "came from the server"
    );
    assert_eq!(
        missing.current_context().message(),
        "instance not found: nobody"
    );

    let invalid = client
        .create_instance(ctx(), Request::new(create("../escape")))
        .await
        .expect_err("bad name");
    assert_eq!(invalid.current_context().code(), Code::InvalidArgument);

    client
        .create_instance(ctx(), Request::new(create("alpha")))
        .await
        .expect("create");
    let dup = client
        .create_instance(ctx(), Request::new(create("alpha")))
        .await
        .expect_err("duplicate");
    assert_eq!(dup.current_context().code(), Code::AlreadyExists);

    server.graceful_shutdown().expect("clean shutdown");
}

#[valtron_test(threads = 8)]
async fn many_calls_do_not_starve_a_later_turn() {
    let server = AgentRpcServer::builder()
        .with_model(mock_model("still answering"))
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = client(&server.base_url());
    for _ in 0..10 {
        client
            .health(ctx(), Request::new(HealthRequest::default()))
            .await
            .expect("health");
    }
    client
        .create_instance(ctx(), Request::new(create("alpha")))
        .await
        .expect("create");
    let reply = client
        .chat(
            ctx(),
            Request::new(ChatRequest {
                instance: "alpha".into(),
                prompt: "hi".into(),
                ..Default::default()
            }),
        )
        .await
        .expect("chat")
        .msg;
    assert_eq!(reply.text, "still answering");
    server.graceful_shutdown().expect("clean shutdown");
}

#[test]
fn a_server_without_an_auth_choice_does_not_build() {
    let err = AgentRpcServer::builder()
        .with_model(mock_model("x"))
        .build()
        .expect_err("auth must be chosen explicitly");
    assert_eq!(
        err.current_context().code(),
        Code::FailedPrecondition,
        "{err:?}"
    );
}
