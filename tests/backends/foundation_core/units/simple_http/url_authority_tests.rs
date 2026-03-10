//! Unit tests for `url::authority` moved into the canonical units test tree.
//!
//! These tests are intentionally lightweight and focus on compile-time / trait
//! properties (Send/Sync, Debug/Clone where available) so they are fast and
//! nondestructive when run as part of the unit test suite.
//!
//! They mirror the intent of the original in-crate tests which validated the
//! presence and basic ergonomics of the authority parsing types without
//! performing heavy I/O.

use foundation_core::wire::simple_http::client::*;
use foundation_core::wire::simple_http::url::*;
use foundation_core::wire::simple_http::*;

#[test]
fn test_authority_type_is_send_sync() {
    // Compile-time assertion that the Authority type is Send + Sync.
    // This ensures it can be used across threads in the client code paths.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<foundation_core::wire::simple_http::url::Authority>();
}

#[test]
fn test_authority_parse_domain() {
    let auth = Authority::parse("example.com").unwrap();
    assert!(matches!(auth.host(), Host::RegName(_)));
    assert_eq!(auth.port(), None);
    assert_eq!(auth.userinfo(), None);
}

#[test]
fn test_authority_parse_domain_with_port() {
    let auth = Authority::parse("example.com:8080").unwrap();
    assert_eq!(auth.port(), Some(8080));
}

#[test]
fn test_authority_parse_ipv4() {
    let auth = Authority::parse("192.168.1.1").unwrap();
    assert!(matches!(auth.host(), Host::Ipv4(_)));
    assert_eq!(auth.port(), None);
}

#[test]
fn test_authority_parse_ipv4_with_port() {
    let auth = Authority::parse("192.168.1.1:8080").unwrap();
    assert!(matches!(auth.host(), Host::Ipv4(_)));
    assert_eq!(auth.port(), Some(8080));
}

#[test]
fn test_authority_parse_ipv6() {
    let auth = Authority::parse("[::1]").unwrap();
    assert!(matches!(auth.host(), Host::Ipv6(_)));
    assert_eq!(auth.port(), None);
}

#[test]
fn test_authority_parse_ipv6_with_port() {
    let auth = Authority::parse("[2001:db8::1]:8080").unwrap();
    assert!(matches!(auth.host(), Host::Ipv6(_)));
    assert_eq!(auth.port(), Some(8080));
}

#[test]
fn test_authority_parse_with_userinfo() {
    let auth = Authority::parse("user:pass@example.com:8080").unwrap();
    assert_eq!(auth.userinfo(), Some("user:pass"));
    assert_eq!(auth.port(), Some(8080));
}

#[test]
fn test_authority_parse_invalid_port() {
    assert!(Authority::parse("example.com:99999").is_err());
    assert!(Authority::parse("example.com:abc").is_err());
}

#[test]
fn test_authority_parse_invalid_ipv6() {
    assert!(Authority::parse("[not::valid").is_err()); // Unclosed bracket
    assert!(Authority::parse("[zzz::1]").is_err()); // Invalid IPv6
}

#[test]
fn test_authority_display() {
    let auth = Authority::parse("user@example.com:8080").unwrap();
    assert_eq!(auth.to_string(), "user@example.com:8080");

    let auth = Authority::parse("[::1]:443").unwrap();
    assert_eq!(auth.to_string(), "[::1]:443");
}
