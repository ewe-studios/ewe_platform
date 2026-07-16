//! Unit tests for Host type.

use foundation_sshkit::Host;
use std::path::PathBuf;

/// The user `Host::parse` fills in when the spec has no `user@` — the current
/// login user (`$USER`/`$LOGNAME`), falling back to `root`. Mirrors the crate's
/// private `default_user()` so the assertion holds in any environment (a bare
/// container has no `$USER` → `root`; a dev machine has one → that user).
fn expected_default_user() -> String {
    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("LOGNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "root".to_string())
}

#[test]
fn test_parse_user_at_host() {
    let h = Host::parse("deploy@example.com");
    assert_eq!(h.user, "deploy");
    assert_eq!(h.hostname, "example.com");
    assert_eq!(h.port, 22);
}

#[test]
fn test_parse_user_at_host_port() {
    let h = Host::parse("root@192.168.1.1:2222");
    assert_eq!(h.user, "root");
    assert_eq!(h.hostname, "192.168.1.1");
    assert_eq!(h.port, 2222);
}

#[test]
fn test_parse_host_only_defaults_to_current_user() {
    let h = Host::parse("server.local");
    assert_eq!(h.user, expected_default_user());
    assert_eq!(h.hostname, "server.local");
    assert_eq!(h.port, 22);
}

#[test]
fn test_parse_host_with_port_only() {
    let h = Host::parse("example.com:2222");
    assert_eq!(h.user, expected_default_user());
    assert_eq!(h.hostname, "example.com");
    assert_eq!(h.port, 2222);
}

#[test]
fn test_with_key() {
    let h = Host::parse("host").with_key("/home/user/.ssh/id_ed25519");
    assert_eq!(h.key_paths.len(), 1);
    assert_eq!(h.key_paths[0], PathBuf::from("/home/user/.ssh/id_ed25519"));
}

#[test]
fn test_with_password() {
    let h = Host::parse("host").with_password("secret");
    assert_eq!(h.password.unwrap(), "secret");
}

#[test]
fn test_via_proxy() {
    let proxy = Host::parse("bastion.example.com:22");
    let h = Host::parse("internal:22").via(proxy);
    assert!(h.proxy.is_some());
    assert_eq!(h.proxy.as_ref().unwrap().hostname, "bastion.example.com");
}

#[test]
fn test_from_str_trait() {
    let h: Host = "user@host:2222".into();
    assert_eq!(h.user, "user");
    assert_eq!(h.port, 2222);
}
