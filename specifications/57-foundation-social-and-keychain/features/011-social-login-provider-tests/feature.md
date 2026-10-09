# F011: Social Login Provider Integration Tests

**Status:** Spec written — pending implementation

**Depends on:** 001 (provider model), 003 (upstream client), 004 (social login flow), 005 (user provisioning), 006 (provider discovery)
**Unblocks:** nothing — this is the verification layer
**Decisions:** [00](../../decisions/00-identity-broker-pattern.md), [01](../../decisions/01-crypto-backend.md)

## WHY

We have all the building blocks for social login (provider model, OIDC/OAuth2
client, auth session store, user provisioning), but the **redirect handler** that
chains them together — `GET /authorize?provider=google → 302 to Google →
Google callback → exchange code → fetch userinfo → provision user → redirect
back` — has no wired route in `IdpServer::dispatch`. The individual pieces are
unit-tested against `TestHttpServer`, but the full flow is not.

We also have no tests that exercise real OAuth2 provider protocols. Each
provider behaves differently:

- **Google** — OIDC with `.well-known/openid-configuration`, `nonce` in ID token,
  `email_verified` claim, `hd` domain hint
- **GitHub** — OAuth2 (no OIDC), Bearer token in `Authorization` header for
  userinfo, `email` requires separate `/user/emails` endpoint
- **Facebook** — OAuth2 with signed_request, `data_access_expiration_time`,
  app-scoped user IDs, token introspection endpoint
- **Twitter/X** — OAuth 1.0a + OAuth 2.0 with PKCE, `expires_in` in seconds,
  `refresh_token` rotation

A test server that responds with the correct JSON is not enough — we need to
exercise the **full protocol dance** against a real OAuth2 server, using our
`#[docker_container]` infrastructure to spin up `ory/hydra` (which can be
configured to emulate each provider's endpoints and response shapes).

## WHAT

### Part A — Social login route handlers

Two new route handlers in `foundation_auth::server::handlers::core`:

1. **`social_authorize`** — `GET /authorize?provider=google&state=...&redirect_uri=...`
   - Looks up `provider` by id in `ProviderService`
   - Creates `UpstreamOidcClient` and calls `configure()` + `authorize_url(state)`
   - Stores `UpstreamAuthSession` (with PKCE verifier, nonce, app's state +
     redirect_uri) via `UpstreamAuthSessionStore`
   - Returns 302 redirect to the upstream provider's authorization URL

2. **`social_callback`** — `GET /callback?provider=google&code=...&state=...`
   - Validates the `state` parameter matches the stored session
   - Calls `UpstreamOidcClient::exchange_code()` to get tokens
   - Calls `UpstreamOidcClient::fetch_userinfo()` to get the profile
   - Calls `ProvisioningService::provision_from_upstream()` to create/link user
   - Deletes the `UpstreamAuthSession` (one-time use)
   - Creates the IdP's own auth code (for the app to exchange)
   - 302 redirects back to the app's `redirect_uri` with `?code=<idp_auth_code>&state=<app_state>`

### Part B — Docker-backed provider simulators

Four `#[docker_container]` test functions, each exercising one provider's full
flow. Each test:

- Starts `ory/hydra` in a Docker container (pre-configured to emulate the
  specific provider's authorize/token/userinfo endpoints)
- Configures a temporary `UpstreamProvider` via `ProviderService`
- Boots a real `IdpServer` on a random port
- Drives the full redirect flow via direct HTTP (no browser needed — the 302
  chain is followed programmatically)
- Asserts the final IdP auth code is valid and exchangeable

The tests are gated behind a Cargo feature `social-integration-tests` (not
`#[ignore]`), so they're opt-in and don't run in CI without Docker.

### Part C — Provider-specific assertions

Each test asserts provider-specific behaviour:

| Provider | Tests |
|----------|-------|
| **Google** | OIDC discovery → authorize → `nonce` in ID token → `email_verified: true` → profile maps via OIDC claims |
| **GitHub** | OAuth2 explicit URLs → authorize → Bearer token on userinfo → `login` maps to username → `/user/emails` fetched for primary email |
| **Facebook** | OAuth2 authorize → token exchange → signed_request parse → `data_access_expiration_time` → app-scoped id |
| **Twitter/X** | OAuth2 PKCE → token exchange → `expires_in` → refresh token rotation → userinfo maps `data.name` → username |

## HOW

### Route wiring

`IdpServer` gains two new optional fields (set via builder):

```rust
pub struct IdpServer<KV> {
    // ... existing fields ...
    /// When set, enables the social login redirect endpoints.
    pub upstream_session_store: Option<Arc<dyn UpstreamAuthSessionStore>>,
    pub provider_service: Option<Arc<ProviderService<TursoStore>>>,
}
```

`IdpServer::dispatch` adds two new branches before the existing ones:

```rust
else if path.contains("/authorize") && query_has_param(url, "provider") {
    self.social_authorize(bag, req).await
}
else if path.contains("/callback") && query_has_param(url, "provider") {
    self.social_callback(bag, req).await
}
```

### Docker test pattern

Same pattern as `docker_macro_tests.rs` but with `ory/hydra` images:

```rust
#[docker_container(
    image = "oryd/hydra:v2.2",
    as = "hydra",
    port = 4444,  // public port
    wait_port = 4444,
    wait_timeout = 30,
    env = [
        ("DSN", "memory"),
        ("URLS_SELF_ISSUER", "http://localhost:4444"),
        ("URLS_LOGIN", "http://localhost:4444/login"),
        ("URLS_CONSENT", "http://localhost:4444/consent"),
        ("SERVE_COOKIES_SAME_SITE_MODE", "Lax"),
    ],
    // Hydra uses host networking to reach our IdP
    network_mode = "host",
)]
```

The `network_mode = "host"` attribute would need adding to the docker_container
macro (or we use the Docker client directly).

### Feature gate

`foundation_auth/Cargo.toml`:

```toml
[features]
social-integration-tests = [
    "server-test",
    "foundation_deployment_platform/docker",
]
```

Tests compile only when the feature is on — no `#[ignore]` needed.

## Task list

1. Implement `social_authorize` handler in `foundation_auth::server::handlers::core`
2. Implement `social_callback` handler
3. Wire both into `IdpServer::dispatch` and `IdpServer::configure`
4. Add `UpstreamAuthSessionStore` to `IdpServer` builder
5. Add `ProviderService` field to `IdpServer` (+ builder method)
6. Add `social-integration-tests` cargo feature to `foundation_auth`
7. Write Google OIDC Docker test (full redirect flow)
8. Write GitHub OAuth2 Docker test (full redirect flow + email endpoint)
9. Write Facebook OAuth2 Docker test (full redirect flow)
10. Write Twitter/X OAuth2 PKCE Docker test (full redirect flow)
11. Ensure `network_mode = "host"` is supported (add to macro or use Docker client)
12. Compile-check with `--features social-integration-tests`; all tests green

## Test plan / success

- `cargo test -p foundation_auth --features social-integration-tests` runs 4
  Docker-backed provider tests, all pass
- Each test exercises: authorize → 302 → callback → exchange_code → fetch_userinfo →
  provision → auth_code → 302
- Provider-specific assertions pass (nonce in Google ID token, GitHub email
  endpoint, Facebook signed_request, Twitter PKCE)
- Tests skip gracefully when Docker is unavailable (print skip, return Ok)
