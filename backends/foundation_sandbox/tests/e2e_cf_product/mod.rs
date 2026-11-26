//! The Cloudflare Sandbox product end-to-end tests of spec 77 feature 17,
//! section 7.
//!
//! They read `EWE_SANDBOX_URL` and the token. When the URL is absent they print
//! the reason and return; `EWE_SANDBOX_E2E_REQUIRED=1` makes that a failure.
//! `infrastructure/sandbox_worker/test/product.sh` starts `wrangler dev` with
//! `wrangler.product.toml` and sets both variables.
//!
//! The fault test `e2e_cf_product_no_slave` reads `EWE_SANDBOX_NOSLAVE_URL`.
//! `product.sh` starts a second Worker whose image has no slave and sets it.

use std::future::Future;
use std::process::Command;

use base64::Engine as _;
use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx};
use foundation_context::Ctx;
use foundation_core::valtron::{execute, StreamIteratorExt};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::client::request_task::HttpExchange;
use foundation_netio::shared::http::{SimpleHeader, SimpleMethod};
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};
use foundation_sandbox::native::connect::ConnectAddr;
use foundation_sandbox::shared::command::{Command as SandboxCommand, ExecEvent, StdinMode};
use foundation_sandbox::shared::control::{
    SandboxHandle, SandboxProvider, SandboxRpcClient, SandboxState,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::ContainerGetRequest;
use foundation_sandbox::shared::rpc::ContainerServiceClient;
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};
use foundation_sandbox::shared::Secret;
use futures::StreamExt;
use serde_json::Value;

/// The path prefix of `SandboxService`.
const SERVICE: &str = "/ewe.sandbox.control.v1.SandboxService";

/// Runs one future on the one pool of this test binary.
fn run<F: Future>(future: F) -> F::Output {
    crate::pool::block_on(future)
}

/// Whether a missing service is a failure.
fn required() -> bool {
    std::env::var("EWE_SANDBOX_E2E_REQUIRED").as_deref() == Ok("1")
}

/// The product Worker URL, when the harness set one.
fn product_url() -> Option<String> {
    std::env::var("EWE_SANDBOX_URL")
        .ok()
        .filter(|url| !url.is_empty())
}

/// The no-slave Worker URL, when the harness set one.
fn no_slave_url() -> Option<String> {
    std::env::var("EWE_SANDBOX_NOSLAVE_URL")
        .ok()
        .filter(|url| !url.is_empty())
}

/// The bearer token of the local Worker.
fn token() -> String {
    std::env::var("EWE_SANDBOX_TOKEN")
        .or_else(|_| std::env::var("EWE_SANDBOX_WORKER_TOKEN"))
        .unwrap_or_else(|_| "test".to_string())
}

/// Prints the reason and returns `true` when the product Worker is absent.
fn skip(name: &str) -> bool {
    if product_url().is_some() {
        return false;
    }
    let reason = format!("{name}: EWE_SANDBOX_URL is absent");
    assert!(!required(), "{reason}");
    println!("{reason}; this test returns");
    true
}

/// A remote client for one URL, with a token or without one.
fn client_at(url: &str, token: Option<&str>) -> SandboxRpcClient {
    let control = SandboxRpcClient::connect(
        Ctx::host(),
        ConnectAddr::Url(url.to_string()),
        SandboxLimits::default(),
    )
    .expect("the Worker transport builds");
    match token {
        Some(token) => control.with_token(Secret::new(token)),
        None => control,
    }
}

/// A remote client for the product Worker.
fn client(token: Option<&str>) -> SandboxRpcClient {
    let url = product_url().expect("the test skipped without a URL");
    client_at(&url, token)
}

/// A Cloudflare container sandbox spec.
fn container_spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::ContainerImage("sandbox".to_string()))
}

/// Whether an error is the named variant.
fn is(error: &ErrorTrace<SandboxError>, predicate: impl Fn(&SandboxError) -> bool) -> bool {
    predicate(error.current_context())
}

/// Whether one program is on the path.
fn which(program: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .status()
        .is_ok_and(|status| status.success())
}

/// Every byte of one file stream.
async fn read_all(handle: &SandboxHandle, path: &str) -> Vec<u8> {
    let mut stream = handle.read_file(path).await.expect("the file read");
    let mut data = Vec::new();
    while let Some(chunk) = stream.next().await {
        data.extend_from_slice(&chunk.expect("a file chunk"));
    }
    data
}

/// Writes one small file.
async fn write_file(handle: &SandboxHandle, path: &str, data: &[u8]) {
    let bytes = data.to_vec();
    let stream = futures::stream::once(async move { Ok(bytes) });
    handle
        .write_file(path, 0o644, stream)
        .await
        .expect("the file write");
}

/// The SHA-256 of some bytes, through the host's hash program.
fn sha256_hex(data: &[u8]) -> Option<String> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ewe-shim-{}-{}.bin",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, data).ok()?;
    let (program, args): (&str, Vec<&std::ffi::OsStr>) = if which("sha256sum") {
        ("sha256sum", vec![path.as_os_str()])
    } else if which("shasum") {
        (
            "shasum",
            vec![
                std::ffi::OsStr::new("-a"),
                std::ffi::OsStr::new("256"),
                path.as_os_str(),
            ],
        )
    } else {
        std::fs::remove_file(&path).ok();
        return None;
    };
    let output = Command::new(program).args(args).output().ok()?;
    std::fs::remove_file(&path).ok();
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_string)
}

/// One raw HTTP exchange.
struct RawResponse {
    status: u16,
    body: Vec<u8>,
}

/// Sends one raw HTTP request and returns its status and body.
///
/// It runs on the test thread: the exchange stream is not `Send`, so the helper
/// cannot live inside the async test body.
fn raw_request(
    method: &SimpleMethod,
    path: &str,
    content_type: &str,
    token: Option<&str>,
    body: Vec<u8>,
) -> RawResponse {
    crate::pool::pool();
    let url = format!(
        "{}{path}",
        product_url().expect("the test skipped without a URL")
    );
    let builder = if method == &SimpleMethod::PUT {
        PreparedRequestBuilder::put(&url)
    } else {
        PreparedRequestBuilder::post(&url)
    }
    .expect("the request URL");
    let mut builder = builder.header(SimpleHeader::CONTENT_TYPE, content_type);
    if let Some(token) = token {
        builder = builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = builder.body_bytes(body).build();
    let client = HttpClientBuilder::new().build();
    let driven = execute(client.open_exchange(request), None).expect("the request starts");
    let mut stream = Box::pin(driven.into_next_stream());
    let mut status = 0_u16;
    let mut body = Vec::new();
    while let Some(event) = futures::executor::block_on(stream.next()) {
        match event {
            HttpExchange::Head { status: head, .. } => {
                status = u16::try_from(head.into_usize()).unwrap_or(0);
            }
            HttpExchange::BodyChunk(chunk) => body.extend_from_slice(&chunk),
            HttpExchange::Failed(error) => panic!("the raw request failed: {error}"),
        }
    }
    RawResponse { status, body }
}

/// One Connect envelope on the response stream.
struct WireFrame {
    flags: u8,
    payload: Vec<u8>,
}

/// One Connect envelope as the request body: flag 0, the length, the message.
fn request_envelope(message: &Value) -> Vec<u8> {
    let payload = serde_json::to_vec(message).expect("the request message");
    let length = u32::try_from(payload.len()).expect("the message length");
    let mut body = Vec::with_capacity(5 + payload.len());
    body.push(0);
    body.extend_from_slice(&length.to_be_bytes());
    body.extend_from_slice(&payload);
    body
}

/// Sends one `Exec` request and parses each answer envelope as it arrives.
fn raw_exec_stream(id: &str) -> (u16, Vec<WireFrame>) {
    crate::pool::pool();
    let message = serde_json::json!({
        "sandbox_id": id,
        "exec_id": "0000000000000001",
        "argv": ["echo", "hello"],
        "env": [],
        "cwd": null,
        "stdin": false,
        "timeout_ms": null,
    });
    let body = request_envelope(&message);
    let url = format!(
        "{}{SERVICE}/Exec",
        product_url().expect("the test skipped without a URL")
    );
    let request = PreparedRequestBuilder::post(&url)
        .expect("the request URL")
        .header(SimpleHeader::CONTENT_TYPE, "application/connect+json")
        .header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token()))
        .body_bytes(body)
        .build();
    let client = HttpClientBuilder::new().build();
    let driven = execute(client.open_exchange(request), None).expect("the request starts");
    let mut stream = Box::pin(driven.into_next_stream());
    let mut status = 0_u16;
    let mut buffer: Vec<u8> = Vec::new();
    let mut frames = Vec::new();
    while let Some(event) = futures::executor::block_on(stream.next()) {
        match event {
            HttpExchange::Head { status: head, .. } => {
                status = u16::try_from(head.into_usize()).unwrap_or(0);
            }
            HttpExchange::BodyChunk(chunk) => {
                buffer.extend_from_slice(&chunk);
                loop {
                    if buffer.len() < 5 {
                        break;
                    }
                    let length =
                        u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]) as usize;
                    if buffer.len() < 5 + length {
                        break;
                    }
                    let flags = buffer[0];
                    let payload = buffer[5..5 + length].to_vec();
                    buffer.drain(..5 + length);
                    frames.push(WireFrame { flags, payload });
                }
            }
            HttpExchange::Failed(error) => panic!("the raw request failed: {error}"),
        }
    }
    assert!(buffer.is_empty(), "the stream ended inside one envelope");
    (status, frames)
}

/// The first scenario: a command runs in the product image, and the shared
/// acceptance scenarios give the same result as feature 08.
#[test]
fn e2e_cf_product_acceptance() {
    if skip("e2e_cf_product_acceptance") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");

        // Run a command.
        let hello = sandbox
            .run(SandboxCommand::new(["echo", "hello"]))
            .await
            .expect("the command");
        assert_eq!(hello.stdout, b"hello\n");
        assert_eq!(hello.status.exit_code(), Some(0));

        // The exit code arrives.
        let failed = sandbox
            .run(SandboxCommand::new(["sh", "-c", "exit 7"]))
            .await
            .expect("the command");
        assert_eq!(failed.status.exit_code(), Some(7));

        // Input reaches the program.
        let mut command = SandboxCommand::new(["cat"]);
        command.stdin = StdinMode::Piped;
        let mut exec = sandbox.exec(command).await.expect("the command starts");
        let mut stdin = exec.stdin().expect("the piped input");
        stdin.write(b"hello").await.expect("the input write");
        stdin.close().await.expect("the input close");
        let mut input_stdout = Vec::new();
        let mut events = exec.events();
        while let Some(event) = events.next().await {
            match event.expect("the event stream") {
                ExecEvent::Stdout(data) => input_stdout.extend_from_slice(&data),
                ExecEvent::Exit(status) => {
                    assert_eq!(status.exit_code(), Some(0));
                    break;
                }
                ExecEvent::Stderr(_) => {}
            }
        }
        assert_eq!(input_stdout, b"hello");

        // Files stay and read back.
        write_file(&sandbox, "notes/a.txt", b"kept").await;
        assert_eq!(read_all(&sandbox, "notes/a.txt").await, b"kept");

        // A second sandbox does not see the file.
        let second = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the second create");
        let result = second.read_file("notes/a.txt").await;
        assert!(
            result
                .err()
                .is_some_and(|error| is(&error, |error| matches!(
                    error,
                    SandboxError::NotFound { .. }
                ))),
            "the second sandbox does not see the file"
        );
        second.stop().await.expect("the second stop");

        // Stop ends the sandbox.
        let id = sandbox.id().clone();
        sandbox.stop().await.expect("the stop");
        let result = SandboxProvider::get(&control, id).await;
        assert!(
            result.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::NotFound { .. }
            ))),
            "Get after a stop is NotFound"
        );
    });
}

/// The shim is still there, with the same content as in Cloudflare's image.
#[test]
fn e2e_cf_product_shim_intact() {
    if skip("e2e_cf_product_shim_intact") {
        return;
    }
    let expected = match std::env::var("EWE_SHIM_SHA256") {
        Ok(value) if !value.is_empty() => value,
        _ => {
            let reason = "e2e_cf_product_shim_intact: EWE_SHIM_SHA256 is absent";
            assert!(!required(), "{reason}");
            println!("{reason}; this test returns");
            return;
        }
    };
    run(async move {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        let present = sandbox
            .run(SandboxCommand::new([
                "/usr/bin/test",
                "-x",
                "/usr/local/bin/sandbox-shim",
            ]))
            .await
            .expect("the test command");
        assert_eq!(present.status.exit_code(), Some(0));
        let data = read_all(&sandbox, "/usr/local/bin/sandbox-shim").await;
        let actual = sha256_hex(&data).expect("a SHA-256 program on the host");
        assert_eq!(actual, expected, "the shim content is the donor's content");
        sandbox.stop().await.expect("the stop");
    });
}

/// The image's own command does not run: the slave is process identifier 1.
#[test]
fn e2e_cf_product_pid1_is_slave() {
    if skip("e2e_cf_product_pid1_is_slave") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        let ps = sandbox
            .run(SandboxCommand::new(["/bin/ps", "-o", "args=", "-p", "1"]))
            .await
            .expect("the ps command");
        // A small base system may have no `ps`. `/proc/1/cmdline` gives the
        // same answer.
        let args = if ps.status.exit_code() == Some(0) && !ps.stdout.is_empty() {
            String::from_utf8_lossy(&ps.stdout).trim().to_string()
        } else {
            let cmdline = read_all(&sandbox, "/proc/1/cmdline").await;
            String::from_utf8_lossy(&cmdline)
                .replace('\0', " ")
                .trim()
                .to_string()
        };
        assert!(
            args.starts_with("/usr/local/bin/agent-slave"),
            "process identifier 1 is the slave: {args}"
        );
        sandbox.stop().await.expect("the stop");
    });
}

/// A plain HTTP unary call gives the same answer as `SandboxRpcClient` (I6).
#[test]
fn e2e_cf_json_unary() {
    if skip("e2e_cf_json_unary") {
        return;
    }
    let id = run(async {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        sandbox.id().as_str().to_string()
    });
    let body = format!(r#"{{"sandbox_id":"{id}"}}"#).into_bytes();
    let response = raw_request(
        &SimpleMethod::POST,
        &format!("{SERVICE}/Get"),
        "application/json",
        Some(&token()),
        body,
    );
    assert_eq!(response.status, 200, "the unary answer is 200");
    let value: Value = serde_json::from_slice(&response.body).expect("a JSON answer");
    assert_eq!(value["id"], id);
    assert_eq!(value["driver"], "cloudflare-container");
    run(async move {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::get(&control, id.parse().expect("the sandbox id"))
            .await
            .expect("the typed get");
        assert_eq!(sandbox.info().id, id);
        assert_eq!(sandbox.info().driver, DriverId::CloudflareContainer);
        assert_eq!(sandbox.info().state, SandboxState::Ready);
        sandbox.stop().await.expect("the stop");
    });
}

/// A plain HTTP streamed call answers with envelopes and ends with flag 0x02
/// (I6).
#[test]
fn e2e_cf_json_stream() {
    if skip("e2e_cf_json_stream") {
        return;
    }
    let id = run(async {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        sandbox.id().as_str().to_string()
    });
    let (status, frames) = raw_exec_stream(&id);
    assert_eq!(status, 200, "the streamed answer is 200");
    assert!(frames.len() >= 2, "the answer has more than one envelope");
    let last = frames.last().expect("the last envelope");
    assert_eq!(last.flags & 0x02, 0x02, "the last envelope ends the stream");

    let mut stdout = Vec::new();
    let mut exit = None;
    for frame in frames.iter().filter(|frame| frame.flags & 0x02 == 0) {
        let value: Value = serde_json::from_slice(&frame.payload).expect("a JSON event");
        if let Some(data) = value.pointer("/event/Stdout/data") {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.as_str().expect("the base64 text"))
                .expect("the base64 bytes");
            stdout.extend_from_slice(&bytes);
        }
        if let Some(code) = value.pointer("/event/Exit/code") {
            exit = code.as_i64();
        }
    }
    assert_eq!(stdout, b"hello\n", "the streamed stdout is hello");
    assert_eq!(exit, Some(0), "the streamed exit code is zero");

    // The typed client gives the same answer.
    run(async move {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::get(&control, id.parse().expect("the sandbox id"))
            .await
            .expect("the typed get");
        let output = sandbox
            .run(SandboxCommand::new(["echo", "hello"]))
            .await
            .expect("the typed command");
        assert_eq!(output.stdout, stdout);
        assert_eq!(output.status.exit_code(), Some(0));
        sandbox.stop().await.expect("the stop");
    });
}

/// A call with no token is refused (I6).
#[test]
fn e2e_cf_json_no_token() {
    if skip("e2e_cf_json_no_token") {
        return;
    }
    let response = raw_request(
        &SimpleMethod::POST,
        &format!("{SERVICE}/List"),
        "application/json",
        None,
        b"{}".to_vec(),
    );
    assert_eq!(response.status, 401, "a call with no token is Unauthorized");
    let value: Value = serde_json::from_slice(&response.body).expect("a JSON error body");
    assert_eq!(value["code"], "unauthenticated");
}

/// A product image with no slave fails the create with a start error.
///
/// `product.sh` starts a second Worker whose image has no second `COPY` line
/// and sets `EWE_SANDBOX_NOSLAVE_URL`.
#[test]
fn e2e_cf_product_no_slave() {
    let Some(url) = no_slave_url() else {
        let reason = "e2e_cf_product_no_slave: EWE_SANDBOX_NOSLAVE_URL is absent";
        assert!(!required(), "{reason}");
        println!("{reason}; this test returns");
        return;
    };
    let token_value = token();
    run(async move {
        let control = client_at(&url, Some(&token_value));
        let result = SandboxProvider::create(&control, container_spec()).await;
        assert!(
            result.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::SubstrateExited { .. } | SandboxError::BootTimeout { .. }
            ))),
            "a create with no slave is SubstrateExited or BootTimeout"
        );
    });
}

/// A `ContainerService` call reaches the product Worker's Durable Object.
#[test]
fn e2e_cf_product_container_route() {
    use std::sync::Arc;

    if skip("e2e_cf_product_container_route") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        let url = product_url().expect("the test skipped without a URL");
        let transport = Arc::new(H1Transport::new(HttpClientBuilder::new().build()));
        let client = ContainerServiceClient::new(
            transport,
            &url,
            ClientOptions::new()
                .with_codec("json")
                .with_header("AUTHORIZATION".to_string(), format!("Bearer {}", token())),
        )
        .expect("the container client builds");
        let ctx = RpcCtx::client(Ctx::host());
        let response = client
            .get(
                ctx,
                Request::new(ContainerGetRequest {
                    container_id: sandbox.id().to_string(),
                }),
            )
            .await
            .expect("the container Get")
            .msg;
        assert!(
            response.container.is_some(),
            "the Durable Object answers with its container"
        );
        sandbox.stop().await.expect("the stop");
    });
}
