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
use foundation_db::core::state::traits::StateStore;
use foundation_db::core::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::traits::Deployable;
use foundation_deployment_hetzner::{HetznerClient, HetznerError, HetznerServer};
use foundation_netio::http::NativeHttpClient;

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

// ── the round trip that bills ────────────────────────────────────────────────

/// Cheapest non-deprecated type, checked against the live API 2026-07-17:
/// €0.0104/hr. A run costs well under a cent.
const LIVE_TYPE: &str = "cx23";
const LIVE_IMAGE: &str = "ubuntu-24.04";
const LIVE_LOCATION: &str = "fsn1";

/// Every server this suite creates starts with this.
///
/// Two jobs: a human scanning the Hetzner console can tell a leaked test box from
/// something real, and [`sweep_leftovers`] can find one a previous run stranded.
const LIVE_PREFIX: &str = "ewe-live-test-";

/// Remove anything an earlier run left behind.
///
/// Not politeness — a stranded VPS **bills forever**. If a previous run was killed
/// between create and destroy (^C, a panic, a dropped connection), its server is
/// still running and still charging. Sweeping first means the damage is bounded by
/// the gap between two runs rather than unbounded.
async fn sweep_leftovers(client: &HetznerClient) {
    let servers = client.list_servers(None).await.unwrap_or_default();
    for server in servers.iter().filter(|s| s.name.starts_with(LIVE_PREFIX)) {
        eprintln!(
            "  sweeping a leftover from a previous run: {} {:?} — it has been billing",
            server.id, server.name
        );
        if let Err(e) = client.delete_server(server.id).await {
            eprintln!("  !! could not sweep {}: {e}", server.id);
        }
    }
}

/// Create a real server, prove the create-or-find rule on it, and destroy it.
///
/// **This bills.** ~€0.0104/hr for the couple of minutes it exists, so a fraction
/// of a cent — but it is real money and a real machine, which is why it is
/// `#[ignore]`d and named for what it does.
///
/// **The structure is the point.** Between "create returned an id" and "destroy
/// succeeded" there is not one `assert!`, `unwrap`, or `expect` — every check
/// collects into a `Result` that is inspected *after* teardown. A panic in that
/// window would unwind past the destroy and strand a billing instance, and
/// spec-53 already learned this the hard way with containers (where the cost was
/// only a held port). Here the cost is money.
#[valtron_test]
#[ignore = "CREATES A REAL SERVER AND BILLS FOR IT; needs HCLOUD_TOKEN"]
async fn deploy_then_destroy_a_real_server() {
    let Some(client) = live_client() else { return };

    sweep_leftovers(&client).await;

    let name = format!(
        "{LIVE_PREFIX}{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );

    let dir = std::env::temp_dir().join(format!("ewe-hz-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let store = FileStateStore::new(&dir, "ewe-live", "test");
    store.init().expect("init state store");
    let provider = ProviderClient::new("ewe-live", "test", store, NativeHttpClient::default());

    let deployable = HetznerServer::new(client.clone(), &name)
        .server_type(LIVE_TYPE)
        .image(LIVE_IMAGE)
        .location(LIVE_LOCATION);

    eprintln!("  creating {name} ({LIVE_TYPE}/{LIVE_IMAGE}/{LIVE_LOCATION}) — this bills");

    // ── everything from here to teardown must not panic ─────────────────────
    let outcome: Result<(), String> = async {
        let first = deployable
            .deploy(0, provider.clone())
            .await
            .map_err(|e| format!("deploy: {e}"))?;

        eprintln!("  live: created {} at {}", first.id, first.public_ip);

        if first.public_ip.is_empty() {
            return Err("a running server reported no public IPv4".to_string());
        }
        if first.name != name {
            return Err(format!("name came back as {:?}, expected {name:?}", first.name));
        }

        // The rule that actually costs money if it is wrong (decision 03 §3): a
        // second deploy must find the box, not bill another one.
        let second = deployable
            .deploy(0, provider.clone())
            .await
            .map_err(|e| format!("second deploy: {e}"))?;
        if second.id != first.id {
            return Err(format!(
                "CREATE-OR-FIND FAILED: second deploy billed a new server {} (first was {})",
                second.id, first.id
            ));
        }
        eprintln!("  live: a second deploy returned {} — no second bill", second.id);

        // And Hetzner agrees there is exactly one.
        let matching = client
            .list_servers(Some(&name))
            .await
            .map_err(|e| format!("list: {e}"))?;
        if matching.len() != 1 {
            return Err(format!("expected exactly 1 server named {name}, Hetzner has {}", matching.len()));
        }

        Ok(())
    }
    .await;

    // ── teardown, unconditionally ───────────────────────────────────────────
    //
    // `destroy` first, because that is the code under test.
    let destroyed = deployable.destroy(0, provider.clone()).await;
    match &destroyed {
        Ok(()) => eprintln!("  live: destroy reported success"),
        Err(e) => eprintln!("  !! destroy failed for {name}: {e}"),
    }

    // Then ask HETZNER, and delete anything still standing.
    //
    // This second sweep is not paranoia, it is the lesson from the first run of
    // this test. `destroy` reads the state store to find what to delete — so if
    // the server was created but we never recorded it, destroy finds nothing,
    // returns Ok("nothing to destroy"), AND LEAVES IT BILLING. That is exactly
    // what happened: Hetzner built the machine, our client failed to parse the
    // 201 that carried its id, and `deploy`'s own unwind could not fire because
    // it never learned the id either. The test printed "destroyed". The server
    // ran on.
    //
    // Teardown for a billing resource cannot be keyed on state we might have
    // failed to write. The vendor is the authority on what exists.
    let mut leaked: Vec<i64> = Vec::new();
    for server in client.list_servers(Some(&name)).await.unwrap_or_default() {
        eprintln!(
            "  !! {} ({}) survived destroy — deleting it directly",
            server.id, server.name
        );
        leaked.push(server.id);
        if let Err(e) = client.delete_server(server.id).await {
            eprintln!("  !! COULD NOT DELETE {} — IT IS STILL BILLING: {e}", server.id);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    // Now it is safe to fail.
    outcome.expect("the round trip");
    destroyed.expect("destroy");
    assert!(
        leaked.is_empty(),
        "destroy said Ok but Hetzner still had {leaked:?} — they have been deleted, but destroy is lying"
    );

    // The state store must be clear, or the next deploy would chase a dead id.
    let recorded: Option<foundation_deployment_hetzner::ServerDeployOutput> =
        deployable.store(&provider).get_typed("0").expect("read state");
    assert!(recorded.is_none(), "destroy left state behind: {recorded:?}");
}

/// `destroy` must clean up a server we never recorded — against the real API.
///
/// **This is the incident, reproduced for real.** Hetzner builds the machine, our
/// client fails to read the 201, `deploy` returns an error having stored nothing —
/// and `destroy` used to read the empty store and report success while the server
/// billed on.
///
/// It is live rather than mocked deliberately. The mock version of this test
/// passed for days *while the bug was shipping*, because a mock answers what you
/// told it to; the one that mattered even asserted the bug outright ("destroy does
/// not call Hetzner to find that out"). Hetzner can be put in this exact state for
/// a fraction of a cent, and then the assertion is about the world rather than
/// about my beliefs.
#[valtron_test]
#[ignore = "CREATES A REAL SERVER AND BILLS FOR IT; needs HCLOUD_TOKEN"]
async fn destroy_cleans_up_a_server_that_was_never_recorded() {
    let Some(client) = live_client() else { return };
    sweep_leftovers(&client).await;

    let name = format!(
        "{LIVE_PREFIX}orphan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );

    let dir = std::env::temp_dir().join(format!("ewe-hz-orphan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let store = FileStateStore::new(&dir, "ewe-live", "test");
    store.init().expect("init state store");
    let provider = ProviderClient::new("ewe-live", "test", store, NativeHttpClient::default());

    // Create it BEHIND the deployable's back — exactly the state a create whose
    // response we could not read leaves behind: the vendor has it, we recorded
    // nothing.
    eprintln!("  creating an unrecorded {name} — this bills");
    let created = client
        .create_server(&foundation_deployment_hetzner::CreateServerRequest {
            name: name.clone(),
            server_type: LIVE_TYPE.to_string(),
            image: LIVE_IMAGE.to_string(),
            location: Some(LIVE_LOCATION.to_string()),
            ssh_keys: Vec::new(),
            user_data: None,
        })
        .await;

    let created_id = match created {
        Ok(server) => {
            eprintln!("  live: Hetzner has {} and our store has nothing", server.id);
            Some(server.id)
        }
        Err(e) => {
            eprintln!("  !! create failed: {e}");
            None
        }
    };

    let deployable = HetznerServer::new(client.clone(), &name);

    // The store is empty. destroy has only the name — which is all it needs.
    let destroyed = deployable.destroy(0, provider.clone()).await;

    let leftover = client.list_servers(Some(&name)).await.unwrap_or_default();
    for server in &leftover {
        eprintln!("  !! {} survived — deleting directly", server.id);
        let _ = client.delete_server(server.id).await;
    }
    let _ = std::fs::remove_dir_all(&dir);

    assert!(created_id.is_some(), "the test needs a server to orphan");
    destroyed.expect("destroy must handle a server it never recorded");
    assert!(
        leftover.is_empty(),
        "destroy reported success but Hetzner still had {leftover:?} — THIS IS THE LEAK"
    );
    eprintln!("  live: destroy found the unrecorded server by name and removed it");
}
