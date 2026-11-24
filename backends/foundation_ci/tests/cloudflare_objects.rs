//! Check the generated R2 client against a local HTTP server.
use foundation_ci::{
    cloudflare::{self, objects::R2Endpoint},
    CiEnvironment,
};
use foundation_cloudflare_storage::shared::ApiClient;
use foundation_core::valtron::{block_on_future, Ctx};
use foundation_netio::HttpClientBuilder;
use foundation_tasks::{
    Cancellation, NativeProcess, OutputStream, ProcessConfig, TaskEnv, TaskOutput, TaskRunner,
};
use futures_util::future::BoxFuture;
use std::{
    io::{self, BufRead, BufReader, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Lines(Mutex<Vec<u8>>);
impl TaskOutput for Lines {
    fn write<'a>(&'a self, _: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(())
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

fn local_pages(
    status: u16,
    replies: Vec<&'static str>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let start = Instant::now();
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock
                            && start.elapsed() < Duration::from_secs(15) =>
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("missing object request: {e}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&socket);
            let mut headers = String::new();
            loop {
                let length = reader.read_line(&mut headers).unwrap();
                assert_ne!(length, 0);
                if headers.ends_with("\r\n\r\n") {
                    break;
                }
            }
            let mut lines = headers.lines();
            requests.push(lines.next().unwrap().to_owned());
            assert!(
                lines.any(|line| line.eq_ignore_ascii_case("authorization: Bearer fixture-value"))
            );
            write!(socket, "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{reply}",reply.len()).unwrap();
        }
        requests
    });
    (base, handle)
}

fn listing(
    base: String,
    token: Option<&'static str>,
) -> (foundation_tasks::TaskResult, Arc<Lines>) {
    let root = tempfile::tempdir().unwrap();
    let ctx = Ctx::host();
    ctx.store(CiEnvironment::new(
        root.path().into(),
        "fixture".into(),
        move |key| match key {
            "CLOUDFLARE_ACCOUNT_ID" => Some("abc123".into()),
            "CLOUDFLARE_API_TOKEN" => token.map(Into::into),
            _ => None,
        },
    ));
    ctx.store(R2Endpoint::new(
        ApiClient::new(HttpClientBuilder::new().build()),
        base,
    ));
    let output = Arc::new(Lines::default());
    ctx.store(TaskEnv {
        process: Arc::new(NativeProcess::new(ProcessConfig::default())),
        output: output.clone(),
        cancellation: Cancellation::new(),
    });
    let mut runner = TaskRunner::new(
        ctx,
        ["fixture", "task", "run", "cf:r2:ls", "demo-bucket"]
            .map(Into::into)
            .into(),
    );
    runner.register(cloudflare::set().unwrap()).unwrap();
    let result = block_on_future(runner.run());
    (result, output)
}

#[test]
fn lists_pages_and_reports_service_errors() {
    let _pool = foundation_core::valtron::initialize_pool(42, Some(4));
    lists_all_pages_without_changing_object_names();
    reports_incomplete_pages_and_missing_credentials();
}

fn lists_all_pages_without_changing_object_names() {
    let (base, server) = local_pages(
        200,
        vec![
            r#"{"success":true,"errors":[],"messages":[],"result":[{"key":"first/file"}],"result_info":{"is_truncated":true,"cursor":"next"}}"#,
            r#"{"success":true,"errors":[],"messages":[],"result":[{"key":"second\nfile"}],"result_info":{"is_truncated":false}}"#,
        ],
    );
    let (result, output) = listing(base, Some("fixture-value"));
    result.unwrap();
    assert_eq!(
        server.join().unwrap(),
        [
            "GET /accounts/abc123/r2/buckets/demo-bucket/objects HTTP/1.1",
            "GET /accounts/abc123/r2/buckets/demo-bucket/objects?cursor=next HTTP/1.1",
        ]
    );
    let bytes = output.0.lock().unwrap();
    let objects: Vec<foundation_cloudflare_storage::shared::R2ObjectInfo> =
        serde_json::Deserializer::from_slice(&bytes)
            .into_iter()
            .collect::<Result<_, _>>()
            .unwrap();
    assert_eq!(
        objects.into_iter().map(|o| o.key).collect::<Vec<_>>(),
        ["first/file", "second\nfile"]
    );
}

fn reports_incomplete_pages_and_missing_credentials() {
    let missing = listing(String::new(), None).0.unwrap_err();
    assert!(missing.to_string().contains("Set CLOUDFLARE_API_TOKEN"));
    let repeated = r#"{"success":true,"errors":[],"messages":[],"result":[],"result_info":{"is_truncated":true,"cursor":"same"}}"#;
    let omitted = r#"{"success":true,"errors":[],"messages":[],"result":[],"result_info":{"is_truncated":true}}"#;
    let rejected = r#"{"success":false,"errors":[],"messages":[],"result":[]}"#;
    for (status, pages, expected) in [
        (200, vec![repeated, repeated], "repeated an object cursor"),
        (200, vec![omitted], "omitted the next object cursor"),
        (200, vec![rejected], "rejected the object listing"),
        (403, vec!["fixture-value"], "HTTP 403"),
    ] {
        let (base, server) = local_pages(status, pages);
        let problem = listing(base, Some("fixture-value"))
            .0
            .unwrap_err()
            .to_string();
        server.join().unwrap();
        assert!(problem.contains(expected), "{problem}");
        assert!(!problem.contains("fixture-value"));
    }
}
