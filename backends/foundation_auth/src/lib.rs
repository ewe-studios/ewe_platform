//! Foundation Auth - Comprehensive authentication infrastructure.
//!
//! This crate provides authentication flows, credential management, and token handling
//! for use with AI inference providers and other services requiring authentication.

// The `Send` check on the deeply nested handler futures needs a deeper limit.
#![recursion_limit = "512"]

pub mod shared;

pub mod server_credential;

/// Lowercase hexadecimal SHA-256 fingerprints (spec 81).
pub mod fingerprint;

/// Signed secrets: the issuer signs with a private key, the verifier checks
/// with public keys only (spec 81, T5).
#[cfg(feature = "secret")]
pub mod secret;

/// Operator key files: OpenSSH, PKCS#8 and SEC1 PEM (spec 81, T7).
#[cfg(feature = "secret")]
pub mod keys;

#[cfg(not(target_family = "wasm"))]
pub mod native;

#[cfg(feature = "server")]
pub mod server;

#[cfg(any(feature = "wasm-bindgen-oauth", feature = "wasm-bindgen-session"))]
pub mod wasm_bindgen;

// Re-export shared items at crate root for backward compatibility.
pub use shared::auth_manager::{AuthManager, AuthManagerConfig, AuthManagerError};
pub use shared::auth_state::{
    AuthEvent, AuthState, AuthStateError, AuthStateMachine, QueuedRequest,
};
pub use shared::auth_token::AuthToken;
pub use shared::credential_store::{
    CredentialStorage, CredentialStore, CredentialStoreError, OAuthState, OAuthTokenStore,
    StoredCredential,
};
pub use shared::types::{
    AuthCredential, AuthProviderEndpoint, Authenticated, AuthenticationErrors,
    AuthenticationResult, AuthenticationStates, JwtCredential, OAuthCredential, OnAuthData,
    SessionCredential,
};

// Feature 01: JWT Verifier
pub use shared::jwt::{
    JwtAlgorithm, JwtError, JwtManager, JwtSigningKey, JwtToken, JwtVerifier, JwtVerifierConfig,
    PublicKeySource, VerifiedClaims,
};

// Feature 02: JWKS Manager
pub use shared::jwks::{Jwk, Jwks, JwksError, JwksManager};

// Feature 03: OIDC Discovery
pub use shared::discovery::{DiscoveryClient, DiscoveryError, OidcDiscovery};

// Feature 03: Unified Upstream OIDC/OAuth2 Client (cross-platform — native + wasm)
pub use shared::upstream_client::{
    GithubEmail, UpstreamClientError, UpstreamOidcClient, UpstreamProfile,
};

// Feature 01/03: Upstream provider models (cross-platform, pure serde)
pub use shared::provider::{ProviderMapping, ProviderType, ProviderUpdate, UpstreamProvider};

// Feature 04: UserInfo Client
pub use shared::userinfo::{UserInfo, UserInfoClient, UserInfoError};

// Feature 06: Token Introspection
pub use shared::introspection::{IntrospectionClient, IntrospectionError, IntrospectionResult};

pub use shared::auth_session::{
    AuthSessionError, UpstreamAuthSession, UpstreamAuthSessionStore, DEFAULT_AUTH_SESSION_TTL_SECS,
};
pub use shared::middleware::{
    extract_bearer_token, extract_session_token, has_scope, optional_auth, require_auth,
    AuthContext, GuardResult,
};

pub use shared::oauth::TokenResponse;
pub use shared::oauth::{OAuthConfig, OAuthConfigBuilder, OAuthError, OAuthManager, PkceChallenge};
pub use shared::oauth_token::OAuthToken;
pub use shared::session::{Session, SessionConfig, SessionError, SessionManager};
pub use shared::two_factor::{
    BackupCodeSet, TOTPSecret, TotpAlgorithm, TwoFactorChallenge, TwoFactorError,
};

// Auth primitives shared by every server (spec 86).
pub use server_credential::{
    bearer_header, AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth,
    Principal, Secret, StaticToken,
};

// Mutual TLS (spec 87): the certificate authority, the trust store, the rustls
// verifier and the native mTLS provider. The `mtls` feature names them: they
// link `ring`, which has C code, so a consumer that must stay free of C code
// leaves the feature off.
#[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
pub mod mtls;
#[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
pub mod pki;
pub mod trust;

// The client-authentication providers of spec 87.
pub mod client_auth;
// One constant-time comparison for the whole workspace (spec 86).
pub use shared::constant_time_eq;

/// The old name of [`Secret`]. It is an alias for one release, then it is
/// removed.
pub use server_credential::ConfidentialText;

// Re-export platform-specific OAuth wrappers.
#[cfg(not(target_family = "wasm"))]
pub use native::oauth::NativeOAuth;

// Feature 05: Password Auth (native only)
#[cfg(not(target_family = "wasm"))]
pub use native::password_auth::{
    LoginRequest, LoginResult, MfaType, PasswordAuthClient, PasswordAuthError,
};

#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-oauth"))]
pub use wasm_bindgen::oauth::WasmOAuth;

#[cfg(target_family = "wasm")]
#[cfg(feature = "wasm-bindgen-session")]
pub use wasm_bindgen::d1_credential_store::D1CredentialStore;

#[cfg(target_family = "wasm")]
#[cfg(feature = "wasm-bindgen-session")]
pub use wasm_bindgen::session::{SessionPayload, WasmSessionManager};
