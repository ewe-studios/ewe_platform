---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
this_file: "specifications/07-foundation-ai/features/00-openai-provider/start.md"
created: 2026-03-20
author: "Main Agent"
---

# Start: OpenAI-Compatible HTTP Provider

## Feature Overview

This feature implements an OpenAI-compatible HTTP provider with comprehensive authentication infrastructure including JWT, OAuth 2.0, API keys, and session-based authentication.

## Required Reading (Before Implementation)

1. **Read `.agents/skills/rust-valtron-usage/skill.md`** — Valtron execution model, stream-returning patterns, sync boundary helpers. MANDATORY — this feature heavily uses `from_future` + `execute` for all HTTP I/O. Methods must return streams, not block.
2. **Read `feature.md`** — Full requirements, Iron Laws, task list.
3. **Read `../../LEARNINGS.md`** — Spec-level learnings including `from_future` patterns.

## Prerequisites

Before starting, ensure you understand:
1. The `foundation_auth` crate structure and existing types
2. The `foundation_core::simple_http` and `foundation_core::event_source` APIs
3. OAuth 2.0 and JWT authentication flows
4. SSE (Server-Sent Events) parsing

## Agent Workflow

### Phase 1: Authentication Infrastructure (foundation_auth)

1. **Read existing foundation_auth**
   - Read: `backends/foundation_auth/src/lib.rs`
   - Read: `backends/foundation_auth/Cargo.toml`
   - Understand existing `AuthCredential`, `AuthenticationErrors`, `Authenticated` types

2. **Create JWT Manager**
   - Create: `backends/foundation_auth/src/jwt.rs`
   - Implement: `JWTManager`, `JWTToken` with automatic refresh
   - Update: `backends/foundation_auth/src/lib.rs` to export new types
   - Update: `backends/foundation_auth/Cargo.toml` if new dependencies needed

3. **Create OAuth Manager**
   - Create: `backends/foundation_auth/src/oauth.rs`
   - Implement: Authorization code flow, client credentials flow, PKCE
   - Test: OAuth URL generation and token exchange parsing

4. **Create Credential Store**
   - Create: `backends/foundation_auth/src/credential_store.rs`
   - Implement: `InMemoryCredentialStore` with `Zeroizing`
   - Test: Secure storage and zeroizing deletion

5. **Create Auth State Machine**
   - Create: `backends/foundation_auth/src/auth_state.rs`
   - Implement: State transitions, concurrent refresh handling
   - Test: State machine with mock flows

6. **Create Two-Factor Handler** (Optional)
   - Create: `backends/foundation_auth/src/two_factor.rs`
   - Implement: TOTP generation and challenge response
   - Test: TOTP code generation and validation

7. **Extend foundation_auth Types**
   - Update: `backends/foundation_auth/src/lib.rs` with new exports
   - Update: `AuthCredential` enum if new variants needed
   - Update: `AuthenticationErrors` with more specific types
   - Test: `cargo test --package foundation_auth`

### Phase 2: OpenAI Provider Implementation (foundation_ai)

8. **Read existing foundation_ai**
   - Read: `backends/foundation_ai/src/types/mod.rs`
   - Read: `backends/foundation_ai/src/errors/mod.rs`
   - Read: `backends/foundation_ai/src/backends/mod.rs`
   - Read: `backends/foundation_ai/src/backends/huggingface.rs` (existing pattern)

9. **Create OpenAI Provider Core**
   - Create: `backends/foundation_ai/src/backends/openai_provider.rs`
   - Implement: `OpenAIProvider`, `OpenAIConfig`, `OpenAIModel`
   - Implement: `ModelProvider` trait
   - Test: Provider creation with mock credentials

10. **Create HTTP Helpers**
    - Create: `backends/foundation_ai/src/backends/openai_helpers.rs`
    - Implement: Request/response types, error mapping, SSE parsing
    - Test: Response parsing with mock JSON

11. **Implement API Endpoints**
    - Implement: `/v1/chat/completions` request/response
    - Implement: `/v1/embeddings` request/response
    - Implement: `/v1/models` endpoint
    - Test: Each endpoint with mock server

12. **Implement Streaming**
    - Implement: `OpenAIStream` with `StreamIterator`
    - Integrate: `foundation_core::event_source::SSEParser`
    - Test: SSE parsing with mock stream

13. **Extend Error Types**
    - Update: `backends/foundation_ai/src/errors/mod.rs`
    - Add: HTTP-specific error variants
    - Test: Error conversion and Display impls

14. **Integration Tests**
    - Create: `backends/foundation_ai/tests/openai_tests.rs`
    - Test: Full authentication + request flows
    - Test: Error handling and retries
    - Test: Streaming end-to-end

### Phase 3: Verification

15. **Run Verification Commands**
    ```bash
    # foundation_auth
    cargo check --package foundation_auth
    cargo clippy --package foundation_auth -- -D warnings
    cargo test --package foundation_auth

    # foundation_ai
    cargo check --package foundation_ai
    cargo clippy --package foundation_ai -- -D warnings
    cargo test --package foundation_ai
    cargo fmt --package foundation_ai -- --check
    ```

16. **Update LEARNINGS.md**
    - Document authentication design decisions
    - Document OAuth flow implementation details
    - Document SSE parsing approach
    - Add any gotchas or lessons learned

## File Checklist

### foundation_auth (Authentication Infrastructure)
- [ ] `src/jwt.rs` - JWT manager and token handling
- [ ] `src/oauth.rs` - OAuth flows and PKCE
- [ ] `src/credential_store.rs` - Secure credential storage
- [ ] `src/auth_state.rs` - Authentication state machine
- [ ] `src/two_factor.rs` - 2FA/MFA handling (optional)
- [ ] `src/lib.rs` - Extended exports
- [ ] `Cargo.toml` - Updated dependencies if needed

### foundation_ai (OpenAI Provider)
- [ ] `src/backends/openai_provider.rs` - Provider implementation
- [ ] `src/backends/openai_helpers.rs` - HTTP helpers and parsing
- [ ] `src/backends/mod.rs` - Add openai_provider module
- [ ] `src/types/mod.rs` - OpenAI-specific types (if needed)
- [ ] `src/errors/mod.rs` - HTTP error variants
- [ ] `tests/openai_tests.rs` - Integration tests

## Key Implementation Details

### JWT Token Structure
```rust
pub struct JWTToken {
    pub access_token: ConfidentialText,
    pub refresh_token: Option<ConfidentialText>,
    pub expires_at: f64,  // Unix timestamp
    pub scope: Option<String>,
}
```

### OAuth Config Structure
```rust
pub struct OAuthConfig {
    pub client_id: ConfidentialText,
    pub client_secret: Option<ConfidentialText>,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub authorization_url: String,
    pub token_url: String,
    pub use_pkce: bool,
}
```

### OpenAI Provider Config
```rust
pub struct OpenAIConfig {
    pub base_url: String,           // "https://api.openai.com"
    pub api_version: String,        // "v1"
    pub timeout_secs: u64,          // 30
    pub max_retries: u32,           // 3
    pub proxy_url: Option<String>,
}
```

## Success Criteria

- [ ] All foundation_auth extensions compile and pass tests
- [ ] All foundation_ai OpenAI provider tests pass
- [ ] `cargo clippy -- -D warnings` passes for both packages
- [ ] `cargo fmt -- --check` passes
- [ ] No TODO/FIXME/stubs remaining
- [ ] LEARNINGS.md updated with implementation notes

## Dependencies to Verify

Ensure these `foundation_core` features are available:
- `simple_http::client::Client` - HTTP client
- `simple_http::url::Uri` - URL handling
- `event_source::SSEParser` - SSE parsing
- `valtron::StreamIterator` - Streaming iterator trait

## Troubleshooting

### Common Issues

1. **OAuth PKCE not working**: Ensure `code_challenge_method=S256` and proper base64url encoding
2. **SSE parsing fails**: Check for double-newline delimiters and `data: ` prefix handling
3. **Token refresh loops**: Ensure refresh buffer (default 5 minutes) is properly configured
4. **Credential leakage**: Verify all secrets use `Zeroizing` and Debug impls are redacted

## Next Steps After Completion

After completing this feature:
1. Proceed to **Feature 01 (llamacpp-integration)** for local inference
2. Or proceed to **Feature 02 (huggingface-provider)** for model discovery

---

_Created: 2026-03-20_
