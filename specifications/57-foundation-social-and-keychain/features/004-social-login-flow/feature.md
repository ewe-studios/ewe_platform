# Feature 04: Social Login Flow (Redirect + Callback)

**Status:** Complete

**Depends on:** `01-provider-model`, `02-provider-migrations`, `03-oauth-upstream-client`

## Summary

Implements the redirect-based social login flow with an `UpstreamAuthSession` KV store. Handles the full round-trip: store session state, redirect user to the upstream provider, receive callback, validate state, exchange codes, and redirect back to the app with the IdP's own auth code.

## What was built

### Files created

- `backends/foundation_auth/src/shared/auth_session.rs` -- `UpstreamAuthSession`, `UpstreamAuthSessionStore` trait, `MemoryAuthSessionStore`

### Key types

| Type | Purpose |
|------|---------|
| `UpstreamAuthSession` | Session state stored during the upstream redirect: `upstream_state`, `app_state`, `app_client_id`, `app_redirect_uri`, PKCE fields, `provider_id`, `our_nonce`, `our_code_verifier`, `created_at`, `expires_at` |
| `UpstreamAuthSessionStore` | Trait: `store`, `get`, `delete` -- async, TTL support |
| `MemoryAuthSessionStore` | In-memory implementation backed by `HashMap + tokio Mutex` with TTL cleanup |

### Session lifecycle

1. **Store:** App requests `/authorize?provider=google` -> IdP generates `upstream_state` + `our_code_verifier` -> stores `UpstreamAuthSession` with `upstream_auth:{upstream_state}` key -> 302 redirect to Google
2. **Retrieve:** Google redirects back with `code` + `state` -> IdP looks up session by state -> validates `app_state` matches -> exchanges upstream code -> provisions user -> creates IdP auth code
3. **Delete:** After successful callback, session is deleted from KV so it cannot be replayed

### TTL

Sessions expire after 600 seconds. The in-memory store performs passive cleanup on `get()` (lazy eviction for expired entries).

## Tests

- `backends/foundation_auth/src/shared/auth_session.rs` (inline `#[cfg(test)]` module) -- **4 unit tests**
  - Store and retrieve session
  - Get non-existent session (returns None)
  - Delete session
  - Expired session retrieval (TTL-based)

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- social login session model
- `decisions/02-storage-abstraction.md` -- KV store abstraction for sessions

## Implementation notes

- **Module placement in `shared/`:** The session model and store trait are cross-platform. The in-memory store is used in server tests; a production deployment would use D1/Turso-backed `KeyValueStore`.
- **PKCE for upstream flow is independent of app's PKCE.** The IdP generates its own code_verifier/code_challenge for the upstream exchange. This means PKCE is validated twice: once with the upstream provider, once when the app exchanges the IdP's auth code.
- **Session cookie on callback:** After successful social login, the IdP creates a session cookie so the user's browser doesn't need to re-authenticate with the upstream provider on every request.
- TTL cleanup is **lazy** (only on `get()`) -- active eviction is not needed since the session is deleted immediately after a successful callback.
