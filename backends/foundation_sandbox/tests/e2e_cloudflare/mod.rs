//! The Cloudflare end-to-end tests of spec 77 feature 08, section 7.
//!
//! They read `EWE_SANDBOX_WORKER_URL` and `EWE_SANDBOX_WORKER_TOKEN`. When the
//! URL is absent they print the reason and return; `EWE_SANDBOX_E2E_REQUIRED=1`
//! makes that a failure. `infrastructure/sandbox_worker/test/run.sh` starts
//! `wrangler dev` and sets both variables.

use std::future::Future;
use std::process::Command;
use std::time::Duration;

use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx};
use foundation_context::Ctx;
use foundation_core::valtron::{execute, StreamIteratorExt};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::client::request_task::HttpExchange;
use foundation_netio::shared::http::{SimpleHeader, SimpleMethod};
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};
use foundation_sandbox::native::connect::ConnectAddr;
use foundation_sandbox::shared::cloudflare::CloudflareConfig;
use foundation_sandbox::shared::command::{Command as SandboxCommand, ExecEvent};
use foundation_sandbox::shared::control::{
    SandboxHandle, SandboxInfo, SandboxProvider, SandboxRpcClient, SandboxState,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::{ContainerGetRequest, ContainerGetResponse};
use foundation_sandbox::shared::rpc::ContainerServiceClient;
use foundation_sandbox::shared::spec::{NetworkMode, RootFs, SandboxSpec};
use foundation_sandbox::shared::Secret;
use futures::StreamExt;

/// The tools that the container tests need.
const DOCKER: &str = "docker";

/// The container application name of the deployment plan.
const CONTAINER_NAME: &str = "ewe-sandbox";

/// Runs one future on the one pool of this test binary.
///
/// The pool is shared with the other Cloudflare tests: a second
/// `initialize_pool` in one binary would block for ever.
fn run<F: Future>(future: F) -> F::Output {
    crate::pool::block_on(future)
}

/// Whether a missing service is a failure.
fn required() -> bool {
    std::env::var("EWE_SANDBOX_E2E_REQUIRED").as_deref() == Ok("1")
}

/// The Worker URL, when the harness set one.
fn worker_url() -> Option<String> {
    std::env::var("EWE_SANDBOX_WORKER_URL")
        .ok()
        .filter(|url| !url.is_empty())
}

/// The bearer token of the local Worker.
fn token() -> String {
    std::env::var("EWE_SANDBOX_WORKER_TOKEN").unwrap_or_else(|_| "test".to_string())
}

/// Prints the reason and returns `true` when the Worker is absent.
fn skip(name: &str) -> bool {
    if worker_url().is_some() {
        return false;
    }
    let reason = format!("{name}: EWE_SANDBOX_WORKER_URL is absent");
    assert!(!required(), "{reason}");
    println!("{reason}; this test returns");
    true
}

/// Prints the reason and returns `true` when a tool is absent.
fn skip_without(tool: &str, name: &str) -> bool {
    if which(tool) {
        return false;
    }
    let reason = format!("{name}: {tool} is absent");
    assert!(!required(), "{reason}");
    println!("{reason}; this test returns");
    true
}

/// Whether one program is on the path.
fn which(program: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .status()
        .is_ok_and(|status| status.success())
}

/// A remote client with a token, or without one.
///
/// The create deadline is the Worker's container boot deadline, not the unary
/// call deadline. A cold container start on a slow runner took about 24 s, and
/// the Worker keeps a create for up to `CloudflareConfig::boot_timeout`. The
/// read and call timeout is the container call deadline: the first call to a
/// cold container took about 5 s on the runner.
fn client(token: Option<&str>) -> SandboxRpcClient {
    let url = worker_url().expect("the test skipped without a URL");
    let worker = CloudflareConfig::default();
    let limits = SandboxLimits::default()
        .with_boot_timeout(worker.boot_timeout)
        .with_rpc_timeout(worker.call_timeout);
    let control = SandboxRpcClient::connect(Ctx::host(), ConnectAddr::Url(url), limits)
        .expect("the Worker transport builds");
    match token {
        Some(token) => control.with_token(Secret::new(token)),
        None => control,
    }
}

/// The layer that one sandbox error names, for a one-line failure message.
fn error_layer(error: &ErrorTrace<SandboxError>) -> &'static str {
    match error.current_context() {
        SandboxError::Unauthorized => "the Worker token check",
        SandboxError::InvalidSpec { .. } => "the Worker route or the request body",
        SandboxError::NotFound { .. } => "the Durable Object (no record)",
        SandboxError::Rpc { code, .. } if code == "not_found" => {
            "the Durable Object router (an empty 404 means the path or method did not match)"
        }
        SandboxError::Rpc { code, .. } if code == "unavailable" => {
            "the Durable Object or the container runtime"
        }
        SandboxError::SubstrateApi { .. } => "the container runtime",
        SandboxError::BootTimeout { .. } => "the container slave",
        SandboxError::HostNotConfigured => "the Worker host bundle",
        _ => "the Worker or the Durable Object",
    }
}

/// Creates one sandbox and names the failing layer in the panic message.
async fn create(control: &SandboxRpcClient, spec: SandboxSpec, what: &str) -> SandboxHandle {
    // Capture the wire form before `create` moves the spec, so an InvalidSpec
    // rejection names the exact value that the Worker received.
    let spec_json =
        serde_json::to_string(&spec).unwrap_or_else(|error| format!("<not serializable: {error}>"));
    match SandboxProvider::create(control, spec).await {
        Ok(sandbox) => sandbox,
        Err(error) => {
            if matches!(error.current_context(), SandboxError::InvalidSpec { .. }) {
                panic!(
                    "{what}: {} [layer: {}]\nspec: {spec_json}",
                    error.current_context(),
                    error_layer(&error)
                );
            }
            panic!(
                "{what}: {} [layer: {}]",
                error.current_context(),
                error_layer(&error)
            );
        }
    }
}

/// A Cloudflare container sandbox spec.
fn container_spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::ContainerImage("sandbox".to_string()))
}

/// A model slave sandbox spec (decision D16).
fn model_spec() -> SandboxSpec {
    let mut spec = container_spec();
    spec.driver = Some(DriverId::CloudflareDo);
    spec
}

/// Whether an error is the named variant.
fn is(error: &ErrorTrace<SandboxError>, predicate: impl Fn(&SandboxError) -> bool) -> bool {
    predicate(error.current_context())
}

/// The identifiers of the Docker containers of the sandbox application.
///
/// The test suite runs one test at a time (`--test-threads=1`), so every
/// matching container belongs to the test that is running.
///
/// The local Cloudflare runtime also runs a proxy sidecar for each container.
/// Its name contains the application name and ends in `-proxy`, and its image is
/// `cloudflare/proxy-everything`. It belongs to the runtime, not to the sandbox
/// image, and it stays after a sandbox stops. The tests count only the workload
/// container.
fn docker_containers() -> Vec<String> {
    let output = Command::new(DOCKER)
        .args(["ps", "--format", "{{.ID}} {{.Names}} {{.Image}}"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(CONTAINER_NAME))
        .filter(|line| !line.contains("-proxy"))
        .filter(|line| !line.contains("proxy-everything"))
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .collect()
}

/// The names of the Docker containers of the sandbox application.
///
/// The failure message of the stop test prints them, so a run can name a
/// container that was not the runtime proxy.
fn docker_container_names() -> Vec<String> {
    let output = Command::new(DOCKER)
        .args(["ps", "--format", "{{.Names}} {{.Image}}"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(CONTAINER_NAME))
        .filter(|line| !line.contains("-proxy"))
        .filter(|line| !line.contains("proxy-everything"))
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .collect()
}

/// Waits until no workload container is listed, or the deadline passes.
///
/// The container runtime removes a destroyed container from `docker ps` after
/// the destroy call returns, so the stop check gives it a short time. A
/// remaining container is returned for the failure message.
fn wait_for_no_docker_containers(timeout: Duration) -> Vec<String> {
    // The test has no injected clock: it counts 250 ms polls. It runs on the
    // real local runtime, so the wall-clock rule of the core libraries does not
    // apply here.
    let polls = (timeout.as_millis() / 250).max(1);
    for _ in 0..polls {
        let containers = docker_containers();
        if containers.is_empty() {
            return containers;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    docker_containers()
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

/// The list of live sandboxes.
async fn list(control: &SandboxRpcClient) -> Vec<SandboxInfo> {
    SandboxProvider::list(control).await.expect("the list")
}

/// One raw HTTP request to the Worker.
struct RawResponse {
    status: u16,
    body: Vec<u8>,
}

/// Sends one raw HTTP request, for a path that the typed client cannot make.
///
/// It runs on the test thread: the exchange stream is not `Send`, so the helper
/// cannot live inside the async test body.
fn raw_request(
    method: &SimpleMethod,
    path: &str,
    token: Option<&str>,
    body: Option<Vec<u8>>,
) -> RawResponse {
    crate::pool::pool();
    let url = format!(
        "{}{path}",
        worker_url().expect("the test skipped without a URL")
    );
    let builder = if method == &SimpleMethod::PUT {
        PreparedRequestBuilder::put(&url)
    } else {
        PreparedRequestBuilder::post(&url)
    }
    .expect("the request URL");
    let mut builder = builder.header(SimpleHeader::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(body) = body {
        builder = builder.body_bytes(body);
    }
    let request = builder.build();
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

/// One `ContainerService` call through the Worker reaches the sandbox DO.
async fn container_get(id: String) -> ContainerGetResponse {
    use std::sync::Arc;

    let url = worker_url().expect("the test skipped without a URL");
    let transport = Arc::new(H1Transport::new(HttpClientBuilder::new().build()));
    let client = ContainerServiceClient::new(
        transport,
        &url,
        ClientOptions::new()
            .with_codec("json")
            .with_header("AUTHORIZATION".to_string(), format!("Bearer {}", token())),
    )
    .expect("the container client builds");
    let ctx = RpcCtx::client(Ctx::host()).with_deadline(Duration::from_secs(30));
    client
        .get(ctx, Request::new(ContainerGetRequest { container_id: id }))
        .await
        .expect("the container Get")
        .msg
}

/// A `ContainerService` call reaches its Durable Object through the Worker.
#[test]
fn e2e_cloudflare_container_route() {
    if skip("e2e_cloudflare_container_route") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        let response = container_get(sandbox.id().to_string()).await;
        assert!(
            response.container.is_some(),
            "the Durable Object answers with its container"
        );
        sandbox.stop().await.expect("the stop");
    });
}

/// A command runs in a Cloudflare container (I8, the first scenario).
#[test]
fn e2e_cloudflare_acceptance() {
    if skip("e2e_cloudflare_acceptance") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        let hello = sandbox
            .run(SandboxCommand::new(["echo", "hello"]))
            .await
            .expect("the command");
        assert_eq!(hello.stdout, b"hello\n");
        assert_eq!(hello.status.exit_code(), Some(0));
        let uname = sandbox
            .run(SandboxCommand::new(["uname", "-s"]))
            .await
            .expect("the command");
        assert_eq!(uname.stdout, b"Linux\n");
        sandbox.stop().await.expect("the stop");
    });
}

/// Every call needs the token (I1).
#[test]
fn e2e_cloudflare_token() {
    if skip("e2e_cloudflare_token") {
        return;
    }
    run(async {
        let without = client(None);
        let list = SandboxProvider::list(&without).await;
        assert!(
            list.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::Unauthorized
            ))),
            "a call with no token is Unauthorized"
        );
        let wrong = client(Some("wrong"));
        let create = SandboxProvider::create(&wrong, container_spec()).await;
        assert!(
            create.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::Unauthorized
            ))),
            "a call with a wrong token is Unauthorized"
        );
    });
}

/// An unknown sandbox is `NotFound`, and `List` does not show it (I5).
#[test]
fn e2e_cloudflare_not_found() {
    if skip("e2e_cloudflare_not_found") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let id = SandboxId::generate(&Ctx::host());
        let result = SandboxProvider::get(&control, id.clone()).await;
        match &result {
            Err(error)
                if is(error, |error| {
                    matches!(error, SandboxError::NotFound { .. })
                }) => {}
            Err(error) => panic!(
                "a scoped call for an unknown sandbox must be NotFound, got {} [layer: {}]",
                error.current_context(),
                error_layer(error)
            ),
            Ok(_) => panic!("a scoped call for an unknown sandbox returned a live sandbox"),
        }
        let sandboxes = list(&control).await;
        assert!(!sandboxes.iter().any(|info| info.id == id.to_string()));
    });
}

/// A bad sandbox identifier is refused before any Durable Object (I3).
#[test]
fn e2e_cloudflare_bad_id() {
    if skip("e2e_cloudflare_bad_id") {
        return;
    }
    let response = raw_request(
        &SimpleMethod::POST,
        "/ewe.sandbox.control.v1.SandboxService/Get",
        Some(&token()),
        Some(br#"{"sandbox_id":"../x"}"#.to_vec()),
    );
    assert_eq!(response.status, 400, "a bad identifier is a bad request");
    let body = String::from_utf8_lossy(&response.body);
    assert!(
        body.contains("invalid_argument"),
        "the Connect error code is invalid_argument: {body}"
    );
}

/// `List` shows live sandboxes only (I6).
#[test]
fn e2e_cloudflare_list() {
    if skip("e2e_cloudflare_list") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let first = create(&control, container_spec(), "the first create").await;
        let second = create(&control, container_spec(), "the second create").await;
        let first_id = first.id().to_string();
        let second_id = second.id().to_string();
        first.stop().await.expect("the first stop");
        let sandboxes = list(&control).await;
        assert!(!sandboxes.iter().any(|info| info.id == first_id));
        assert!(sandboxes.iter().any(|info| info.id == second_id));
        second.stop().await.expect("the second stop");
    });
}

/// A stale index entry is removed on the first call to its sandbox.
#[test]
fn e2e_cloudflare_stale_index() {
    if skip("e2e_cloudflare_stale_index") || skip_without(DOCKER, "e2e_cloudflare_stale_index") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        let id = sandbox.id().to_string();
        let Some(container) = docker_containers().first().cloned() else {
            println!(
                "e2e_cloudflare_stale_index: no Docker container of {CONTAINER_NAME}; \
                 this test returns"
            );
            sandbox.stop().await.expect("the stop");
            return;
        };
        let status = Command::new(DOCKER)
            .args(["kill", &container])
            .status()
            .expect("docker kill");
        assert!(status.success(), "the container was killed");
        std::thread::sleep(Duration::from_secs(2));
        let result = SandboxProvider::get(&control, sandbox.id().clone()).await;
        assert!(
            result.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::NotFound { .. }
            ))),
            "the call is NotFound"
        );
        let sandboxes = list(&control).await;
        assert!(!sandboxes.iter().any(|info| info.id == id));
    });
}

/// The internal index paths are reachable only from the Worker.
#[test]
fn index_put_delete_list() {
    if skip("index_put_delete_list") {
        return;
    }
    let response = raw_request(
        &SimpleMethod::PUT,
        "/index/x",
        Some(&token()),
        Some(b"{}".to_vec()),
    );
    assert_eq!(response.status, 404, "an outside index call is not found");
    let body = String::from_utf8_lossy(&response.body);
    assert!(body.contains("not_found"), "the code is not_found: {body}");

    run(async {
        // The effect of the internal paths through the public calls.
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        let id = sandbox.id().to_string();
        assert!(list(&control).await.iter().any(|info| info.id == id));
        sandbox.stop().await.expect("the stop");
        assert!(!list(&control).await.iter().any(|info| info.id == id));
    });
}

/// Two sandboxes are separate.
#[test]
fn e2e_cloudflare_two_sandboxes() {
    if skip("e2e_cloudflare_two_sandboxes") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let first = create(&control, container_spec(), "the first create").await;
        let second = create(&control, container_spec(), "the second create").await;
        // A write does not make missing parent directories. The relative path
        // joins the workdir, so `/workspace/tmp` must exist first (issue #1154).
        first
            .run(SandboxCommand::new(["mkdir", "-p", "/workspace/tmp"]))
            .await
            .expect("the directory");
        write_file(&first, "tmp/a", b"first").await;
        // The same relative path in the second sandbox names a missing file
        // under its own workdir, so the read is `NotFound`.
        let result = second.read_file("tmp/a").await;
        assert!(
            result
                .err()
                .is_some_and(|error| is(&error, |error| matches!(
                    error,
                    SandboxError::NotFound { .. }
                ))),
            "the second sandbox does not see the file"
        );
        first.stop().await.expect("the first stop");
        second.stop().await.expect("the second stop");
    });
}

/// No internet unless asked.
#[test]
fn e2e_cloudflare_no_internet() {
    if skip("e2e_cloudflare_no_internet") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let mut spec = container_spec();
        spec.network = NetworkMode::None;
        let sandbox = create(&control, spec, "the create").await;
        let output = sandbox
            .run(SandboxCommand::new([
                "sh",
                "-c",
                "wget -q -T 3 -O- http://example.com",
            ]))
            .await
            .expect("the command");
        assert_ne!(output.status.exit_code(), Some(0));
        sandbox.stop().await.expect("the stop");
    });
}

/// A large response arrives in order (I4).
#[test]
fn e2e_cloudflare_large_stream() {
    if skip("e2e_cloudflare_large_stream") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        let mut handle = sandbox
            .exec(SandboxCommand::new([
                "sh",
                "-c",
                "yes x | head -c 20000000",
            ]))
            .await
            .expect("the command starts");
        let mut events = handle.events();
        let mut total = 0_usize;
        let mut frames = 0_u64;
        // The command prints `x` then a newline, over and over. A frame can
        // split a pair, so the check follows the pattern across frame
        // boundaries instead of expecting every frame to hold whole pairs.
        let mut expected = b'x';
        while let Some(event) = events.next().await {
            match event.expect("the event stream") {
                ExecEvent::Stdout(data) => {
                    for &byte in &data {
                        assert_eq!(byte, expected, "the x and newline pattern is in order");
                        expected = if expected == b'x' { b'\n' } else { b'x' };
                    }
                    total += data.len();
                    frames += 1;
                }
                ExecEvent::Exit(_) => break,
                ExecEvent::Stderr(_) => {}
            }
        }
        assert!(frames > 1, "the answer arrived in more than one frame");
        assert_eq!(total, 20_000_000, "every byte arrived");
        sandbox.stop().await.expect("the stop");
    });
}

/// Stop destroys the container.
#[test]
fn e2e_cloudflare_stop() {
    if skip("e2e_cloudflare_stop") || skip_without(DOCKER, "e2e_cloudflare_stop") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        sandbox.stop().await.expect("the stop");
        let result = SandboxProvider::get(&control, sandbox.id().clone()).await;
        assert!(
            result.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::NotFound { .. }
            ))),
            "Get after a stop is NotFound"
        );
        let left = wait_for_no_docker_containers(Duration::from_secs(10));
        let names = docker_container_names();
        assert!(
            left.is_empty(),
            "no Docker container of {CONTAINER_NAME} is left; left: {names:?}"
        );
    });
}

/// The slave and the Worker report the same version (I9).
#[test]
fn e2e_cloudflare_versions() {
    if skip("e2e_cloudflare_versions") {
        return;
    }
    run(async {
        let control = client(Some(&token()));
        let first = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        let second = SandboxProvider::create(&control, container_spec())
            .await
            .expect("the create");
        let first_info = first.info();
        let second_info = second.info();
        let first_slave = first_info.slave.expect("the slave info");
        let second_slave = second_info.slave.expect("the slave info");
        let reports = SandboxProvider::probe(&control).await.expect("the probe");
        let worker_version = reports
            .iter()
            .find(|report| report.driver == DriverId::CloudflareContainer)
            .and_then(|report| {
                report
                    .checks
                    .iter()
                    .find(|check| check.name == "worker_version")
            })
            .map(|check| check.message.clone())
            .expect("the probe names the Worker version");
        assert_eq!(
            first_slave.slave_version, worker_version,
            "the slave and the Worker have one version"
        );
        assert_eq!(second_slave.slave_version, worker_version);
        first.stop().await.expect("the first stop");
        second.stop().await.expect("the second stop");
    });
}

/// A model sandbox starts no container and runs one turn (I14, decision D16).
#[test]
fn e2e_cloudflare_model_sandbox() {
    if skip("e2e_cloudflare_model_sandbox") || skip_without(DOCKER, "e2e_cloudflare_model_sandbox")
    {
        return;
    }
    run(async {
        let before = docker_containers().len();
        let control = client(Some(&token()));
        let sandbox = create(&control, model_spec(), "the create").await;
        let after = docker_containers().len();
        assert_eq!(before, after, "a model sandbox starts no container");
        let turn = sandbox
            .run(SandboxCommand::new(["echo", "hello"]))
            .await
            .expect("the turn");
        assert_eq!(turn.stdout, b"hello from the fake model server");
        assert_eq!(turn.status.exit_code(), Some(0));
        write_file(&sandbox, "notes/a.txt", b"kept").await;
        assert_eq!(read_all(&sandbox, "notes/a.txt").await, b"kept");
        sandbox.stop().await.expect("the stop");
    });
}

/// The token check covers the slave path (I14).
#[test]
fn e2e_cloudflare_model_token() {
    if skip("e2e_cloudflare_model_token") {
        return;
    }
    run(async {
        let wrong = client(Some("wrong"));
        let create = SandboxProvider::create(&wrong, model_spec()).await;
        assert!(
            create.is_err_and(|error| is(&error, |error| matches!(
                error,
                SandboxError::Unauthorized
            ))),
            "a model sandbox with a wrong token is Unauthorized"
        );
    });
}

/// Creates the sandbox that the restart harness restarts (invariant I7 of
/// feature 07). The harness sets `EWE_SANDBOX_RESTART_CREATE=1` and
/// `EWE_SANDBOX_RESTART_ID_FILE`; the test leaves the sandbox running.
#[test]
fn e2e_cloudflare_restart_create() {
    if std::env::var("EWE_SANDBOX_RESTART_CREATE").as_deref() != Ok("1") {
        println!("e2e_cloudflare_restart_create: this test runs only from test/run.sh");
        return;
    }
    if skip("e2e_cloudflare_restart_create") {
        return;
    }
    let path =
        std::env::var("EWE_SANDBOX_RESTART_ID_FILE").expect("EWE_SANDBOX_RESTART_ID_FILE is set");
    run(async move {
        let control = client(Some(&token()));
        let sandbox = create(&control, container_spec(), "the create").await;
        std::fs::write(&path, sandbox.id().as_str()).expect("the identifier file");
    });
}

/// A restarted Durable Object finds its sandbox, or finds none, exactly once.
///
/// The harness performs the restart. It sets `EWE_SANDBOX_RESTART_ID` and
/// `EWE_SANDBOX_RESTART_STATE` (`ready` or `absent`) after the restart.
#[test]
fn e2e_cloudflare_restart() {
    if skip("e2e_cloudflare_restart") {
        return;
    }
    let Ok(id) = std::env::var("EWE_SANDBOX_RESTART_ID") else {
        println!(
            "e2e_cloudflare_restart: no restart was made; the harness sets \
             EWE_SANDBOX_RESTART_ID and EWE_SANDBOX_RESTART_STATE"
        );
        return;
    };
    let expected = std::env::var("EWE_SANDBOX_RESTART_STATE").unwrap_or_default();
    let id: SandboxId = id.parse().expect("a valid sandbox identifier");
    run(async move {
        let control = client(Some(&token()));
        match expected.as_str() {
            "ready" => {
                let sandbox = SandboxProvider::get(&control, id)
                    .await
                    .expect("the sandbox survived");
                assert_eq!(sandbox.info().state, SandboxState::Ready);
                let output = sandbox
                    .run(SandboxCommand::new(["echo", "hello"]))
                    .await
                    .expect("the command after the restart");
                assert_eq!(output.stdout, b"hello\n");
                sandbox.stop().await.expect("the stop");
            }
            "absent" => {
                let result = SandboxProvider::get(&control, id.clone()).await;
                assert!(
                    result.is_err_and(|error| is(&error, |error| matches!(
                        error,
                        SandboxError::NotFound { .. }
                    ))),
                    "the sandbox is gone"
                );
                assert!(!list(&control)
                    .await
                    .iter()
                    .any(|info| info.id == id.to_string()));
            }
            other => panic!("EWE_SANDBOX_RESTART_STATE must be ready or absent, not {other}"),
        }
    });
}

/// A `Provider` starts a Cloudflare sandbox.
#[cfg(feature = "provider")]
#[test]
fn provider_e2e_cloudflare() {
    use foundation_deployment_platform::providers::Provider;
    use foundation_sandbox::providers::SandboxDeploymentProvider;

    if skip("provider_e2e_cloudflare") {
        return;
    }
    crate::pool::pool();
    let control: std::sync::Arc<dyn SandboxProvider> = std::sync::Arc::new(client(Some(&token())));
    let provider = SandboxDeploymentProvider::cloudflare(control);
    let handle = provider.launch(&container_spec()).expect("the launch");
    assert!(provider.is_running(&handle));
    provider.stop(&handle).expect("the stop");
    assert!(!provider.is_running(&handle));
}
