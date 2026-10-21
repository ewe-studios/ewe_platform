//! `AuthManager` tests (moved from the inline module in
//! `src/shared/auth_manager.rs`). They use only the public API and the
//! in-memory credential store.

use foundation_auth::shared::auth_manager::{AuthManager, AuthManagerConfig, AuthManagerError};
use foundation_auth::shared::auth_state::AuthState;
use foundation_auth::shared::credential_store::{CredentialStorage, OAuthTokenStore};
use foundation_auth::shared::jwks::JwksManager;
use foundation_auth::shared::jwt::{JwtError, JwtToken};
use foundation_auth::shared::oauth_token::OAuthToken;
use foundation_auth::shared::session::{SessionConfig, SessionManager};
use foundation_auth::shared::types::{AuthCredential, ConfidentialText};
use foundation_auth::OAuthCredential;
use foundation_core::valtron::valtron_test;

fn test_config() -> AuthManagerConfig {
    AuthManagerConfig::new().with_refresh_buffer(300)
}

fn test_manager() -> AuthManager {
    AuthManager::new(CredentialStorage::memory(), test_config())
}

async fn store_valid_oauth_token(store: &CredentialStorage) {
    let token = OAuthToken {
        access_token: "valid_access_token".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("valid_refresh_token".to_string()),
        scope: Some("openid profile".to_string()),
        id_token: None,
    };
    store.store_oauth_token("default", &token).await.unwrap();
}

async fn store_expired_oauth_token(store: &CredentialStorage, with_refresh: bool) {
    let token = OAuthToken {
        access_token: "expired_access_token".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(0),
        refresh_token: if with_refresh {
            Some("expired_refresh_token".to_string())
        } else {
            None
        },
        scope: None,
        id_token: None,
    };
    store.store_oauth_token("default", &token).await.unwrap();
}

// `OAuthCredential::expires` is an `f64` of Unix seconds; those are far below
// 2^52, so the cast is exact.
#[allow(clippy::cast_precision_loss)]
fn oauth_credential(expires_in_secs: i64) -> AuthCredential {
    AuthCredential::OAuth(OAuthCredential {
        expires: (chrono::Utc::now().timestamp() + expires_in_secs) as f64,
        access_token: ConfidentialText::new("oauth_access".to_string()),
        refresh_token: Some(ConfidentialText::new("oauth_refresh".to_string())),
    })
}

// ===== Initial state =====

#[test]
fn test_initial_state() {
    let mgr = test_manager();
    assert!(!mgr.is_authenticated());
    assert_eq!(mgr.state(), AuthState::Unauthenticated);
}

// ===== init_from_store =====

#[valtron_test]
async fn test_init_from_store_valid_token() {
    let mut mgr = test_manager();
    store_valid_oauth_token(mgr.store()).await;

    let result = mgr.init_from_store().await.unwrap();
    assert!(result.is_some());
    assert!(mgr.is_authenticated());
    assert_eq!(mgr.state(), AuthState::Authenticated);
    assert!(!result.unwrap().is_expired());
}

#[valtron_test]
async fn test_init_from_store_expired_with_refresh() {
    let mut mgr = test_manager();
    store_expired_oauth_token(mgr.store(), true).await;

    let result = mgr.init_from_store().await.unwrap();
    assert!(result.is_none());
    assert_eq!(mgr.state(), AuthState::TokenExpired);
    assert!(mgr.jwt_manager().get_token().is_some());
}

#[valtron_test]
async fn test_init_from_store_expired_no_refresh() {
    let mut mgr = test_manager();
    store_expired_oauth_token(mgr.store(), false).await;

    let result = mgr.init_from_store().await.unwrap();
    assert!(result.is_none());
    assert_eq!(mgr.state(), AuthState::Unauthenticated);
    // The expired token is cleared from the store.
    assert!(mgr.store().get_oauth_token("default").await.unwrap().is_none());
}

#[valtron_test]
async fn test_init_from_store_empty() {
    let mut mgr = test_manager();
    let result = mgr.init_from_store().await.unwrap();
    assert!(result.is_none());
    assert_eq!(mgr.state(), AuthState::Unauthenticated);
}

// ===== authenticate =====

#[valtron_test]
async fn test_authenticate_oauth() {
    let mut mgr = test_manager();
    let result = mgr.authenticate(oauth_credential(3600)).await;
    assert!(result.is_ok());
    assert!(mgr.is_authenticated());
    assert_eq!(mgr.state(), AuthState::Authenticated);

    // Token should be persisted
    let stored = mgr.store().get_oauth_token("default").await.unwrap();
    assert_eq!(stored.unwrap().access_token, "oauth_access");
}

#[valtron_test]
async fn test_authenticate_unsupported_credential() {
    let mut mgr = test_manager();
    let cred = AuthCredential::UsernameAndPassword {
        username: "user".to_string(),
        password: ConfidentialText::new("pass".to_string()),
    };

    let result = mgr.authenticate(cred).await;
    assert!(result.is_err());
    assert_eq!(mgr.state(), AuthState::Failed);
}

// ===== get_valid_token =====

#[valtron_test]
async fn test_get_valid_token_authenticated() {
    let mut mgr = test_manager();
    store_valid_oauth_token(mgr.store()).await;
    mgr.init_from_store().await.unwrap();

    let token = mgr.get_valid_token().unwrap();
    assert!(!token.is_expired());
}

#[test]
fn test_get_valid_token_not_authenticated() {
    let mgr = test_manager();
    assert!(mgr.get_valid_token().is_err());
}

#[valtron_test]
async fn test_get_valid_token_near_expiry() {
    let mut mgr = test_manager();
    // Authenticated, but the token expires within the refresh buffer (300s).
    mgr.authenticate(oauth_credential(60)).await.unwrap();
    assert!(mgr.is_authenticated());

    let result = mgr.get_valid_token();
    assert!(matches!(result, Err(AuthManagerError::TokenExpired)));
}

#[valtron_test]
async fn test_get_valid_token_when_expired() {
    let mut mgr = test_manager();
    store_expired_oauth_token(mgr.store(), true).await;
    mgr.init_from_store().await.unwrap();
    assert_eq!(mgr.state(), AuthState::TokenExpired);

    let result = mgr.get_valid_token();
    assert!(matches!(result, Err(AuthManagerError::TokenExpired)));
}

// ===== refresh =====

#[valtron_test]
async fn test_refresh_success() {
    let mut mgr = test_manager();
    store_valid_oauth_token(mgr.store()).await;
    mgr.init_from_store().await.unwrap();

    let new_expires = chrono::Utc::now().timestamp() + 7200;
    let result = mgr
        .refresh(|_old_refresh| {
            Ok(JwtToken::from_parts(
                "new_access".to_string(),
                Some("new_refresh".to_string()),
                new_expires,
                None,
                None,
                None,
            ))
        })
        .await;

    assert!(result.is_ok());
    assert!(mgr.is_authenticated());
    assert_eq!(mgr.jwt_manager().get_token().unwrap().access_token(), "new_access");
    // The new token is persisted.
    let stored = mgr.store().get_oauth_token("default").await.unwrap().unwrap();
    assert_eq!(stored.access_token, "new_access");
}

#[valtron_test]
async fn test_refresh_failure() {
    let mut mgr = test_manager();
    store_valid_oauth_token(mgr.store()).await;
    mgr.init_from_store().await.unwrap();

    let result = mgr
        .refresh(|_| {
            Err(AuthManagerError::Jwt(JwtError::RefreshFailed(
                "server error".to_string(),
            )))
        })
        .await;

    assert!(result.is_err());
    assert_eq!(mgr.state(), AuthState::Failed);
    assert!(mgr.jwt_manager().get_token().is_none());
    assert!(mgr.store().get_oauth_token("default").await.unwrap().is_none());
}

#[valtron_test]
async fn test_refresh_no_token() {
    let mut mgr = test_manager();
    let result = mgr.refresh(|_| unreachable!()).await;
    assert!(matches!(result, Err(AuthManagerError::NoRefreshToken)));
}

#[valtron_test]
async fn test_refresh_from_token_expired_state() {
    let mut mgr = test_manager();
    store_expired_oauth_token(mgr.store(), true).await;
    mgr.init_from_store().await.unwrap();
    assert_eq!(mgr.state(), AuthState::TokenExpired);

    let new_expires = chrono::Utc::now().timestamp() + 7200;
    let result = mgr
        .refresh(|old_refresh| {
            assert_eq!(old_refresh, "expired_refresh_token");
            Ok(JwtToken::from_parts(
                "refreshed_access".to_string(),
                Some("refreshed_refresh".to_string()),
                new_expires,
                None,
                None,
                None,
            ))
        })
        .await;

    assert!(result.is_ok());
    assert!(mgr.is_authenticated());
}

// ===== logout =====

#[valtron_test]
async fn test_logout() {
    let mut mgr = test_manager();
    store_valid_oauth_token(mgr.store()).await;
    mgr.init_from_store().await.unwrap();
    assert!(mgr.is_authenticated());

    mgr.logout(None).await.unwrap();

    assert!(!mgr.is_authenticated());
    assert_eq!(mgr.state(), AuthState::Unauthenticated);
    assert!(mgr.jwt_manager().get_token().is_none());
    assert!(mgr.store().get_oauth_token("default").await.unwrap().is_none());
}

#[valtron_test]
async fn test_logout_with_session_manager() {
    let signing_key = vec![0xAB; 32];
    let session_mgr =
        SessionManager::new(CredentialStorage::memory(), SessionConfig::default(), &signing_key)
            .unwrap();

    let mut mgr = test_manager().with_session_manager(session_mgr);

    // Create a session so the logout has one to revoke.
    let (session, _) = mgr
        .session_manager()
        .unwrap()
        .create_session_async("test_user", None, None)
        .await
        .unwrap();

    store_valid_oauth_token(mgr.store()).await;
    mgr.init_from_store().await.unwrap();

    mgr.logout(Some("test_user")).await.unwrap();
    assert!(!mgr.is_authenticated());
    assert_eq!(mgr.state(), AuthState::Unauthenticated);

    let revoked = mgr
        .session_manager()
        .unwrap()
        .revoke_all_sessions_async("test_user")
        .await
        .unwrap();
    assert_eq!(revoked, 0, "logout already revoked session {}", session.id);
}

// ===== reset =====

#[valtron_test]
async fn test_reset_from_failed() {
    let mut mgr = test_manager();
    let cred = AuthCredential::UsernameAndPassword {
        username: "user".to_string(),
        password: ConfidentialText::new("pass".to_string()),
    };
    assert!(mgr.authenticate(cred).await.is_err());
    assert_eq!(mgr.state(), AuthState::Failed);

    mgr.reset();
    assert_eq!(mgr.state(), AuthState::Unauthenticated);
    assert!(!mgr.is_authenticated());
}

#[valtron_test]
async fn test_reset_from_authenticated() {
    let mut mgr = test_manager();
    store_valid_oauth_token(mgr.store()).await;
    mgr.init_from_store().await.unwrap();
    assert!(mgr.is_authenticated());

    mgr.reset();
    assert!(!mgr.is_authenticated());
}

// ===== builder methods =====

#[test]
fn test_with_jwks_manager() {
    let mgr = test_manager().with_jwks_manager(JwksManager::new(
        "https://example.com/.well-known/jwks.json".to_string(),
        None,
    ));
    assert!(mgr.jwks_manager().is_some());
}

// ===== config =====

#[test]
fn test_config_builder() {
    let config = AuthManagerConfig::new()
        .with_token_storage_key("custom:key")
        .with_refresh_buffer(60);
    assert_eq!(config.token_storage_key, "custom:key");
    assert_eq!(config.refresh_buffer_seconds, 60);
}

#[test]
fn test_config_debug() {
    let config = AuthManagerConfig::new();
    let debug = format!("{config:?}");
    assert!(debug.contains("AuthManagerConfig"));
}
