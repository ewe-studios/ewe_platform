//! Central authentication lifecycle manager.
//!
//! Coordinates `JwtManager`, `SessionManager`, `AuthStateMachine`, and `CredentialStorage`
//! into a single coherent interface. Handles authentication, token refresh,
//! persistence, and logout.

use crate::shared::credential_store::{CredentialStorage, CredentialStoreError, OAuthTokenStore};
use crate::shared::jwt::{JwtError, JwtManager, JwtToken, JwtVerifier};
use crate::shared::jwks::{JwksError, JwksManager};
use crate::shared::oauth_token::OAuthToken;
use crate::shared::session::{SessionError, SessionManager};
use crate::shared::types::{AuthCredential, AuthenticationErrors, ConfidentialText};
use crate::AuthToken;

use super::auth_state::{AuthEvent, AuthState, AuthStateError, AuthStateMachine};

/// Configuration for [`AuthManager`].
pub struct AuthManagerConfig {
    /// Key for persisting JWT tokens.
    pub token_storage_key: String,
    /// Buffer seconds before expiry to trigger refresh (default: 300s = 5 min).
    pub refresh_buffer_seconds: i64,
}

impl Default for AuthManagerConfig {
    fn default() -> Self {
        Self {
            token_storage_key: String::from("jwt:token"),
            refresh_buffer_seconds: 300,
        }
    }
}

impl core::fmt::Debug for AuthManagerConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AuthManagerConfig")
            .field("token_storage_key", &self.token_storage_key)
            .field("refresh_buffer_seconds", &self.refresh_buffer_seconds)
            .finish()
    }
}

impl AuthManagerConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_token_storage_key(mut self, key: impl Into<String>) -> Self {
        self.token_storage_key = key.into();
        self
    }

    #[must_use]
    pub fn with_refresh_buffer(mut self, seconds: i64) -> Self {
        self.refresh_buffer_seconds = seconds;
        self
    }
}

/// Central authentication lifecycle manager.
pub struct AuthManager {
    store: CredentialStorage,
    jwt_manager: JwtManager,
    state_machine: AuthStateMachine,
    config: AuthManagerConfig,
    session_mgr: Option<SessionManager<CredentialStorage>>,
    jwt_verifier: Option<JwtVerifier>,
    jwks_manager: Option<JwksManager>,
}

/// Auth manager error type.
#[derive(Debug)]
pub enum AuthManagerError {
    /// Credential store error.
    Store(CredentialStoreError),
    /// JWT error.
    Jwt(JwtError),
    /// Auth state machine error.
    AuthState(AuthStateError),
    /// Authentication failed.
    Authentication(AuthenticationErrors),
    /// Session management error.
    Session(SessionError),
    /// JWKS error.
    Jwks(JwksError),
    /// No refresh token available.
    NoRefreshToken,
    /// Token expired.
    TokenExpired,
}

impl core::fmt::Display for AuthManagerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "Credential store error: {e}"),
            Self::Jwt(e) => write!(f, "JWT error: {e}"),
            Self::AuthState(e) => write!(f, "Auth state error: {e}"),
            Self::Authentication(e) => write!(f, "Authentication failed: {e}"),
            Self::Session(e) => write!(f, "Session error: {e}"),
            Self::Jwks(e) => write!(f, "JWKS error: {e}"),
            Self::NoRefreshToken => write!(f, "No refresh token available"),
            Self::TokenExpired => write!(f, "Token expired"),
        }
    }
}

impl std::error::Error for AuthManagerError {}

impl From<CredentialStoreError> for AuthManagerError {
    fn from(e: CredentialStoreError) -> Self {
        Self::Store(e)
    }
}

impl From<JwtError> for AuthManagerError {
    fn from(e: JwtError) -> Self {
        Self::Jwt(e)
    }
}

impl From<AuthStateError> for AuthManagerError {
    fn from(e: AuthStateError) -> Self {
        Self::AuthState(e)
    }
}

impl From<SessionError> for AuthManagerError {
    fn from(e: SessionError) -> Self {
        Self::Session(e)
    }
}

impl From<JwksError> for AuthManagerError {
    fn from(e: JwksError) -> Self {
        Self::Jwks(e)
    }
}

impl AuthManager {
    /// Create a new AuthManager.
    #[must_use]
    pub fn new(store: CredentialStorage, config: AuthManagerConfig) -> Self {
        let jwt_manager =
            JwtManager::new().with_refresh_buffer(config.refresh_buffer_seconds);
        Self {
            store,
            jwt_manager,
            state_machine: AuthStateMachine::new(),
            config,
            session_mgr: None,
            jwt_verifier: None,
            jwks_manager: None,
        }
    }

    /// Attach a session manager for session-based auth support.
    #[must_use]
    pub fn with_session_manager(mut self, session_mgr: SessionManager<CredentialStorage>) -> Self {
        self.session_mgr = Some(session_mgr);
        self
    }

    /// Attach a JWT verifier for token signature validation.
    #[must_use]
    pub fn with_jwt_verifier(mut self, verifier: JwtVerifier) -> Self {
        self.jwt_verifier = Some(verifier);
        self
    }

    /// Attach a JWKS manager for key rotation and remote key fetching.
    #[must_use]
    pub fn with_jwks_manager(mut self, manager: JwksManager) -> Self {
        self.jwks_manager = Some(manager);
        self
    }

    /// On application startup — load persisted credential, validate, init state.
    ///
    /// If a valid token is found, transitions to `Authenticated` and returns it.
    /// If the token is expired but a refresh token exists, transitions to `TokenExpired`
    /// (caller should then call `refresh()`).
    /// If no token is found or it's expired with no refresh token, stays `Unauthenticated`.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if loading from the store fails.
    pub fn init_from_store(&mut self) -> Result<Option<AuthToken>, AuthManagerError> {
        let token_result = self.store.get_oauth_token("default");
        match token_result {
            Ok(Some(oauth_token)) => {
                let expires_at = oauth_token
                    .expires_in
                    .map(|exp| chrono::Utc::now().timestamp() + exp as i64)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp() + 3600);

                let jwt_token = JwtToken::from_parts(
                    oauth_token.access_token.clone(),
                    oauth_token.refresh_token.clone(),
                    expires_at,
                    oauth_token.scope.clone(),
                    None,
                    None,
                );

                if !jwt_token.is_expired() {
                    self.jwt_manager.set_token(jwt_token);
                    let _ =
                        self.state_machine
                            .transition_to(AuthEvent::AuthenticateStarted);
                    let _ =
                        self.state_machine
                            .transition_to(AuthEvent::AuthenticateCompleted);
                    return Ok(Some(AuthToken::OAuth {
                        access_token: ConfidentialText::new(oauth_token.access_token),
                        refresh_token: oauth_token.refresh_token.map(ConfidentialText::new),
                        token_type: oauth_token.token_type,
                        expires_at: expires_at as f64,
                        scope: oauth_token.scope,
                    }));
                }

                // Token expired — keep refresh token available if present
                if oauth_token.refresh_token.is_some() {
                    self.jwt_manager.set_token(jwt_token);
                    let _ =
                        self.state_machine
                            .transition_to(AuthEvent::AuthenticateStarted);
                    let _ =
                        self.state_machine
                            .transition_to(AuthEvent::AuthenticateCompleted);
                    let _ = self.state_machine.transition_to(AuthEvent::TokenExpired);
                    return Ok(None);
                }

                // Expired with no refresh token — clear everything
                let _ = self.clear_stored_tokens();
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AuthManagerError::Store(e)),
        }
    }

    /// Authenticate — login, persist credential, transition to Authenticated.
    ///
    /// For OAuth credentials: already has token, just persist.
    /// For other credential types: returns `InvalidCredentials` (external auth
    /// services must be called before constructing the `AuthCredential`).
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if authentication fails or state transition
    /// is invalid.
    pub async fn authenticate(
        &mut self,
        credential: AuthCredential,
    ) -> Result<AuthToken, AuthManagerError> {
        self.state_machine
            .transition_to(AuthEvent::AuthenticateStarted)?;

        match credential {
            AuthCredential::OAuth(oauth) => {
                let token = AuthToken::OAuth {
                    access_token: oauth.access_token.clone(),
                    refresh_token: oauth.refresh_token.clone(),
                    token_type: "Bearer".to_string(),
                    expires_at: oauth.expires,
                    scope: None,
                };
                self.persist_token(&token)?;

                // Build JwtToken for lifecycle tracking
                let jwt_token = JwtToken::from_parts(
                    oauth.access_token.get(),
                    oauth.refresh_token.as_ref().map(|rt| rt.get()),
                    oauth.expires as i64,
                    None,
                    None,
                    None,
                );
                self.jwt_manager.set_token(jwt_token);

                self.state_machine
                    .transition_to(AuthEvent::AuthenticateCompleted)?;
                Ok(token)
            }
            _ => {
                let _ = self.state_machine.transition_to(AuthEvent::RefreshFailed);
                Err(AuthManagerError::Authentication(
                    AuthenticationErrors::InvalidCredentials,
                ))
            }
        }
    }

    /// Get valid token — check state machine, return token.
    ///
    /// If the token is expired or near-expiry (within refresh buffer), returns
    /// `TokenExpired` so the caller can call `refresh()`.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if there is no token or it has expired.
    pub fn get_valid_token(&self) -> Result<AuthToken, AuthManagerError> {
        match self.state_machine.current() {
            AuthState::Authenticated => {
                if let Some(token) = self.jwt_manager.get_token() {
                    if token.expires_within(self.config.refresh_buffer_seconds) {
                        return Err(AuthManagerError::TokenExpired);
                    }
                    return Ok(AuthToken::OAuth {
                        access_token: ConfidentialText::new(token.access_token()),
                        refresh_token: token.refresh_token().map(ConfidentialText::new),
                        token_type: "Bearer".to_string(),
                        expires_at: token.expires_at as f64,
                        scope: token.scope.clone(),
                    });
                }
                Err(AuthManagerError::NoRefreshToken)
            }
            AuthState::TokenExpired | AuthState::Refreshing => {
                Err(AuthManagerError::TokenExpired)
            }
            _ => Err(AuthManagerError::Authentication(
                AuthenticationErrors::InvalidCredentials,
            )),
        }
    }

    /// Refresh — refresh JWT, persist new refresh token, handle rotation.
    ///
    /// The caller provides the refresh function since the actual HTTP call
    /// depends on the OAuth provider.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if there is no refresh token or the refresh
    /// function fails.
    pub async fn refresh<F>(&mut self, refresh_fn: F) -> Result<(), AuthManagerError>
    where
        F: FnOnce(String) -> Result<JwtToken, AuthManagerError>,
    {
        let Some(refresh_token) = self
            .jwt_manager
            .get_token()
            .and_then(|t| t.refresh_token())
        else {
            return Err(AuthManagerError::NoRefreshToken);
        };

        // Transition to Refreshing (from Authenticated or TokenExpired)
        self.state_machine
            .transition_to(AuthEvent::TokenExpired)
            .ok();
        self.state_machine
            .transition_to(AuthEvent::RefreshStarted)?;

        match refresh_fn(refresh_token) {
            Ok(new_token) => {
                self.jwt_manager.set_token(new_token.clone());
                self.persist_jwt_token(&new_token)?;
                let _ = self
                    .state_machine
                    .handle_event(AuthEvent::RefreshCompleted);
                Ok(())
            }
            Err(e) => {
                let _ = self.state_machine.transition_to(AuthEvent::RefreshFailed);
                let _ = self.clear_stored_tokens();
                self.jwt_manager.clear_token();
                Err(e)
            }
        }
    }

    /// Logout — clear tokens, reset state machine.
    ///
    /// If a user ID is known, also revokes all sessions via the session manager.
    /// Pass `None` to skip session revocation.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if store or session operations fail.
    pub async fn logout(&mut self, user_id: Option<&str>) -> Result<(), AuthManagerError> {
        if let (Some(ref session_mgr), Some(uid)) = (&self.session_mgr, user_id) {
            session_mgr.revoke_all_sessions(uid)?;
        }

        self.jwt_manager.clear_token();
        self.clear_stored_tokens()?;
        let _ = self.state_machine.transition_to(AuthEvent::Logout);

        Ok(())
    }

    /// Check if currently authenticated.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.state_machine.current() == AuthState::Authenticated
    }

    /// Get the current auth state.
    #[must_use]
    pub fn state(&self) -> AuthState {
        self.state_machine.current()
    }

    /// Reset the auth state (for retry after failure).
    pub fn reset(&mut self) {
        self.state_machine.reset();
    }

    /// Borrow the credential store.
    #[must_use]
    pub fn store(&self) -> &CredentialStorage {
        &self.store
    }

    /// Borrow the JWT verifier (if configured).
    #[must_use]
    pub fn jwt_verifier(&self) -> Option<&JwtVerifier> {
        self.jwt_verifier.as_ref()
    }

    /// Borrow the JWKS manager (if configured).
    #[must_use]
    pub fn jwks_manager(&self) -> Option<&JwksManager> {
        self.jwks_manager.as_ref()
    }

    /// Mutably borrow the JWKS manager (if configured).
    pub fn jwks_manager_mut(&mut self) -> Option<&mut JwksManager> {
        self.jwks_manager.as_mut()
    }

    /// Borrow the session manager (if configured).
    #[must_use]
    pub fn session_manager(&self) -> Option<&SessionManager<CredentialStorage>> {
        self.session_mgr.as_ref()
    }

    // ===========================================================================
    // Internal helpers
    // ===========================================================================

    fn persist_token(&self, token: &AuthToken) -> Result<(), AuthManagerError> {
        match token {
            AuthToken::OAuth {
                access_token,
                refresh_token,
                expires_at,
                scope,
                ..
            } => {
                let oauth_token = OAuthToken {
                    access_token: access_token.get(),
                    token_type: "Bearer".to_string(),
                    expires_in: Some(
                        (*expires_at as i64 - chrono::Utc::now().timestamp()).max(0) as u64,
                    ),
                    refresh_token: refresh_token.as_ref().map(|rt| rt.get()),
                    scope: scope.clone(),
                    id_token: None,
                };
                self.store.store_oauth_token("default", &oauth_token)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn persist_jwt_token(&self, token: &JwtToken) -> Result<(), AuthManagerError> {
        let oauth_token = OAuthToken {
            access_token: token.access_token(),
            token_type: "Bearer".to_string(),
            expires_in: Some(
                (token.expires_at - chrono::Utc::now().timestamp()).max(0) as u64,
            ),
            refresh_token: token.refresh_token(),
            scope: token.scope.clone(),
            id_token: None,
        };
        self.store.store_oauth_token("default", &oauth_token)?;
        Ok(())
    }

    fn clear_stored_tokens(&self) -> Result<(), AuthManagerError> {
        self.store
            .delete_oauth_token("default")
            .map_err(AuthManagerError::Store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::session::SessionConfig;

    fn init_valtron() {
        foundation_core::valtron::single::initialize_pool(42);
    }

    fn test_config() -> AuthManagerConfig {
        AuthManagerConfig::new().with_refresh_buffer(300)
    }

    fn test_manager() -> AuthManager {
        init_valtron();
        let store = CredentialStorage::memory();
        AuthManager::new(store, test_config())
    }

    /// Drive a future that completes synchronously (no real async I/O).
    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        let mut f = std::pin::pin!(f);
        let waker = noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        match f.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("future returned Pending — expected sync completion"),
        }
    }

    fn noop_waker() -> std::task::Waker {
        use std::task::{RawWaker, RawWakerVTable, Waker};
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, no_op, no_op, no_op),
            )
        }
        unsafe {
            Waker::from_raw(RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, no_op, no_op, no_op),
            ))
        }
    }

    fn store_valid_oauth_token(store: &CredentialStorage) {
        let token = OAuthToken {
            access_token: "valid_access_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("valid_refresh_token".to_string()),
            scope: Some("openid profile".to_string()),
            id_token: None,
        };
        store.store_oauth_token("default", &token).unwrap();
    }

    fn store_expired_oauth_token(store: &CredentialStorage, with_refresh: bool) {
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
        store.store_oauth_token("default", &token).unwrap();
    }

    // ===== Initial state =====

    #[test]
    fn test_initial_state() {
        let mgr = test_manager();
        assert!(!mgr.is_authenticated());
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
    }

    // ===== init_from_store =====

    #[test]
    fn test_init_from_store_valid_token() {
        let mut mgr = test_manager();
        store_valid_oauth_token(&mgr.store);

        let result = mgr.init_from_store().unwrap();
        assert!(result.is_some());
        assert!(mgr.is_authenticated());
        assert_eq!(mgr.state(), AuthState::Authenticated);

        let token = result.unwrap();
        assert!(!token.is_expired());
    }

    #[test]
    fn test_init_from_store_expired_with_refresh() {
        let mut mgr = test_manager();
        store_expired_oauth_token(&mgr.store, true);

        let result = mgr.init_from_store().unwrap();
        assert!(result.is_none());
        assert_eq!(mgr.state(), AuthState::TokenExpired);
        assert!(mgr.jwt_manager.get_token().is_some());
    }

    #[test]
    fn test_init_from_store_expired_no_refresh() {
        let mut mgr = test_manager();
        store_expired_oauth_token(&mgr.store, false);

        let result = mgr.init_from_store().unwrap();
        assert!(result.is_none());
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
    }

    #[test]
    fn test_init_from_store_empty() {
        let mut mgr = test_manager();
        let result = mgr.init_from_store().unwrap();
        assert!(result.is_none());
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
    }

    // ===== authenticate =====

    #[test]
    fn test_authenticate_oauth() {
        let mut mgr = test_manager();
        let expires = chrono::Utc::now().timestamp() as f64 + 3600.0;
        let cred = AuthCredential::OAuth(crate::OAuthCredential {
            expires,
            access_token: ConfidentialText::new("oauth_access".to_string()),
            refresh_token: Some(ConfidentialText::new("oauth_refresh".to_string())),
        });

        let result = block_on(mgr.authenticate(cred));
        assert!(result.is_ok());
        assert!(mgr.is_authenticated());
        assert_eq!(mgr.state(), AuthState::Authenticated);

        // Token should be persisted
        let stored = mgr.store.get_oauth_token("default").unwrap();
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().access_token, "oauth_access");
    }

    #[test]
    fn test_authenticate_unsupported_credential() {
        let mut mgr = test_manager();
        let cred = AuthCredential::UsernameAndPassword {
            username: "user".to_string(),
            password: ConfidentialText::new("pass".to_string()),
        };

        let result = block_on(mgr.authenticate(cred));
        assert!(result.is_err());
        assert_eq!(mgr.state(), AuthState::Failed);
    }

    // ===== get_valid_token =====

    #[test]
    fn test_get_valid_token_authenticated() {
        let mut mgr = test_manager();
        store_valid_oauth_token(&mgr.store);
        mgr.init_from_store().unwrap();

        let result = mgr.get_valid_token();
        assert!(result.is_ok());
        let token = result.unwrap();
        assert!(!token.is_expired());
    }

    #[test]
    fn test_get_valid_token_not_authenticated() {
        let mgr = test_manager();
        let result = mgr.get_valid_token();
        assert!(result.is_err());
    }

    #[test]
    fn test_get_valid_token_expired() {
        let mut mgr = test_manager();

        // Manually set up an authenticated state with a near-expiry token
        let soon = chrono::Utc::now().timestamp() + 60; // 1 minute from now
        mgr.jwt_manager.set_token(JwtToken::from_parts(
            "expiring_token".to_string(),
            Some("refresh".to_string()),
            soon,
            None,
            None,
            None,
        ));
        let _ = mgr
            .state_machine
            .transition_to(AuthEvent::AuthenticateStarted);
        let _ = mgr
            .state_machine
            .transition_to(AuthEvent::AuthenticateCompleted);

        // Token within refresh buffer (300s) — should return TokenExpired
        let result = mgr.get_valid_token();
        assert!(matches!(result, Err(AuthManagerError::TokenExpired)));
    }

    #[test]
    fn test_get_valid_token_during_refresh() {
        let mut mgr = test_manager();
        let _ = mgr
            .state_machine
            .transition_to(AuthEvent::AuthenticateStarted);
        let _ = mgr
            .state_machine
            .transition_to(AuthEvent::AuthenticateCompleted);
        let _ = mgr.state_machine.transition_to(AuthEvent::TokenExpired);
        let _ = mgr.state_machine.transition_to(AuthEvent::RefreshStarted);

        let result = mgr.get_valid_token();
        assert!(matches!(result, Err(AuthManagerError::TokenExpired)));
    }

    // ===== refresh =====

    #[test]
    fn test_refresh_success() {
        let mut mgr = test_manager();
        store_valid_oauth_token(&mgr.store);
        mgr.init_from_store().unwrap();

        let new_expires = chrono::Utc::now().timestamp() + 7200;
        let result = block_on(mgr.refresh(|_old_refresh| {
            Ok(JwtToken::from_parts(
                "new_access".to_string(),
                Some("new_refresh".to_string()),
                new_expires,
                None,
                None,
                None,
            ))
        }));

        assert!(result.is_ok());
        assert!(mgr.is_authenticated());
        assert_eq!(
            mgr.jwt_manager.get_token().unwrap().access_token(),
            "new_access"
        );
    }

    #[test]
    fn test_refresh_failure() {
        let mut mgr = test_manager();
        store_valid_oauth_token(&mgr.store);
        mgr.init_from_store().unwrap();

        let result = block_on(mgr.refresh(|_| {
            Err(AuthManagerError::Jwt(JwtError::RefreshFailed(
                "server error".to_string(),
            )))
        }));

        assert!(result.is_err());
        assert_eq!(mgr.state(), AuthState::Failed);
        assert!(mgr.jwt_manager.get_token().is_none());
    }

    #[test]
    fn test_refresh_no_token() {
        let mut mgr = test_manager();
        let result = block_on(mgr.refresh(|_| unreachable!()));
        assert!(matches!(result, Err(AuthManagerError::NoRefreshToken)));
    }

    #[test]
    fn test_refresh_from_token_expired_state() {
        let mut mgr = test_manager();
        store_expired_oauth_token(&mgr.store, true);
        mgr.init_from_store().unwrap();
        assert_eq!(mgr.state(), AuthState::TokenExpired);

        let new_expires = chrono::Utc::now().timestamp() + 7200;
        let result = block_on(mgr.refresh(|old_refresh| {
            assert_eq!(old_refresh, "expired_refresh_token");
            Ok(JwtToken::from_parts(
                "refreshed_access".to_string(),
                Some("refreshed_refresh".to_string()),
                new_expires,
                None,
                None,
                None,
            ))
        }));

        assert!(result.is_ok());
        assert!(mgr.is_authenticated());
    }

    // ===== logout =====

    #[test]
    fn test_logout() {
        let mut mgr = test_manager();
        store_valid_oauth_token(&mgr.store);
        mgr.init_from_store().unwrap();
        assert!(mgr.is_authenticated());

        block_on(mgr.logout(None)).unwrap();

        assert!(!mgr.is_authenticated());
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
        assert!(mgr.jwt_manager.get_token().is_none());

        // Stored tokens should be cleared
        let stored = mgr.store.get_oauth_token("default").unwrap();
        assert!(stored.is_none());
    }

    #[test]
    fn test_logout_with_session_manager() {
        init_valtron();
        let store = CredentialStorage::memory();
        let session_store = CredentialStorage::memory();
        let signing_key = vec![0xAB; 32];
        let session_mgr =
            SessionManager::new(session_store, SessionConfig::default(), &signing_key).unwrap();

        let mut mgr = AuthManager::new(store, test_config()).with_session_manager(session_mgr);

        // Create a session first so we can verify revocation
        mgr.session_mgr
            .as_ref()
            .unwrap()
            .create_session("test_user", None, None)
            .unwrap();

        store_valid_oauth_token(&mgr.store);
        mgr.init_from_store().unwrap();

        block_on(mgr.logout(Some("test_user"))).unwrap();
        assert!(!mgr.is_authenticated());
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
    }

    // ===== reset =====

    #[test]
    fn test_reset_from_failed() {
        let mut mgr = test_manager();
        let _ = mgr
            .state_machine
            .transition_to(AuthEvent::AuthenticateStarted);
        let _ = mgr.state_machine.transition_to(AuthEvent::RefreshFailed);
        assert_eq!(mgr.state(), AuthState::Failed);

        mgr.reset();
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
        assert!(!mgr.is_authenticated());
    }

    #[test]
    fn test_reset_from_authenticated() {
        let mut mgr = test_manager();
        store_valid_oauth_token(&mgr.store);
        mgr.init_from_store().unwrap();
        assert!(mgr.is_authenticated());

        mgr.reset();
        assert!(!mgr.is_authenticated());
    }

    // ===== builder methods =====

    #[test]
    fn test_with_jwks_manager() {
        let mgr = test_manager()
            .with_jwks_manager(JwksManager::new("https://example.com/.well-known/jwks.json".to_string(), None));
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
}
