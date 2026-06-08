# LEARNINGS

## Past Discoveries and Mistakes

### 2026-06-06: Spec Review — Async/Sync Pattern Corrections

**Issue 1: Sync/async direction was backwards in Feature 14 (Cedar).**
The spec showed sync implementations bridged to async via `spawn_blocking`, but the
correct pattern is async-first with valtron sync wrappers. Async contains the I/O logic;
sync bridges to async via valtron `from_future` + `collect_one`.

**Issue 2: Tokio was incorrectly referenced.**
Tokio has no role in the valtron bridging pattern. Both native and wasm sync wrappers
use valtron as the execution engine. Tokio references were replaced with valtron patterns.

**Issue 3: QueryStream/AsyncQueryStream parity mismatch.**
`foundation_db`'s `QueryStore` (sync) returns a valtron StreamIterator while
`AsyncQueryStore` (async) returns `Vec<T>`. This defeats streaming for async.
Feature 00 was created to fix this — `AsyncQueryStream` wraps valtron's
StreamIterator as `futures_core::Stream` for parity.

**Issue 4: foundation_nativeapis vs foundation_db roles unclear.**
Clarified: foundation_db = database storage (Turso, libsql, D1).
foundation_nativeapis = filesystem operations. CredentialStorage primarily uses
foundation_db; foundation_nativeapis only for file-based caching.

**Issue 5: WASM git HTTP fallback was the wrong default.**
gix supports WASM via gix-protocol with custom HTTP transport. The design now uses
a pluggable `PolicyFetcher` trait with `GitPolicyFetcher` (gix-based) as primary
and `HttpPolicyFetcher` as fallback.

**Issue 6: Handler types needed clarification.**
foundation_http has two handler traits: `ServeWriter` (sync, native) and `WebServe`
(async, wasm). Features 09 and 12 were updated to describe both and explain how
ServeWriter bridges to async via valtron.

### 2026-06-08: Feature 08 — AuthManager Design Decisions

**Decision 1: Optional SessionManager, JwtVerifier, JwksManager.**
AuthManager owns these as `Option` fields with builder methods (`with_session_manager()`,
`with_jwt_verifier()`, `with_jwks_manager()`). Not every consumer needs all components —
a client-side app may only need token management, while a resource server needs verification.

**Decision 2: logout() takes explicit user_id for session revocation.**
Since the access token may be opaque (not a JWT with a `sub` claim), the AuthManager
cannot reliably extract a user ID from the token. The caller passes `user_id: Option<&str>`
to `logout()`. Pass `None` to skip session revocation.

**Decision 3: init_from_store sets TokenExpired when refresh is available.**
When the persisted token is expired but a refresh token exists, init_from_store transitions
to `TokenExpired` (not `Unauthenticated`). The caller should then call `refresh()`. This
avoids sync→async bridging in init_from_store, since refreshing requires HTTP I/O.

**Decision 4: State machine transitions in init_from_store.**
Must go through `AuthenticateStarted` → `AuthenticateCompleted` → (optionally `TokenExpired`)
because the state machine requires sequential valid transitions. Skipping `AuthenticateStarted`
would fail silently.

### 2026-06-08: Features 09-12 — IdP Server Implementation

**Decision 1: Handlers write raw HTTP/1.1 to `dyn Write` directly.**
`foundation_http`'s `respond::json()` takes `&mut impl Write` (requires `Sized`), but
`ServeWriter::serve_writer` receives `&mut dyn Write` (unsized). Rather than fighting
the type system with double-borrow tricks, handlers write HTTP response lines directly
via `write!()` + `write_all()`. Simple, no extra dependencies.

**Decision 2: Scaffold handlers return error JSON, not panics.**
Endpoints that need a storage backend (token, authorize, device_authorize) return
proper OIDC error JSON responses (`unsupported_grant_type`, `server_error`) instead of
panicking or returning 501. This lets discovery/JWKS work immediately while storage
integration is added incrementally.

**Decision 3: IdpConfig stored in ContextBag.**
`IdpServer::http_app()` stores `IdpConfig` in the `ContextBag`. Handlers retrieve it
via `bag.get::<IdpConfig>()` in their `ServeWriterFactory::create()`. This follows the
established foundation_http pattern — no global state, no `Arc` threading.

**Decision 4: SimpleMethod variants are UPPERCASE (GET, POST, not Get, Post).**
`foundation_netio::SimpleMethod` uses all-caps variants matching HTTP method names.

### 2026-06-08: Feature 14 — Cedar Policy Engine (Phase 1)

**Decision 1: cedar-policy v4.11 — use latest stable.**
The spec targets 4.11. `Schema::from_cedarschema_str()` returns `(Schema, warnings)`
tuple, while `Schema::from_json_str()` returns `Schema` directly. Don't destructure
with tuple pattern on `from_json_str`.

**Decision 2: `is_authorized_with_entities` uses caller's entities.**
The method accepts entities from the caller for evaluation — does NOT fall back to
`self.entities`. The caller is responsible for providing the full entity set needed
for the request.

**Decision 3: Phase 1 = core engine + in-memory store only.**
Storage backends (local file, R2, D1, git), wasm backends, HTTP middleware, and
valtron bridging are Phase 2+. The core `CedarEngine`, `CedarRequest`, `CedarResponse`,
`PolicyStore` trait, and `InMemoryPolicyStore` are sufficient for the initial scaffold.

### 2026-06-06: Git Storage Capability Split

Git operations belong in `foundation_nativeapis` as a generic `PolicyFetcher` trait.
`foundation_cedar` consumes this trait — it knows Cedar paths and parsing, not git.

