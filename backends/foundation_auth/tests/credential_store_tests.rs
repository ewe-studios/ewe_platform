//! CredentialStorage tests — Turso backend with shared valtron pool.

use foundation_auth::shared::credential_store::{
    CredentialStorage, CredentialStore, OAuthTokenStore, StoredCredential,
};
use foundation_auth::shared::oauth_token::OAuthToken;
use foundation_db::{StorageBackend, StorageProvider};
use std::sync::Mutex;

/// Shared Valtron pool guard — initialized once and reused across all tests
/// to avoid parallel tests interfering with each other's thread pool.
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

fn sqlite_store() -> (CredentialStorage, tempfile::TempDir) {
    init_valtron();
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("auth.sqlite");
    let store = CredentialStorage::turso(db_path.to_str().expect("non-utf8 tempdir path"))
        .expect("init turso credential storage");
    (store, dir)
}

#[test]
fn sqlite_store_basic_roundtrip() {
    let (store, _dir) = sqlite_store();

    store
        .set("test_key", "test_value")
        .expect("Failed to set credential");

    let value: String = store
        .get("test_key")
        .expect("Failed to get credential")
        .expect("Credential not found");
    assert_eq!(value, "test_value");

    assert!(store.exists("test_key").expect("Failed to check existence"));
    assert!(!store
        .exists("nonexistent")
        .expect("Failed to check existence"));

    store
        .delete("test_key")
        .expect("Failed to delete credential");
    assert!(!store.exists("test_key").expect("Failed to check existence"));
}

#[test]
fn sqlite_store_oauth_helpers() {
    let (store, _dir) = sqlite_store();

    let token = OAuthToken {
        access_token: "access_123".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("refresh_456".to_string()),
        scope: Some("openid profile".to_string()),
        id_token: None,
    };

    store
        .store_oauth_token("test_provider", &token)
        .expect("Failed to store OAuth token");

    let retrieved = store
        .get_oauth_token("test_provider")
        .expect("Failed to get OAuth token")
        .expect("OAuth token not found");
    assert_eq!(retrieved.access_token, "access_123");
    assert_eq!(retrieved.refresh_token, Some("refresh_456".to_string()));

    store
        .store_oauth_state("test_state", "verifier_abc", 9_999_999_999)
        .expect("Failed to store OAuth state");

    let state = store
        .get_oauth_state("test_state")
        .expect("Failed to get OAuth state")
        .expect("OAuth state not found");
    assert_eq!(state.code_verifier, "verifier_abc");
    assert!(!state.is_expired());
}

#[test]
fn sqlite_store_list_keys_filters_by_prefix() {
    let (store, _dir) = sqlite_store();

    let _ = store.set("oauth:provider1", "value1").unwrap();
    let _ = store.set("oauth:provider2", "value2").unwrap();
    let _ = store.set("jwt:token", "value3").unwrap();

    let keys = store.list_keys(None).unwrap();
    assert_eq!(keys.len(), 3);

    let keys = store.list_keys(Some("oauth:")).unwrap();
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

#[test]
fn credential_storage_wraps_arbitrary_provider() {
    init_valtron();
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("wrap.sqlite");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_string_lossy().into_owned(),
    })
    .expect("init turso provider");
    let store = CredentialStorage::new(provider);
    let _ = store.set("k", "v").unwrap();
    let got: String = store.get("k").unwrap().unwrap();
    assert_eq!(got, "v");
}
