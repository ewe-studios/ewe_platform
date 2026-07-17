//! Tests for [`HetznerClient`]'s credential resolution and secret hygiene
//! (spec-56 decision 02). No network.
//!
//! **On the environment:** `std::env::set_var` is process-global and these tests
//! run threaded, so the resolution *order* is tested through
//! [`credential_vars`] — a pure function — rather than by exporting variables and
//! racing. The one test that does touch the environment is deliberately about the
//! absent case, where there is nothing to race over.

use foundation_deployment_hetzner::client::{credential_vars, API_VERSION, VENDOR_VARS};
use foundation_deployment_hetzner::{HetznerClient, HetznerError};

// ── which variables, in which order ──────────────────────────────────────────

#[test]
fn the_override_is_the_vendor_name_with_a_prefix() {
    // The point of decision 02 §1: strip EWE_ and you are back at the vendor's
    // variable. There is no third spelling to invent, remember, or get wrong.
    //
    // Both entries are **Hetzner Cloud** API tokens, verified against each tool's
    // source: `hcloud`/Terraform read HCLOUD_TOKEN, and lego reads
    // HETZNER_API_TOKEN for `api.hetzner.cloud/v1` — the same endpoint we call.
    // Deliberately absent: HETZNER_API_KEY/HETZNER_TOKEN (the *DNS* API, a
    // different product) and TF_VAR_hcloud_token (Terraform's namespace, where
    // the suffix is the config author's choice).
    assert_eq!(
        VENDOR_VARS,
        &["HCLOUD_TOKEN", "HETZNER_API_TOKEN"],
        "the Cloud-API spellings that are live in the wild"
    );
    for var in credential_vars() {
        let stripped = var.strip_prefix("EWE_").unwrap_or(&var);
        assert!(
            VENDOR_VARS.contains(&stripped),
            "{var} is not a vendor name or a derived override"
        );
    }
}

#[test]
fn every_override_is_tried_before_every_vendor_name() {
    // Not "each override before its own twin" — an EWE_ variable says "use THIS
    // token for this workspace", and that intent should beat any vendor variable
    // lying around in the environment.
    let vars = credential_vars();
    let last_override = vars.iter().rposition(|v| v.starts_with("EWE_")).expect("has overrides");
    let first_vendor = vars
        .iter()
        .position(|v| !v.starts_with("EWE_"))
        .expect("has vendor names");
    assert!(
        last_override < first_vendor,
        "all EWE_* must precede all vendor-native, got {vars:?}"
    );
    assert_eq!(
        vars,
        vec![
            "EWE_HCLOUD_TOKEN",
            "EWE_HETZNER_API_TOKEN",
            "HCLOUD_TOKEN",
            "HETZNER_API_TOKEN",
        ]
    );
}

// ── the absent case ──────────────────────────────────────────────────────────

#[test]
fn no_token_names_every_variable_it_looked_for() {
    // A "credentials not found" that does not say what it looked for is a
    // scavenger hunt (decision 02 §2) — and with four names it is a long one.
    let err = HetznerError::NoCredentials {
        looked_for: credential_vars(),
    };
    let msg = err.to_string();
    for expected in credential_vars() {
        assert!(msg.contains(&expected), "{expected} missing from: {msg}");
    }
}

#[test]
fn the_dns_api_variables_are_not_read() {
    // The trap: Hetzner's DNS API (dns.hetzner.com) is a different product with
    // different tokens. Reading one would send a DNS token to the Cloud API, earn
    // a 401, and have us report "check your token" when the truth is "that token
    // is for another product". lego deprecates its own DNS-API path for the same
    // reason.
    //
    // TF_VAR_hcloud_token is out for a different reason: TF_VAR_* is Terraform's
    // namespace, and the suffix is whatever a config author named their variable.
    for never in ["HETZNER_API_KEY", "HETZNER_TOKEN", "TF_VAR_hcloud_token"] {
        assert!(
            !credential_vars().iter().any(|v| v == never),
            "{never} must not be read"
        );
    }
}

#[test]
fn from_env_without_any_variable_set_fails_rather_than_building_an_anonymous_client() {
    // Guarded: only meaningful when the developer running the suite has no token
    // exported. Skipping beats a test that fails on a Hetzner user's machine.
    if credential_vars().iter().any(|v| std::env::var(v).is_ok()) {
        eprintln!("SKIP: a Hetzner token is exported in this environment");
        return;
    }
    match HetznerClient::from_env() {
        Err(HetznerError::NoCredentials { looked_for }) => {
            assert_eq!(looked_for, credential_vars());
        }
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("built a client with no credentials"),
    }
}

// ── secret hygiene ───────────────────────────────────────────────────────────

#[test]
fn debug_never_prints_the_token() {
    // Decision 02 §4. A derived Debug on a struct holding a bearer token is one
    // tracing::debug! away from a token in a log — and logs get shipped.
    let client = HetznerClient::new("super-secret-token-value");
    let debug = format!("{client:?}");
    assert!(
        !debug.contains("super-secret-token-value"),
        "the token leaked into Debug: {debug}"
    );
    assert!(debug.contains("redacted"), "and it should say so: {debug}");

    // The alternate/pretty formatter is a separate code path.
    let pretty = format!("{client:#?}");
    assert!(!pretty.contains("super-secret-token-value"), "leaked in {{:#?}}: {pretty}");
}

#[test]
fn no_error_variant_interpolates_a_token() {
    // Errors get logged, wrapped and reported. None of these should ever be able
    // to carry the secret, so none of them holds it in the first place.
    let errors = [
        HetznerError::Unauthorized,
        HetznerError::RateLimited { retry_after_secs: Some(30) },
        HetznerError::Api {
            status: 422,
            code: "invalid_input".into(),
            message: "server_type is required".into(),
        },
        HetznerError::Transport("connection refused".into()),
        HetznerError::NoCredentials { looked_for: credential_vars() },
    ];
    for err in errors {
        let rendered = format!("{err} {err:?}");
        assert!(!rendered.contains("secret"), "{rendered}");
    }
}

#[test]
fn unauthorized_is_its_own_variant_and_says_what_to_check() {
    // "Your token is wrong" and "the network is down" want different responses
    // from the caller (decision 02 §3).
    let err = HetznerError::Unauthorized;
    assert_ne!(err, HetznerError::Transport("401".into()));
    let msg = err.to_string();
    assert!(msg.contains("401"), "{msg}");
    assert!(msg.contains("token"), "says what to check: {msg}");
}

// ── the constructors ─────────────────────────────────────────────────────────

#[test]
fn the_api_version_is_pinned_not_passed() {
    // An API version is not a per-call decision (feature 00 §3).
    let client = HetznerClient::new("t");
    assert_eq!(client.api_version(), "v1");
    assert_eq!(API_VERSION, "v1");
    assert!(client.base_url().ends_with("/v1"), "{}", client.base_url());
}

#[test]
fn with_base_url_points_the_client_at_a_mock() {
    let client = HetznerClient::new("t").with_base_url("http://127.0.0.1:8080");
    assert_eq!(client.base_url(), "http://127.0.0.1:8080");
    assert_eq!(client.token(), "t", "the token survives the rebuild");
}
