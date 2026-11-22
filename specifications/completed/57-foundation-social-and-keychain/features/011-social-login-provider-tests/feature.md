# F011: Social Login End-to-End Correctness and Provider Tests

**Status:** Complete — verified on 2026-11-23
**Depends on:** 001 (provider model), 003 (upstream client), 004 (social login flow), 005 (user provisioning), 006 (provider discovery)
**Unblocks:** nothing — this is the verification layer
**Decisions:** [00](../../decisions/00-identity-broker-pattern.md), [01](../../decisions/01-crypto-backend.md)

_Software is written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay
as they are._

**Abbreviations in this file.** Identity Provider (IdP); Open Authorization (OAuth) version 2
(OAuth2); OpenID Connect (OIDC); Proof Key for Code Exchange (PKCE); JavaScript Object Notation
(JSON); JSON Web Key Set (JWKS); JSON Web Token (JWT); Hypertext Transfer Protocol (HTTP);
Transport Layer Security (TLS); Uniform Resource Identifier (URI); Uniform Resource Locator
(URL); commit, checkout, and Git terms stay as they are.

## WHY

The building blocks for social login exist: the provider model, the OIDC/OAuth2 client, the
auth session store and user provisioning. The two route handlers also exist in
`foundation_auth/src/server/handlers/core.rs` (`social_authorize`, `social_callback`).

A code review on 2026-11-23 found that the wired flow cannot work against a real provider:

1. **Wrong upstream redirect URI.** `social_authorize` builds the upstream client with the
   *app's* redirect URI. `social_callback` builds it with the literal `"unused"`. A provider
   rejects the token exchange unless this value equals the one in the authorize request. It
   must be the IdP's own callback URL.
2. **The OIDC nonce is never checked.** `social_authorize` makes a nonce and stores it in the
   session. `social_callback` never verifies it against the ID token. It also never verifies
   the ID token signature, issuer or audience. A forged or substituted token passes.
3. **GitHub has no email.** GitHub userinfo holds `null` for a private email. Decision 00
   requires a second call to `/user/emails`. The code has no such call.

The four provider paths are also untested end to end. Each provider has a different wire
shape. A test that returns generic JSON proves nothing.

## WHAT

### Part A — Fix the social flow

1. **The upstream redirect URI is the IdP callback URL.**
   - Add `IdpServer::route_prefix` (default `"/idp"`) and `with_route_prefix(prefix)`.
     `http_app` and a new `http_app_with_prefix(prefix)` set it. The value must match the
     prefix that `register_routes` uses.
   - Add one private helper that returns
     `{issuer_url}{prefix}/callback?provider={percent-encoded id}`. Both handlers call it.
   - Store nothing new in the session: the helper is deterministic.
   - The token request sends this same URI.

2. **Verify the OIDC ID token.** After `exchange_code`, when `provider_type == Oidc`:
   - Require an `id_token` in the token response. Its absence is a 400 error.
   - Read the discovery document that the client already fetched. Get `jwks_url()`.
   - Fetch the keys with `JwksManager` and the injected `Ctx`, find the key by `kid`, and
     verify the token with `JwtVerifier`: issuer = discovery issuer, audience =
     `provider.client_id`, clock = `ctx.clock()`.
   - Compare the `nonce` claim with `session.nonce`. A mismatch is a 400 error.
   - The session is already consumed by `take`, so a failure cannot be retried with the same
     state.

3. **GitHub email fallback.** In `social_callback`, when the provider id is `github`
   (case-insensitive) and the profile has no email:
   - GET `{userinfo_url}/emails` with `Authorization: Bearer <token>`.
   - Take the first entry with `primary == true && verified == true`. Else take the first
     entry with `verified == true`.
   - Put that address in the profile. Never invent an address.

4. **No Facebook or Twitter code change is needed.** The generic OAuth2 path and the
   provider mapping cover their wire shape. The tests prove it. Facebook `signed_request` is
   for the app-side JavaScript flow, not for a server-side code exchange. Twitter/X OAuth
   version 1 is out of scope; we use its OAuth2 with PKCE.

5. **Send the upstream client secret.** A real Google, GitHub or Facebook web client is
   confidential and rejects a token exchange without its secret. The provider row keeps the
   secret encrypted. `IdpServer::with_provider_service` gives the handlers the
   `ProviderService` that decrypts it. The token form carries the secret. A provider with a
   secret, but an IdP with no service, is an error; the code never tries a public-client
   exchange. The value is zeroed on drop and never logged.

### Part B — A local provider simulator

No local form of Google, GitHub, Facebook or Twitter exists. The tests therefore run a real
HTTP server that speaks each provider's wire shape. The test file says this in its module
documentation. The simulator is a test double for the **provider only**: the IdP, the
upstream client, the redirect chain and the token exchange are the real code over real TCP.

The simulator is one module in the test tree:
`foundation_auth/tests/integration/social_upstream/`.
- It binds `127.0.0.1:0` with `foundation_http` and serves a small route set per provider.
- It records the requests it receives (method, path, query, headers, form fields) so a test
  can assert what the client sent.
- It holds the authorize code, the expected PKCE challenge and the nonce for one flow.
- It can sign an ID token with its own Ed25519 key and publish the matching JWKS.
- It can return a bad signature, a wrong nonce or a wrong `aud` on request.

Per-provider routes:

| Provider | Endpoints | Wire details the test asserts |
|---|---|---|
| Google (OIDC) | discovery, JWKS, authorize, token, userinfo | discovery is fetched; the authorize URL has `code_challenge` (PKCE), `nonce` and our `redirect_uri`; the token form carries the same `redirect_uri` and the verifier; the ID token signature, `aud`, `iss` and `nonce` are checked; `email_verified` maps to the profile. |
| GitHub (OAuth2) | authorize, token, userinfo, userinfo + `/emails` | explicit URLs, no discovery; userinfo carries the Bearer token; `login` maps to the username; the email comes from `/user/emails`, because userinfo has `email: null`. |
| Facebook (OAuth2) | authorize, token, userinfo | Graph-shaped userinfo with `id` and `name`; the mapping gives `sub` and the display name. |
| Twitter/X (OAuth2) | authorize, token, userinfo | PKCE in the authorize URL and the token form; `expires_in` and `refresh_token` stay on the token object; `data.name` maps through the username or name field. |

### Part C — The four tests

Each test:
1. Starts the simulator and reads its address.
2. Stores an `UpstreamProvider` row through the query store (the same path as the admin API).
3. Starts a real `IdpServer` with `with_social_login` and an `issuer_url` equal to the test
   server's origin, so the IdP callback URL points at the test server.
4. Calls `GET {prefix}/authorize?provider=<id>&client_id=<app>&redirect_uri=<app uri>&state=<s>`.
   Asserts a 302 to the simulator, then calls the simulator's `/authorize`, which 302s back to
   the IdP `/callback`.
5. Calls the IdP callback. Asserts a 302 to the app `redirect_uri` with `code` and the app
   `state`.
6. Exchanges the IdP code for tokens at `POST {prefix}/token` and asserts the user exists.
7. Asserts the simulator saw exactly the expected requests and form fields.

Negative tests, one each:
- A used `state` is refused (the session is one time).
- An ID token with a wrong nonce is refused.
- An ID token with a bad signature is refused.
- An unknown `provider` is refused.
- An inactive provider is refused.
- An unregistered app `redirect_uri` is refused.

### Feature gate

`foundation_auth/Cargo.toml` gains:

```toml
social-integration-tests = ["server-test"]
```

The simulator tests live in the `server-test` integration target and compile only with this
feature. They need no Docker. Their module documentation states why a simulator replaces the
provider.

## HOW

### Route wiring (already present)

`dispatch` sends `GET */authorize?provider=...` to `social_authorize` and
`GET */callback?provider=...` to `social_callback`. Both paths exist. This feature changes the
two handlers and the `IdpServer` prefix field. The routes in `routes.rs` stay as they are.

### Prefix and callback URL

```rust
impl<KV: KeyValueStore + Clone + 'static> IdpServer<KV> {
    /// The prefix the OIDC routes are registered under. Default "/idp".
    pub fn with_route_prefix(mut self, prefix: impl Into<String>) -> Self;

    /// The absolute callback URL registered with the upstream provider.
    fn upstream_callback_url(&self, provider_id: &str) -> String {
        let origin = self.config.issuer_url.trim_end_matches('/');
        let prefix = self.route_prefix.trim_end_matches('/');
        let id = urlencoding::encode(provider_id);
        format!("{origin}{prefix}/callback?provider={id}")
    }
}
```

`http_app()` keeps the current `"/idp"` default and sets `route_prefix` too. A new
`http_app_with_prefix(prefix)` registers the routes and sets the field in one call.

### Error shape

A failed check is a `400` (`IdpError::BadRequest`) with a short message. The message never
contains the token, the code or the nonce.

## Task list

1. Add `route_prefix`, `with_route_prefix` and `http_app_with_prefix` to `IdpServer`. Keep
   `http_app()` behaviour.
2. Add `upstream_callback_url` and use it in `social_authorize` and `social_callback`.
3. Verify the OIDC ID token (signature, issuer, audience, nonce) in `social_callback`.
4. Add the GitHub `/user/emails` fallback.
5. Send the decrypted upstream client secret at the token exchange, through
   `IdpServer::with_provider_service`, and add the two client-secret tests.
6. Add the `social-integration-tests` feature.
7. Write the simulator module with the four provider route sets and their recorders.
8. Write the four provider tests.
9. Write the six negative tests and the two client-secret tests.
10. Bump `foundation_auth` 0.0.12 to 0.0.13 and update every consumer requirement.
11. Update `progress.md` and `LEARNINGS.md`.

## Tests

```bash
cargo test -p foundation_auth --features social-integration-tests --test integration_tests social_upstream
cargo test -p foundation_auth
cargo fmt --all -- --check
cargo clippy -p foundation_auth --all-targets --no-deps -- -D warnings
cargo check -p foundation_auth --target wasm32-unknown-unknown --no-default-features --features wasm,secret
```

## Done when

- The authorize URL sent upstream carries the IdP callback URL, and the token form carries the
  same value.
- An OIDC ID token with a bad signature, a wrong audience, a wrong issuer or a wrong nonce is
  refused, and a test proves each case.
- The Google, GitHub, Facebook and Twitter-shaped simulator tests pass over real TCP.
- No test reads a body into memory when it can stream it. Test bodies are small and are parsed,
  which rule 4 permits.
- `progress.md` and `LEARNINGS.md` record the design changes and the result.

## Out of scope

- Upstream token refresh and rotation. The client keeps the issued refresh token; no refresh
  call exists yet. File a GitHub issue if the tests show a gap.
- Per-provider callback paths (decision 00). The shared callback with a `provider` query
  parameter stays, because the route table and the existing handlers use it. Record the
  deviation in `LEARNINGS.md`.
- Facebook `signed_request` and Twitter OAuth version 1. The server-side code flow does not
  use them.
- A Docker image of `ory/hydra`. Hydra is a different OAuth2 server, not a stand-in for the
  four providers; it cannot serve GitHub's `/user/emails` or the provider-specific JSON. The
  simulator replaces it. Record this in `LEARNINGS.md`.
