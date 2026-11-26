//! The bootstrap payload rules of a browser (spec 83 feature 05, section 6.4).

use foundation_agent_rpc::core::bootstrap::TlsTrust;
use foundation_agent_rpc::core::browser::{check_browser_support, SIGNED_SECRET_MODE};

fn modes(modes: &[&str]) -> Vec<String> {
    modes.iter().map(|mode| (*mode).to_string()).collect()
}

#[test]
fn an_enrolled_or_bearer_client_on_public_roots_is_accepted() {
    check_browser_support(TlsTrust::SystemRoots, &modes(&["mtls", SIGNED_SECRET_MODE]))
        .expect("public roots and a signed secret are usable from a page");
}

#[test]
fn a_loopback_plaintext_host_is_accepted_for_development() {
    check_browser_support(TlsTrust::Plaintext, &modes(&[SIGNED_SECRET_MODE]))
        .expect("decode already limited plaintext to a loopback host");
}

#[test]
fn a_pinned_certificate_is_refused() {
    let error = check_browser_support(
        TlsTrust::PinnedSha256([7; 32]),
        &modes(&[SIGNED_SECRET_MODE]),
    )
    .expect_err("a browser cannot pin a server certificate");
    assert!(error.to_string().contains("pin"));
}

#[test]
fn a_host_without_access_secrets_is_refused() {
    let error = check_browser_support(TlsTrust::SystemRoots, &modes(&["mtls"]))
        .expect_err("mutual TLS alone is not reachable from a page");
    assert!(error.to_string().contains("access secrets"));
}
