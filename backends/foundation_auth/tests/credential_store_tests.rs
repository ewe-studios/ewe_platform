//! `CredentialStorage` tests — Turso backend, async (`#[valtron_test]`).

use foundation_auth::shared::credential_store::{
    AsyncCredentialStore, CredentialStorage, OAuthTokenStore, StoredCredential,
};
use foundation_auth::shared::oauth_token::OAuthToken;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_core::valtron::valtron_test;

async fn sqlite_store() -> (CredentialStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("auth.sqlite");
    let url = db_path.to_str().expect("non-utf8 tempdir path").to_owned();
    let store = CredentialStorage::turso(&url)
        .await
        .expect("init turso credential storage");
    (store, dir)
}

#[valtron_test]
async fn sqlite_store_basic_roundtrip() {
    let (store, _dir) = sqlite_store().await;

    store
        .set_async("test_key", "test_value").await
        .expect("Failed to set credential");

    let value: String = store
        .get_async("test_key").await
        .expect("Failed to get credential")
        .expect("Credential not found");
    assert_eq!(value, "test_value");

    assert!(store.exists_async("test_key").await.expect("Failed to check existence"));
    assert!(!store
        .exists_async("nonexistent").await
        .expect("Failed to check existence"));

    store
        .delete_async("test_key").await
        .expect("Failed to delete credential");
    assert!(!store.exists_async("test_key").await.expect("Failed to check existence"));
}

#[valtron_test]
async fn sqlite_store_oauth_helpers() {
    let (store, _dir) = sqlite_store().await;

    let token = OAuthToken {
        access_token: "access_123".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("refresh_456".to_string()),
        scope: Some("openid profile".to_string()),
        id_token: None,
    };

    store
        .store_oauth_token("test_provider", &token).await
        .expect("Failed to store OAuth token");

    let retrieved = store
        .get_oauth_token("test_provider").await
        .expect("Failed to get OAuth token")
        .expect("OAuth token not found");
    assert_eq!(retrieved.access_token, "access_123");
    assert_eq!(retrieved.refresh_token, Some("refresh_456".to_string()));

    store
        .store_oauth_state("test_state", "verifier_abc", 9_999_999_999).await
        .expect("Failed to store OAuth state");

    let state = store
        .get_oauth_state("test_state").await
        .expect("Failed to get OAuth state")
        .expect("OAuth state not found");
    assert_eq!(state.code_verifier, "verifier_abc");
    assert!(!state.is_expired());
}

#[valtron_test]
async fn sqlite_store_list_keys_filters_by_prefix() {
    let (store, _dir) = sqlite_store().await;

    store.set_async("oauth:provider1", "value1").await.unwrap();
    store.set_async("oauth:provider2", "value2").await.unwrap();
    store.set_async("jwt:token", "value3").await.unwrap();

    let keys = store.list_keys_async(None).await.unwrap();
    assert_eq!(keys.len(), 3);

    let keys = store.list_keys_async(Some("oauth:")).await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"oauth:provider1".to_string()));
    assert!(keys.contains(&"oauth:provider2".to_string()));
}

#[test]
fn stored_credential_tracks_access() {
    let mut stored = StoredCredential::new("secret_data");
    assert!(stored.created_at > 0);
    assert!(stored.last_accessed_at.is_none());

    stored.mark_accessed();
    assert!(stored.last_accessed_at.is_some());
}

#[valtron_test]
async fn credential_storage_wraps_arbitrary_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("wrap.sqlite");
    let url = db_path.to_string_lossy().into_owned();
    let provider = StorageProvider::new(StorageBackend::Turso { url })
        .await
        .expect("init turso provider");
    let store = CredentialStorage::new(provider);
    store.set_async("k", "v").await.unwrap();
    let got: String = store.get_async("k").await.unwrap().unwrap();
    assert_eq!(got, "v");
}
