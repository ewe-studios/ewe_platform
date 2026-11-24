//! Configuration tests (spec 82 feature 01, T1).

mod support;

use foundation_registrar::{RegistrarConfig, RegistrarError, RegistrarMode};

fn args(extra: &[&str]) -> Vec<String> {
    let mut items = vec![
        "ewe-registrar".to_string(),
        "--host-id".to_string(),
        "host-17".to_string(),
        "--proxy".to_string(),
        "https://127.0.0.1:9443".to_string(),
    ];
    items.extend(extra.iter().map(|value| (*value).to_string()));
    items
}

#[test]
fn host_mode_without_an_advertise_address_is_a_config_error() {
    let error = RegistrarConfig::from_args(args(&["--mode", "host"]))
        .expect_err("host mode needs an advertise address");
    match error {
        RegistrarError::Config(message) => assert!(
            message.contains("advertise_host"),
            "the message names the field: {message}"
        ),
        other => panic!("expected a config error, got {other:?}"),
    }
}

#[test]
fn container_mode_needs_no_advertise_address() {
    let config =
        RegistrarConfig::from_args(args(&["--mode", "container"])).expect("container mode parses");
    assert_eq!(config.mode, RegistrarMode::Container);
    assert_eq!(config.host_id, "host-17");
    assert_eq!(config.proxy, "https://127.0.0.1:9443");
}

#[test]
fn host_mode_with_an_advertise_address_parses() {
    let config =
        RegistrarConfig::from_args(args(&["--mode", "host", "--advertise-host", "10.0.0.2"]))
            .expect("host mode with an address");
    assert_eq!(config.mode, RegistrarMode::Host);
    assert_eq!(config.advertise_host.as_deref(), Some("10.0.0.2"));
}

#[test]
fn a_unix_socket_proxy_is_accepted() {
    let config = RegistrarConfig::from_args([
        "ewe-registrar".to_string(),
        "--mode".to_string(),
        "container".to_string(),
        "--host-id".to_string(),
        "host-17".to_string(),
        "--proxy".to_string(),
        "unix:///run/ewe/registration.sock".to_string(),
    ])
    .expect("a Unix socket is a valid proxy");
    assert!(config.proxy.starts_with("unix://"));
}

#[test]
fn a_missing_host_id_is_a_config_error() {
    let error = RegistrarConfig::from_args([
        "ewe-registrar".to_string(),
        "--mode".to_string(),
        "container".to_string(),
        "--proxy".to_string(),
        "https://127.0.0.1:9443".to_string(),
    ])
    .expect_err("host_id is required");
    assert!(matches!(error, RegistrarError::Config(_)));
}

#[test]
fn an_unknown_mode_is_a_config_error() {
    let error = RegistrarConfig::from_args(args(&["--mode", "sidecar"]))
        .expect_err("an unknown mode is refused");
    assert!(matches!(error, RegistrarError::Config(_)));
}

#[test]
fn the_watched_entries_default_to_all() {
    let config =
        RegistrarConfig::from_args(args(&["--mode", "container"])).expect("container mode parses");
    assert!(config.watches("anything"));
}

#[test]
fn the_watched_entries_filter_applies() {
    let config = RegistrarConfig::from_args(args(&[
        "--mode",
        "container",
        "--entries",
        "agent-rpc,vector-db",
    ]))
    .expect("container mode parses");
    assert!(config.watches("agent-rpc"));
    assert!(config.watches("vector-db"));
    assert!(!config.watches("ripgrep"));
}
