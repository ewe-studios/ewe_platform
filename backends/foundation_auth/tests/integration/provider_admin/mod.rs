//! Provider admin API handler tests (spec-57, F006).
//!
//! Tests the handler functions directly — no HTTP server needed.
//! Each handler takes a ProviderService + JSON body and returns HandlerResponse.

#![cfg(feature = "server-test")]

use std::sync::Arc;

use foundation_auth::server::handlers::provider_admin::{
    self, CreateProviderRequest, ProviderEntry, SetSecretRequest,
};
use foundation_auth::server::models::provider::{ProviderType, UpstreamProvider};
use foundation_auth::server::services::provider_service::{ProviderCrypto, ProviderService};
use foundation_core::valtron::valtron_test;
use foundation_db::{QueryStore, StorageBackend, StorageProvider};
use tempfile::TempDir;

fn make_service() -> (ProviderService<StorageProvider>, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("admin_test.db");
    let store = Arc::new(
        StorageProvider::new(StorageBackend::Turso {
            url: db_path.to_str().unwrap().to_string(),
        })
        .expect("init turso"),
    );
    let crypto = ProviderCrypto::from_passphrase("test-key", "default");
    (ProviderService::new(store, crypto), dir)
}

// ── Discovery ───────────────────────────────────────────────────────────

#[valtron_test]
fn extended_discovery_lists_active_providers() {
    let (svc, _tmp) = make_service();

    // Create a few providers
    let google = sample_google();
    svc.create(google.clone()).expect("create google");

    let mut github = sample_github();
    github.is_active = false;
    svc.create(github).expect("create github");

    let providers = svc.find_active().expect("find_active");
    assert_eq!(providers.len(), 1, "only google is active");

    use foundation_auth::server::config::IdpConfig;
    let config = IdpConfig::new("https://auth.example.com".into());
    let resp = provider_admin::extended_discovery(&config, "/idp", &providers).expect("discovery");
    assert_eq!(resp.status, 200);

    let doc: serde_json::Value = resp.body;
    assert_eq!(doc["providers_supported"].as_array().unwrap().len(), 1);
    assert_eq!(doc["providers_supported"][0]["id"], "google");
    assert!(doc["social_login_endpoint"].as_str().unwrap().contains("/authorize"));
}

// ── List all ────────────────────────────────────────────────────────────

#[valtron_test]
fn list_all_includes_inactive() {
    let (svc, _tmp) = make_service();
    svc.create(sample_google()).expect("create google");
    svc.create(sample_github()).expect("create github");

    let resp = provider_admin::list_all(&svc).expect("list_all");
    let body: serde_json::Value = resp.body;
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 2);
}

// ── Get one ─────────────────────────────────────────────────────────────

#[valtron_test]
fn get_one_returns_provider() {
    let (svc, _tmp) = make_service();
    svc.create(sample_google()).expect("create");

    let resp = provider_admin::get_one(&svc, "google").expect("get");
    let entry: ProviderEntry =
        serde_json::from_value(resp.body).expect("deserialize");
    assert_eq!(entry.id, "google");
    assert_eq!(entry.name, "Google");
    assert_eq!(entry.provider_type, "oidc");
}

#[valtron_test]
fn get_one_missing_returns_error() {
    let (svc, _tmp) = make_service();
    let err = provider_admin::get_one(&svc, "nope").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// ── Create ──────────────────────────────────────────────────────────────

#[valtron_test]
fn create_provider_with_discovery_url() {
    let (svc, _tmp) = make_service();

    let req = CreateProviderRequest {
        id: "google".into(),
        name: "Google".into(),
        provider_type: "oidc".into(),
        client_id: "test-client".into(),
        discovery_url: Some("https://accounts.google.com/.well-known/openid-configuration".into()),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        scopes: vec!["openid".into(), "email".into()],
        client_secret: Some("secret-123".into()),
    };

    let resp = provider_admin::create(&svc, req).expect("create");
    let entry: ProviderEntry =
        serde_json::from_value(resp.body).expect("deserialize");
    assert_eq!(entry.id, "google");
    assert!(entry.is_active);
}

#[valtron_test]
fn create_rejects_invalid_provider_type() {
    let (svc, _tmp) = make_service();
    let mut req = create_req("bad");
    req.provider_type = "saml".into();
    let err = provider_admin::create(&svc, req).unwrap_err();
    assert!(err.to_string().contains("unknown provider_type"));
}

// ── Update ──────────────────────────────────────────────────────────────

#[valtron_test]
fn update_changes_name_and_scopes() {
    let (svc, _tmp) = make_service();
    svc.create(sample_google()).expect("create");

    let req = CreateProviderRequest {
        id: "google".into(),
        name: "Google SSO Updated".into(),
        provider_type: "oidc".into(),
        client_id: "test-client".into(),
        discovery_url: None,
        authorization_url: Some("https://example.com/auth".into()),
        token_url: Some("https://example.com/token".into()),
        userinfo_url: None,
        scopes: vec!["openid".into()],
        client_secret: None,
    };

    let resp = provider_admin::update(&svc, "google", req).expect("update");
    let entry: ProviderEntry =
        serde_json::from_value(resp.body).expect("deserialize");
    assert_eq!(entry.name, "Google SSO Updated");
    assert_eq!(entry.scopes, vec!["openid"]);
}

// ── Delete ──────────────────────────────────────────────────────────────

#[valtron_test]
fn delete_deactivates_provider() {
    let (svc, _tmp) = make_service();
    svc.create(sample_google()).expect("create");

    let resp = provider_admin::delete(&svc, "google").expect("delete");
    let body: serde_json::Value = resp.body;
    assert_eq!(body["deleted"], "google");

    assert!(svc.find_by_id("google").unwrap().is_none());
}

// ── Set secret ──────────────────────────────────────────────────────────

#[valtron_test]
fn set_secret_encrypts_and_stores() {
    let (svc, _tmp) = make_service();
    svc.create(sample_google()).expect("create");

    let req = SetSecretRequest {
        secret: "my-secret".into(),
    };
    let resp = provider_admin::set_secret(&svc, "google", req).expect("set_secret");
    assert_eq!(resp.body["ok"], true);

    let stored = svc.get_secret("google").expect("get_secret");
    assert_eq!(stored, "my-secret");
}

#[valtron_test]
fn set_secret_on_missing_provider_errors() {
    let (svc, _tmp) = make_service();
    let req = SetSecretRequest {
        secret: "secret".into(),
    };
    let err = provider_admin::set_secret(&svc, "nope", req).unwrap_err();
    assert!(err.to_string().contains("not found") || err.to_string().contains("Not"));
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn sample_google() -> UpstreamProvider {
    UpstreamProvider {
        id: "google".into(),
        name: "Google".into(),
        provider_type: ProviderType::Oidc,
        client_id: "google-client-id".into(),
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        discovery_url: Some(
            "https://accounts.google.com/.well-known/openid-configuration".into(),
        ),
        scopes: vec!["openid".into(), "email".into(), "profile".into()],
        is_active: true,
        mapping_config: Default::default(),
        created_at: 0,
        updated_at: 0,
    }
}

fn sample_github() -> UpstreamProvider {
    UpstreamProvider {
        id: "github".into(),
        name: "GitHub".into(),
        provider_type: ProviderType::Oauth2,
        client_id: "github-client-id".into(),
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: Some("https://github.com/login/oauth/authorize".into()),
        token_url: Some("https://github.com/login/oauth/access_token".into()),
        userinfo_url: Some("https://api.github.com/user".into()),
        discovery_url: None,
        scopes: vec!["user:email".into()],
        is_active: false,
        mapping_config: Default::default(),
        created_at: 0,
        updated_at: 0,
    }
}

fn create_req(id: &str) -> CreateProviderRequest {
    CreateProviderRequest {
        id: id.into(),
        name: "Test Provider".into(),
        provider_type: "oidc".into(),
        client_id: "test-client".into(),
        discovery_url: Some("https://example.com/.well-known/openid-configuration".into()),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        scopes: vec![],
        client_secret: None,
    }
}
