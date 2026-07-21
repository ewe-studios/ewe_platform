//! ProviderService integration tests — real Turso DB, no mocks.
//!
//! Exercises the full CRUD + secret encryption round-trip against a real
//! SQLite (Turso) database with migrations 024/025 applied via init_schema.

#![cfg(feature = "server-test")]

use std::sync::Arc;

use foundation_auth::server::models::provider::{
    ProviderMapping, ProviderType, ProviderUpdate, UpstreamProvider,
};
use foundation_auth::server::services::provider_service::{ProviderCrypto, ProviderService};
use foundation_core::valtron::valtron_test;
use foundation_db::{QueryStore, StorageBackend, StorageProvider};
use tempfile::TempDir;

/// Build a real Turso-backed query store with all migrations applied.
///
/// Returns the `TempDir` guard alongside the store: Turso holds the DB file
/// open, so the directory must outlive the store. The caller binds the guard
/// for the test's duration; when it drops at end of test, the temp dir (and
/// DB file) are removed — no leak.
fn make_store() -> (Arc<StorageProvider>, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("provider_test.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    })
    .expect("init turso");
    (Arc::new(provider), dir)
}

/// Build a service over a fresh store. The returned `TempDir` must be held for
/// the test's lifetime (bind it, e.g. `let (svc, _tmp) = make_service();`).
fn make_service() -> (ProviderService<StorageProvider>, TempDir) {
    let (store, dir) = make_store();
    let crypto = ProviderCrypto::from_passphrase("test-passphrase-abc123", "default");
    (ProviderService::new(store, crypto), dir)
}

fn sample_provider() -> UpstreamProvider {
    UpstreamProvider {
        id: "google".into(),
        name: "Google".into(),
        provider_type: ProviderType::Oidc,
        client_id: "client-abc.apps.googleusercontent.com".into(),
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
        mapping_config: ProviderMapping::default(),
        created_at: 0,
        updated_at: 0,
    }
}

// ── Valid input ──────────────────────────────────────────────────────────

#[valtron_test]
fn create_and_find_round_trips() {
    let (svc, _tmp) = make_service();
    let created = svc.create(sample_provider()).expect("create");
    assert_eq!(created.id, "google");
    assert!(created.created_at > 0, "created_at should be stamped");
    assert!(created.client_secret_ciphertext.is_none());

    let found = svc.find_by_id("google").expect("find").expect("present");
    assert_eq!(found.name, "Google");
    assert_eq!(found.provider_type, ProviderType::Oidc);
    assert_eq!(found.scopes, vec!["openid", "email", "profile"]);
    assert_eq!(
        found.mapping_config.subject_field,
        ProviderMapping::default().subject_field
    );
    assert!(found.is_active);
}

#[valtron_test]
fn secret_encrypt_decrypt_round_trips() {
    let (svc, _tmp) = make_service();
    svc.create(sample_provider()).expect("create");

    svc.set_secret("google", "GOCSPX-super-secret").expect("set_secret");

    // Decrypt returns the original plaintext.
    let secret = svc.get_secret("google").expect("get_secret");
    assert_eq!(secret, "GOCSPX-super-secret");

    // The stored blob is NOT the plaintext (encrypted at rest).
    let stored = svc.find_by_id("google").unwrap().unwrap();
    let blob = stored.client_secret_ciphertext.expect("ciphertext present");
    assert_ne!(blob.as_slice(), b"GOCSPX-super-secret");
    assert!(blob.len() > "GOCSPX-super-secret".len(), "nonce + tag overhead");
}

#[valtron_test]
fn secret_survives_new_service_with_same_passphrase() {
    // Decrypt must be deterministic across service instances (restart safety):
    // a fresh ProviderCrypto derived from the same passphrase decrypts.
    let (store, _tmp) = make_store();

    let svc1 = ProviderService::new(
        store.clone(),
        ProviderCrypto::from_passphrase("shared-passphrase", "default"),
    );
    svc1.create(sample_provider()).expect("create");
    svc1.set_secret("google", "the-secret").expect("set_secret");

    let svc2 = ProviderService::new(
        store,
        ProviderCrypto::from_passphrase("shared-passphrase", "default"),
    );
    assert_eq!(svc2.get_secret("google").unwrap(), "the-secret");
}

#[valtron_test]
fn wrong_passphrase_fails_to_decrypt() {
    let (store, _tmp) = make_store();
    let svc1 = ProviderService::new(
        store.clone(),
        ProviderCrypto::from_passphrase("right-passphrase", "default"),
    );
    svc1.create(sample_provider()).expect("create");
    svc1.set_secret("google", "the-secret").expect("set_secret");

    let svc2 = ProviderService::new(
        store,
        ProviderCrypto::from_passphrase("wrong-passphrase", "default"),
    );
    // AEAD authentication fails with the wrong key.
    assert!(svc2.get_secret("google").is_err());
}

#[valtron_test]
fn update_applies_partial_changes() {
    let (svc, _tmp) = make_service();
    svc.create(sample_provider()).expect("create");

    let updated = svc
        .update(
            "google",
            ProviderUpdate {
                name: Some("Google SSO".into()),
                is_active: Some(false),
                scopes: Some(vec!["openid".into()]),
                ..Default::default()
            },
        )
        .expect("update");
    assert_eq!(updated.name, "Google SSO");
    assert!(!updated.is_active);
    assert_eq!(updated.scopes, vec!["openid"]);
    // Untouched fields remain.
    assert_eq!(updated.client_id, "client-abc.apps.googleusercontent.com");
}

#[valtron_test]
fn find_active_filters_inactive() {
    let (svc, _tmp) = make_service();
    svc.create(sample_provider()).expect("create google");

    let mut github = sample_provider();
    github.id = "github".into();
    github.name = "GitHub".into();
    github.provider_type = ProviderType::Oauth2;
    github.discovery_url = None;
    github.authorization_url = Some("https://github.com/login/oauth/authorize".into());
    github.token_url = Some("https://github.com/login/oauth/access_token".into());
    github.is_active = false;
    svc.create(github).expect("create github");

    let active = svc.find_active().expect("find_active");
    assert_eq!(active.len(), 1, "only google is active");
    assert_eq!(active[0].id, "google");
}

#[valtron_test]
fn delete_removes_provider() {
    let (svc, _tmp) = make_service();
    svc.create(sample_provider()).expect("create");
    svc.delete("google").expect("delete");
    assert!(svc.find_by_id("google").unwrap().is_none());
}

// ── Invalid input ────────────────────────────────────────────────────────

#[valtron_test]
fn create_rejects_empty_id() {
    let (svc, _tmp) = make_service();
    let mut p = sample_provider();
    p.id = "".into();
    assert!(svc.create(p).is_err());
}

#[valtron_test]
fn create_rejects_empty_client_id() {
    let (svc, _tmp) = make_service();
    let mut p = sample_provider();
    p.client_id = "".into();
    assert!(svc.create(p).is_err());
}

#[valtron_test]
fn create_rejects_missing_endpoints() {
    let (svc, _tmp) = make_service();
    let mut p = sample_provider();
    p.discovery_url = None;
    p.authorization_url = None;
    p.token_url = None;
    // Neither discovery nor authorize+token → invalid.
    assert!(svc.create(p).is_err());
}

#[valtron_test]
fn oauth2_with_only_authorize_url_is_rejected() {
    let (svc, _tmp) = make_service();
    let mut p = sample_provider();
    p.discovery_url = None;
    p.authorization_url = Some("https://example.com/authorize".into());
    p.token_url = None; // missing token url
    assert!(svc.create(p).is_err());
}

#[valtron_test]
fn set_secret_on_missing_provider_is_not_found() {
    let (svc, _tmp) = make_service();
    assert!(svc.set_secret("nonexistent", "x").is_err());
}

#[valtron_test]
fn get_secret_without_set_is_error() {
    let (svc, _tmp) = make_service();
    svc.create(sample_provider()).expect("create");
    // No secret set yet.
    assert!(svc.get_secret("google").is_err());
}

#[valtron_test]
fn update_missing_provider_is_not_found() {
    let (svc, _tmp) = make_service();
    let r = svc.update("nope", ProviderUpdate::default());
    assert!(r.is_err());
}

// ── Edge cases ───────────────────────────────────────────────────────────

#[valtron_test]
fn find_missing_returns_none() {
    let (svc, _tmp) = make_service();
    assert!(svc.find_by_id("nope").unwrap().is_none());
}

#[valtron_test]
fn delete_missing_is_ok() {
    let (svc, _tmp) = make_service();
    // Idempotent: deleting a non-existent id succeeds.
    assert!(svc.delete("nope").is_ok());
}

#[valtron_test]
fn find_active_empty_when_none() {
    let (svc, _tmp) = make_service();
    assert!(svc.find_active().unwrap().is_empty());
}

#[valtron_test]
fn oauth2_provider_with_explicit_urls_is_valid() {
    let (svc, _tmp) = make_service();
    let mut p = sample_provider();
    p.id = "github".into();
    p.provider_type = ProviderType::Oauth2;
    p.discovery_url = None;
    p.authorization_url = Some("https://github.com/login/oauth/authorize".into());
    p.token_url = Some("https://github.com/login/oauth/access_token".into());
    p.userinfo_url = Some("https://api.github.com/user".into());
    let created = svc.create(p).expect("oauth2 provider with explicit urls is valid");
    assert_eq!(created.provider_type, ProviderType::Oauth2);
}
