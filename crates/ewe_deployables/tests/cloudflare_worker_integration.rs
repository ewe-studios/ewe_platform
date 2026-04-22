//! Integration tests for Cloudflare Worker deployable.
//!
//! WHY: Verify the simplified 2-method Deployable trait works end-to-end
//!      with state persistence. The HTTP layer is tested at the API level;
//!      here we focus on deployable state management and trait contract.
//!
//! WHAT: Tests state store integration, error handling, and task contract.
//!
//! HOW: Uses FileStateStore for persistence and TestHttpsServer + StaticSocketAddr
//!      for intercepting HTTPS calls to the Cloudflare API.

use std::net::SocketAddr;

use ewe_deployables::cloudflare::{CloudflareWorker, CloudflareWorkerError};
use ewe_deployables::{Deployable, Deploying};

use foundation_core::valtron::{execute, Stream};
use foundation_core::wire::simple_http::client::StaticSocketAddr;
use foundation_db::state::traits::StateStore;
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::types::WorkerDeployment;
use foundation_testing::http::{test_tls_connector, HttpResponse, TestHttpsServer};

/// Build a ProviderClient pointing at the HTTPS test server.
fn make_client(
    server: &TestHttpsServer,
    state_store: FileStateStore,
) -> ProviderClient<FileStateStore, StaticSocketAddr> {
    let port = server.port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let dns = StaticSocketAddr::new(addr);
    let http_client =
        foundation_core::wire::simple_http::client::SimpleHttpClient::with_resolver(dns)
            .with_tls_connector(test_tls_connector())
            .with_connection_pool();

    ProviderClient::new("test-project", "dev", state_store, http_client)
}

type TestWorker = CloudflareWorker<StaticSocketAddr>;

/// Helper to extract the final Ready value from a driven task iterator.
fn collect_task_result<I>(driven: &mut impl Iterator<Item = Stream<I, Deploying>>) -> Option<I> {
    for item in driven {
        if let Stream::Next(result) = item {
            return Some(result);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// State management tests
// ---------------------------------------------------------------------------

#[test]
fn test_namespace_isolation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::start();
    let client = make_client(&server, state_store);
    let worker = TestWorker::new("test", "/dev/null", "account");

    let store = worker.store(&client);
    store.store_typed("key", &"worker-value").unwrap();

    let val: Option<String> = store.get_typed("key").unwrap();
    assert_eq!(val, Some("worker-value".to_string()));

    let raw_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    let raw_keys: Vec<_> = raw_store
        .list()
        .unwrap()
        .filter_map(|v| match v {
            foundation_core::valtron::ThreadedValue::Value(Ok(k)) => Some(k),
            _ => None,
        })
        .collect();

    assert!(
        raw_keys
            .iter()
            .any(|k| k.contains("cloudflare/workers/script")),
        "expected namespaced key in store, got: {raw_keys:?}"
    );
}

#[test]
fn test_list_returns_only_namespaced_keys() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::start();
    let client = make_client(&server, state_store);
    let worker = TestWorker::new("test", "/dev/null", "account");

    let store = worker.store(&client);
    store.store_typed("1", &"one").unwrap();
    store.store_typed("2", &"two").unwrap();

    let keys: Vec<_> = store
        .list()
        .unwrap()
        .filter_map(|v| match v {
            foundation_core::valtron::ThreadedValue::Value(Ok(k)) => Some(k),
            _ => None,
        })
        .collect();

    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"1".to_string()));
    assert!(keys.contains(&"2".to_string()));
}

#[test]
fn test_remove_deletes_key() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::start();
    let client = make_client(&server, state_store);
    let worker = TestWorker::new("test", "/dev/null", "account");

    let store = worker.store(&client);
    store.store_typed("to-remove", &42).unwrap();
    assert!(store.get_typed::<i32>("to-remove").unwrap().is_some());

    store.remove("to-remove").unwrap();
    assert!(store.get_typed::<i32>("to-remove").unwrap().is_none());
}

// ---------------------------------------------------------------------------
// Deploy/destroy error tests (no HTTP call needed)
// ---------------------------------------------------------------------------

#[test]
fn test_deploy_script_not_found() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::start();
    let client = make_client(&server, state_store);

    let worker = TestWorker::new("no-script", "/nonexistent/script.js", "test-account");
    let result = worker.deploy(0, client);
    assert!(result.is_err());

    match result {
        Err(CloudflareWorkerError::IoError { path, .. }) => {
            assert!(path.contains("script.js"));
        }
        Err(e) => panic!("expected IoError, got: {e:?}"),
        Ok(_) => panic!("expected error but deploy succeeded"),
    }
}

#[test]
fn test_destroy_without_state_fails() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::with_response(|_req| HttpResponse::ok(b"{}"));
    let client = make_client(&server, state_store);

    let worker = TestWorker::new("ghost", "/dev/null", "test-account");

    let result = worker.destroy(0, client);
    assert!(result.is_err());

    match result {
        Err(CloudflareWorkerError::ApiError(msg)) => {
            assert!(msg.contains("No state found"));
            assert!(msg.contains("ghost"));
        }
        Err(e) => panic!("expected ApiError, got: {e:?}"),
        Ok(_) => panic!("expected error but destroy succeeded"),
    }
}

// ---------------------------------------------------------------------------
// Deploy/destroy flow tests with HTTPS mock server
// ---------------------------------------------------------------------------

#[test]
fn test_deploy_success_stores_state() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::with_response(|_req| {
        HttpResponse::ok(br#"{"success":true,"result":{"id":"deploy-abc123"}}"#)
    });

    let client = make_client(&server, state_store);

    let script_path = temp_dir.path().join("worker.js");
    std::fs::write(&script_path, "export default {}").unwrap();

    let worker = TestWorker::new("my-worker", script_path.to_str().unwrap(), "my-account");
    let task_result = worker.deploy(0, client.clone()).unwrap();

    let mut driven = execute(task_result, None).unwrap();
    let result = collect_task_result(&mut driven);
    let deployment = result
        .expect("should produce result")
        .expect("deploy should succeed");

    assert_eq!(deployment.script_name, "my-worker");
    assert_eq!(deployment.account_id, "my-account");
    assert_eq!(deployment.deployment_id, "deploy-abc123");

    let store = worker.store(&client);
    let saved: Option<WorkerDeployment> = store.get_typed("0").unwrap();
    assert!(saved.is_some());
    let saved = saved.unwrap();
    assert_eq!(saved.script_name, "my-worker");
    assert_eq!(saved.deployment_id, "deploy-abc123");
}

#[test]
fn test_deploy_api_error() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server =
        TestHttpsServer::with_response(|_req| HttpResponse::status(500, "Internal Server Error"));

    let client = make_client(&server, state_store);

    let script_path = temp_dir.path().join("worker.js");
    std::fs::write(&script_path, "export default {}").unwrap();

    let worker = TestWorker::new("test", script_path.to_str().unwrap(), "test-account");
    let task_result = worker.deploy(0, client).unwrap();

    let mut driven = execute(task_result, None).unwrap();
    let result = collect_task_result(&mut driven);

    assert!(
        result.expect("task should produce a result").is_err(),
        "deploy should fail with 500 from server"
    );
}

#[test]
fn test_destroy_success_removes_state() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let deploy_server = TestHttpsServer::with_response(|_req| {
        HttpResponse::ok(br#"{"success":true,"result":{"id":"deploy-xyz"}}"#)
    });

    let client = make_client(&deploy_server, state_store);

    let script_path = temp_dir.path().join("worker.js");
    std::fs::write(&script_path, "export default {}").unwrap();

    let worker = TestWorker::new(
        "destroy-test",
        script_path.to_str().unwrap(),
        "test-account",
    );

    let deploy_task = worker.deploy(0, client.clone()).unwrap();
    let mut deploy_driven = execute(deploy_task, None).unwrap();
    let deploy_result = collect_task_result(&mut deploy_driven);
    assert!(deploy_result.unwrap().is_ok());

    let before: Option<WorkerDeployment> = worker.store(&client).get_typed("0").unwrap();
    assert!(before.is_some());

    let destroy_server = TestHttpsServer::with_response(|_req| HttpResponse::ok(b"{}"));
    let destroy_state = FileStateStore::with_root(temp_dir.path().to_path_buf());
    let destroy_client = make_client(&destroy_server, destroy_state);

    let destroy_task = worker.destroy(0, destroy_client).unwrap();
    let mut destroy_driven = execute(destroy_task, None).unwrap();
    let destroy_result = collect_task_result(&mut destroy_driven);
    assert!(destroy_result.unwrap().is_ok());

    let after: Option<WorkerDeployment> = worker.store(&client).get_typed("0").unwrap();
    assert!(after.is_none());
}

#[test]
fn test_deploy_yields_pending_states() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let temp_dir = tempfile::tempdir().unwrap();
    let state_store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    state_store.init().unwrap();

    let server = TestHttpsServer::with_response(|_req| {
        HttpResponse::ok(br#"{"success":true,"result":{"id":"pending-test"}}"#)
    });

    let client = make_client(&server, state_store);

    let script_path = temp_dir.path().join("worker.js");
    std::fs::write(&script_path, "export default {}").unwrap();

    let worker = TestWorker::new("pending", script_path.to_str().unwrap(), "account");
    let task = worker.deploy(0, client).unwrap();
    let mut driven = execute(task, None).unwrap();

    let mut saw_pending = false;
    let mut saw_next = false;
    for item in driven.by_ref() {
        match &item {
            Stream::Pending(Deploying::Processing) => saw_pending = true,
            Stream::Next(Ok(_)) => saw_next = true,
            _ => {}
        }
    }

    assert!(saw_pending, "should have seen a Pending state");
    assert!(saw_next, "should have seen a Next result");
}
