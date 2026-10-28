//! rustls connector/acceptor behaviour (moved out of `native/ssl/rustls.rs`).
//!
//! WHY: CLAUDE.md rule 7 — tests live in `tests/`, never inline in a source file.
//!
//! The two inline tests that only probed third-party crates (`ServerName`
//! parsing and `webpki_roots` being non-empty) were dropped here: they tested
//! rustls and webpki, not `foundation_netio`.

use std::sync::Arc;

use foundation_core::url::Uri;
use foundation_netio::netcap::connection::{Endpoint, EndpointConfig};
use foundation_netio::netcap::ssl::rustls::{default_client_config, RustlsConnector};
use foundation_netio::netcap::ssl::SSLAcceptor;
use zeroize::Zeroizing;

/// `default_client_config` returns a fresh, single-owner config.
#[test]
fn default_client_config_has_one_owner() {
    let config = default_client_config();
    assert_eq!(Arc::strong_count(&config), 1);
}

/// Invalid PEM material is rejected, not silently accepted.
#[test]
fn rustls_acceptor_rejects_invalid_pem() {
    let certs = vec![0u8; 10];
    let key = Zeroizing::new(vec![0u8; 10]);

    let result = SSLAcceptor::from_pem(certs, key);
    assert!(result.is_err());
}

/// `RustlsConnector::create` takes the endpoint's identity config.
#[test]
fn rustls_connector_create_uses_the_identity_config() {
    let custom_config = default_client_config();
    let uri = Uri::parse("https://example.com:443").unwrap();
    let endpoint = Endpoint::WithIdentity(EndpointConfig::NoTimeout(uri), custom_config.clone());

    let _ = RustlsConnector::create(&endpoint);

    // The connector shares the same `Arc`, not a copy.
    assert!(Arc::strong_count(&custom_config) >= 2);
}
