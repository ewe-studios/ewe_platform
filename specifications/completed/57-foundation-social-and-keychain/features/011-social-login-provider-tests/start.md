---
feature: "011-social-login-provider-tests"
spec: "57-foundation-social-and-keychain"
depends:
  - "001-provider-model"
  - "003-oauth-upstream-client"
  - "004-social-login-flow"
  - "005-user-provisioning"
  - "006-provider-discovery"
status: "complete"
stages:
  - "A-social-flow-fixes"
  - "B-local-provider-simulator"
  - "C-provider-and-negative-tests"
---

# Feature 011: Social Login End-to-End Correctness and Provider Tests

## Current state (measured 2026-11-23)

The route handlers exist. `social_authorize` and `social_callback` are in
`foundation_auth/src/server/handlers/core.rs`, and `dispatch` sends
`GET */authorize?provider=...` and `GET */callback?provider=...` to them.

The handlers have three defects (feature.md section "WHY"):
1. The upstream redirect URI is the app's URI, or the literal `"unused"`.
2. The OIDC ID token is never verified, and its nonce is never checked.
3. GitHub's `/user/emails` fallback is absent.

No test drives the full redirect flow. The building blocks are tested alone.

## Stage order

1. **Social flow fixes** — the IdP callback URL, ID token verification and the GitHub
   email fallback. Unlocks the tests.
2. **Local provider simulator** — a real HTTP server in the test tree with the
   Google, GitHub, Facebook and Twitter wire shapes. The tests state why a
   simulator replaces the provider.
3. **Provider and negative tests** — the four provider flows and the six refusal
   cases.

`ory/hydra` is not used. Hydra is a different OAuth2 server, and it cannot serve
GitHub's `/user/emails` or the provider-specific JSON. The reason is recorded in
feature.md and goes into `LEARNINGS.md` after the change.
