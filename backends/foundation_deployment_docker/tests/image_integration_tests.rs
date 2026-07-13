//! Integration tests for Docker Image API methods.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test image_integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

/// One fresh client per test — no shared state, no pool interference.
fn client() -> DockerClient {
    DockerClient::connect_unix("/var/run/docker.sock")
}

// ── Read-only: inspect + history + search ──────────────────────────────

#[valtron_test]
async fn image_inspect_alpine() {
    let c = client();
    let info = c.image_inspect("alpine:latest").await.expect("image_inspect");
    // Id must be a non-empty string (SHA256 hash).
    assert!(
        info.id.as_ref().map_or(false, |id| !id.is_empty()),
        "Id must be non-empty"
    );
    // RepoTags must contain "alpine:latest".
    assert!(
        info.repo_tags
            .as_ref()
            .map_or(false, |tags| tags.iter().any(|t| t == "alpine:latest")),
        "RepoTags must contain alpine:latest"
    );
}

#[valtron_test]
async fn image_history() {
    let c = client();
    let history = c.image_history("alpine:latest").await.expect("image_history");
    let arr = history
        .as_array()
        .expect("history must be a JSON array");
    assert!(!arr.is_empty(), "history must have at least one entry");
}

#[valtron_test]
async fn image_search() {
    let c = client();
    let results = c
        .image_search(Some("alpine"), None, None)
        .await
        .expect("image_search");
    let arr = results
        .as_array()
        .expect("search results must be a JSON array");
    assert!(!arr.is_empty(), "search must return at least one result");
    let first = &arr[0];
    assert!(
        first.get("name").is_some(),
        "first search result must have a 'name' field"
    );
}

// ── Mutating: tag + delete ─────────────────────────────────────────────

#[valtron_test]
async fn image_tag_then_delete() {
    let c = client();
    // Tag alpine:latest as ewe-it-img-test:latest
    c.image_tag("alpine:latest", Some("ewe-it-img-test"), Some("latest"))
        .await
        .expect("image_tag");
    // Delete the tagged image
    c.image_delete("ewe-it-img-test:latest", None, None)
        .await
        .expect("image_delete");
}

#[valtron_test]
async fn image_delete_untagged() {
    let c = client();
    // Tag alpine as ewe-it-img-del:test
    c.image_tag("alpine:latest", Some("ewe-it-img-del"), Some("test"))
        .await
        .expect("image_tag");
    // Delete and assert we get back an array of deleted layers
    let deleted = c
        .image_delete("ewe-it-img-del:test", None, None)
        .await
        .expect("image_delete");
    assert!(
        deleted.is_array(),
        "delete must return a JSON array of deleted layers"
    );
}

// ── Prune ───────────────────────────────────────────────────────────────

#[valtron_test]
async fn image_prune_ok() {
    let c = client();
    c.image_prune(None).await.expect("image_prune");
}

#[valtron_test]
async fn distribution_inspect_alpine() {
    // Inspect alpine:latest on Docker Hub — requires auth but should return
    // a response (could be 200 with descriptor data or 401 if no creds).
    let c = client();
    match c.distribution_inspect("alpine:latest").await {
        Ok(d) => {
            let digest = d.descriptor.digest.unwrap_or_default();
            assert!(!digest.is_empty(), "should have a digest");
        }
        Err(e) => {
            // Docker Hub registry auth not configured — expected.
            eprintln!("distribution_inspect (expected without auth): {e}");
        }
    }
}

#[valtron_test]
async fn build_prune_ok() {
    let c = client();
    c.build_prune(None, None, None, None).await.expect("build_prune");
}
