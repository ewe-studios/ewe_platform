//! Shared auth modules (wasm32-compatible, pure logic).

pub mod auth_manager;   // Feature 08: Auth Manager
pub mod auth_state;
pub mod auth_session;       // Feature 04: Upstream auth session store
pub mod auth_token;
pub mod credential_store;
pub mod discovery;           // Feature 03: OIDC Discovery
pub mod upstream_client;     // Feature 03: Unified Upstream OIDC/OAuth2 Client
pub mod introspection;  // Feature 06: Token Introspection
pub mod jwt;
pub mod jwks;           // Feature 02: JWKS Manager
pub mod middleware;
pub mod oauth;
pub mod oauth_token;
pub mod password_hash;  // PBKDF2-HMAC-SHA256 + Argon2id password hashing
pub mod provider;       // Feature 01/03: Upstream provider models (cross-platform, pure serde)
pub mod session;
pub mod two_factor;
pub mod types;
pub mod userinfo;       // Feature 04: UserInfo Client
