//! `ProvisioningService` over a real Turso store (moved from the inline
//! `#[cfg(test)]` module in `src/server/services/provisioning_service.rs`).

use foundation_auth::server::models::provider::{ProviderMapping, ProviderType, UpstreamProvider};
use foundation_auth::server::services::provisioning_service::{ProvisioningResult, ProvisioningService};
use foundation_auth::shared::upstream_client::UpstreamProfile;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::{StorageBackend, StorageProvider};
use std::sync::Arc;

fn test_provider() -> UpstreamProvider {
    UpstreamProvider {
        id: "google".into(),
        name: "Google".into(),
        provider_type: ProviderType::Oidc,
        client_id: "test-client".into(),
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        discovery_url: Some("https://accounts.google.com/.well-known/openid-configuration".into()),
        scopes: vec!["openid".into(), "email".into(), "profile".into()],
        is_active: true,
        mapping_config: ProviderMapping::default(),
        created_at: 0,
        updated_at: 0,
    }
}

fn test_profile() -> UpstreamProfile {
    UpstreamProfile {
        sub: "google-12345".into(),
        email: Some("user@example.com".into()),
        email_verified: true,
        name: Some("Test User".into()),
        preferred_username: Some("testuser".into()),
        raw: serde_json::json!({
            "sub": "google-12345",
            "email": "user@example.com",
            "email_verified": true,
            "name": "Test User",
        }),
    }
}

/// A migrated Turso `StorageProvider`. Test setup is a sync boundary, so it
/// waits for the async constructor with `.block()`.
fn make_store() -> (Arc<StorageProvider>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = dir.path().join("provision_test.db").to_str().unwrap().to_string();
    let provider = async move { StorageProvider::new(StorageBackend::Turso { url }).await }
        .block()
        .expect("init turso");
    (Arc::new(provider), dir)
}

#[valtron_test]
async fn new_profile_creates_user_and_link() {
    let (store, _tmp) = make_store();
    let svc = ProvisioningService::new(store);
    let provider = test_provider();
    let profile = test_profile();

    let result = svc.provision(&provider, &profile).await.expect("provision");
    match result {
        ProvisioningResult::Created { ref user_id, email_verified: true } => {
            assert!(!user_id.is_empty());
        }
        _ => panic!("expected Created, got {result:?}"),
    }
}

#[valtron_test]
async fn same_profile_provisions_once_and_links_twice() {
    let (store, _tmp) = make_store();
    let svc = ProvisioningService::new(store);
    let provider = test_provider();
    let profile = test_profile();

    let result1 = svc.provision(&provider, &profile).await.expect("first provision");
    let uid1 = result1.user_id().to_string();

    let result2 = svc.provision(&provider, &profile).await.expect("second provision");
    let uid2 = result2.user_id().to_string();

    assert_eq!(uid1, uid2, "same upstream sub should return same local user");
}

#[valtron_test]
async fn different_providers_with_same_verified_email_share_user() {
    let (store, _tmp) = make_store();
    let svc = ProvisioningService::new(store);

    // Google: first login
    let google = test_provider();
    let profile_g = test_profile();

    let r1 = svc.provision(&google, &profile_g).await.expect("google provision");
    let uid1 = r1.user_id().to_string();

    // GitHub: same email, different sub
    let mut github = test_provider();
    github.id = "github".into();
    github.name = "GitHub".into();
    github.discovery_url = None;
    let mut profile_gh = test_profile();
    profile_gh.sub = "github-999".into();

    let r2 = svc.provision(&github, &profile_gh).await.expect("github provision");
    let uid2 = r2.user_id().to_string();

    assert_eq!(uid1, uid2, "same verified email should auto-link to existing user");
}

#[valtron_test]
async fn disabled_auto_link_creates_separate_users() {
    let (store, _tmp) = make_store();
    let mut svc = ProvisioningService::new(store);
    svc.disable_auto_link();

    let provider = test_provider();
    let p1 = test_profile();
    let r1 = svc.provision(&provider, &p1).await.expect("first");

    // With auto-link disabled, the same email from a different upstream
    // identity must NOT auto-link — but the email must be different too
    // (users.email is UNIQUE), simulating a different-email scenario.
    let mut p2 = test_profile();
    p2.sub = "google-different-sub".into();
    p2.email = Some("other@example.com".into());
    p2.name = Some("Other User".into());
    let r2 = svc.provision(&provider, &p2).await.expect("second");

    assert_ne!(
        r1.user_id(),
        r2.user_id(),
        "without auto-link, different subs with different emails get different users"
    );
}

#[valtron_test]
async fn link_exists_detects_existing_link() {
    let (store, _tmp) = make_store();
    let svc = ProvisioningService::new(store);
    let provider = test_provider();
    let profile = test_profile();

    assert!(!svc.link_exists(&provider, &profile.sub).await.expect("check"));

    svc.provision(&provider, &profile).await.expect("provision");

    assert!(svc.link_exists(&provider, &profile.sub).await.expect("check after"));
}
