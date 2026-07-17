//! `server_ops` against a mock Hetzner API (spec-56 F01).
//!
//! **Why a mock and not a fixture:** the interesting behaviour is in what we
//! *send* and how we react to what comes back — the create body, the poll loop,
//! the 401/429 mapping. A serde round-trip proves none of that.
//!
//! No account, no network beyond loopback. The live path stays unverified until
//! someone runs it with a real `HCLOUD_TOKEN` — that is called out in the
//! feature, and mock evidence is not "it works".

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use foundation_core::valtron::valtron_test;
use foundation_deployment_hetzner::{CreateServerRequest, HetznerClient, HetznerError, ServerStatus};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader};
use foundation_testing::http::{HttpResponse, TestHttpServer};

/// A server, as Hetzner would send it.
///
/// **Built from the generated types rather than hand-written JSON.** Hetzner's
/// server object has 20 required fields, only four of which any test here cares
/// about; spelling out the other sixteen would mean a fixture that breaks on
/// every regeneration for reasons unrelated to what is being tested. `Default`
/// fills them, and serialising the real type guarantees the fixture matches the
/// shape the code actually parses.
fn server_json(id: i64, name: &str, status: &str, ip: &str) -> String {
    use foundation_deployment_hetzner::generated::servers::{
        GetServerResponseServer, GetServerResponseServerPublicNet,
        GetServerResponseServerPublicNetIpv4,
    };

    let server = GetServerResponseServer {
        id,
        name: name.to_string(),
        status: status.to_string(),
        public_net: GetServerResponseServerPublicNet {
            ipv4: Some(GetServerResponseServerPublicNetIpv4 {
                ip: ip.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    serde_json::to_string(&server).expect("a generated type serializes")
}

/// A response with a real body.
///
/// `HttpResponse::status(code, text)` puts `text` in the **status line**, not the
/// body — which is right for a reason phrase and wrong for a JSON error envelope.
fn json_response(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        status_text: match status {
            201 => "Created".to_string(),
            409 => "Conflict".to_string(),
            422 => "Unprocessable Entity".to_string(),
            500 => "Internal Server Error".to_string(),
            _ => "OK".to_string(),
        },
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
    }
}

/// A create-server response, built from the generated types for the same reason
/// [`server_json`] is.
fn create_response_json(id: i64, name: &str, status: &str, ip: &str) -> String {
    use foundation_deployment_hetzner::generated::servers::{
        CreateServerResponse, CreateServerResponseServer, CreateServerResponseServerPublicNet,
        CreateServerResponseServerPublicNetIpv4,
    };

    let response = CreateServerResponse {
        server: CreateServerResponseServer {
            id,
            name: name.to_string(),
            status: status.to_string(),
            public_net: CreateServerResponseServerPublicNet {
                ipv4: Some(CreateServerResponseServerPublicNetIpv4 {
                    ip: ip.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        },
        root_password: Some("hunter2".to_string()),
        ..Default::default()
    };
    serde_json::to_string(&response).expect("a generated type serializes")
}

/// An ssh-key list response.
fn ssh_keys_json(keys: &[(i64, &str, &str)]) -> String {
    use foundation_deployment_hetzner::generated::ssh_keys::{
        ListSshKeysResponse, ListSshKeysResponseSshKeysItem,
    };

    let response = ListSshKeysResponse {
        ssh_keys: keys
            .iter()
            .map(|(id, name, fingerprint)| ListSshKeysResponseSshKeysItem {
                id: *id,
                name: (*name).to_string(),
                fingerprint: (*fingerprint).to_string(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    serde_json::to_string(&response).expect("a generated type serializes")
}

/// A single-ssh-key (create) response.
fn ssh_key_json(id: i64, name: &str, fingerprint: &str) -> String {
    use foundation_deployment_hetzner::generated::ssh_keys::{
        CreateSshKeyResponse, CreateSshKeyResponseSshKey,
    };

    let response = CreateSshKeyResponse {
        ssh_key: CreateSshKeyResponseSshKey {
            id,
            name: name.to_string(),
            fingerprint: fingerprint.to_string(),
            ..Default::default()
        },
    };
    serde_json::to_string(&response).expect("a generated type serializes")
}

/// A list-servers response.
fn servers_list_json(servers: &[(i64, &str, &str, &str)]) -> String {
    use foundation_deployment_hetzner::generated::servers::{
        ListServersResponse, ListServersResponseServersItem,
        ListServersResponseServersItemPublicNet, ListServersResponseServersItemPublicNetIpv4,
    };

    let response = ListServersResponse {
        servers: servers
            .iter()
            .map(|(id, name, status, ip)| ListServersResponseServersItem {
                id: *id,
                name: (*name).to_string(),
                status: (*status).to_string(),
                public_net: ListServersResponseServersItemPublicNet {
                    ipv4: Some(ListServersResponseServersItemPublicNetIpv4 {
                        ip: (*ip).to_string(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    serde_json::to_string(&response).expect("a generated type serializes")
}

fn client_for(server: &TestHttpServer) -> HetznerClient {
    // Inject the token rather than exporting it: std::env is process-global and
    // these tests run threaded (decision 02 §5).
    HetznerClient::new("test-token").with_base_url(server.base_url().to_string())
}

// ── create ───────────────────────────────────────────────────────────────────

#[valtron_test]
async fn create_server_sends_what_hetzner_needs_to_build_the_box() {
    let seen = Arc::new(Mutex::new(String::new()));
    let captured = Arc::clone(&seen);

    let server = TestHttpServer::with_response(move |req| {
        *captured.lock().unwrap() = match &req.body {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            other => panic!("unexpected request body shape: {other:?}"),
        };
        json_response(201, &create_response_json(42, "web-1", "initializing", "1.2.3.4"))
    });

    let client = client_for(&server);
    let created = client
        .create_server(&CreateServerRequest {
            name: "web-1".into(),
            server_type: "cx22".into(),
            image: "ubuntu-24.04".into(),
            location: Some("nbg1".into()),
            ssh_keys: vec![7, 9],
            user_data: Some("#cloud-config\nruncmd: [echo hi]".into()),
        })
        .await
        .expect("creates");

    let body: serde_json::Value = serde_json::from_str(&seen.lock().unwrap()).expect("sent JSON");
    assert_eq!(body["name"], "web-1");
    assert_eq!(body["server_type"], "cx22");
    assert_eq!(body["image"], "ubuntu-24.04");
    assert_eq!(body["location"], "nbg1");
    // Hetzner takes ids or names here; we always send ids, as strings.
    assert_eq!(body["ssh_keys"], serde_json::json!(["7", "9"]));
    // The cloud-init payload is what makes hardening-before-first-boot possible
    // (feature 04) — dropping it silently would produce an unhardened box.
    assert!(
        body["user_data"].as_str().unwrap().contains("#cloud-config"),
        "user_data must reach Hetzner: {body}"
    );
    // A created-but-off server bills and cannot be reached.
    assert_eq!(body["start_after_create"], true);

    assert_eq!(created.id, 42);
    assert_eq!(created.status, ServerStatus::Initializing, "not usable yet");
}

#[valtron_test]
async fn create_server_sends_the_token_as_a_bearer() {
    let auth = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&auth);

    let server = TestHttpServer::with_response(move |req| {
        *captured.lock().unwrap() = req
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|v| v.first().cloned());
        json_response(201, &create_response_json(1, "n", "initializing", "1.2.3.4"))
    });

    let _ = client_for(&server)
        .create_server(&CreateServerRequest {
            name: "n".into(),
            server_type: "cx22".into(),
            image: "ubuntu-24.04".into(),
            ..Default::default()
        })
        .await;

    assert_eq!(
        auth.lock().unwrap().as_deref(),
        Some("Bearer test-token"),
        "every call carries the token"
    );
}

// ── the error mapping ────────────────────────────────────────────────────────

#[valtron_test]
async fn a_401_is_unauthorized_not_a_generic_failure() {
    // "Your token is wrong" and "the network is down" want different responses
    // from the caller (decision 02 §3).
    let server = TestHttpServer::with_response(|_| {
        json_response(401, r#"{"error":{"code":"unauthorized","message":"unable to authenticate"}}"#)
    });

    let err = client_for(&server)
        .list_servers(None)
        .await
        .expect_err("401");
    assert_eq!(err, HetznerError::Unauthorized, "got {err}");
}

#[valtron_test]
async fn a_429_carries_how_long_to_wait() {
    let reset = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 30;

    let server = TestHttpServer::with_response(move |_| HttpResponse {
        status: 429,
        status_text: "Too Many Requests".to_string(),
        headers: vec![
            ("RateLimit-Reset".to_string(), reset.to_string()),
            ("Content-Length".to_string(), "0".to_string()),
        ],
        body: Vec::new(),
    });

    match client_for(&server).list_servers(None).await {
        // Hetzner sends a unix timestamp; a caller wants "how long do I wait",
        // so the conversion belongs in the crate, not every call site.
        Err(HetznerError::RateLimited { retry_after_secs }) => {
            let secs = retry_after_secs.expect("the header said when");
            assert!(secs <= 30 && secs > 25, "expected ~30s, got {secs}");
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[valtron_test]
async fn a_429_without_the_header_still_maps_rather_than_inventing_a_backoff() {
    let server = TestHttpServer::with_response(|_| HttpResponse::status(429, ""));
    match client_for(&server).list_servers(None).await {
        Err(HetznerError::RateLimited { retry_after_secs: None }) => {}
        other => panic!("expected RateLimited with no hint, got {other:?}"),
    }
}

#[valtron_test]
async fn an_api_error_keeps_hetzners_own_code_and_message() {
    // The status number alone does not say what to fix.
    let server = TestHttpServer::with_response(|_| {
        json_response(422, r#"{"error":{"code":"invalid_input","message":"server_type cx99 does not exist"}}"#)
    });

    match client_for(&server)
        .create_server(&CreateServerRequest {
            name: "n".into(),
            server_type: "cx99".into(),
            image: "ubuntu-24.04".into(),
            ..Default::default()
        })
        .await
    {
        Err(HetznerError::Api { status, code, message }) => {
            assert_eq!(status, 422);
            assert_eq!(code, "invalid_input", "the stable string, not just a number");
            assert!(message.contains("cx99"), "{message}");
        }
        other => panic!("expected Api, got {other:?}"),
    }
}

#[valtron_test]
async fn an_unparseable_error_body_is_reported_rather_than_discarded() {
    // An error we cannot parse is still the only evidence of what went wrong.
    let server = TestHttpServer::with_response(|_| json_response(500, "<html>oops</html>"));
    match client_for(&server).list_servers(None).await {
        Err(HetznerError::Api { code, message, .. }) => {
            assert_eq!(code, "unparseable");
            assert!(message.contains("oops"), "the body survives: {message}");
        }
        other => panic!("expected Api, got {other:?}"),
    }
}

// ── get / list / delete ──────────────────────────────────────────────────────

#[valtron_test]
async fn get_server_reads_the_ip_out_of_public_net() {
    // The field that was an opaque blob until the resolve_schema_key fix.
    let server = TestHttpServer::with_response(|_| {
        HttpResponse::ok(
            format!(r#"{{"server":{}}}"#, server_json(42, "web-1", "running", "1.2.3.4")).as_bytes(),
        )
    });

    let found = client_for(&server)
        .get_server(42)
        .await
        .expect("gets")
        .expect("exists");
    assert_eq!(found.public_ipv4.as_deref(), Some("1.2.3.4"));
    assert_eq!(found.status, ServerStatus::Running);
}

#[valtron_test]
async fn a_missing_server_is_none_rather_than_an_error() {
    // A 404 here is an answer, not a failure — destroy relies on being able to ask.
    let server = TestHttpServer::with_response(|_| {
        json_response(404, r#"{"error":{"code":"not_found","message":"nope"}}"#)
    });
    assert_eq!(client_for(&server).get_server(42).await.expect("asks"), None);
}

#[valtron_test]
async fn find_server_by_name_filters_at_hetzner_not_locally() {
    // A local filter over an unpaginated list would miss servers past page 1.
    let query = Arc::new(Mutex::new(String::new()));
    let captured = Arc::clone(&query);

    let server = TestHttpServer::with_response(move |req| {
        *captured.lock().unwrap() = req.path.to_string();
        HttpResponse::ok(servers_list_json(&[(42, "web-1", "running", "1.2.3.4")]).as_bytes())
    });

    let found = client_for(&server)
        .find_server_by_name("web-1")
        .await
        .expect("looks up");
    assert_eq!(found.map(|s| s.id), Some(42));
    assert!(
        query.lock().unwrap().contains("name=web-1"),
        "the filter goes to Hetzner: {}",
        query.lock().unwrap()
    );
}

#[valtron_test]
async fn deleting_a_server_that_is_already_gone_succeeds() {
    // Idempotent: the caller's goal — "this server does not exist" — already
    // holds. Failing would strand state pointing at nothing.
    let server = TestHttpServer::with_response(|_| {
        json_response(404, r#"{"error":{"code":"not_found","message":"gone"}}"#)
    });
    client_for(&server).delete_server(42).await.expect("already gone is fine");
}

// ── await_running ────────────────────────────────────────────────────────────

#[valtron_test]
async fn await_running_polls_until_the_server_settles() {
    // Hetzner's create returns while the box is still building; a caller that
    // trusted it would try to SSH into nothing.
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);

    let server = TestHttpServer::with_response(move |_| {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        let status = if n < 2 { "initializing" } else { "running" };
        HttpResponse::ok(
            format!(r#"{{"server":{}}}"#, server_json(42, "web-1", status, "1.2.3.4")).as_bytes(),
        )
    });

    let ready = client_for(&server).await_running(42).await.expect("settles");
    assert_eq!(ready.status, ServerStatus::Running);
    assert!(
        calls.load(Ordering::SeqCst) >= 3,
        "it polled rather than returning the first answer"
    );
}

#[valtron_test]
async fn await_running_returns_on_a_settled_failure_instead_of_spinning() {
    // A loop watching only for Running spins the full 5-minute timeout on a box
    // that will never reach it. `off` is settled — report it and let the caller
    // decide.
    let server = TestHttpServer::with_response(|_| {
        HttpResponse::ok(format!(r#"{{"server":{}}}"#, server_json(42, "w", "off", "1.2.3.4")).as_bytes())
    });

    let settled = client_for(&server).await_running(42).await.expect("returns");
    assert_eq!(settled.status, ServerStatus::Off, "settled, not running");
}

#[valtron_test]
async fn a_server_that_vanishes_mid_poll_is_not_reported_as_a_timeout() {
    // It is a different problem and deserves a different message — and waiting
    // five minutes to say so would be wrong twice.
    let server = TestHttpServer::with_response(|_| {
        json_response(404, r#"{"error":{"code":"not_found","message":"gone"}}"#)
    });

    match client_for(&server).await_running(42).await {
        Err(HetznerError::Api { status: 404, message, .. }) => {
            assert!(message.contains("disappeared"), "{message}");
        }
        other => panic!("expected a 404 Api error, got {other:?}"),
    }
}

// ── ssh keys ─────────────────────────────────────────────────────────────────

#[valtron_test]
async fn ensure_ssh_key_returns_the_existing_key_when_hetzner_says_duplicate() {
    // A deploy re-run must not fail because it already succeeded once. Identity
    // comes from Hetzner — we never fingerprint locally, because a disagreement
    // with their rule would register duplicates while we believed otherwise.
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);

    let server = TestHttpServer::with_response(move |_| {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            // POST /ssh_keys — Hetzner rejects the duplicate.
            json_response(409, r#"{"error":{"code":"uniqueness_error","message":"ssh key already exists"}}"#)
        } else {
            // GET /ssh_keys — ask Hetzner which one it is.
            HttpResponse::ok(ssh_keys_json(&[(7, "deploy", "aa:bb:cc")]).as_bytes())
        }
    });

    let key = client_for(&server)
        .ensure_ssh_key("deploy", "ssh-ed25519 AAAA")
        .await
        .expect("finds the existing key");
    assert_eq!(key.id, 7);
    assert_eq!(key.fingerprint, "aa:bb:cc", "Hetzner computed it, not us");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "create, then look up");
}

#[valtron_test]
async fn a_duplicate_whose_key_is_then_missing_is_a_real_conflict() {
    // The name collides with a key holding different bytes. Papering over that
    // would have the deploy trust a key it cannot log in with.
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);

    let server = TestHttpServer::with_response(move |_| {
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            json_response(409, r#"{"error":{"code":"uniqueness_error","message":"exists"}}"#)
        } else {
            HttpResponse::ok(ssh_keys_json(&[]).as_bytes())
        }
    });

    match client_for(&server).ensure_ssh_key("deploy", "ssh-ed25519 AAAA").await {
        Err(HetznerError::Api { code, message, .. }) => {
            assert_eq!(code, "uniqueness_error");
            assert!(message.contains("different name"), "says what happened: {message}");
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
}

#[valtron_test]
async fn ensure_ssh_key_creates_when_there_is_none() {
    let server = TestHttpServer::with_response(|_| {
        json_response(201, r#"{"ssh_key":{"id":9,"name":"deploy","fingerprint":"dd:ee:ff","public_key":"ssh-ed25519 BBBB","labels":{},"created":"x"}}"#)
    });

    let key = client_for(&server)
        .ensure_ssh_key("deploy", "ssh-ed25519 BBBB")
        .await
        .expect("creates");
    assert_eq!(key.id, 9);
    assert_eq!(key.fingerprint, "dd:ee:ff");
}
