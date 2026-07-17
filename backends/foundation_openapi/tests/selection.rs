//! Tests for [`foundation_openapi::Selection`] — the allowlist that decides which
//! slice of a vendor spec gets generated (spec-56 feature 00).
//!
//! WHY: vendor specs are enormous and provider crates use a sliver — Linode's is
//! 9.3 MB across 334 paths for the ~6 endpoints a VPS crate calls. Cargo features
//! gate compilation, not generation, so without selection the dead code is still
//! emitted and committed.
//!
//! The load-bearing assertions here are the *negative* ones: it is easy to write a
//! selector that keeps everything and still passes a "does it keep what I asked
//! for" test.

use foundation_openapi::{glob_match, OpenApiSpec, Selection};

/// Two paths, three operations, distinct tags/ids — enough to tell selection
/// mechanisms apart.
fn spec() -> OpenApiSpec {
    serde_json::from_str(
        r#"{
        "openapi": "3.0.1",
        "info": { "title": "t", "version": "1.0.0" },
        "paths": {
            "/servers": {
                "get":  { "operationId": "list-servers",  "tags": ["Servers"], "responses": {} },
                "post": { "operationId": "post-server",   "tags": ["Servers"], "responses": {} }
            },
            "/servers/{id}": {
                "delete": { "operationId": "delete-server", "tags": ["Servers"], "responses": {} }
            },
            "/servers/{id}/actions": {
                "post": { "operationId": "server-action", "tags": ["Actions"], "responses": {} }
            },
            "/ssh_keys": {
                "get": { "operationId": "list-keys", "tags": ["SSHKeys"], "responses": {} }
            },
            "/kubernetes/clusters": {
                "get": { "operationId": "list-clusters", "tags": ["Kubernetes"], "responses": {} }
            }
        }
    }"#,
    )
    .expect("fixture spec parses")
}

// ── the default: nothing asked for means everything ──────────────────────────

#[test]
fn an_empty_selection_keeps_the_whole_spec() {
    // Existing providers (cloudflare, stripe, …) generate unchanged.
    let sel = Selection::all();
    assert!(sel.is_all());
    assert_eq!(sel.selected_paths(&spec()).len(), 5, "every path is kept");
}

// ── paths ────────────────────────────────────────────────────────────────────

#[test]
fn an_exact_path_keeps_only_that_path() {
    let kept = Selection::all().paths(["/servers"]).selected_paths(&spec());

    assert!(kept.contains("/servers"));
    // The negative half is the point: everything else is gone.
    assert!(!kept.contains("/servers/{id}"), "a sibling path is not implied");
    assert!(!kept.contains("/kubernetes/clusters"));
    assert_eq!(kept.len(), 1);
}

#[test]
fn a_single_star_matches_one_segment_not_a_subtree() {
    let kept = Selection::all()
        .paths(["/servers/*"])
        .selected_paths(&spec());

    assert!(kept.contains("/servers/{id}"), "one segment matches");
    assert!(
        !kept.contains("/servers/{id}/actions"),
        "`*` must not swallow a deeper segment — otherwise asking for the \
         collection alone would be impossible"
    );
    assert!(!kept.contains("/servers"), "`/servers/*` needs a segment after it");
}

#[test]
fn a_double_star_matches_a_subtree() {
    let kept = Selection::all()
        .paths(["/servers/**"])
        .selected_paths(&spec());

    assert!(kept.contains("/servers/{id}"));
    assert!(kept.contains("/servers/{id}/actions"), "`**` spans segments");
    assert!(!kept.contains("/ssh_keys"));
}

#[test]
fn several_patterns_union() {
    let kept = Selection::all()
        .paths(["/servers", "/ssh_keys"])
        .selected_paths(&spec());

    assert_eq!(kept.len(), 2);
    assert!(kept.contains("/servers") && kept.contains("/ssh_keys"));
}

// ── tags and operation ids ───────────────────────────────────────────────────

#[test]
fn a_tag_keeps_every_path_carrying_it() {
    let kept = Selection::all().tags(["Servers"]).selected_paths(&spec());

    assert!(kept.contains("/servers"));
    assert!(kept.contains("/servers/{id}"));
    assert!(
        !kept.contains("/servers/{id}/actions"),
        "that operation is tagged Actions, not Servers"
    );
    assert!(!kept.contains("/kubernetes/clusters"));
}

#[test]
fn an_operation_id_keeps_just_its_path() {
    let kept = Selection::all()
        .operations(["delete-server"])
        .selected_paths(&spec());

    assert_eq!(kept.len(), 1);
    assert!(kept.contains("/servers/{id}"));
}

#[test]
fn paths_tags_and_ids_are_ored_not_anded() {
    // They are three ways of naming the same thing, not a filter chain: asking
    // for a path AND an unrelated tag should yield both, not their intersection.
    let kept = Selection::all()
        .paths(["/ssh_keys"])
        .tags(["Kubernetes"])
        .selected_paths(&spec());

    assert!(kept.contains("/ssh_keys"), "matched by path");
    assert!(kept.contains("/kubernetes/clusters"), "matched by tag");
    assert_eq!(kept.len(), 2);
}

#[test]
fn a_path_survives_when_only_some_of_its_operations_match() {
    // /servers has GET (list-servers) and POST (post-server); ask for POST only.
    let sel = Selection::all().operations(["post-server"]);
    let spec = spec();
    let kept = sel.selected_paths(&spec);
    assert!(kept.contains("/servers"), "the path is kept for its POST");

    // …and the unselected GET on that same path is still excluded.
    let item = &spec.paths["/servers"];
    assert!(sel.includes("/servers", item.post.as_ref().unwrap()));
    assert!(
        !sel.includes("/servers", item.get.as_ref().unwrap()),
        "keeping the path must not smuggle its other operations in"
    );
}

// ── glob semantics, directly ─────────────────────────────────────────────────

#[test]
fn glob_semantics() {
    assert!(glob_match("/servers", "/servers"));
    assert!(glob_match("/servers/{id}", "/servers/{id}"), "braces are literal");
    assert!(glob_match("/servers/*", "/servers/{id}"));
    assert!(!glob_match("/servers/*", "/servers/{id}/actions"));
    assert!(glob_match("/servers/**", "/servers/{id}/actions"));
    assert!(glob_match("/servers/**", "/servers/{id}"));
    assert!(glob_match("/**", "/anything/at/all"));
    assert!(!glob_match("/servers", "/servers/{id}"));
    assert!(!glob_match("/servers", "/ssh_keys"));
    // A prefix is not a match: substring matching would make `/server` catch
    // `/servers`, which is the classic footgun here.
    assert!(!glob_match("/server", "/servers"));
}

// ── the real thing ───────────────────────────────────────────────────────────

/// Linode's real spec: 9.3 MB, 334 paths, and we want six endpoints.
///
/// Skips when the owner-supplied clone is not present, so this is not a hard
/// dependency of the test suite.
#[test]
fn linode_real_spec_narrows_334_paths_to_the_handful_we_use() {
    const SPEC: &str = "/home/darkvoid/Boxxed/@formulas/src.rust/src.cloud_providers/\
                        src.linode/linode-api-openapi/openapi.json";
    let Ok(raw) = std::fs::read_to_string(SPEC) else {
        eprintln!("SKIP: Linode spec not present at {SPEC}");
        return;
    };

    let spec: OpenApiSpec = serde_json::from_str(&raw).expect("linode spec parses");
    assert!(
        spec.paths.len() > 300,
        "sanity: the real spec should be huge, got {} paths",
        spec.paths.len()
    );

    // Exactly what feature 03 needs — note Linode's `{apiVersion}` path prefix.
    let kept = Selection::all()
        .paths([
            "/{apiVersion}/linode/instances",
            "/{apiVersion}/linode/instances/{linodeId}",
            "/{apiVersion}/profile/sshkeys",
        ])
        .selected_paths(&spec);

    assert_eq!(kept.len(), 3, "only the paths asked for: {kept:?}");
    assert!(kept.contains("/{apiVersion}/linode/instances"));

    // This is the assertion the whole feature exists for.
    assert!(
        spec.paths.len() - kept.len() > 300,
        "the other 300+ paths are dropped before anything is generated"
    );
}
