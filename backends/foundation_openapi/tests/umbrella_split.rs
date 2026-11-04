//! `rewrite_umbrella_for_split_str`: the umbrella crate depends on its group crates
//! (and the shared types crate) only on native targets, because its `generated`
//! module is native-only and each group crate pulls the native thread pool
//! (`foundation_netio/multi`), which does not compile for wasm32.

use foundation_openapi::unified::{rewrite_umbrella_for_split_str, ApiGroup, NATIVE_ONLY_CFG};

/// An umbrella manifest as an older generator left it: group crates and the
/// types crate in the all-target `[dependencies]` table.
const OLD_MANIFEST: &str = r#"[package]
name = "foundation_deployment_example"
version = "0.1.0"

[dependencies]
foundation_example_dns = { path = "../foundation_example_dns", version = "0.1.0", optional = true }
foundation_cloudflare_types = { path = "../foundation_cloudflare_types", version = "0.1.0", optional = true }
sha2 = "0.10"

[dependencies.serde]
workspace = true

[target.'cfg(not(target_family = "wasm"))'.dependencies.foundation_netio]
path = "../foundation_netio"
version = "0.0.4"

[features]
example = []
example_all = []
default = ["example"]
"#;

fn group(name: &str) -> ApiGroup {
    ApiGroup {
        name: name.to_string(),
        endpoints: Vec::new(),
        response_types: Vec::new(),
        request_types: Vec::new(),
    }
}

fn rewrite(manifest: &str) -> toml_edit::DocumentMut {
    let out = rewrite_umbrella_for_split_str(manifest, "example", &[group("dns"), group("zones")])
        .expect("rewrite");
    out.parse().expect("output parses")
}

#[test]
fn group_crates_are_native_only_dependencies() {
    let doc = rewrite(OLD_MANIFEST);
    let native = &doc["target"][NATIVE_ONLY_CFG]["dependencies"];
    for name in [
        "foundation_example_dns",
        "foundation_example_zones",
        "foundation_cloudflare_types",
    ] {
        assert_eq!(
            native[name]["optional"].as_bool(),
            Some(true),
            "{name} must be an optional native-only dependency"
        );
    }
}

#[test]
fn group_crates_leave_the_all_target_table() {
    let doc = rewrite(OLD_MANIFEST);
    let all = doc["dependencies"].as_table_like().expect("[dependencies]");
    for name in [
        "foundation_example_dns",
        "foundation_example_zones",
        "foundation_cloudflare_types",
        "sha2",
    ] {
        assert!(
            all.get(name).is_none(),
            "{name} must not be an all-target dependency"
        );
    }
    assert!(
        all.get("serde").is_some(),
        "unrelated dependencies are kept"
    );
}

#[test]
fn existing_native_entries_are_kept_and_rewrite_is_stable() {
    let once =
        rewrite_umbrella_for_split_str(OLD_MANIFEST, "example", &[group("dns"), group("zones")])
            .expect("first rewrite");
    let doc: toml_edit::DocumentMut = once.parse().expect("parses");
    assert!(
        doc["target"][NATIVE_ONLY_CFG]["dependencies"]["foundation_netio"]
            .get("path")
            .is_some(),
        "the hand-written native netio entry is kept"
    );
    let twice = rewrite_umbrella_for_split_str(&once, "example", &[group("dns"), group("zones")])
        .expect("second rewrite");
    assert_eq!(once, twice, "a second rewrite changes nothing");
}
