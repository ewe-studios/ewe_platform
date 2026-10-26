//! Hetzner label values for the deployment identity (moved from the inline
//! test module of `src/deployable.rs`).

use foundation_deployment_hetzner::deployable::{label_value, HetznerServer, IDENTITY_LABEL};


#[test]
fn an_identity_is_a_legal_hetzner_label_value() {
    // The rule, from Hetzner's spec: <=63 chars, begins and ends
    // [a-z0-9A-Z], with -_. and alphanumerics between. No slashes — they are
    // only legal in a KEY, separating the prefix.
    //
    // NAMESPACE is "hetzner/cloud/servers", so the obvious
    // format!("{NAMESPACE}/{id}") gives "hetzner/cloud/servers/0", which the
    // real API rejects: `invalid label_selector: value contains invalid
    // characters or is malformed`. Every mock accepted it.
    let value = HetznerServer::identity(0);
    assert_eq!(value, "hetzner.cloud.servers.0");
    assert_valid(&value);

    // The key may carry a prefix — that is where a slash is allowed.
    assert!(IDENTITY_LABEL.contains('/'), "the key is prefixed: {IDENTITY_LABEL}");
    assert!(
        !IDENTITY_LABEL.starts_with("hetzner.cloud/"),
        "that prefix is reserved by Hetzner"
    );
}

#[test]
fn a_hostile_namespace_still_yields_a_legal_value() {
    for (ns, id) in [
        ("a/b/c", 0),
        ("with spaces", 3),
        ("sym+bols!", 9),
        ("trailing/", 1),
        ("ünicode", 2),
    ] {
        let value = label_value(ns, id);
        assert_valid(&value);
    }
}

#[test]
fn an_over_long_namespace_is_truncated_and_still_ends_alphanumeric() {
    // Truncating at 63 can leave a trailing '.', which is itself invalid —
    // the fix must not create the bug it is preventing.
    let value = label_value(&"x".repeat(80), 0);
    assert!(value.len() <= 63, "{} chars", value.len());
    assert_valid(&value);

    let value = label_value(&format!("{}.", "y".repeat(62)), 0);
    assert_valid(&value);
}

fn assert_valid(value: &str) {
    assert!(!value.is_empty(), "empty");
    assert!(value.len() <= 63, "{value:?} is {} chars", value.len());
    assert!(
        value.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()),
        "{value:?} must begin alphanumeric"
    );
    assert!(
        value.chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric()),
        "{value:?} must end alphanumeric"
    );
    assert!(
        value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'),
        "{value:?} has an illegal character"
    );
}
