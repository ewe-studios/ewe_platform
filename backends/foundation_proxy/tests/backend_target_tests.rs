//! Unit tests for `BackendTarget` deserialization, defaults, and protocol
//! inference. These are the invariants the router and load balancer rely on.

use foundation_proxy::config::{BackendProtocol, BackendState, BackendTarget};
use foundation_proxy::{ProxyConfig, ServiceConfig};

/// WHY: TOML ergonomics — a bare string backend must still yield a full target.
/// WHAT: `["http://localhost:3000"]` deserializes with default weight 1 and
/// default max_connections 1024.
#[test]
fn test_bare_string_backend_gets_defaults() {
    let toml = r#"
domain = "example.com"
public_ip = "1.2.3.4"

[ssl]
provider = "None"

[[services]]
name = "app"
host = "app.example.com"
backends = ["http://localhost:3000"]
"#;
    let config: ProxyConfig = toml::from_str(toml).expect("parse toml");
    let backend = &config.services[0].backends[0];
    assert_eq!(backend.url, "http://localhost:3000");
    assert_eq!(backend.weight, 1, "bare string must default weight to 1");
    assert_eq!(
        backend.max_connections, 1024,
        "bare string must default max_connections to 1024"
    );
}

/// WHY: Operators tune weight and connection caps per backend.
/// WHAT: A backend table with explicit weight/max_connections is honoured, and a
/// table omitting them falls back to defaults.
#[test]
fn test_table_backend_honours_and_defaults_fields() {
    let toml = r#"
domain = "example.com"
public_ip = "1.2.3.4"

[ssl]
provider = "None"

[[services]]
name = "app"
host = "app.example.com"

[[services.backends]]
url = "http://a:3000"
weight = 5
max_connections = 64

[[services.backends]]
url = "http://b:3000"
"#;
    let config: ProxyConfig = toml::from_str(toml).expect("parse toml");
    let backends = &config.services[0].backends;
    assert_eq!(backends.len(), 2);
    assert_eq!(backends[0].weight, 5);
    assert_eq!(backends[0].max_connections, 64);
    assert_eq!(backends[1].weight, 1, "omitted weight defaults to 1");
    assert_eq!(backends[1].max_connections, 1024);
}

/// WHY: Decision 14 infers the protocol from the URL scheme; there is no
/// `proto` field. Wrong inference routes RDP through an HTTP parser.
/// WHAT: `http`/`https`/`tcp` schemes map to the matching `BackendProtocol`, and
/// a scheme-less URL defaults to HTTP.
#[test]
fn test_protocol_inference_from_scheme() {
    assert_eq!(
        BackendTarget::new("http://x:1").protocol(),
        BackendProtocol::Http
    );
    assert_eq!(
        BackendTarget::new("https://x:1").protocol(),
        BackendProtocol::Https
    );
    assert_eq!(
        BackendTarget::new("tcp://x:1").protocol(),
        BackendProtocol::Tcp
    );
    assert_eq!(
        BackendTarget::new("udp://x:1").protocol(),
        BackendProtocol::Udp,
        "udp:// scheme maps to Udp"
    );
    assert_eq!(
        BackendTarget::new("x:1").protocol(),
        BackendProtocol::Http,
        "scheme-less URL defaults to HTTP"
    );
}

/// WHY: UDP passthrough needs the same authority stripping as TCP.
/// WHAT: `udp://host:port/path` → `host:port`.
#[test]
fn test_udp_authority_stripping() {
    assert_eq!(
        BackendTarget::new("udp://127.0.0.1:9000/ignored").authority(),
        "127.0.0.1:9000"
    );
    assert_eq!(
        BackendTarget::new("udp://dns.example.com:53").authority(),
        "dns.example.com:53"
    );
}

/// WHY: TCP passthrough connects to a raw socket address, not a URL.
/// WHAT: `authority()` strips scheme and path, leaving `host:port`.
#[test]
fn test_authority_strips_scheme_and_path() {
    assert_eq!(
        BackendTarget::new("tcp://127.0.0.1:9000/ignored").authority(),
        "127.0.0.1:9000"
    );
    assert_eq!(
        BackendTarget::new("http://host:8080").authority(),
        "host:8080"
    );
}

/// WHY: The builder is the programmatic config path; it must reflect explicit
/// weight/max_connections.
/// WHAT: `ServiceConfig::backend_target` stores the full target.
#[test]
fn test_builder_backend_target() {
    let svc = ServiceConfig::new("api", "api.example.com").backend_target(
        BackendTarget::new("http://a:1")
            .with_weight(3)
            .with_max_connections(10),
    );
    assert_eq!(svc.backends[0].weight, 3);
    assert_eq!(svc.backends[0].max_connections, 10);
}

/// WHY: `BackendState` gates rotation; `Active` is the only routable default.
/// WHAT: Default is `Active`; lowercase serde round-trips.
#[test]
fn test_backend_state_default_and_serde() {
    assert_eq!(BackendState::default(), BackendState::Active);
    let json = serde_json::to_string(&BackendState::Draining).unwrap();
    assert_eq!(json, "\"draining\"");
    let back: BackendState = serde_json::from_str("\"paused\"").unwrap();
    assert_eq!(back, BackendState::Paused);
}
