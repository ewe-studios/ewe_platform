//! T10 — end to end over gRPC and gRPC-Web (Connect is covered by
//! `connect_unary_tests` / `connect_stream_tests`), unary and streaming, on
//! real loopback TCP, with `grpc-status` asserted.
//!
//! Plus the feature's headline case on a real model: one loaded candle model,
//! two instances, concurrent chats, exactly one load.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event;
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_agent_rpc::proto::v1::{ChatRequest, CreateInstanceRequest};
use foundation_agent_rpc::{AgentRpcError, AgentRpcServer, ModelHandle, RunningServer};
use foundation_ai::agentic::testing::{mock_text, MockModelProvider};
use foundation_connectrpc::{
    ClientOptions, Code, Ctx, H1Transport, H2Transport, Request, Transport,
};
use foundation_core::valtron::valtron_test;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::http::SimpleHeader;

fn ctx() -> Ctx {
    Ctx::background().with_deadline(Duration::from_secs(60))
}

fn mock_server(chunks: &[&str]) -> RunningServer {
    let mut mock = MockModelProvider::new();
    mock.on_any(chunks.iter().map(|c| mock_text(c)).collect());
    AgentRpcServer::builder()
        .with_model(ModelHandle::from_router("mock", "mock", mock.into_router()))
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves")
}

/// gRPC over HTTP/2 cleartext.
fn grpc_client(server: &RunningServer) -> AgentServiceClient {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    AgentServiceClient::new(
        transport,
        &server.base_url(),
        ClientOptions::new().with_grpc(),
    )
    .expect("grpc client")
}

/// gRPC-Web over HTTP/1.1.
fn grpc_web_client(server: &RunningServer) -> AgentServiceClient {
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(Arc::new(NativeHttpClient::from_system())));
    AgentServiceClient::new(
        transport,
        &server.base_url(),
        ClientOptions::new().with_grpc_web(),
    )
    .expect("grpc-web client")
}

fn create(name: &str) -> Request<CreateInstanceRequest> {
    Request::new(CreateInstanceRequest {
        name: name.into(),
        ..Default::default()
    })
}

fn chat(instance: &str, prompt: &str) -> Request<ChatRequest> {
    Request::new(ChatRequest {
        instance: instance.into(),
        prompt: prompt.into(),
        ..Default::default()
    })
}

/// Unary + server-streaming + an error, over one client.
async fn round_trip(
    client: &AgentServiceClient,
) -> foundation_connectrpc::Response<foundation_agent_rpc::proto::v1::ChatResponse> {
    client
        .create_instance(ctx(), create("alpha"))
        .await
        .expect("create");

    let reply = client
        .chat(ctx(), chat("alpha", "hi"))
        .await
        .expect("unary chat");
    assert_eq!(reply.msg.text, "one two");

    let mut stream = client
        .chat_stream(ctx(), chat("alpha", "again"))
        .await
        .expect("open stream");
    let mut texts = Vec::new();
    let mut done = false;
    while let Some(event) = stream.receive().await.expect("receive") {
        match event.event {
            Some(Event::Text(t)) => texts.push(t),
            Some(Event::Done(_)) => done = true,
            Some(Event::Failed(f)) => panic!("turn failed: {}", f.message),
            _ => {}
        }
    }
    assert_eq!(texts, vec!["one ".to_string(), "two".to_string()]);
    assert!(done, "terminal Done event");

    let err = client
        .chat(ctx(), chat("nobody", "hi"))
        .await
        .expect_err("missing instance");
    assert_eq!(err.current_context().code(), Code::NotFound, "{err:?}");
    assert!(
        err.current_context().is_wire_error(),
        "decoded from the server's status"
    );
    assert_eq!(
        err.current_context().message(),
        "instance not found: nobody"
    );
    reply
}

#[valtron_test(threads = 8)]
async fn grpc_unary_and_streaming_round_trip_with_grpc_status() {
    let server = mock_server(&["one ", "two"]);
    let reply = round_trip(&grpc_client(&server)).await;
    let status = reply
        .trailers()
        .get(&SimpleHeader::from("grpc-status".to_string()))
        .and_then(|v| v.first())
        .cloned();
    assert_eq!(
        status.as_deref(),
        Some("0"),
        "grpc-status trailer on success"
    );
    server.graceful_shutdown().expect("shutdown");
}

#[valtron_test(threads = 8)]
async fn grpc_web_unary_and_streaming_round_trip() {
    let server = mock_server(&["one ", "two"]);
    round_trip(&grpc_web_client(&server)).await;
    server.graceful_shutdown().expect("shutdown");
}

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join(name)
}

/// The feature's test case 1 on a real model: one candle load, two instances,
/// concurrent gRPC chats; both complete and the loader ran once.
#[valtron_test(threads = 8)]
async fn two_instances_chat_concurrently_on_one_loaded_model() {
    let loads = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&loads);
    let model = ModelHandle::load_with("tiny-llama", move || {
        use foundation_ai::backends::candle::CandleBackend;
        use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};
        counter.fetch_add(1, Ordering::SeqCst);
        CandleBackend::cpu()
            .get_model_by_spec(ModelSpec {
                name: "tiny-llama".into(),
                id: ModelId::Name("tiny-llama".into(), None),
                devices: None,
                model_location: Some(fixture_dir("tiny-random-LlamaForCausalLM")),
                lora_location: None,
            })
            .map_err(|e| AgentRpcError::ModelLoad(e.to_string()).into())
    })
    .expect("model loads");

    let server = AgentRpcServer::builder()
        .with_model(model)
        .insecure_plaintext()
        .build()
        .expect("server builds")
        .serve("127.0.0.1:0")
        .expect("serves");
    let client = grpc_client(&server);
    for name in ["left", "right"] {
        client
            .create_instance(
                ctx(),
                Request::new(CreateInstanceRequest {
                    name: name.into(),
                    max_tokens: Some(8),
                    temperature: Some(0.0),
                    ..Default::default()
                }),
            )
            .await
            .expect("create");
    }

    let (left, right) = futures::join!(
        client.chat(ctx(), chat("left", "Hello there.")),
        client.chat(ctx(), chat("right", "What is the sea?")),
    );
    let (left, right) = (left.expect("left chat").msg, right.expect("right chat").msg);
    assert!(
        left.message_count > 0 && right.message_count > 0,
        "both turns ran"
    );
    assert_eq!(
        loads.load(Ordering::SeqCst),
        1,
        "two instances, one model load"
    );

    server.graceful_shutdown().expect("shutdown");
}
