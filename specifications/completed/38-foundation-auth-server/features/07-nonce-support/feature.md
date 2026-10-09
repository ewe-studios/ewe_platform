---
feature: "Nonce Support"
description: "OIDC anti-replay nonce in auth requests, ID token nonce validation"
status: "completed"
priority: "medium"
depends_on: ["01-jwt-verifier"]
estimated_effort: "small"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 1
  uncompleted: 0
  total: 1
  completion_percentage: 100%
---

# Feature 07: Nonce Support

## Description

Add OIDC anti-replay nonce support to the OAuth flow. The nonce prevents authorization code replay attacks by binding the ID token to a specific authorization request.

## Module

`backends/foundation_auth/src/shared/oauth.rs` — extend existing module

## API Surface

### Changes to `OAuthConfig`
```rust
pub struct OAuthConfig {
    // ... existing fields ...
    /// OIDC nonce for anti-replay protection.
    pub nonce: Option<String>,
}

impl OAuthConfigBuilder {
    #[must_use]
    pub fn nonce(mut self, nonce: String) -> Self {
        self.config.nonce = Some(nonce);
        self
    }
}
```

### Changes to authorization URL generation
```rust
impl OAuthManager {
    /// Generate a random nonce for OIDC.
    #[must_use]
    pub fn generate_nonce() -> String;

    // get_authorization_url() extended to include nonce in query params when set
}
```

### Nonce validation in ID token
```rust
impl VerifiedClaims {
    /// Check if the nonce in the ID token matches the expected nonce.
    #[must_use]
    pub fn verify_nonce(&self, expected_nonce: &str) -> bool;
}
```

## Implementation Details

### Nonce generation
- 32 random bytes, base64url encoded (same pattern as `generate_state()`)
- Stored alongside OAuth state for later validation

### Authorization URL
- When `nonce` is set on `OAuthConfig`, append `nonce=<value>` to query params
- Only relevant for OIDC (response_type=code, scope includes openid)

### ID token validation
- After verifying JWT signature, extract `nonce` claim from ID token
- Compare with expected nonce (stored from auth request)
- Mismatch → reject the token

### Where nonce is stored
- Alongside `OAuthState` in credential store: key = `oauth:nonce:{state}`
- Value = nonce string
- Expired when OAuth state expires

## Dependencies

- No new dependencies — uses existing `rand`, `base64`

## Testing

- Nonce included in authorization URL when set
- Nonce NOT included in URL when None
- `verify_nonce` matches → true
- `verify_nonce` mismatch → false
- Nonce generation produces unique values
