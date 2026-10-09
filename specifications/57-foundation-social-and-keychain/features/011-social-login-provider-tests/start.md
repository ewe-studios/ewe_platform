---
feature: "011-social-login-provider-tests"
spec: "57-foundation-social-and-keychain"
depends:
  - "001-provider-model"
  - "003-oauth-upstream-client"
  - "004-social-login-flow"
  - "005-user-provisioning"
  - "006-provider-discovery"
status: "pending"
stages:
  - "A-social-login-route-handlers"
  - "B-docker-provider-simulators"
  - "C-provider-specific-assertions"
---

# Feature 011: Social Login Provider Integration Tests

## Current state

The building blocks are all implemented and tested in isolation:
- `UpstreamProvider` model + `ProviderService` CRUD (F001)
- `UpstreamOidcClient` with discovery, authorize_url, exchange_code, fetch_userinfo (F003)
- `UpstreamAuthSession` store + TTL (F004)
- `ProvisioningService` for create/link from upstream profiles (F005)
- Provider admin API (F006)
- Cross-platform HTTP client (F007)

But the **social login route handlers** that chain these together (`GET
/authorize?provider=google → 302 → upstream → callback → exchange → provision →
redirect`) are not wired in `IdpServer::dispatch`. The `authorize` handler
currently only handles the IdP's own password-based login — it returns
`{"status": "login_required"}` and never redirects.

No integration tests exercise the full OAuth2 redirect flow against a real
OAuth2 server. The Docker test infrastructure (`#[docker_container]`, `ory/hydra`)
gives us a clean way to do this without external dependencies.

This feature builds the route handlers and the Docker-backed provider tests.

## Stage order

1. **Social login route handlers** (`social_authorize` + `social_callback`) —
   wires the existing building blocks into `IdpServer::dispatch`. Unlocks
   stages B + C.
2. **Docker-backed provider simulators** — `ory/hydra` containers configured to
   emulate Google, GitHub, Facebook, Twitter/X endpoints.
3. **Provider-specific assertions** — nonce, email_verified, PKCE, refresh
   rotation, app-scoped IDs, etc.
