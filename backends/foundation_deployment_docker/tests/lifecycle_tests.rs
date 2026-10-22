//! End-to-end tests for `DockerClient` over a mock Docker daemon on a Unix socket.
//!
//! WHY: Proves the generated async `*_request` functions + hand-written
//! `DockerClient` actually round-trip over the Unix-socket transport (Feature 02)
//! — version negotiation and serde (de)serialization of PascalCase Docker JSON
//! via per-field `#[serde(rename)]` — without a real dockerd.
//!
//! WHAT: A tiny `UnixListener` HTTP server that routes by request path and
//! returns canned JSON, plus `#[valtron_test]` async fns that drive each
//! `DockerClient` method via `.await`.
//!
//! HOW: `#[valtron_test]` accepts `async fn` (feature 00-F3); no manual
//! `block_on_future` needed.

#![cfg(all(unix, feature = "docker"))]

mod mock_socket;

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_deployment_docker::generated::version::SystemVersion;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ewe_docker_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Parse `"major.minor"` for the mock's version checks.
fn parse_version(v: &str) -> (u32, u32) {
    let (major, minor) = v.split_once('.').expect("major.minor");
    (major.parse().expect("major"), minor.parse().expect("minor"))
}

/// Route a request path to a canned response, the way a real daemon whose
/// newest API version is `api_version` (default `"1.53"`) and oldest is
/// `"1.24"` would:
///
/// - `GET /version` answers unversioned, and also under any version it accepts;
/// - a request for a version newer than `api_version` gets `400`, with the
///   message Docker sends;
/// - `GET /v…/_ping` and `GET /v…/info` answer under any accepted version.
fn route(method: &str, path: &str, api_version: Option<&str>) -> (u16, String) {
    let version = api_version.unwrap_or("1.53");
    let (requested, p) = match path.strip_prefix("/v") {
        Some(rest) => match rest.split_once('/') {
            Some((v, r)) => (Some(v), r),
            None => (None, path.trim_start_matches('/')),
        },
        None => (None, path.trim_start_matches('/')),
    };
    if let Some(requested) = requested {
        if parse_version(requested) > parse_version(version) {
            return (
                400,
                format!(
                    "{{\"message\":\"client version {requested} is too new. \
                     Maximum supported API version is {version}\"}}"
                ),
            );
        }
    }
    match (method, p) {
        ("GET", "version") => (
            200,
            format!(
                r#"{{"ApiVersion":"{version}","MinAPIVersion":"1.24","Os":"linux","Arch":"amd64","Version":"27.0.0"}}"#
            ),
        ),
        ("GET", "_ping") => (200, "OK".to_string()),
        ("GET", "info") => (200, r#"{"Containers":0}"#.to_string()),
        _ => (404, format!("{{\"message\":\"no route for {method} {p}\"}}")),
    }
}

/// Start a mock Docker daemon on `socket_path`. Serves until `stop` is set.
/// `api_version` is the `ApiVersion` value the mock reports in `GET /version`.
fn mock_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>, api_version: Option<&'static str>) {
    mock_daemon_recording(socket_path, stop, api_version, Arc::new(Mutex::new(Vec::new())));
}

/// [`mock_daemon`], also recording every request's path into `seen`.
fn mock_daemon_recording(
    socket_path: PathBuf,
    stop: Arc<AtomicBool>,
    api_version: Option<&'static str>,
    seen: Arc<Mutex<Vec<String>>>,
) {
    mock_socket::serve(&socket_path, stop, move |mut stream| {
        let Some(req) = mock_socket::read_request(&mut stream) else {
            return;
        };
        let (method, path) = (req.method.as_str(), req.target.as_str());
        seen.lock().expect("request log lock").push(path.to_string());
        let (status, body) = route(method, path, api_version);
        let reason = match status {
            204 => "No Content",
            400 => "Bad Request",
            404 => "Not Found",
            _ => "OK",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });
}

#[valtron_test]
async fn docker_version_parses_pascalcase_json() {
    // WHY: Proves per-field #[serde(rename)] is emitted by the generator so
    // PascalCase Docker JSON (ApiVersion, Os, Arch) deserializes into snake_case
    // Rust fields (api_version, os, arch).
    let socket = temp_socket_path("version");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), None);
    let client = DockerClient::connect_unix(&socket);

    let v = client.version().await.expect("version() should succeed");
    assert_eq!(v.api_version.as_deref(), Some("1.53"));
    assert_eq!(v.os.as_deref(), Some("linux"));
    assert_eq!(v.arch.as_deref(), Some("amd64"));
    assert_eq!(v.version.as_deref(), Some("27.0.0"));

    mock_socket::stop(&socket, &stop);
}

#[valtron_test]
async fn docker_version_twice_sequential() {
    // WHY: Proves the mock daemon handles multiple sequential requests over the
    // same connection without leaking state.
    let socket = temp_socket_path("version2x");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), None);
    let client = DockerClient::connect_unix(&socket);

    let v1 = client.version().await.expect("first version()");
    assert_eq!(v1.api_version.as_deref(), Some("1.53"));

    let v2 = client.version().await.expect("second version()");
    assert_eq!(v2.api_version.as_deref(), Some("1.53"));

    mock_socket::stop(&socket, &stop);
}

#[valtron_test]
async fn docker_negotiate_version_lowers_to_older_server() {
    let socket = temp_socket_path("negotiate_down");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), Some("1.41"));

    let client = DockerClient::connect_unix(&socket);
    assert_eq!(client.api_version(), "1.53", "starts at client default");

    client
        .negotiate_version()
        .await
        .expect("negotiate_version should succeed");

    assert_eq!(
        client.api_version(),
        "1.41",
        "lowered to server cap when server is older"
    );

    mock_socket::stop(&socket, &stop);
}

#[valtron_test]
async fn docker_negotiate_version_stays_when_server_is_newer() {
    let socket = temp_socket_path("negotiate_up");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), Some("1.99"));

    let client = DockerClient::connect_unix(&socket);
    client
        .negotiate_version()
        .await
        .expect("negotiate_version should succeed");

    assert_eq!(
        client.api_version(),
        "1.53",
        "stays at client default when server is newer"
    );

    mock_socket::stop(&socket, &stop);
}

/// The CI regression: a daemon older than the client rejects `/v1.53/…` with
/// 400. The first request must negotiate (unversioned `GET /version`) and then
/// go out with the daemon's version — without anyone calling
/// `negotiate_version` by hand.
#[valtron_test]
async fn first_request_negotiates_with_an_older_daemon() {
    let socket = temp_socket_path("negotiate_lazy");
    let stop = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(Vec::new()));
    mock_daemon_recording(socket.clone(), stop.clone(), Some("1.47"), seen.clone());

    let client = DockerClient::connect_unix(&socket);
    let info = client.system_info().await.expect("info against a 1.47 daemon");
    assert_eq!(info["Containers"], 0);
    assert_eq!(client.api_version(), "1.47");

    // A clone shares the negotiated version: no second /version round trip.
    let clone = client.clone();
    clone.system_ping().await.expect("ping with the negotiated version");

    let paths = seen.lock().expect("request log lock").clone();
    assert_eq!(paths, vec!["/version", "/v1.47/info", "/v1.47/_ping"], "requests sent");

    mock_socket::stop(&socket, &stop);
}

/// Without negotiation, the same daemon answers 400. That is an API error
/// from a daemon that is very much reachable, never a transport failure.
#[valtron_test]
async fn a_too_new_version_is_an_api_error_not_unreachable() {
    let socket = temp_socket_path("too_new");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), Some("1.47"));

    let client = DockerClient::connect_unix(&socket);
    // Bypass negotiation on purpose: build the URL with the default version.
    let url = format!("{}/_ping", client.base_url());
    let builder = foundation_netio::PreparedRequestBuilder::get(&url).expect("request");
    let response = client.http().send_async(builder.build()).await.expect("daemon answers");
    let status: usize = response.get_status().into();
    assert_eq!(status, 400, "an older daemon rejects /v1.53/");

    // And through the error mapping the client uses:
    let err = foundation_deployment_docker::DockerError::from(
        foundation_deployment_docker::generated::shared::ApiError::HttpStatus {
            code: 400,
            headers: response.get_headers_ref().clone(),
            body: None,
        },
    );
    assert!(!err.is_unreachable(), "{err}");

    mock_socket::stop(&socket, &stop);
}

/// A daemon whose oldest supported version is newer than the client's newest
/// shares no version with it: negotiation fails, and says why.
#[valtron_test]
async fn negotiation_reports_a_daemon_with_no_common_version() {
    let socket = temp_socket_path("no_overlap");
    let stop = Arc::new(AtomicBool::new(false));
    // The mock's MinAPIVersion is 1.24, so a client of 1.53 always overlaps
    // with it; check the error path through the pure function the client uses.
    mock_daemon(socket.clone(), stop.clone(), Some("1.60"));
    let client = DockerClient::connect_unix(&socket);
    let v = client.version().await.expect("version");
    assert_eq!(v.min_api_version.as_deref(), Some("1.24"));
    let err = foundation_deployment_docker::client::negotiate_api_version("1.20", v.api_version.as_deref(), v.min_api_version.as_deref())
        .expect_err("1.20 is older than the daemon's minimum");
    assert!(
        matches!(err, foundation_deployment_docker::DockerError::UnsupportedApiVersion { .. }),
        "{err:?}"
    );
    assert!(!err.is_unreachable());

    mock_socket::stop(&socket, &stop);
}

/// No daemon at the socket: negotiation fails as a transport error (the daemon
/// is unreachable), and nothing is cached, so a later request tries again.
#[valtron_test]
async fn negotiation_with_no_daemon_is_unreachable_and_not_cached() {
    let socket = temp_socket_path("absent");
    let client = DockerClient::connect_unix(&socket);
    let err = client.negotiate_version().await.expect_err("no daemon there");
    assert!(err.is_unreachable(), "{err:?}");
    assert_eq!(client.api_version(), "1.53");

    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), Some("1.45"));
    client.negotiate_version().await.expect("daemon is up now");
    assert_eq!(client.api_version(), "1.45");

    mock_socket::stop(&socket, &stop);
}

#[test]
fn system_version_deserializes_pascalcase_json_via_per_field_rename() {
    // WHY: Unit test proving the generator emits per-field #[serde(rename)] for
    // properties whose PascalCase original name differs from the snake_case Rust
    // field name. This handles Docker's mixed-casing (ApiVersion + architecture).
    let json = r#"{"ApiVersion":"1.53","Os":"linux","Arch":"amd64","Version":"27.0.0"}"#;
    let v: SystemVersion = serde_json::from_str(json).expect("deserialize PascalCase JSON");
    assert_eq!(v.api_version.as_deref(), Some("1.53"));
    assert_eq!(v.os.as_deref(), Some("linux"));
    assert_eq!(v.arch.as_deref(), Some("amd64"));
    assert_eq!(v.version.as_deref(), Some("27.0.0"));
}
