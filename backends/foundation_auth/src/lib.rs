//! Foundation Auth - Comprehensive authentication infrastructure.
//!
//! This crate provides authentication flows, credential management, and token handling
//! for use with AI inference providers and other services requiring authentication.

pub mod shared;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

// Re-export shared items at crate root for backward compatibility.
pub use shared::types::{
    AuthCredential, Authenticated, AuthenticationErrors, AuthenticationResult,
    AuthenticationStates, ConfidentialText, JwtCredential, OnAuthData, OAuthCredential,
    AuthProviderEndpoint, SessionCredential,
};
pub use shared::auth_state::{AuthEvent, AuthState, AuthStateError, AuthStateMachine, QueuedRequest};
pub use shared::auth_token::AuthToken;
pub use shared::credential_store::{
    CredentialStorage, CredentialStore, CredentialStoreError, OAuthState, OAuthTokenStore,
    StoredCredential,
};
pub use shared::jwt::{JwtError, JwtManager, JwtToken};
pub use shared::middleware::{
    extract_bearer_token, extract_session_token, has_scope, optional_auth, require_auth,
    AuthContext, GuardResult,
};
pub use shared::oauth::{OAuthConfig, OAuthError, PkceChallenge};
pub use shared::oauth_token::OAuthToken;
pub use shared::session::{Session, SessionConfig, SessionError, SessionManager};
pub use shared::two_factor::{BackupCodeSet, TOTPSecret, TwoFactorChallenge, TwoFactorError};

// Re-export native-only items.
#[cfg(not(target_arch = "wasm32"))]
pub use native::oauth::OAuthManager;
