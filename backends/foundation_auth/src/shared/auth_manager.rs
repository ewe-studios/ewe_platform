//! Central authentication lifecycle manager.
//!
//! Coordinates `JwtManager`, `AuthStateMachine`, and `CredentialStorage` into
//! a single coherent interface. Handles authentication, token refresh,
//! persistence, and logout.
//!
//! ## Async-First Design
//!
//! Auth methods are async primary. The CredentialStore already bridges async
//! via valtron internally, so callers in async context call `authenticate().await`
//! directly. Sync callers use `CredentialStorage` (which wraps async via valtron
//! at the store level) — no valtron bridging needed at the AuthManager level.

use crate::shared::credential_store::{
    CredentialStorage, CredentialStoreError, OAuthTokenStore,
};
use crate::shared::jwt::{JwtError, JwtManager, JwtToken};
use crate::shared::oauth_token::OAuthToken;
use crate::shared::types::{
    AuthCredential, AuthenticationErrors, ConfidentialText,
};
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

impl AuthManagerConfig {
    /// Create a new config with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the token storage key.
    #[must_use]
    pub fn with_token_storage_key(mut self, key: impl Into<String>) -> Self {
        self.token_storage_key = key.into();
        self
    }

    /// Set the refresh buffer in seconds.
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
    /// No refresh token available.
    NoRefreshToken,
    /// Token expired.
    TokenExpired,
}

impl core::fmt::Display for AuthManagerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AuthManagerError::Store(e) => write!(f, "Credential store error: {e}"),
            AuthManagerError::Jwt(e) => write!(f, "JWT error: {e}"),
            AuthManagerError::AuthState(e) => write!(f, "Auth state error: {e}"),
            AuthManagerError::Authentication(e) => write!(f, "Authentication failed: {e}"),
            AuthManagerError::NoRefreshToken => write!(f, "No refresh token available"),
            AuthManagerError::TokenExpired => write!(f, "Token expired"),
        }
    }
}

impl std::error::Error for AuthManagerError {}

impl From<CredentialStoreError> for AuthManagerError {
    fn from(e: CredentialStoreError) -> Self {
        AuthManagerError::Store(e)
    }
}

impl From<JwtError> for AuthManagerError {
    fn from(e: JwtError) -> Self {
        AuthManagerError::Jwt(e)
    }
}

impl From<AuthStateError> for AuthManagerError {
    fn from(e: AuthStateError) -> Self {
        AuthManagerError::AuthState(e)
    }
}

impl AuthManager {
    /// Create a new AuthManager.
    #[must_use]
    pub fn new(store: CredentialStorage, config: AuthManagerConfig) -> Self {
        Self {
            store,
            jwt_manager: JwtManager::new(),
            state_machine: AuthStateMachine::new(),
            config,
        }
    }

    /// On application startup — load persisted credential, validate, init state.
    /// Returns the loaded credential if available and valid.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if loading or validation fails.
    pub fn init_from_store(&mut self) -> Result<Option<AuthToken>, AuthManagerError> {
        // Try to load persisted OAuth token
        let token_result = self.store.get_oauth_token("default");
        match token_result {
            Ok(Some(oauth_token)) => {
                // Build a JwtToken for lifetime management
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
                    let _ = self.state_machine.transition_to(AuthEvent::AuthenticateCompleted);
                    return Ok(Some(AuthToken::OAuth {
                        access_token: ConfidentialText::new(oauth_token.access_token),
                        refresh_token: oauth_token.refresh_token.map(ConfidentialText::new),
                        token_type: oauth_token.token_type,
                        expires_at: expires_at as f64,
                        scope: oauth_token.scope,
                    }));
                }

                // Token expired — clear it
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
    /// For other credential types: caller should have wired up the actual
    /// auth service — this returns `InvalidCredentials` until integrated.
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
                // OAuth credential — already has token, just persist
                let token = AuthToken::OAuth {
                    access_token: oauth.access_token.clone(),
                    refresh_token: oauth.refresh_token.clone(),
                    token_type: "Bearer".to_string(),
                    expires_at: oauth.expires,
                    scope: None,
                };
                self.persist_token(&token)?;
                let _ = self.state_machine.transition_to(AuthEvent::AuthenticateCompleted);
                Ok(token)
            }
            // Other credential types need external auth service integration
            _ => {
                let _ = self.state_machine.transition_to(AuthEvent::RefreshFailed);
                Err(AuthManagerError::Authentication(
                    AuthenticationErrors::InvalidCredentials,
                ))
            }
        }
    }

    /// Get valid token — check state machine, return token.
    /// If token is expired/near-expiry, returns `TokenExpired`.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if there is no token or it has expired.
    pub fn get_valid_token(&self) -> Result<AuthToken, AuthManagerError> {
        match self.state_machine.current() {
            AuthState::Authenticated => {
                // Check if token expires within buffer
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
            AuthState::Refreshing => Err(AuthManagerError::TokenExpired),
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
        // Get refresh token
        let Some(refresh_token) = self
            .jwt_manager
            .get_token()
            .and_then(|t| t.refresh_token())
        else {
            return Err(AuthManagerError::NoRefreshToken);
        };

        // Transition to refreshing
        self.state_machine
            .transition_to(AuthEvent::RefreshStarted)?;

        // Call refresh function
        match refresh_fn(refresh_token) {
            Ok(new_token) => {
                self.jwt_manager.set_token(new_token.clone());
                self.persist_jwt_token(&new_token)?;
                let _ = self.state_machine.transition_to(AuthEvent::RefreshCompleted);
                Ok(())
            }
            Err(e) => {
                let _ = self.state_machine.transition_to(AuthEvent::RefreshFailed);
                let _ = self.clear_stored_tokens();
                Err(e)
            }
        }
    }

    /// Logout — clear tokens, reset state machine.
    ///
    /// # Errors
    ///
    /// Returns `AuthManagerError` if store operations fail.
    pub async fn logout(&mut self) -> Result<(), AuthManagerError> {
        // Clear JWT from manager
        self.jwt_manager.clear_token();

        // Clear stored tokens
        self.clear_stored_tokens()?;

        // Reset state machine
        self.state_machine.transition_to(AuthEvent::Logout)?;

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

    // ===========================================================================
    // Internal helpers
    // ===========================================================================

    /// Persist a token to the credential store.
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
                    expires_in: Some((*expires_at as i64 - chrono::Utc::now().timestamp()) as u64),
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

    /// Persist a JWT token to the credential store.
    fn persist_jwt_token(&self, token: &JwtToken) -> Result<(), AuthManagerError> {
        let oauth_token = OAuthToken {
            access_token: token.access_token(),
            token_type: "Bearer".to_string(),
            expires_in: Some(
                (token.expires_at - chrono::Utc::now().timestamp()).max(0) as u64
            ),
            refresh_token: token.refresh_token(),
            scope: token.scope.clone(),
            id_token: None,
        };
        self.store.store_oauth_token("default", &oauth_token)?;
        Ok(())
    }

    /// Clear all stored tokens from the credential store.
    fn clear_stored_tokens(&self) -> Result<(), AuthManagerError> {
        self.store
            .delete_oauth_token("default")
            .map_err(AuthManagerError::Store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_config() -> AuthManagerConfig {
        AuthManagerConfig::new()
    }

    fn test_manager() -> AuthManager {
        let store = CredentialStorage::memory();
        AuthManager::new(store, test_config())
    }

    #[test]
    fn test_initial_state() {
        let mgr = test_manager();
        assert!(!mgr.is_authenticated());
        assert_eq!(mgr.state(), AuthState::Unauthenticated);
    }

    #[test]
    fn test_get_valid_token_no_auth() {
        let mgr = test_manager();
        let result = mgr.get_valid_token();
        assert!(result.is_err());
    }

    #[test]
    fn test_reset() {
        let mut mgr = test_manager();
        mgr.state_machine
            .transition_to(AuthEvent::AuthenticateStarted)
            .unwrap();
        mgr.state_machine
            .transition_to(AuthEvent::AuthenticateCompleted)
            .unwrap();
        assert!(mgr.is_authenticated());
        mgr.reset();
        assert!(!mgr.is_authenticated());
    }
}
