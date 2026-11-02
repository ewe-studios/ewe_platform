//! Hub client against a local test server (no network, no token).
//!
//! The client's endpoint points at `TestHttpServer`, so these cover the request
//! shape and the answer handling of the `DynNetClient` path: JSON answers, a
//! download streamed to disk, an error answer that must write nothing, a
//! transport failure that must not read as "does not exist", and a commit
//! body that keeps binary bytes intact.
//!
//! Every Hub operation is now async, so each test is a `#[valtron_test] async
//! fn` that `.await`s the operation under valtron's pool.

use std::sync::{Arc, Mutex};

use foundation_core::valtron::valtron_test;
use foundation_deployment_huggingface::{
    client, repository, AddSource, CommitOperation, HFClientBuilder, HuggingFaceError,
    RepoCreateCommitParams, RepoDownloadFileParams,
};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_testing::http::{HttpResponse, TestHttpServer};

fn json(body: &str) -> HttpResponse {
    let mut response = HttpResponse::ok(body.as_bytes().to_vec());
    response.headers[0] = ("Content-Type".to_string(), "application/json".to_string());
    response
}

fn with_body(code: u16, text: &str, body: &[u8]) -> HttpResponse {
    let mut response = HttpResponse::status(code, text);
    response.headers = vec![("Content-Length".to_string(), body.len().to_string())];
    response.body = body.to_vec();
    response
}

fn body_bytes(body: &SendSafeBody) -> Vec<u8> {
    match body {
        SendSafeBody::Bytes(b) => b.clone(),
        SendSafeBody::Text(t) => t.as_bytes().to_vec(),
        // The test server reads the whole request body before the handler.
        _ => panic!("the test server hands the handler a read body"),
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hf-offline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[valtron_test]
async fn hub_whoami_awaits_without_blocking() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    let seen = Arc::new(Mutex::new(None::<String>));
    let seen_in = Arc::clone(&seen);
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_for_handler = Arc::clone(&ticks);
    let ticks_for_ticker = Arc::clone(&ticks);
    let ticks_at_answer = Arc::new(Mutex::new(None::<usize>));
    let ticks_at_answer_in = Arc::clone(&ticks_at_answer);

    let server = TestHttpServer::with_response(move |req| {
        let auth = req
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|v| v.first().cloned());
        *seen_in.lock().unwrap() = auth;
        // Hold the answer for 300 ms. A sync client would freeze the valtron
        // worker for this long, so the ticker below could make no progress.
        // The async client must park and let the ticker keep ticking.
        std::thread::sleep(Duration::from_millis(300));
        *ticks_at_answer_in.lock().unwrap() = Some(ticks_for_handler.load(Ordering::SeqCst));
        json(r#"{"name":"tester","type":"user"}"#)
    });

    let hf = HFClientBuilder::new()
        .endpoint(server.base_url())
        .token("hf_test")
        .build()
        .expect("client");

    let whoami = client::whoami(&hf);
    let ticker = async move {
        for _ in 0..50 {
            foundation_core::valtron::timer::sleep_for(Duration::from_millis(10)).await;
            ticks_for_ticker.fetch_add(1, Ordering::SeqCst);
        }
    };
    let (user, ()) = futures_lite::future::zip(whoami, ticker).await;
    let user = user.expect("whoami");

    assert_eq!(user.username, "tester");
    assert_eq!(seen.lock().unwrap().as_deref(), Some("Bearer hf_test"));
    assert!(
        ticks.load(Ordering::SeqCst) > 0,
        "the ticker made no progress"
    );
    assert!(
        ticks_at_answer.lock().unwrap().unwrap() > 0,
        "the ticker had already ticked when the delayed answer was sent, so the await did not block the engine"
    );
}

#[valtron_test]
async fn hub_download_streams_to_file() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(64 * 1024).collect();
    let to_serve = payload.clone();
    let server = TestHttpServer::with_response(move |req| {
        assert!(
            req.path
                .url
                .ends_with("/owner/model/resolve/main/weights.bin"),
            "download URL: {}",
            req.path.url
        );
        let mut response = HttpResponse::ok(to_serve.clone());
        response.headers[0] = (
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        );
        response
    });

    let hf = HFClientBuilder::new()
        .endpoint(server.base_url())
        .build()
        .expect("client");
    let dir = temp_dir("download");
    let path = repository::repo_download_file(
        &hf.model("owner", "model"),
        &RepoDownloadFileParams {
            filename: "weights.bin".to_string(),
            revision: None,
            directory: dir.clone(),
        },
    )
    .await
    .expect("download");

    assert_eq!(std::fs::read(&path).expect("read download"), payload);
    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn download_error_answer_writes_nothing() {
    let server =
        TestHttpServer::with_response(|_req| with_body(404, "Not Found", b"Entry not found"));

    let hf = HFClientBuilder::new()
        .endpoint(server.base_url())
        .build()
        .expect("client");
    let dir = temp_dir("download-404");
    let err = repository::repo_download_file(
        &hf.model("owner", "model"),
        &RepoDownloadFileParams {
            filename: "missing.bin".to_string(),
            revision: None,
            directory: dir.clone(),
        },
    )
    .await
    .expect_err("a 404 must be an error");

    match err {
        HuggingFaceError::Http { status, body, .. } => {
            assert_eq!(status, 404);
            assert!(body.contains("Entry not found"), "message: {body}");
        }
        other => panic!("expected an HTTP error, got {other:?}"),
    }
    assert!(
        !dir.join("missing.bin").exists(),
        "an error answer must not become the file"
    );
    assert!(
        !dir.join("missing.bin.part").exists(),
        "no partial file is left"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn error_body_is_read_only_up_to_the_limit() {
    let big = vec![b'x'; 10_000];
    let server = TestHttpServer::with_response(move |_req| with_body(500, "Error", &big));

    let hf = HFClientBuilder::new()
        .endpoint(server.base_url())
        .max_error_body(100)
        .build()
        .expect("client");
    let err = client::whoami(&hf)
        .await
        .expect_err("a 500 must be an error");

    match err {
        HuggingFaceError::Http { status, body, .. } => {
            assert_eq!(status, 500);
            let xs = body.chars().filter(|c| *c == 'x').count();
            assert_eq!(xs, 100, "only max_error_body bytes are kept: {body}");
        }
        other => panic!("expected an HTTP error, got {other:?}"),
    }
}

#[valtron_test]
async fn repo_exists_reports_a_transport_failure_as_an_error() {
    // Bind and drop a listener so the port is closed.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port();
    let hf = HFClientBuilder::new()
        .endpoint(format!("http://127.0.0.1:{port}"))
        .build()
        .expect("client");

    let result = repository::repo_exists(&hf.model("owner", "model")).await;
    assert!(
        result.is_err(),
        "an unreachable Hub is an error, not \"does not exist\": {result:?}"
    );
}

#[valtron_test]
async fn repo_exists_maps_status_to_bool() {
    let server = TestHttpServer::with_response(|req| {
        if req.path.url.contains("/api/models/owner/present") {
            json(r#"{"id":"owner/present"}"#)
        } else {
            with_body(404, "Not Found", b"Repository not found")
        }
    });
    let hf = HFClientBuilder::new()
        .endpoint(server.base_url())
        .build()
        .expect("client");

    assert!(repository::repo_exists(&hf.model("owner", "present"))
        .await
        .expect("exists"));
    assert!(!repository::repo_exists(&hf.model("owner", "absent"))
        .await
        .expect("exists"));
}

#[valtron_test]
async fn commit_keeps_binary_file_bytes_intact() {
    let binary: Vec<u8> = vec![0x00, 0xff, 0xfe, 0x80, 0x0d, 0x0a, 0xc3];
    let received = Arc::new(Mutex::new(Vec::<u8>::new()));
    let received_in = Arc::clone(&received);
    let server = TestHttpServer::with_response(move |req| {
        *received_in.lock().unwrap() = body_bytes(&req.body);
        json("{}")
    });
    let hf = HFClientBuilder::new()
        .endpoint(server.base_url())
        .token("hf_test")
        .build()
        .expect("client");

    repository::repo_create_commit(
        &hf.model("owner", "model"),
        &RepoCreateCommitParams {
            operations: vec![CommitOperation::Add {
                path_in_repo: "blob.bin".to_string(),
                source: AddSource::Bytes(binary.clone()),
            }],
            commit_message: "add blob".to_string(),
            commit_description: None,
            revision: None,
            create_pr: None,
        },
    )
    .await
    .expect("commit");

    let body = received.lock().unwrap().clone();
    assert!(
        body.windows(binary.len()).any(|w| w == binary.as_slice()),
        "the multipart body must carry the file's bytes unchanged"
    );
}
