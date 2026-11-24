# Design Decisions — Social Login / Upstream IdP Federation

**Status:** Resolved (2026-07-18)

## D01: Identity Broker vs Direct Passthrough

### Context
When integrating Google/Facebook/etc., there are two fundamentally different approaches:

1. **Identity broker (this choice):** The IdP redirects to Google, gets back a profile, creates/links a local user, and mints its own JWT. Downstream apps always receive tokens from the IdP and never know Google was involved.
2. **Direct passthrough:** The IdP would forward Google's tokens directly to the app. The app would receive Google-signed JWTs and validate against Google's JWKS.

### Decision: Identity Broker

The IdP is always the single token-issuing authority. Upstream providers are **authentication sources only**.

**Why:**
- **One token authority.** Apps validate against one JWKS endpoint, one issuer. Adding a new provider doesn't require app changes.
- **Consistent claims.** Google's `sub` is different from Facebook's `id`. The IdP normalizes all providers into one `User` entity with consistent claims (`sub`, `email`, `name`).
- **Revocation control.** If you remove Google as a provider, existing users still have local accounts. Their tokens still work. Direct passthrough would break them.
- **Account linking.** A user who first logs in with Google and later connects GitHub is one user. The IdP owns that identity graph, not Google.
- **Audit trail.** All authentication events go through one place — the IdP's audit log sees everything.

**Consequence:** The IdP must handle the full OAuth dance with every upstream provider on every login. This adds ~200-800ms latency per social login. This is acceptable because it happens once per session (subsequent logins use the IdP's session/refresh token).

## D02: Provider Selection Mechanism

### Context
An app using the IdP needs to tell it which upstream provider to use (if any). Options:

1. **Query param on /authorize:** `GET /authorize?client_id=myapp&provider=google`
2. **Separate endpoint:** `GET /auth/v1/providers/google/start`
3. **App configuration:** Each registered OAuthClient has a `default_provider` field

### Decision: Query param on /authorize, with login page fallback

- `GET /idp/authorize?client_id=myapp&provider=google` — bypass login page, go straight to Google
- `GET /idp/authorize?client_id=myapp` (no provider) — show IdP login page with provider buttons
- `GET /idp/authorize?client_id=myapp&provider=unknown` — return error JSON (don't silently fall back)

**Why:**
- **Existing /authorize is extended, not replaced.** The authorize handler already validates client_id, redirect_uri, PKCE. Adding an optional `provider` param is a minimal change that preserves the existing flow.
- **App controls the experience.** An app that only wants Google login can hardcode `?provider=google`. An app that wants choice omits it.
- **No default provider.** If the app doesn't specify, the user chooses on the IdP's login page. This prevents silent provider switching and keeps the IdP's UI as the fallback.
- **Error on unknown provider.** Failing loudly prevents misconfiguration from silently breaking login.

## D03: Callback URL Strategy

### Context
The upstream provider redirects back after authentication. Where should it go?

1. **Per-provider callback:** `GET /auth/v1/providers/{provider_id}/callback` (each provider gets its own URL registered in Google's console)
2. **Single shared callback:** `GET /auth/v1/providers/callback?provider=google` (one URL for all, disambiguate by state)
3. **Dynamic callback:** Constructed at runtime from the IdP's issuer URL

### Decision: Per-provider callback at `/auth/v1/providers/{id}/callback`

Each upstream provider has a unique callback URL that the IdP registers with that provider when configured.

**Why:**
- **No disambiguation needed.** The provider ID is in the URL path. We don't need to parse state to figure out which provider is calling back.
- **Provider registration is explicit.** When you add Google in the admin UI, it shows: "Register this callback URL in your Google Cloud Console: `https://your-idp.example.com/auth/v1/providers/google/callback`". This is clearer than a generic URL.
- **Isolates provider errors.** If Google's callback fails, it doesn't affect the shared callback endpoint.
- **State still needed for CSRF.** The `state` param protects against CSRF attacks — we validate it matches the pending upstream auth session.

## D04: Double-State Correlation

### Context
The OIDC authorization code flow requires a `state` parameter for CSRF protection. In the social login flow, there are **two** OAuth dances chained:

1. App → IdP (app's `state` for CSRF against the IdP)
2. IdP → Upstream provider (upstream `state` for CSRF against Google)

Options:

1. **Reuse one state:** Use the same `state` for both flows
2. **Independent states:** Generate a new `state` for the upstream flow, correlate server-side
3. **Encode app state in upstream state:** `upstream_state = base64(app_state + provider_id)`

### Decision: Independent states, correlated in an `UpstreamAuthSession` record

The IdP generates its own `state` for the app's flow and a separate `state` for the upstream provider. Both are stored in an `UpstreamAuthSession` record keyed by the upstream state.

**Why:**
- **One state is not safe.** Reusing the app's state for the upstream flow means Google's callback would contain a state that the app generated. If the app's state is predictable (a known vulnerability in many apps), an attacker could forge Google callbacks.
- **Encoding couples the flows.** If `upstream_state = encode(app_state)`, then the upstream state is derivable from the app state. An attacker who compromises the app's state generation compromises both flows.
- **Independent states isolate risk.** The upstream state is a fresh random value generated by the IdP. The app never sees it. The app only sees its own state returned in the final redirect.
- **Server-side correlation is simple.** `UpstreamAuthSession { upstream_state, app_state, app_client_id, app_redirect_uri, pkce_challenge, nonce, expires_at }`. When Google calls back with `state=xyz`, we look up the session, validate, and proceed.

**Storage:** Stored in `KeyValueStore` with TTL (matches the upstream provider's auth code TTL, typically 600s). No SQL migration needed — `MemoryStorage`/KV is sufficient for ephemeral sessions.

## D05: User Provisioning Strategy

### Context
When a user logs in via Google for the first time, the IdP must decide: create a new user, link to an existing user, or reject?

1. **Always create new:** Every provider login creates a new local user, even if the same email exists
2. **Auto-link by email:** If a user with the same email exists, link the provider to that account
3. **Always reject if no link:** Only pre-linked providers can log in

### Decision: Auto-link by verified email, otherwise create new

- If the upstream profile has a **verified email** that matches an existing local user → link the provider to that account
- If no email match → create a new local user with the upstream profile data
- If the upstream email is **not verified** → create a new user (don't auto-link to an unverified email)

**Why:**
- **Verified email is the identity bridge.** Google, GitHub, Apple all provide `email_verified` (or equivalent). This is a trustworthy signal that the same person owns both identities.
- **Auto-link reduces friction.** A user who registered with email/password and later clicks "Login with Google" (same email) should get their existing account, not a duplicate.
- **Unverified email is not trustworthy.** Some providers return email without verification status, or return `verified=false`. Auto-linking in this case would let someone claim another user's account by registering the same email on a provider.
- **Create-new is the safe default.** If nothing matches, the user gets a new account. This is correct for genuine new users.

**Edge case — duplicate email conflict:** If Google returns `alice@gmail.com` (verified) but the IdP already has `alice@gmail.com` registered with a password, we link. If Alice didn't expect this, she now has an account with two login methods. This is the desired behavior — the alternative (rejecting the login) is worse UX. The user can unlink later in account settings.

## D06: Provider Secret Storage

### Context
Upstream providers require `client_secret` (OAuth2 confidential client) or equivalent (Apple's private key, GitHub's app secret). These must be stored securely.

1. **Plaintext in database:** Simplest, but secrets are exposed if the database is compromised
2. **Encrypted in database:** Encrypt with a key stored in environment/config
3. **foundation_keychain (spec-57):** Use the keychain crate for encrypted storage

### Decision: Encrypt with Argon2id-derived key, with foundation_keychain as future upgrade

Provider secrets are encrypted using a symmetric cipher (ChaCha20-Poly1305 via `chacha20poly1305` crate). The encryption key is derived from a `PROVIDER_SECRET_KEY` environment variable via Argon2id (to prevent brute-force if the env var is weak).

**Why:**
- **Plaintext is unacceptable.** Provider secrets in a Turso/D1 database that's synced or backed up would be exposed.
- **foundation_keychain is overkill right now.** The keychain crate (spec-57) is designed for user credentials (Bitwarden-compatible). Provider secrets are server configuration, not user data. The keychain can be adopted later as a backend.
- **Argon2id derivation is a belt-and-suspenders.** Even if `PROVIDER_SECRET_KEY` is a short string, Argon2id makes brute-force expensive. The derived key is what encrypts the secrets, not the env var directly.
- **ChaCha20-Poly1305 is the right cipher.** It's authenticated encryption (AEAD), fast in software, no hardware dependency. Used by libsodium, WireGuard, and many production systems.

**Migration:** Enhance migration 006 — rename `client_secret_encrypted` to `client_secret_ciphertext` and add `encryption_key_id` for future key rotation support.

## D07: Upstream Provider Client Architecture

### Context
Each upstream provider (Google, Facebook, GitHub, Apple) has slightly different OAuth/OIDC behavior:

- Google: Full OIDC (discovery, userinfo, ID token)
- GitHub: OAuth2 only (no OIDC userinfo, uses `/user` REST API)
- Facebook: OAuth2 with custom userinfo endpoint
- Apple: OIDC but signs ID tokens differently, uses private key auth

Options for the client architecture:

1. **One generic client:** Handles all providers through configuration (discovery URL, token URL, userinfo URL, claim mapping)
2. **Per-provider clients:** `GoogleClient`, `GitHubClient`, `FacebookClient` each implement a trait
3. **Generic base + provider overrides:** A `UpstreamOidcClient` handles 90% of providers, with trait-based hooks for exceptions

### Decision: Generic base (`UpstreamOidcClient`) + `ProviderBackend` trait for exceptions

The `UpstreamOidcClient` handles OIDC discovery, authorize URL construction, code exchange, and ID token validation. Providers that deviate from OIDC implement `ProviderBackend`:

```rust
pub trait ProviderBackend: Send + Sync {
    /// Override authorize URL construction (for providers that don't support OIDC discovery)
    fn authorize_url(&self, provider: &UpstreamProvider, state: &str, nonce: &str) -> Result<Url, Error>;

    /// Override token exchange (for providers with non-standard token responses)
    fn exchange_code(&self, http: &dyn HttpClient, provider: &UpstreamProvider, code: &str, redirect_uri: &str, code_verifier: &str) -> Result<TokenResponse, Error>;

    /// Override user profile fetch (for providers without OIDC userinfo)
    fn fetch_profile(&self, http: &dyn HttpClient, provider: &UpstreamProvider, access_token: &str) -> Result<ProviderProfile, Error>;

    /// Map upstream profile claims to our User fields
    fn map_claims(&self, raw_claims: &serde_json::Value) -> Result<ProviderProfile, Error>;
}
```

Default implementations return `None` (use the generic OIDC path). Only providers that need customization override.

**Why:**
- **Most providers are standard OIDC.** Google, Microsoft, Apple, GitLab, Keycloak, Auth0 — all support OIDC discovery and userinfo. The generic client handles them with zero per-provider code.
- **GitHub is the main outlier.** It has OAuth2 but no OIDC userinfo. It needs a custom `fetch_profile` (GET `https://api.github.com/user`) and `map_claims` (GitHub uses `login` not `preferred_username`, no `email_verified` — emails come from a separate `/user/emails` endpoint).
- **Facebook is partially standard.** It has a userinfo endpoint but not OIDC discovery. It needs custom `authorize_url` and `fetch_profile`.
- **Trait-based means adding a provider is just a struct + impl.** No modifying the generic client. Each provider's quirks are isolated.
- **Avoids the enum explosion of per-provider clients.** With 10+ providers, a `match provider_type { Google => ..., GitHub => ... }` becomes unmaintainable.

**Default provider backends shipped:** `GoogleBackend` (standard OIDC, zero overrides), `GitHubBackend` (OAuth2 + REST API), `FacebookBackend` (OAuth2 + custom userinfo). Additional providers can be added by users implementing `ProviderBackend`.

## D08: Token Issuance After Social Login

### Context
After the IdP receives the upstream profile and creates/links the local user, it needs to return tokens to the app. Two options:

1. **Issue auth code directly to app's redirect_uri:** Skip the app's /authorize → /token flow and redirect with an auth code immediately
2. **Complete the /authorize flow normally:** Create an auth code in the IdP's own flow, redirect to the app, app exchanges for tokens

### Decision: Complete the /authorize flow — issue an IdP auth code, let the app exchange

After social login succeeds, the IdP creates its own authorization code (stored in migration 017 `authorization_codes`) and redirects the user's browser to the app's `redirect_uri?code=OUR_CODE&state=APP_STATE`. The app then calls `POST /idp/token` to exchange for JWTs.

**Why:**
- **Consistent app integration.** The app uses the same flow whether the user logged in with password, passkey, or Google. The app always calls `POST /idp/token` with an auth code.
- **PKCE is enforced.** The app's PKCE challenge (from the original `/authorize` request) is applied to the token exchange. The social login layer doesn't bypass this.
- **Scopes are controlled by the IdP.** The app requested `openid profile email` in its `/authorize` call. The IdP issues tokens with those scopes, regardless of what the upstream provider returned.
- **Session is created.** The IdP creates a session cookie for the user's browser (for subsequent logins without re-authenticating with Google). The auth code flow naturally includes session creation.
- **Device code flow works too.** The same pattern applies: upstream login succeeds → device code is authorized → device polls and gets tokens.

## D09: Error Handling on Upstream Failure

### Context
The upstream provider can fail at multiple points: user denies consent, network timeout, invalid response, provider outage. The user is mid-login — what do they see?

### Decision: Redirect to app with error, never hang

When an upstream flow fails:
- The IdP redirects the user's browser to the app's `redirect_uri` with `error=...&error_description=...&state=APP_STATE` (standard OAuth2 error response in the redirect)
- The user sees the app's error handling (e.g., "Google login failed, try again")
- The IdP logs the error for audit

**Why:**
- **The app owns the UX.** The IdP is an API — it doesn't render HTML error pages for the app's users. Redirecting with error params lets the app display its own error UI.
- **Standard OAuth2.** The `error`/`error_description`/`error_uri` params in the redirect are the standard way OAuth2 communicates errors to the client.
- **No hanging browser.** If the IdP tried to show an error page, the user would be stuck on the IdP's domain, not the app's.

**Error codes:** Use standard OAuth2 error codes (`access_denied`, `server_error`, `temporarily_unavailable`, `invalid_request`) plus a custom `provider_error` for upstream API failures.

## D10: Migration Strategy for 006/007

### Context
Migrations 006 and 007 already exist but were designed for client-side outbound OAuth, not IdP-facing provider configuration. Options:

1. **Reuse as-is and add columns via ALTER TABLE** in a new migration
2. **Create new tables** (leave 006/007 orphaned, create `idp_providers` and `idp_provider_secrets`)
3. **Rename via migration** (ALTER TABLE to rename, add columns)

### Decision: Create new migration 024 for `upstream_providers` and 025 for `user_provider_links`

Leave migrations 006/007 untouched (they may be used by the client-side OAuth code in `shared/oauth.rs`). Create fresh tables:

- **024: `upstream_providers`** — provider configuration (id, name, provider_type, client_id, client_secret_ciphertext, encryption_key_id, authorization_url, token_url, userinfo_url, discovery_url, scopes, is_active, mapping_config, created_at, updated_at)
- **025: `user_provider_links`** — links users to upstream identities (id, user_id, provider_id, upstream_subject, upstream_email, upstream_username, created_at, last_used_at)

**Why:**
- **Don't touch existing migrations.** They may be referenced by client-side code. Renaming or altering them risks breaking existing deployments.
- **Clean table names.** `upstream_providers` and `user_provider_links` clearly describe the IdP-facing purpose, unlike `oauth_credentials` which is ambiguous.
- **Separation of concerns.** Client-side outbound OAuth (app calls Google API directly) and IdP-facing social login (IdP calls Google for authentication) are different use cases with different lifecycles.

---

_Decisions recorded: 2026-07-17_
