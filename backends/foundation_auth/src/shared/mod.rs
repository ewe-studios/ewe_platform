//! Shared auth modules (wasm32-compatible, pure logic).

pub mod auth_state;
pub mod auth_token;
pub mod credential_store;
pub mod discovery;      // Feature 03: OIDC Discovery
pub mod introspection;  // Feature 06: Token Introspection
pub mod jwt;
pub mod jwks;           // Feature 02: JWKS Manager
pub mod middleware;
pub mod oauth;
pub mod oauth_token;
pub mod session;
pub mod types;
pub mod two_factor;
pub mod userinfo;       // Feature 04: UserInfo Client
