//! Shared auth modules (wasm32-compatible, pure logic).

/// Constant-time equality of two byte strings.
///
/// The running time depends on the lengths only. A difference in any byte
/// position gives the same work as a difference in any other position, so the
/// comparison does not tell an attacker where the values differ.
///
/// This is the one comparison in the workspace. Use it for every credential
/// comparison; do not write another copy.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// Feature 08: Auth Manager.
pub mod auth_manager;
pub mod auth_session; // Feature 04: Upstream auth session store
pub mod auth_state;
pub mod auth_token;
pub mod body; // Shared cap on error-body reads.
pub mod clock; // Wall-clock conversions from the injected engine clock.
pub mod credential_store;
pub mod discovery; // Feature 03: OIDC Discovery
pub mod introspection; // Feature 06: Token Introspection
pub mod jwks; // Feature 02: JWKS Manager
pub mod jwt;
pub mod middleware;
pub mod oauth;
pub mod oauth_token;
pub mod password_hash; // PBKDF2-HMAC-SHA256 + Argon2id password hashing
pub mod provider; // Feature 01/03: Upstream provider models (cross-platform, pure serde)
pub mod session;
pub mod two_factor;
pub mod types;
pub mod upstream_client; // Feature 03: Unified Upstream OIDC/OAuth2 Client
pub mod userinfo; // Feature 04: UserInfo Client
