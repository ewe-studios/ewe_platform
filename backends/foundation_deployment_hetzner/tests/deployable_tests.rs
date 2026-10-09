//! [`HetznerServer`] failure paths a real provider will not perform on demand.
//!
//! **Everything a real Hetzner can be asked to do is tested in `live_tests.rs`,
//! not here.** This crate exists to exercise a provider's API, so confidence has
//! to come from the provider. The mocks that used to live here — first deploy,
//! create-or-find, destroy, instance ids — are gone: the live lifecycle proves all
//! of them on a real machine, for a fraction of a cent.
//!
//! That is not a stylistic preference, it is the record. Across this feature:
//!
//! | Source | Bugs found |
//! |---|---|
//! | Real vendor specs | 6 |
//! | The live API | 4 |
//! | **31 mock tests** | **0** |
//!
//! And worse than nothing twice over: one mock **asserted the bug** (`destroy`
//! "does not call Hetzner to find that out" — the exact reasoning that stranded a
//! billing server), and every mock here happily accepted a label value Hetzner
//! rejects as malformed. A mock answers what you told it; teach it your
//! misunderstanding and it will agree with you forever.
//!
//! **What is left has to earn it.** Each test below reproduces a fault the vendor
//! will not produce to order — an unreadable 201, a server that settles `off`, one
//! with no public IPv4. If a case *can* be created at Hetzner, it belongs in
//! `live_tests.rs`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStore;
use foundation_db::core::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::traits::Deployable;
use foundation_deployment_hetzner::generated::servers::{
    CreateServerResponse, CreateServerResponseServer, CreateServerResponseServerPublicNet,
    CreateServerResponseServerPublicNetIpv4, GetServerResponse, GetServerResponseServer,
    GetServerResponseServerPublicNet, GetServerResponseServerPublicNetIpv4, ListServersResponse,
    ListServersResponseServersItem, ListServersResponseServersItemPublicNet,
    ListServersResponseServersItemPublicNetIpv4,
};
use foundation_deployment_hetzner::{HetznerClient, HetznerServer};
use foundation_netio::shared::client::dns::SystemDnsResolver;
use foundation_netio::http::NativeHttpClient;
use foundation_testing::http::{HttpResponse, TestHttpServer};

/// Every request the mock saw, as `METHOD path`.
type Calls = Arc<Mutex<Vec<String>>>;

fn get_server_json(id: i64, name: &str, status: &str, ip: &str) -> String {
    serde_json::to_string(&GetServerResponse {
        server: Some(GetServerResponseServer {
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
        }),
    })
    .expect("serializes")
}

fn create_server_json(id: i64, name: &str, status: &str, ip: &str) -> String {
    serde_json::to_string(&CreateServerResponse {
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
    })
    .expect("serializes")
}

fn list_servers_json(servers: &[(i64, &str, &str, &str)]) -> String {
    serde_json::to_string(&ListServersResponse {
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
    })
    .expect("serializes")
}

fn json(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
    }
}

/// A `ProviderClient` whose state store is a fresh temp dir.
///
/// The HTTP client here goes unused — `HetznerServer` carries its own, pointed at
/// the mock — but the trait requires one.
fn provider_client(dir: &std::path::Path) -> ProviderClient<FileStateStore, SystemDnsResolver> {
    let store = FileStateStore::new(dir, "ewe-test", "dev");
    store.init().expect("init state store");
    ProviderClient::new("ewe-test", "dev", store, NativeHttpClient::default())
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ewe-hz-deploy-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A mock that records each call and answers from `responder`.
fn mock(responder: impl Fn(&str, usize) -> HttpResponse + Send + 'static) -> (TestHttpServer, Calls) {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&calls);
    let n = AtomicUsize::new(0);

    let server = TestHttpServer::with_response(move |req| {
        let call = format!("{:?} {}", req.method, req.path);
        recorded.lock().unwrap().push(call.clone());
        responder(&call, n.fetch_add(1, Ordering::SeqCst))
    });
    (server, calls)
}

/// Whether this call is deploy/destroy enumerating its own instances.
///
/// They ask by **label** now — `?label_selector=ewe-deployment=hetzner/cloud/servers/0`
/// — because a label is stamped in the create request and survives a rename, where
/// a name is a user-chosen attribute and an id is an opaque number Hetzner assigns.
/// (Kept matching `name=` too: `find_server_by_name` is still public API.)
fn is_list_query(call: &str) -> bool {
    call.contains("label_selector") || call.contains("name=")
}

fn server_for(mock: &TestHttpServer, name: &str) -> HetznerServer {
    HetznerServer::new(
        HetznerClient::new("test-token").with_base_url(mock.base_url().to_string()),
        name,
    )
}

// ── create-or-find ───────────────────────────────────────────────────────────





#[valtron_test]
async fn a_dying_server_does_not_count_as_alive() {
    // "Alive" must mean alive, not "the API answered" — a `deleting` server still
    // resolves, and returning one hands the caller a corpse (decision 03 §3).
    let dir = tmpdir("dying");
    let (mock_server, calls) = mock(|call, _| {
        if call.contains("POST") {
            json(201, &create_server_json(43, "web-1", "initializing", "5.6.7.8"))
        } else if is_list_query(call) {
            json(200, &list_servers_json(&[]))
        } else if call.contains("/servers/42") {
            json(200, &get_server_json(42, "web-1", "deleting", "1.2.3.4"))
        } else {
            json(200, &get_server_json(43, "web-1", "running", "5.6.7.8"))
        }
    });

    let deployable = server_for(&mock_server, "web-1");
    let client = provider_client(&dir);
    deployable
        .store(&client)
        .store_typed(
            "0",
            &foundation_deployment_hetzner::ServerDeployOutput {
                provider: "hetzner".to_string(),
                id: 42,
                name: "web-1".to_string(),
                public_ip: "1.2.3.4".to_string(),
                identity: Some("ewe-deployment=hetzner/cloud/servers/0".to_string()),
                // What a previous deploy of this same declaration would have
                // written. Omitting it would make deploy call the record stale —
                // correctly, which is the point of the field.
                declared: Some(foundation_deployment_hetzner::deployable::ServerDeclaration {
                    name: "web-1".to_string(),
                    server_type: "cx23".to_string(),
                    image: "ubuntu-24.04".to_string(),
                    location: None,
                }),
            },
        )
        .expect("seeds state");

    let out = deployable.deploy(0, client.clone()).await.expect("replaces the corpse");
    assert_eq!(out.id, 43, "did not return the deleting server");
    assert!(
        calls.lock().unwrap().iter().any(|c| c.contains("POST")),
        "a dying box is gone, so a new one is created"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── failure unwinds the bill ─────────────────────────────────────────────────

#[valtron_test]
async fn a_failure_after_create_destroys_the_server_rather_than_stranding_a_bill() {
    // The mirror of spec-53's stranded container, except this one costs money
    // (decision 03 §5).
    let dir = tmpdir("unwind");
    let (mock_server, calls) = mock(|call, _| {
        if call.contains("POST") {
            json(201, &create_server_json(42, "web-1", "initializing", "1.2.3.4"))
        } else if is_list_query(call) {
            json(200, &list_servers_json(&[]))
        } else if call.contains("DELETE") {
            json(200, "{}")
        } else {
            // await_running fails: the box settled `off`, so output_for refuses it.
            json(200, &get_server_json(42, "web-1", "off", "1.2.3.4"))
        }
    });

    let err = server_for(&mock_server, "web-1")
        .deploy(0, provider_client(&dir))
        .await
        .expect_err("the box never came up");
    assert!(err.to_string().contains("not running"), "{err}");

    let seen = calls.lock().unwrap().clone();
    assert!(
        seen.iter().any(|c| c.contains("DELETE")),
        "a created-but-unusable server must be destroyed, not left billing: {seen:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn keep_on_failure_leaves_the_server_for_inspection() {
    let dir = tmpdir("keep");
    let (mock_server, calls) = mock(|call, _| {
        if call.contains("POST") {
            json(201, &create_server_json(42, "web-1", "initializing", "1.2.3.4"))
        } else if is_list_query(call) {
            json(200, &list_servers_json(&[]))
        } else if call.contains("DELETE") {
            json(200, "{}")
        } else {
            json(200, &get_server_json(42, "web-1", "off", "1.2.3.4"))
        }
    });

    let _ = server_for(&mock_server, "web-1")
        .keep_on_failure(true)
        .deploy(0, provider_client(&dir))
        .await
        .expect_err("still fails");

    assert!(
        !calls.lock().unwrap().iter().any(|c| c.contains("DELETE")),
        "keep_on_failure means the box stays — and keeps billing, which the log says"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn a_server_with_no_public_ip_is_refused_rather_than_returned() {
    // Nothing downstream can reach it, and SshHardening would fail with a much
    // less useful message.
    let dir = tmpdir("no-ip");
    let (mock_server, _) = mock(|call, _| {
        if call.contains("POST") {
            json(201, &create_server_json(42, "web-1", "initializing", ""))
        } else if is_list_query(call) {
            json(200, &list_servers_json(&[]))
        } else if call.contains("DELETE") {
            json(200, "{}")
        } else {
            json(200, &get_server_json(42, "web-1", "running", ""))
        }
    });

    let err = server_for(&mock_server, "web-1")
        .deploy(0, provider_client(&dir))
        .await
        .expect_err("no address");
    assert!(err.to_string().contains("no public IPv4"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── destroy ──────────────────────────────────────────────────────────────────



// ── instances are independent ────────────────────────────────────────────────


// ── the leak that actually happened ──────────────────────────────────────────



#[valtron_test]
async fn a_create_whose_response_we_cannot_read_still_yields_the_server() {
    // A mock earns its place here: Hetzner will not return an unparseable 201 on
    // demand, and this is the failure that shipped — the vendor built the machine
    // and our decode of the receipt failed.
    //
    // The create response is a HINT, not the record. Everything identifying the
    // instance — the name, the slot's label — went out in the REQUEST, so the
    // machine carries them whether or not the reply is readable. Destroying it and
    // returning an error (my first design) throws away exactly what the caller
    // asked for because we could not read a receipt.
    let dir = tmpdir("create-unreadable");
    let deleted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&deleted);
    let exists = Arc::new(Mutex::new(false));
    let state = Arc::clone(&exists);

    let (mock_server, _) = mock(move |call, _| {
        if call.contains("POST") {
            // 201: the server EXISTS from here on. The body is garbage, so we
            // never learn its id from the response.
            *state.lock().unwrap() = true;
            json(201, r#"{"server":"this is not a server object"}"#)
        } else if call.contains("DELETE") {
            log.lock().unwrap().push(call.to_string());
            json(200, "{}")
        } else if is_list_query(call) {
            if *state.lock().unwrap() {
                json(200, &list_servers_json(&[(42, "web-1", "running", "1.2.3.4")]))
            } else {
                json(200, &list_servers_json(&[]))
            }
        } else {
            json(200, &get_server_json(42, "web-1", "running", "1.2.3.4"))
        }
    });

    let out = server_for(&mock_server, "web-1")
        .deploy(0, provider_client(&dir))
        .await
        .expect("the server exists and carries our label — deploy must adopt it");

    assert_eq!(out.id, 42, "adopted the server the create actually made");
    assert_eq!(out.public_ip, "1.2.3.4", "and read its details back from Hetzner");
    assert!(
        deleted.lock().unwrap().is_empty(),
        "it must NOT destroy a server the caller asked for just because the receipt was \
         unreadable: {:?}",
        deleted.lock().unwrap()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
