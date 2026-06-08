//! Foundation Auth - Comprehensive authentication infrastructure.
//!
//! This crate provides authentication flows, credential management, and token handling
//! for use with AI inference providers and other services requiring authentication.

pub mod shared;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(feature = "server")]
pub mod server;

#[cfg(any(feature = "wasm-bindgen-oauth", feature = "wasm-bindgen-session"))]
pub mod wasm_bindgen;

// Re-export shared items at crate root for backward compatibility.
pub use shared::types::{
    AuthCredential, Authenticated, AuthenticationErrors, AuthenticationResult,
    AuthenticationStates, ConfidentialText, JwtCredential, OnAuthData, OAuthCredential,
    AuthProviderEndpoint, SessionCredential,
};
pub use shared::auth_manager::{AuthManager, AuthManagerConfig, AuthManagerError};
pub use shared::auth_state::{AuthEvent, AuthState, AuthStateError, AuthStateMachine, QueuedRequest};
pub use shared::auth_token::AuthToken;
pub use shared::credential_store::{
    AsyncCredentialStore, CredentialStorage, CredentialStore, CredentialStoreError, OAuthState, OAuthTokenStore,
    StoredCredential,
};

// Feature 01: JWT Verifier
pub use shared::jwt::{
    JwtAlgorithm, JwtError, JwtManager, JwtToken, JwtVerifier, JwtVerifierConfig, JwtSigningKey,
    PublicKeySource, VerifiedClaims,
};

// Feature 02: JWKS Manager
pub use shared::jwks::{Jwk, Jwks, JwksError, JwksManager};

// Feature 03: OIDC Discovery
pub use shared::discovery::{DiscoveryClient, DiscoveryError, OidcDiscovery};

// Feature 04: UserInfo Client
pub use shared::userinfo::{UserInfo, UserInfoClient, UserInfoError};

// Feature 06: Token Introspection
pub use shared::introspection::{IntrospectionClient, IntrospectionError, IntrospectionResult};

pub use shared::middleware::{
    extract_bearer_token, extract_session_token, has_scope, optional_auth, require_auth,
    AuthContext, GuardResult,
};
pub use shared::oauth::{OAuthConfig, OAuthConfigBuilder, OAuthError, OAuthManager, PkceChallenge};
pub use shared::oauth::TokenResponse;
pub use shared::oauth_token::OAuthToken;
pub use shared::session::{Session, SessionConfig, SessionError, SessionManager};
pub use shared::two_factor::{BackupCodeSet, TOTPSecret, TwoFactorChallenge, TwoFactorError};

// Re-export platform-specific OAuth wrappers.
#[cfg(not(target_arch = "wasm32"))]
pub use native::oauth::NativeOAuth;

// Feature 05: Password Auth (native only)
#[cfg(not(target_arch = "wasm32"))]
pub use native::password_auth::{
    LoginResult, LoginRequest, MfaType, PasswordAuthClient, PasswordAuthError,
};

#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-oauth"))]
pub use wasm_bindgen::oauth::WasmOAuth;

#[cfg(target_arch = "wasm32")]
#[cfg(feature = "wasm-bindgen-session")]
pub use wasm_bindgen::d1_credential_store::D1CredentialStore;

#[cfg(target_arch = "wasm32")]
#[cfg(feature = "wasm-bindgen-session")]
pub use wasm_bindgen::session::{SessionPayload, WasmSessionManager};
