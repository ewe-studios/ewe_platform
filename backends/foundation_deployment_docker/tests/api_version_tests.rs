//! Docker API version negotiation, and what a client reports before it.
//!
//! WHY: A client that asks an older daemon for a newer API version gets
//! `400 Bad Request` on every call, so the version it settles on must be
//! `min(client, daemon)` compared as numbers, and a non-overlapping or
//! malformed answer must be an error rather than a guess.
//!
//! WHAT: [`negotiate_api_version`] on its own, and the URLs a fresh,
//! not-yet-negotiated [`DockerClient`] builds. No network.

#![cfg(feature = "docker")]

use foundation_deployment_docker::client::negotiate_api_version;
use foundation_deployment_docker::{DockerClient, DockerError, DEFAULT_API_VERSION};

#[test]
fn negotiation_lowers_to_an_older_daemon() {
    // GitHub's ubuntu runners: a daemon older than the client.
    let v = negotiate_api_version("1.53", Some("1.48"), Some("1.24")).expect("versions overlap");
    assert_eq!(v, "1.48");
}

#[test]
fn negotiation_keeps_the_client_version_for_a_newer_daemon() {
    let v = negotiate_api_version("1.53", Some("1.56"), Some("1.40")).expect("versions overlap");
    assert_eq!(v, "1.53");
}

#[test]
fn negotiation_keeps_an_equal_version() {
    let v = negotiate_api_version("1.53", Some("1.53"), None).expect("versions overlap");
    assert_eq!(v, "1.53");
}

#[test]
fn negotiation_compares_numerically_not_lexically() {
    // As strings "1.9" > "1.10"; as versions 1.9 is the older one.
    let v = negotiate_api_version("1.10", Some("1.9"), None).expect("versions overlap");
    assert_eq!(v, "1.9");
    let v = negotiate_api_version("1.9", Some("1.10"), None).expect("versions overlap");
    assert_eq!(v, "1.9");
}

#[test]
fn negotiation_fails_when_the_daemon_minimum_is_newer_than_the_client() {
    let err = negotiate_api_version("1.53", Some("1.60"), Some("1.55")).expect_err("no overlap");
    match err {
        DockerError::UnsupportedApiVersion {
            client_max,
            server_min,
            server_max,
        } => {
            assert_eq!(client_max, "1.53");
            assert_eq!(server_min, "1.55");
            assert_eq!(server_max, "1.60");
        }
        other => panic!("expected UnsupportedApiVersion, got {other:?}"),
    }
}

#[test]
fn negotiation_fails_without_a_daemon_api_version() {
    let err = negotiate_api_version("1.53", None, Some("1.24")).expect_err("no ApiVersion");
    assert!(matches!(err, DockerError::JsonParse(_)), "got {err:?}");
}

#[test]
fn negotiation_rejects_malformed_versions() {
    for bad in ["", "1", "one.two", "1.x", "1.2.3"] {
        let err = negotiate_api_version("1.53", Some(bad), None)
            .expect_err("a malformed ApiVersion must not be guessed at");
        assert!(
            matches!(err, DockerError::JsonParse(_)),
            "{bad:?}: got {err:?}"
        );
    }
}

#[test]
fn a_fresh_client_reports_the_default_version_until_negotiated() {
    let c = DockerClient::connect_unix("/nonexistent/docker.sock");
    assert_eq!(c.api_version(), DEFAULT_API_VERSION);
    assert_eq!(
        c.base_url(),
        format!("http://localhost/v{DEFAULT_API_VERSION}")
    );
    assert_eq!(c.unversioned_base_url(), "http://localhost");
}
