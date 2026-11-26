//! The tests of the model slave and its connector (decision D16).

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::{Request, RpcCtx};
use foundation_sandbox::shared::cloudflare::testing::{
    slave_handler, FakeModelClient, MemoryDoStorage, MemoryServiceBinding,
};
use foundation_sandbox::shared::cloudflare::{
    DoSlaveConnector, ModelBackend, ModelConfig, ModelSlave,
};
use foundation_sandbox::shared::control::testing::ScriptedSlave;
use foundation_sandbox::shared::control::SlaveClient;
use foundation_sandbox::shared::driver::SlaveEndpoint;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::SlaveConnector;
use foundation_sandbox::shared::rpc::messages::{
    ExecEventKind, ExecRequest, FileReadRequest, FileWriteBeginRequest, FileWriteChunkRequest,
    FileWriteCommitRequest,
};
use foundation_sandbox::shared::rpc::SlaveService;

use super::support::{self, block_on};

fn config(backend: Option<ModelBackend>) -> ModelConfig {
    ModelConfig::new().with_backend(backend)
}

fn cloudflare_ai() -> ModelBackend {
    ModelBackend::CloudflareAi {
        model: "test-model".to_string(),
    }
}

fn exec_request() -> ExecRequest {
    ExecRequest {
        exec_id: "0123456789abcdef".to_string(),
        argv: vec!["say".to_string(), "hello".to_string()],
        env: Vec::new(),
        cwd: None,
        stdin: false,
        timeout_ms: None,
    }
}

#[test]
fn model_slave_exec() {
    let (ctx, _clock, _timer) = support::ctx(60, true, false);
    let storage = Arc::new(MemoryDoStorage::new());
    let client = Arc::new(FakeModelClient::answer(b"hello".to_vec()));
    let slave = ModelSlave::new(
        ctx.clone(),
        storage,
        client.clone(),
        config(Some(cloudflare_ai())),
        1024,
    );
    let mut stream = block_on(slave.exec(RpcCtx::client(ctx), Request::new(exec_request())))
        .expect("the turn opens");
    let mut events = Vec::new();
    while let Some(item) = block_on(futures::StreamExt::next(&mut stream)) {
        events.push(item.expect("an event"));
    }
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0].event, ExecEventKind::Started { .. }));
    assert!(matches!(
        &events[1].event,
        ExecEventKind::Stdout { data } if data == b"hello"
    ));
    assert!(matches!(
        events[2].event,
        ExecEventKind::Exit { code: Some(0), .. }
    ));
    assert_eq!(client.prompts(), vec!["say hello".to_string()]);
}

#[test]
fn model_slave_answer_cap() {
    let (ctx, _clock, _timer) = support::ctx(61, true, false);
    let storage = Arc::new(MemoryDoStorage::new());
    let client = Arc::new(FakeModelClient::answer(vec![b'x'; 64]));
    let slave = ModelSlave::new(
        ctx.clone(),
        storage,
        client,
        config(Some(cloudflare_ai())).with_max_answer_bytes(8),
        1024,
    );
    let mut stream = block_on(slave.exec(RpcCtx::client(ctx), Request::new(exec_request())))
        .expect("the turn opens");
    let item = block_on(futures::StreamExt::next(&mut stream)).expect("one item");
    let error = item.expect_err("the answer is too large");
    assert!(matches!(
        SandboxError::from_connect(error.current_context()),
        SandboxError::Rpc { code, .. } if code == "resource_exhausted"
    ));
}

#[test]
fn model_slave_turn_timeout() {
    let (ctx, _clock, _timer) = support::ctx(62, true, false);
    let storage = Arc::new(MemoryDoStorage::new());
    let client = Arc::new(FakeModelClient::never());
    let slave = ModelSlave::new(
        ctx.clone(),
        storage,
        client,
        config(Some(cloudflare_ai())).with_turn_timeout(Duration::from_millis(10)),
        1024,
    );
    let mut stream = block_on(slave.exec(RpcCtx::client(ctx), Request::new(exec_request())))
        .expect("the turn opens");
    let item = block_on(futures::StreamExt::next(&mut stream)).expect("one item");
    let error = item.expect_err("the turn passes its deadline");
    assert!(matches!(
        SandboxError::from_connect(error.current_context()),
        SandboxError::Rpc { code, .. } if code == "deadline_exceeded"
    ));
}

#[test]
fn model_slave_file_table() {
    let (ctx, _clock, _timer) = support::ctx(63, true, false);
    let storage = Arc::new(MemoryDoStorage::new());
    let client = Arc::new(FakeModelClient::answer(b"ok".to_vec()));
    let slave = ModelSlave::new(
        ctx.clone(),
        storage.clone() as Arc<dyn foundation_sandbox::shared::cloudflare::DoStorage>,
        client,
        config(Some(cloudflare_ai())),
        4,
    );
    block_on(slave.file_write_begin(
        RpcCtx::client(ctx.clone()),
        Request::new(FileWriteBeginRequest {
            path: "note.txt".to_string(),
            mode: 0o644,
        }),
    ))
    .expect("begin");
    let next = block_on(slave.file_write_chunk(
        RpcCtx::client(ctx.clone()),
        Request::new(FileWriteChunkRequest {
            upload_id: "note.txt".to_string(),
            offset: 0,
            data: b"hello".to_vec(),
        }),
    ))
    .expect("chunk");
    assert_eq!(next.msg.next_offset, 5);
    let commit = block_on(slave.file_write_commit(
        RpcCtx::client(ctx.clone()),
        Request::new(FileWriteCommitRequest {
            upload_id: "note.txt".to_string(),
            size: 5,
        }),
    ))
    .expect("commit");
    assert_eq!(commit.msg.size, 5);

    let mut chunks = block_on(slave.file_read(
        RpcCtx::client(ctx.clone()),
        Request::new(FileReadRequest {
            path: "note.txt".to_string(),
        }),
    ))
    .expect("read");
    let mut bytes = Vec::new();
    while let Some(item) = block_on(futures::StreamExt::next(&mut chunks)) {
        bytes.extend_from_slice(&item.expect("a chunk").data);
    }
    assert_eq!(bytes, b"hello");

    let error = block_on(slave.file_write_begin(
        RpcCtx::client(ctx),
        Request::new(FileWriteBeginRequest {
            path: "../escape".to_string(),
            mode: 0o644,
        }),
    ))
    .expect_err("a parent path is refused");
    assert!(matches!(
        SandboxError::from_connect(error.current_context()),
        SandboxError::InvalidSpec { field, .. } if field == "path"
    ));
}

#[test]
fn do_slave_connector_calls() {
    let (ctx, _clock, _timer) = support::ctx(64, true, false);
    let scripted = Arc::new(ScriptedSlave::new());
    let handler = slave_handler(Arc::clone(&scripted));
    let binding = Arc::new(MemoryServiceBinding::new().with_slave(handler));
    let connector = DoSlaveConnector::new(binding.clone());
    let endpoint = SlaveEndpoint::CloudflareDo {
        binding: "SLAVE".to_string(),
        name: "abc123abc123".parse().expect("id"),
    };
    let limits = support::limits();
    let transport = connector
        .connect(&ctx, &endpoint, &limits)
        .expect("the transport builds");
    let client = SlaveClient::new(ctx, transport, limits).expect("client");
    let info = block_on(client.info(std::time::Duration::from_secs(5))).expect("info");
    assert_eq!(info.slave_version, "test");
    assert_eq!(binding.fetches().len(), 1);
    assert_eq!(binding.fetches()[0].0, "SLAVE");
}

#[test]
fn do_slave_connector_failure() {
    let (ctx, _clock, _timer) = support::ctx(65, true, false);
    let binding = Arc::new(MemoryServiceBinding::new().with_failure(
        foundation_sandbox::shared::cloudflare::PortError::Other("down".to_string()),
    ));
    let connector = DoSlaveConnector::new(binding);
    let endpoint = SlaveEndpoint::CloudflareDo {
        binding: "SLAVE".to_string(),
        name: "abc123abc123".parse().expect("id"),
    };
    let limits = support::limits();
    let transport = connector
        .connect(&ctx, &endpoint, &limits)
        .expect("the transport builds");
    let client = SlaveClient::new(ctx, transport, limits).expect("client");
    let error = block_on(client.info(std::time::Duration::from_secs(5))).expect_err("unavailable");
    assert!(matches!(
        error.current_context(),
        SandboxError::Rpc { code, .. } if code == "unavailable"
    ));
}

/// The `OpenRouter` stream parser reads one delta and ignores the end marker.
#[test]
fn open_router_stream_events() {
    use foundation_sandbox::shared::cloudflare::{open_router_delta, push_answer};

    let delta = r#"data: {"choices":[{"delta":{"content":"hello "}}]}"#;
    assert_eq!(open_router_delta(delta).as_deref(), Some("hello "));
    assert_eq!(open_router_delta("data: [DONE]"), None);
    assert_eq!(open_router_delta("event: ping"), None);
    assert_eq!(open_router_delta("data: not json"), None);
    assert_eq!(
        open_router_delta(r#"data: {"choices":[{"delta":{}}]}"#),
        None
    );

    let mut answer = Vec::new();
    assert!(push_answer(&mut answer, b"hello ", 10));
    assert_eq!(answer, b"hello ");
    assert!(
        !push_answer(&mut answer, b"world", 10),
        "the cap is 10 bytes"
    );
    assert_eq!(answer, b"hello ", "a refused part is not appended");
}
