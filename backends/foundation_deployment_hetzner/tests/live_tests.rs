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
use foundation_deployment_hetzner::{
    HetznerClient, HetznerError, HetznerServer, ServerDeployOutput,
};
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

// ── the one test that bills ──────────────────────────────────────────────────

/// Cheapest non-deprecated type, checked against the live API 2026-07-17:
/// €0.0104/hr. This test exists for ~30s.
const LIVE_TYPE: &str = "cx23";
const LIVE_IMAGE: &str = "ubuntu-24.04";
const LIVE_LOCATION: &str = "fsn1";

/// Every server this suite creates starts with this, so a human scanning the
/// Hetzner console can tell a leaked test box from something real.
const LIVE_PREFIX: &str = "ewe-live-test-";

/// Remove anything an earlier run left behind.
///
/// Not politeness — a stranded VPS **bills forever**. If a run was killed between
/// create and destroy (`^C`, a panic, a dropped connection), its server is still
/// running and still charging. Sweeping first bounds the damage by the gap between
/// two runs rather than leaving it open-ended.
async fn sweep_leftovers(client: &HetznerClient) {
    let servers = client.list_servers(None).await.unwrap_or_default();
    for server in servers.iter().filter(|s| s.name.starts_with(LIVE_PREFIX)) {
        eprintln!("  sweeping a leftover: {} {:?} — it has been billing", server.id, server.name);
        if let Err(e) = client.delete_server(server.id).await {
            eprintln!("  !! could not sweep {}: {e}", server.id);
        }
    }
}

/// The whole lifecycle, on **one** server.
///
/// **Deliberately one test and one machine.** Every behaviour below used to have
/// its own test creating its own box; three creates and three deletes per run is
/// indistinguishable from abuse to a provider, and it is someone's real account.
/// The orphan case does not need a second server either — deleting the state file
/// reproduces exactly what an unreadable create leaves behind.
///
/// **This bills.** ~€0.0104/hr for ~30s. The structure holds no `assert!`/`unwrap`
/// between create and destroy: every check collects into a `Result` inspected
/// *after* teardown, because a panic there unwinds straight past the cleanup.
///
/// What it proves, in order:
///
/// 1. `deploy` creates the server and records state that **fully articulates** it;
/// 2. a second `deploy` returns the **same id** — no second bill;
/// 3. a declaration that has changed underneath the record is **refused**, not
///    silently served the old server;
/// 4. with the record gone, `destroy` still finds the machine **by its label** and
///    removes it — the leak that shipped on 2026-07-17.
#[valtron_test]
#[ignore = "CREATES A REAL SERVER AND BILLS FOR IT; needs HCLOUD_TOKEN"]
async fn the_server_lifecycle_against_the_real_api() {
    let Some(client) = live_client() else { return };
    sweep_leftovers(&client).await;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = format!("{LIVE_PREFIX}{}-{stamp}", std::process::id());
    let renamed = format!("{LIVE_PREFIX}renamed-{}-{stamp}", std::process::id());

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

    eprintln!("  creating {name} ({LIVE_TYPE}/{LIVE_LOCATION}) — this bills");

    // ── nothing may panic between here and teardown ─────────────────────────
    let outcome: Result<(), String> = async {
        // 1. Deploy, and the state it writes.
        let first = deployable
            .deploy(0, provider.clone())
            .await
            .map_err(|e| format!("deploy: {e}"))?;
        eprintln!("  live: created {} at {}", first.id, first.public_ip);

        if first.public_ip.is_empty() {
            return Err("a running server reported no public IPv4".into());
        }

        // The state must articulate the instance well enough to zero in on it —
        // an id alone is a number Hetzner assigned that proves nothing.
        let recorded: ServerDeployOutput = deployable
            .store(&provider)
            .get_typed("0")
            .map_err(|e| format!("read state: {e}"))?
            .ok_or("deploy recorded nothing")?;

        if recorded.provider != "hetzner" {
            return Err(format!("state does not say whose it is: {recorded:?}"));
        }
        if recorded.id != first.id || recorded.name != name {
            return Err(format!("state does not match what was built: {recorded:?}"));
        }
        match &recorded.identity {
            Some(identity) => {
                // The label is what lets us find this machine again without the id.
                let (key, value) = identity.split_once('=').ok_or("identity is not key=value")?;
                let by_label = client
                    .find_servers_by_label(key, value)
                    .await
                    .map_err(|e| format!("enumerate by label: {e}"))?;
                if !by_label.iter().any(|s| s.id == first.id) {
                    return Err(format!(
                        "the recorded identity {identity:?} does not find the server at Hetzner — \
                         the state cannot zero in on its own instance: {by_label:?}"
                    ));
                }
                eprintln!("  live: state's identity {identity:?} enumerates the server from Hetzner");
            }
            None => return Err("state records no identity — an id alone is not findable".into()),
        }
        match &recorded.declared {
            Some(declared) if declared.name == name && declared.server_type == LIVE_TYPE => {}
            other => return Err(format!("state does not record what was asked for: {other:?}")),
        }

        // 2. A second deploy must not bill again.
        let second = deployable
            .deploy(0, provider.clone())
            .await
            .map_err(|e| format!("second deploy: {e}"))?;
        if second.id != first.id {
            return Err(format!(
                "CREATE-OR-FIND FAILED: a second deploy billed server {} (first was {})",
                second.id, first.id
            ));
        }
        eprintln!("  live: a second deploy returned {} — no second bill", second.id);

        // 3. The code changes under the record: refuse, do not serve the old box.
        let changed = HetznerServer::new(client.clone(), &renamed)
            .server_type(LIVE_TYPE)
            .image(LIVE_IMAGE)
            .location(LIVE_LOCATION);
        match changed.deploy(0, provider.clone()).await {
            Ok(out) => {
                return Err(format!(
                    "a renamed declaration was served server {} ({:?}) from stale state instead \
                     of failing",
                    out.id, out.name
                ))
            }
            Err(HetznerError::Api { ref code, ref message, .. })
                if code == "state_declaration_mismatch" =>
            {
                if !message.contains("Destroy instance 0") {
                    return Err(format!("the refusal must say what to do: {message}"));
                }
                eprintln!("  live: a changed declaration was refused rather than served the old server");
            }
            Err(e) => return Err(format!("wrong error for stale state: {e}")),
        }
        let built = client
            .list_servers(Some(&renamed))
            .await
            .map_err(|e| format!("list: {e}"))?;
        if !built.is_empty() {
            return Err(format!("refusing still billed a server: {built:?}"));
        }

        // 4. Lose the record. This is exactly what a create whose response we
        //    could not read leaves behind: Hetzner has the machine, we have
        //    nothing. `destroy` must still find it — by the label the CREATE
        //    REQUEST carried, which is on the server regardless of what came back.
        deployable
            .store(&provider)
            .remove("0")
            .map_err(|e| format!("drop state: {e}"))?;
        eprintln!("  live: dropped the state record — Hetzner has {} and we have nothing", first.id);

        Ok(())
    }
    .await;

    // ── teardown, unconditionally ───────────────────────────────────────────
    let destroyed = deployable.destroy(0, provider.clone()).await;
    match &destroyed {
        Ok(()) => eprintln!("  live: destroy found the unrecorded server and removed it"),
        Err(e) => eprintln!("  !! destroy failed for {name}: {e}"),
    }

    // Ask Hetzner, not ourselves.
    let mut leaked = Vec::new();
    for n in [&name, &renamed] {
        for server in client.list_servers(Some(n)).await.unwrap_or_default() {
            eprintln!("  !! {} ({}) survived — deleting directly", server.id, server.name);
            leaked.push(server.id);
            let _ = client.delete_server(server.id).await;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    outcome.expect("the lifecycle");
    destroyed.expect("destroy must handle a server it has no record of");
    assert!(leaked.is_empty(), "servers survived teardown: {leaked:?}");
}
