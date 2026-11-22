# LEARNINGS

## Past Discoveries and Mistakes

### 2026-07-17: Spec Creation — Initial Design

**Migrations 006/007 are orphaned client-side tables.** They were created for outbound OAuth in the client library (`shared/oauth.rs`) but never wired up. Do not reuse them for IdP provider config — create new migrations 024/025 with clear `upstream_providers` and `user_provider_links` names.

**Rauthy exploration doesn't cover social login.** The exploration files (`/home/darkvoid/Boxxed/@dev/repo-expolorations/src.auth/src.rauthy/`) cover auth flows, OIDC/OAuth2, JWT, but not social/upstream provider integration. Design decisions were made from first principles (OIDC spec, OAuth2 spec, rauthy's admin UI mentions).

**Identity broker is the right pattern for our use case.** The user wants one central IdP that apps authenticate against, with multiple upstream login options. This means the IdP mints its own JWTs and upstream providers are just authentication sources — not token issuers for downstream apps.

---

### 2026-11-23: F011 — Social login end to end

**The nonce was in the session but not in the authorize URL.** `OAuthConfig`
leaves `nonce` as `None`, and `get_authorization_url` adds a nonce only when it
is set. The old `social_authorize` made a nonce and stored it in the session, but
the upstream authorize request carried none. A real provider therefore returned
an ID token with no nonce, or another value. `social_authorize` now makes one
nonce and passes it to the new
`UpstreamOidcClient::authorize_url_with_nonce`, so the URL, the token and the
session carry the same value.

**The callback URL helper lives on `IdpHandlerCore`, not `IdpServer`.** The
feature text put it on `IdpServer`, but a handler only sees the `Ctx` (config,
storage and the route prefix). The helper is private on `IdpHandlerCore`, and
both handlers call it. `IdpServer` keeps the `route_prefix` field and stores it
in the `Ctx` as `RoutePrefix`, so the handler core reads it when the routes are
registered.

**The shared callback stays, with `?provider=` in the query.** Decision 00
prefers `/providers/{id}/callback`. The route table and both handlers already use
`{prefix}/callback?provider={id}`, so F011 keeps that shape. The simulator
appends `code` and `state` with `&`, because the registered callback URL already
has a query string.

**`Jwk::to_verifier` requires a `kid` in the ID token header.** jwt-simple fails
with `MissingJWTKeyIdentifier` when `required_key_id` is set and the header has
no `kid`. The handler reads the `kid` from the token header and picks that key.
The simulator signs with `Ed25519KeyPair::generate().with_key_id(...)`, so the
test proves the path a real provider uses.

**The provider client secret must reach the token exchange.** A real Google,
GitHub or Facebook web client is confidential and rejects an exchange without
its secret. The handler now uses the `ProviderService` of the server:
`IdpServer::with_provider_service` puts it in the `Ctx`, and
`ProviderService::decrypt_provider_secret` decrypts the secret of the loaded
provider into a `Zeroizing<String>`. The secret is sent in the token form and
never enters a log line or an error message. A provider that has a secret, but
an IdP with no `ProviderService`, is a `500`: the code never falls back to a
public-client exchange. The simulator now rejects a wrong or missing secret,
and two tests prove both paths.

**Twitter/X nests its user object in `data`.** `UpstreamProfile::from_userinfo`
read only top-level claims. A mapping field name may now be a dotted path
(`"data.name"`), so the Twitter/X shape maps without per-provider code. A plain
name still reads a top-level claim.

**Hydra is not a substitute for the four providers.** `ory/hydra` is an OAuth2
server, but it cannot serve GitHub's `/user/emails` list or the provider-specific
JSON shapes. The tests use a local simulator in
`foundation_auth/tests/integration/social_upstream/`, built with
`foundation_http` on `127.0.0.1:0`. The IdP, the upstream client and the redirect
chain are the real code over real TCP. The tests use raw HTTP so every `302` is
visible; the standard client follows redirects and hides them.

---
