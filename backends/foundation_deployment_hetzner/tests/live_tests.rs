//! Against Hetzner's real API (spec-56 F01, "live path").
//!
//! **Why these exist:** every other test in this crate answers what we told a mock
//! to answer. That proves our logic, not that Hetzner agrees with our
//! understanding of it. Three of this feature's bugs were found only by moving off
//! fixtures onto a real transport, and the same reasoning applies one level up.
//!
//! **They skip unless a token is exported**, so the suite stays green for anyone
//! without a Hetzner account:
//!
//! ```text
//! HCLOUD_TOKEN=… cargo test --profile uat -p foundation_deployment_hetzner --test live_tests -- --ignored --nocapture
//! ```
//!
//! **`#[ignore]` is deliberate.** These talk to a real account over the internet
//! and count against a 3600/hour rate limit. A test that bills money or leaks
//! state must be opted into, never swept up by a bare `cargo test`.
//!
//! Everything here is **read-only**. Creating a server bills real money, so that
//! is a separate, explicitly-named test below.

use foundation_core::valtron::valtron_test;
use foundation_deployment_hetzner::{HetznerClient, HetznerError};

/// A client from the environment, or `None` when nobody exported a token.
fn live_client() -> Option<HetznerClient> {
    match HetznerClient::from_env() {
        Ok(client) => Some(client),
        Err(HetznerError::NoCredentials { looked_for }) => {
            eprintln!("SKIP: no Hetzner token — set one of {}", looked_for.join(" or "));
            None
        }
        Err(e) => panic!("unexpected: {e}"),
    }
}

// ── read-only ────────────────────────────────────────────────────────────────

#[valtron_test]
#[ignore = "talks to Hetzner's real API; needs HCLOUD_TOKEN"]
async fn list_servers_against_the_real_api() {
    let Some(client) = live_client() else { return };

    // The whole generated + hand-written stack, end to end: bearer auth, the real
    // TLS transport, Hetzner's actual JSON, our types.
    let servers = client.list_servers(None).await.expect("Hetzner answers");
    eprintln!("  live: {} server(s) in the project", servers.len());
    for s in &servers {
        eprintln!("    {} {:?} {:?}", s.id, s.name, s.public_ipv4);
    }
}

#[valtron_test]
#[ignore = "talks to Hetzner's real API; needs HCLOUD_TOKEN"]
async fn list_ssh_keys_against_the_real_api() {
    let Some(client) = live_client() else { return };

    let keys = client.list_ssh_keys().await.expect("Hetzner answers");
    eprintln!("  live: {} ssh key(s)", keys.len());
    for k in &keys {
        // The fingerprint is Hetzner's, which is the point of ensure_ssh_key not
        // computing its own.
        eprintln!("    {} {:?} {}", k.id, k.name, k.fingerprint);
    }
}

#[valtron_test]
#[ignore = "talks to Hetzner's real API; needs HCLOUD_TOKEN"]
async fn a_name_that_matches_nothing_returns_none_not_an_error() {
    let Some(client) = live_client() else { return };

    let found = client
        .find_server_by_name("ewe-definitely-does-not-exist-9c3f1a")
        .await
        .expect("a lookup that finds nothing is still a successful lookup");
    assert_eq!(found, None);
}

#[valtron_test]
#[ignore = "talks to Hetzner's real API; needs HCLOUD_TOKEN"]
async fn a_missing_server_id_is_none_rather_than_an_error() {
    let Some(client) = live_client() else { return };

    // Proves the 404-is-an-answer mapping against Hetzner's real 404, not our
    // mock's idea of one. `destroy` depends on this.
    let found = client.get_server(1).await.expect("404 is an answer, not a failure");
    assert_eq!(found, None, "server id 1 should not exist in this project");
}

#[valtron_test]
#[ignore = "talks to Hetzner's real API; needs HCLOUD_TOKEN"]
async fn a_bad_token_is_unauthorized_against_the_real_api() {
    // The mock returns whatever 401 body we wrote. This asserts Hetzner's real
    // rejection maps to the variant a caller branches on — the one case where
    // being wrong means telling someone their network is down when their token is
    // simply wrong.
    if live_client().is_none() {
        return; // no account to point at
    }
    let client = HetznerClient::new("definitely-not-a-real-token");
    let err = client.list_servers(None).await.expect_err("Hetzner rejects it");
    assert_eq!(err, HetznerError::Unauthorized, "got {err}");
    eprintln!("  live: a bad token maps to {err}");
}
