//! `rewrite_features_for_groups`: regenerating a provider rewrites only its own
//! features in the crate's manifest.

use foundation_openapi::unified::rewrite_features_for_groups;

const MANIFEST: &str = r#"[package]
name = "foundation_deployment_example"
version = "0.1.0"

# Only the REST client needs this.
[dependencies.serde]
workspace = true

[features]
# Everything, for a consumer that wants the whole API.
example = ["example_dns", "example_old"]
example_all = ["example_dns", "example_old"]
example_dns = []
# Removed upstream; regeneration must drop it.
example_old = []
default = ["example"]
workers = ["dep:worker"]
"#;

fn groups(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

fn features(manifest: &str) -> toml_edit::Table {
    let doc: toml_edit::DocumentMut = manifest.parse().unwrap();
    doc["features"].as_table().unwrap().clone()
}

fn list(table: &toml_edit::Table, key: &str) -> Vec<String> {
    table[key]
        .as_array()
        .unwrap_or_else(|| panic!("feature `{key}` missing"))
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_provider_and_all_features_list_exactly_the_current_groups() {
    let out = rewrite_features_for_groups(MANIFEST, "example", &groups(&["dns", "zones"])).unwrap();
    let f = features(&out);

    assert_eq!(list(&f, "example"), ["example_dns", "example_zones"]);
    assert_eq!(list(&f, "example_all"), ["example_dns", "example_zones"]);
    assert_eq!(list(&f, "example_zones"), Vec::<String>::new());
    assert!(
        !f.contains_key("example_old"),
        "a group that no longer exists keeps no feature"
    );
}

#[test]
fn everything_the_generator_does_not_own_is_left_as_written() {
    let out = rewrite_features_for_groups(MANIFEST, "example", &groups(&["dns"])).unwrap();

    assert!(out.contains("# Only the REST client needs this."));
    assert!(out.contains("# Everything, for a consumer that wants the whole API."));
    assert!(out.starts_with("[package]\nname = \"foundation_deployment_example\""));
    let f = features(&out);
    assert_eq!(list(&f, "default"), ["example"]);
    assert_eq!(list(&f, "workers"), ["dep:worker"]);
}

#[test]
fn rewriting_an_up_to_date_manifest_changes_nothing() {
    let once =
        rewrite_features_for_groups(MANIFEST, "example", &groups(&["dns", "zones"])).unwrap();
    let twice = rewrite_features_for_groups(&once, "example", &groups(&["dns", "zones"])).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn a_sub_provider_refreshes_its_parent_feature() {
    let manifest = "[features]\ngcp = []\ngcp_all = []\ngcp_dns = []\ngcp_dns_zones = []\n";
    let out = rewrite_features_for_groups(manifest, "gcp/admin", &groups(&["users"])).unwrap();
    let f = features(&out);

    assert_eq!(list(&f, "gcp_admin"), ["gcp_admin_users"]);
    assert_eq!(list(&f, "gcp_admin_all"), ["gcp_admin_users"]);
    // Sub-providers only: not `gcp_all`, not a group feature like `gcp_dns_zones`.
    assert_eq!(list(&f, "gcp"), ["gcp_admin", "gcp_dns"]);
}

#[test]
fn a_manifest_without_features_is_an_error() {
    let err =
        rewrite_features_for_groups("[package]\nname = \"x\"\n", "example", &groups(&["dns"]));
    assert!(err.is_err());
}

#[test]
fn a_manifest_that_already_matches_is_returned_byte_for_byte() {
    // Written the way the committed provider manifests are: inline single-line
    // lists here, multi-line there. Matching contents must not be reformatted.
    let manifest = "[features]\n\
        example = [\n    \"example_dns\",\n]\n\
        example_all = [\"example_dns\"]\n\
        example_dns = []\n";
    let out = rewrite_features_for_groups(manifest, "example", &groups(&["dns"])).unwrap();
    assert_eq!(out, manifest);
}

#[test]
fn hand_written_entries_in_the_provider_feature_are_kept() {
    // Docker's `docker` feature also turns on the optional `tar` dependency.
    let manifest = "[features]\n\
        example = [\n    \"dep:tar\",\n    \"example_dns\",\n]\n\
        example_all = [\"example_dns\"]\n\
        example_dns = []\n";
    let out = rewrite_features_for_groups(manifest, "example", &groups(&["dns", "zones"])).unwrap();
    let f = features(&out);
    assert_eq!(
        list(&f, "example"),
        ["dep:tar", "example_dns", "example_zones"]
    );
    assert_eq!(list(&f, "example_all"), ["example_dns", "example_zones"]);

    // Same entries in another order is not a change.
    let reordered = "[features]\nexample = [\"example_dns\", \"dep:tar\"]\nexample_all = [\"example_dns\"]\nexample_dns = []\n";
    let out = rewrite_features_for_groups(reordered, "example", &groups(&["dns"])).unwrap();
    assert_eq!(out, reordered);
}

#[test]
fn a_hand_ordered_features_table_is_not_re_sorted_when_nothing_is_added() {
    let manifest = "[features]\nzeta = []\nexample = [\"example_dns\"]\nexample_all = [\"example_dns\"]\nexample_dns = []\nalpha = []\n";
    let out = rewrite_features_for_groups(manifest, "example", &groups(&["dns"])).unwrap();
    assert_eq!(out, manifest);
}
