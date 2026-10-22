---
feature: "Async-Only Storage"
description: "Move every remaining sync storage caller onto the Async* traits, make StateStore async, drop the blocking Default impls, then delete the sync storage traits and Blocking<S>"
status: "in-progress"
priority: "high"
depends_on: []
builds_on:
  - "PR #130 (valtron async-first API, blocking_boundary!)"
  - "PR #131 (foundation_db I/O stores async-only, Blocking<S>)"
  - "PR #132 (foundation_ai AgentSession on the Async* traits)"
estimated_effort: "large"
created: 2026-10-21
updated: 2026-10-21
branch: "db/async-callers"
backlog: "docs/bug-backlog.md §11 (and the §12/§13/§14/§15 entries named below)"
---

# Feature 04: Async-Only Storage

## 1. Goal and why

After this feature there are **no sync storage traits anywhere in the
workspace**. Storage is reached only through the `Async*` traits in
`foundation_db` (`AsyncKeyValueStore`, `AsyncQueryStore`, `AsyncBlobStore`,
`AsyncDocumentStore`, `AsyncRateLimiterStore`, and the auth/state traits built on
them) and is `.await`ed all the way up. Sync code exists only at a real
boundary — a sync `main`, a sync trait owned by a third-party crate, a sync
POSIX-style API — and waits there with valtron's `.block()` /
`block_on_future`, compiled only where blocking is possible
(`foundation_core::blocking_boundary!`).

Why:

- **Cloudflare Workers can't block.** A Worker isolate has one thread and a JS
  event loop. Anything that waits synchronously on I/O either spins forever or
  freezes the isolate. PRs #130/#131 made blocking a compile error on JS-hosted
  wasm (`.block()` / `Blocking<S>` don't exist there), so every caller still on
  a sync storage trait is code that cannot run on Workers.
- **Blocking inside async code ties up a valtron worker.** The foundation_auth
  IdP handlers are already `async fn`s but call sync storage, which waits on
  the executor from inside a task. With every worker waiting, nothing makes
  progress (the hang class recorded in `learnings.md`).
- **Two code paths rot.** Sync twins next to async methods (`PowService` /
  `AsyncPowService`, `HandlerStorage` / `AsyncHandlerStorage`,
  `MigrationRunner::run` / `run_async`, the `PasskeyStore` / `AsyncPasskeyStore`
  pairs) drift apart. One async path, tested once.

Non-goals:

- foundation_vectors' own `VectorStore` (sync) trait for the in-memory, fjall
  and sqlite-vec stores. It is not a `foundation_db` storage trait, and those
  backends do no async I/O. Only the two SQL-backed stores move (§3.4).
- foundation_nativeapis' sync VFS traits (`VfsFileSystem`, `DeltaStore`, …).
  They are a POSIX-shaped boundary (FUSE, NFS, the LD_PRELOAD shim). The D1/R2
  deltas stop reaching storage through sync traits; the VFS traits stay (§3.7).
- `NativeOAuth`'s sync HTTP twins (backlog §12): not storage.

## 2. Current state (grep of the whole workspace, 2026-10-21)

Searched `backends/`, `crates/`, `apps/` and `examples/` (cf-agent-loop,
cf-cloud-provider, cf-login-app, platform_android and platform_ios are their
own workspace roots) for the sync traits, `Blocking`, `.block()`,
`block_on_future`, and `Default` impls that block.

### 2.1 Sync storage traits (definitions, `foundation_db`)

| Item | File |
|------|------|
| `KeyValueStore`, `BlobStore`, `QueryStore`, `RateLimiterStore`, `DocumentStore`; `StorageItemStream` (sync valtron stream type used only by them) | `backends/foundation_db/src/core/storage_provider.rs` |
| `StorageCleanup: QueryStore` + blanket `impl<T: QueryStore>` | `backends/foundation_db/src/core/cleanup.rs` |
| `PasskeyStore`, `TosStore`, `AuthStore` (sync auth stores) | `backends/foundation_db/src/core/auth_store.rs` |
| `MigrationRunner::run(&dyn QueryStore)` | `backends/foundation_db/src/core/schema/migrations.rs` |
| `StateStore` (sync, returns `StateStoreStream<T>` = boxed iterator of `ThreadedValue`) | `backends/foundation_db/src/core/state/traits.rs` |
| `Blocking<S>` + `HAS_BLOCKING` | `backends/foundation_db/src/blocking.rs`, `src/lib.rs` |
| Re-exports of the sync traits | `backends/foundation_db/src/lib.rs` (`pub use core::storage_provider::{…}`) |

### 2.2 Callers of the sync storage traits

| Crate | Item | File | Uses |
|-------|------|------|------|
| foundation_auth | `storage::*` free fns (`find_client_by_id`, `store_auth_code`, `find_user_by_email`, `create_user`, passkey/ToS fns, …) | `src/server/storage.rs` | `&dyn QueryStore` |
| foundation_auth | `HandlerStorage<KV: KeyValueStore>` { `query_store: Arc<dyn QueryStore>`, `cache: KV` } + sync `PasskeyStore`/`TosStore`/`AuthStore` impls; `AsyncHandlerStorage` (async twin) | `src/server/storage.rs` | sync traits |
| foundation_auth | `PowService<KV: KeyValueStore>` (+ `AsyncPowService` twin) | `src/server/services/pow_service.rs` | `KeyValueStore` |
| foundation_auth | `TosService<KV: KeyValueStore>` | `src/server/services/tos_service.rs` | `QueryStore` via `HandlerStorage` |
| foundation_auth | `WebAuthnService<S: AuthStore, KV: KeyValueStore>` | `src/server/services/webauthn_service.rs` | `AuthStore`, `KeyValueStore` |
| foundation_auth | `ProviderService<QS: QueryStore>` | `src/server/services/provider_service.rs` | `QueryStore` |
| foundation_auth | `ProvisioningService<QS: QueryStore>` | `src/server/services/provisioning_service.rs` | `QueryStore` |
| foundation_auth | `IdpHandlerCore<KV: KeyValueStore = Blocking<MemoryStorage>>`; `lookup_upstream_provider`, `provision_from_upstream_profile`, `find_user_by_email_lax` | `src/server/handlers/core.rs` | `&dyn QueryStore`, `Blocking` |
| foundation_auth | `provider_admin::{list_all, get_one, create, update, delete, set_secret}<QS: QueryStore>` | `src/server/handlers/provider_admin.rs` | `QueryStore` |
| foundation_auth | `ServeAdapter` (`bag.get::<HandlerStorage<Blocking<MemoryStorage>>>()`, `NoOpStore: QueryStore`) | `src/server/handlers/serve_adapter.rs` | `Blocking`, `QueryStore` |
| foundation_auth | `IdpServer<KV: KeyValueStore = Blocking<MemoryStorage>>` | `src/server/idp_server.rs` | `Blocking` |
| foundation_auth | `CredentialStore` (sync trait) impl for `CredentialStorage` through `Blocking<StorageProvider>`; `OAuthTokenStore: CredentialStore` (blanket) | `src/shared/credential_store.rs` | `Blocking`, `KeyValueStore` |
| foundation_auth | `SessionManager<S: CredentialStore>` sync methods (`create_session`, `get_session`, `revoke_session`, `revoke_all_sessions`) | `src/shared/session.rs` | `CredentialStore` |
| foundation_auth | `AuthManager` (`init_from_store`, `get_valid_token`, and the async `authenticate`/`refresh`/`logout` calling sync storage) | `src/shared/auth_manager.rs` | `CredentialStore`, `OAuthTokenStore` |
| foundation_auth | `SessionService` (sync wrapper over `SessionManager<CredentialStorage>`) | `src/server/services/session_service.rs` | `CredentialStore` |
| foundation_cedar | `KvPolicyStore<'a, S: KeyValueStore>` | `src/storage/kv_store.rs` | `KeyValueStore` |
| foundation_cedar | `SqlPolicyStore<'a>` { `store: &'a dyn QueryStore` } | `src/storage/sql_store.rs` | `QueryStore` |
| foundation_vectors | `SqlVectorStore<Q: QueryStore>` (`impl VectorStore`) | `src/sql_store.rs` | `QueryStore` |
| foundation_vectors | `LibsqlVectorStore<Q: QueryStore>` (`impl VectorStore`) | `src/libsql_vector_store.rs` | `QueryStore` |
| foundation_vectors | `schema::run_migrations(&dyn QueryStore)` | `src/schema/mod.rs` | `QueryStore` |
| foundation_cronjobs | `CronScheduler<QS: QueryStore>` (`save`, `load`) | `src/scheduler.rs` | `QueryStore` |
| foundation_nativeapis | `D1Delta` { `store: Blocking<D1Store>` }, sync VFS impls; async VFS impls that call the sync ones | `src/native/vfs/d1_delta/mod.rs` | `Blocking`, `BlobStore`, `KeyValueStore` |
| foundation_nativeapis | `R2Delta` { `store: Blocking<R2Store>` }, same shape | `src/native/vfs/r2_delta/mod.rs` | `Blocking`, `BlobStore`, `KeyValueStore` |
| foundation_nativeapis | `FjallDocumentStore<V>` implements sync `DocumentStore` | `src/shared/vfs/fjall_document_store.rs` | `DocumentStore` |
| foundation_deployment | `Deployable::Store: StateStore`; `Deployable::store()` → `NamespacedStore<Self::Store>`; `ProviderClient<S: StateStore, R>` | `src/traits.rs`, `src/provider_client.rs` | `StateStore` |
| foundation_deployment_docker | `deploy`/`destroy` call `NamespacedStore::{store_typed, get_typed, remove}` inside async blocks | `src/deployable.rs` | `StateStore` |
| foundation_deployment_hetzner | same | `src/deployable.rs` | `StateStore` |
| ewe_deployables | `CloudflareWorker`, `CloudRun*` deployables, same | `src/cloudflare/worker.rs`, `src/gcp/cloud_run.rs` | `StateStore` |
| foundation_proxy | `ProxyStateStore` { `store: FileStateStore` } `load`/`save`/`save_backend_state` (called from `ProxyServer::start` → `restore_persisted_state`, and `ProxyState::persist_backend_state` from the admin control path) | `src/persistence.rs`, `src/server.rs`, `src/state.rs`, `src/control.rs` | `StateStore` |
| foundation_db (internal) | `StoreStateTask`, `StoreStateIdentifierTask` (valtron `TaskIterator`s that drain `StateStore` streams) | `src/core/state/store_state_task.rs` | `StateStore` |
| foundation_db (internal) | `NamespacedStore<S: StateStore>` | `src/core/state/namespaced.rs` | `StateStore` |

Tests that reach the sync traits (all change with their crate):
foundation_db `tests/{blocking_tests, blocking_executor_tests, cleanup_tests,
default_stores_tests, document_store_tests, json_blobstore_tests,
json_file_default_tests, libsql_document_store_tests, libsql_storage_tests,
memory_json_store_tests, r2_blobstore_tests, turso_storage_tests,
d1_kvstore_tests, d1r2_document_store_tests, state_store_tests,
state_namespaced_tests, store_state_task}.rs`; foundation_auth
`tests/integration/{mod, e2e, idp_server, provider_admin, provider_service}`,
`tests/units/{pow_service, server_services/provisioning_service, shared/session}`,
`tests/{credential_store_tests, oauth_tests}.rs`; foundation_cedar
`tests/kv_store.rs`; foundation_cronjobs `tests/scheduler_tests.rs`;
foundation_vectors `tests/{sql_vector_store_tests, libsql_vector_store_tests}.rs`;
foundation_nativeapis `tests/{fjall_document_store, d1_delta_vfs, r2_delta_vfs}`;
foundation_deployment_docker/hetzner `tests/deployable_tests.rs`, hetzner
`tests/live_tests.rs`; ewe_deployables `tests/cloudflare_worker_integration.rs`;
foundation_proxy `tests/persistence_tests.rs`.

No sync-storage use in `apps/answerme-agent`, `examples/cf-agent-loop`,
`examples/cf-cloud-provider` (already on the async `AgentSession`) or
`examples/cf-login-app` (already uses `MigrationRunner::run_async` and
`AsyncCredentialStore`).

### 2.3 `.block()` on a storage future, and blocking `Default`s

| Where | What |
|-------|------|
| `foundation_db/src/native/turso_backend.rs` | `impl Default for TursoStorage`: `.block()` on `new(":memory:")` + `init_schema()` |
| `foundation_db/src/native/libsql_store.rs` | `impl Default for LibsqlStore`: `.block()` on `new_kv(":memory:")` + migrations |
| `foundation_db/src/native/libsql_store.rs` | `StateStore::init` and `StateStore::sync_remote` `.block()` on libsql futures |
| `foundation_db/src/native/mod.rs` | `create_state_store`: `.block()` on `LibsqlStore::from_env_state` |
| `foundation_db/src/native/d1_kvstore.rs` | `StateStore for D1Store` uses the blocking `execute_sql` (sync HTTP `send()`) |
| `foundation_db/src/native/r2_blobstore.rs` | `StateStore for R2Store` uses sync HTTP `send()` |
| `foundation_db/src/blocking.rs` | `Blocking<S>` itself (`sync_collect_one`, `.blocking()`) |

Callers that need the blocking `Default`s:
`foundation_ai/src/agentic/tools/agent.rs` (`AgentTool<D, M>` with
`D: AsyncDocumentStore + Default`, `M: MemoryStore + Default`, building each
sub-agent's stores with `D::default()` / `M::default()`), and the deprecated
`AgentSession::builder_for` (`foundation_ai/src/agentic/session.rs`).

`block_on_future` uses outside storage (unchanged, genuine boundaries):
`foundation_keychain/src/bin/keychain.rs` (sync `main`), the
`#[valtron]`/`#[valtron_test]`/docker macros, `foundation_proxy`'s
health/rpc_relay threads.

## 3. Per-crate changes

Notation: **before → after**. All async traits use `#[async_trait::async_trait]`
as their sync predecessors' async twins already do.

### 3.1 foundation_auth — IdP server (`--features server`)

`storage.rs`:

- Every free fn becomes `async` over `&dyn AsyncQueryStore`; the `_async`
  duplicates are deleted (one name per operation):
  - `fn find_client_by_id(&dyn QueryStore, &str) -> Result<Option<OAuthClient>, StorageOpError>`
    → `async fn find_client_by_id(&dyn AsyncQueryStore, &str) -> Result<Option<OAuthClient>, StorageOpError>`
  - same for `store_auth_code`, `find_auth_code`, `delete_auth_code`,
    `store_refresh_token`, `find_refresh_token_by_hash`,
    `mark_refresh_token_rotated`, `store_device_code`, `find_device_code`,
    `approve_device_code`, `find_user_by_email`, `update_user_lockout`,
    `find_user_by_id`, `update_user_profile`, `update_user_password`,
    `soft_delete_user`, `create_user`, `store_passkey`, `find_passkey_by_id`,
    `find_passkey_by_credential_id`, `update_passkey_counter`,
    `update_passkey_name`, `delete_passkey`, `store_tos_acceptance`,
    `find_tos_acceptance`, `find_latest_tos_acceptance`.
  - `fn find_passkeys_by_user(&dyn QueryStore, &str) -> Result<Vec<Passkey>, _>`
    and `find_passkeys_by_user_stream_async` → one
    `async fn find_passkeys_by_user(&dyn AsyncQueryStore, &str) -> Result<AsyncStorageItemStream<'static, StoredPasskey>, StorageOpError>` (streams; nothing collected).
  - Deleted: `find_user_by_email_async`, `store_passkey_async`, … (all `_async`
    names), `collect_one_row` (sync stream drain).
- `pub struct HandlerStorage<KV: KeyValueStore> { query_store: Arc<dyn QueryStore>, cache: KV }`
  → `pub struct HandlerStorage<KV: AsyncKeyValueStore> { query_store: Arc<dyn AsyncQueryStore>, cache: KV }`;
  `HandlerStorage::new(Arc<dyn AsyncQueryStore>, KV)`.
- `HandlerStorage` implements `AsyncPasskeyStore`, `AsyncTosStore`,
  `AsyncAuthStore` (moved from `AsyncHandlerStorage`). Its sync
  `PasskeyStore`/`TosStore`/`AuthStore` impls are deleted.
  `AsyncHandlerStorage` is deleted.
- Fixes on the way: `create_user` returns `StorageOpError::Parse` when the
  metadata can't be serialised (was silently `"null"`); `parse_user_row`
  reports a metadata parse error (was `.ok()`).

Services:

- `PowService<KV: KeyValueStore>` (sync) and `AsyncPowService<AKV>` → one
  `PowService<KV: AsyncKeyValueStore>` with
  `async fn generate_challenge(&self) -> Result<PowChallenge, StorageError>` and
  `async fn verify_solution(&self, &PowSolution) -> bool`. `AsyncPowService`
  is deleted. The expired-entry eviction logs its error instead of `let _ =`.
- `TosService<KV: KeyValueStore>` → `TosService<KV: AsyncKeyValueStore>`;
  `fn needs_acceptance(&self, &str) -> bool` → `async fn needs_acceptance(&self, &str) -> Result<bool, StorageOpError>`
  (a storage error is no longer reported as "needs acceptance");
  `fn accept(..) -> Result<(), StorageOpError>` → `async fn accept(..)`. The
  versions mutex is no longer held across the storage call.
- `WebAuthnService<S: AuthStore, KV: KeyValueStore>` →
  `WebAuthnService<S: AsyncAuthStore + Send + Sync, KV: AsyncKeyValueStore>`;
  `register_start`, `register_finish`, `auth_start`, `auth_finish`,
  `passkey_login_start`, `passkey_login_finish`, `delete_passkey`,
  `rename_passkey`, `list_passkeys` become `async fn` (same arguments and
  results). A user's passkeys are collected only where webauthn-rs needs the
  whole set (a slice); the count is bounded by what one user registers.
- `ProviderService<QS: QueryStore + 'static>` →
  `ProviderService<QS: AsyncQueryStore + ?Sized + 'static>` (so
  `ProviderService<dyn AsyncQueryStore>` works); `create`, `find_by_id`,
  `find_active`, `list_all`, `update`, `delete`, `set_secret`, `get_secret`
  become `async fn`. `find_active`/`list_all` keep returning `Vec` (an admin
  listing of configured providers, small and needed whole for the JSON reply).
- `ProvisioningService<QS: QueryStore + 'static>` →
  `ProvisioningService<QS: AsyncQueryStore + ?Sized + 'static>`; `provision`,
  `link_exists`, `get_user_links` become `async fn`.
- `SessionService` (sync methods over the sync `SessionManager`) → `async fn`
  `create_session`, `get_session`, `revoke_session`, `revoke_all_sessions`
  over the async `SessionManager` methods (§3.2).

Handlers:

- `IdpHandlerCore<KV: KeyValueStore = Blocking<MemoryStorage>>` →
  `IdpHandlerCore<KV: AsyncKeyValueStore + Clone = MemoryStorage>`. Its
  handlers are already `async fn`; each storage/service call gains `.await`.
  `lookup_upstream_provider`, `provision_from_upstream_profile`,
  `find_user_by_email_lax` become `async fn` over `&dyn AsyncQueryStore`.
- Bugs fixed while there: `login` stops discarding the
  `update_user_lockout` result and computes `attempts_remaining` from the
  updated user; `request_password_reset` stops discarding a storage error
  (the reply still doesn't reveal whether the email exists);
  `provision_from_upstream_profile` inserts into `username` (the
  `preferred_username` column doesn't exist) and stops discarding the lookup
  error.
- `provider_admin::{list_all, get_one, create, update, delete, set_secret}<QS: QueryStore>`
  → `async fn …<QS: AsyncQueryStore + ?Sized + 'static>`.
- `IdpServer<KV: KeyValueStore + 'static = Blocking<MemoryStorage>>` →
  `IdpServer<KV: AsyncKeyValueStore + Clone + 'static = MemoryStorage>`.
  `http_app` stores `Arc<HandlerStorage<KV>>` in the `ContextBag`.
- `ServeAdapter` → `ServeAdapter<KV: AsyncKeyValueStore + Clone = MemoryStorage>`;
  `create` looks up `Arc<HandlerStorage<KV>>` (what `IdpServer` and the tests
  store; the old lookup of `HandlerStorage<_>` always missed, because the bag
  keys by exact type) and panics if it is absent, like the `IdpConfig` lookup.
  The silent empty `NoOpStore` fallback is deleted.
- **The 17 failing IdP tests (backlog §12) are fixed here.** Two causes,
  found while converting: (1) `run_sync` mapped every handler `IdpError`
  into `Internal` (500), so each 4xx came back as 500; (2) the server hands
  request bodies over as a lazy stream (`SendSafeBody::Stream`/`ChunkedStream`)
  and the handlers read only `Text`/`Bytes`, so every form/JSON body looked
  empty. `ServeAdapter::serve` now reads the body bounded by the new
  `IdpConfig::max_request_body_bytes` (default 1 MiB, nginx's
  `client_max_body_size`; over the limit → `413`, new
  `IdpError::PayloadTooLarge`), and `run_sync` returns the handler's own
  error. Two e2e tests asserted responses that contradict spec 48 (reset is
  `202`, registration returns `id`) and are corrected.
- **Sync boundary that stays:** `foundation_http`'s `Serve::serve` is a sync
  trait. `ServeAdapter::serve` already runs the async `dispatch` on valtron
  (`execute` + `collect_one`); that stays, unchanged.

### 3.2 foundation_auth — shared credential/session layer

The sync `CredentialStore` is the last thing in foundation_auth that needs
`Blocking`. It goes, with its dependents:

- `trait CredentialStore` (sync) — **deleted**. `AsyncCredentialStore` is the
  only credential trait.
- `impl CredentialStore for CredentialStorage` (behind `blocking_boundary!`,
  via `Blocking<StorageProvider>`) — deleted.
- `trait OAuthTokenStore: CredentialStore` → `trait OAuthTokenStore: AsyncCredentialStore`
  (`#[async_trait(?Send)]`, matching `AsyncCredentialStore`), all helpers
  `async fn` (`store_oauth_token`, `get_oauth_token`, `delete_oauth_token`,
  `store_oauth_state`, `get_oauth_state`, `delete_oauth_state`); blanket impl
  for `T: AsyncCredentialStore`.
- `SessionManager<S>`: the sync block `impl<S: CredentialStore>`
  (`create_session`, `get_session`, `revoke_session`, `revoke_all_sessions`) is
  deleted; the async block (`*_async`) stays. Rename question: see §9 Q2.
- `AsyncCredentialStore` (and `OAuthTokenStore`) futures become `Send`
  natively (`#[async_trait]`) and stay `?Send` on wasm
  (`#[cfg_attr(target_family = "wasm", async_trait(?Send))]`). Natively the
  session/auth futures then run on valtron's thread pool (e.g. inside the IdP
  handler tasks and `#[valtron_test]`); the wasm-only D1 impls keep `?Send`.
- `AuthManager`: `init_from_store` (reads storage) becomes `async fn`;
  `get_valid_token` reads only the in-memory token and stays sync;
  `authenticate`, `refresh`, `logout` (already async) await the async store;
  the private `persist_token`, `persist_jwt_token`, `clear_stored_tokens`
  become async and their errors are no longer dropped. New accessor
  `jwt_manager()`. Its inline `#[cfg(test)]` module moves to
  `tests/auth_manager_tests.rs` (public API only). `AuthManager` stops being compiled out on JS-hosted wasm
  (backlog §12 entry "`AuthManager` calls the sync `CredentialStore`" is fixed
  and deleted).
- `foundation_db::wasm::WasmCredentialStore` / `wasm::session` (their docs say
  they exist only because the sync trait did): left as is in this feature;
  backlog entry to fold them into `CredentialStorage` (§6).

### 3.3 foundation_cedar (`--features db`)

`PolicyStore` is the trait the storage-backed policy sources implement; the
SQL/KV ones do I/O, so it becomes async:

- `pub trait PolicyStore: Debug + Send + Sync { fn load_policies(&self) -> Result<String, CedarError>; fn load_schema(..); fn load_entities(..) -> Result<Option<String>, _>; fn version(..) }`
  → `#[async_trait] pub trait PolicyStore: Debug + Send + Sync` with the same
  four methods as `async fn`.
- `InMemoryPolicyStore`, `FilePolicyStore` implement the async trait (bodies
  unchanged; the file store reads small policy files with `std::fs`).
- `pub fn load_from_store(&dyn PolicyStore) -> Result<CedarEngine, CedarError>`
  → `pub async fn load_from_store(&dyn PolicyStore) -> Result<CedarEngine, CedarError>`.
- `KvPolicyStore<'a, S: KeyValueStore>` → `KvPolicyStore<'a, S: AsyncKeyValueStore>`
  (reads with `get_async`).
- `SqlPolicyStore<'a> { store: &'a dyn QueryStore }` →
  `SqlPolicyStore<'a> { store: &'a dyn AsyncQueryStore }`;
  `fn ensure_table(&self)` → `async fn ensure_table(&self)`; row reads pull
  the first row from the async stream.
- `Cargo.toml`: adds `async-trait`; `db` now pulls `foundation_db` and
  `futures-lite` (the unused optional `foundation_core` dependency goes);
  dev-dependencies `foundation_core` (for `#[valtron_test]`),
  `foundation_db` with `turso`, and `tempfile`.
- `SqlPolicyStore::load_entities` reads the nullable `entities_json` as
  `Option<String>` (NULL or empty → `None`); it used to fail on NULL.
- New `tests/sql_store.rs`: the first tests of `SqlPolicyStore`, over a
  temporary Turso database.
- Tests: `tests/{kv_store, traits, resolver, file_store}.rs` become
  `#[valtron_test]` async tests (or drive the future with `block_on_future`
  where a sync test is wanted).

### 3.4 foundation_vectors (`--features sql-store`)

The two SQL-backed stores move from the sync `VectorStore` to the existing
`AsyncVectorStore`:

- `SqlVectorStore<Q: QueryStore>` → `SqlVectorStore<Q: AsyncQueryStore + ?Sized>`;
  `fn new(Arc<Q>, VectorStoreConfig) -> Result<Self, _>` (runs migrations
  synchronously) → `async fn new(..)`. `impl VectorStore` → `impl AsyncVectorStore`
  (`insert_async`, `delete_async`, `search_async`, `get_async`, `len_async`,
  `config`).
- `LibsqlVectorStore<Q: QueryStore>` → `LibsqlVectorStore<Q: AsyncQueryStore + ?Sized>`;
  `new` → `async fn new`; `impl VectorStore` → `impl AsyncVectorStore`;
  `with_namespace_strategy` unchanged.
- `schema::run_migrations(&dyn QueryStore) -> Result<usize, _>` →
  `async fn run_migrations(&dyn AsyncQueryStore) -> Result<usize, _>`.
- `AsyncVectorStore::len_async` returns `usize` and so can't report a storage
  error: backlog entry (the SQL stores map an error to 0 today; this feature
  keeps the signature and logs). See §9 Q4.
- Tests: `tests/{sql_vector_store_tests, libsql_vector_store_tests}.rs` await
  the stores directly (no `Blocking`).
- Search no longer collects the namespace: a new public `TopK` accumulator
  (`src/flat.rs`; `flat_top_k`/`flat_top_k_owned` now use it) ranks rows as
  they stream in, O(k) memory.
- Errors stop being swallowed: metadata encode/decode and libSQL's
  `vector_extract` parse report `VectorStoreError::Backend` (a bad component
  used to be dropped, shifting every later dimension).
- `sql-store` now pulls `futures-lite` instead of `foundation_core`.

### 3.5 foundation_cronjobs

- `CronScheduler<QS: QueryStore + 'static>` → `CronScheduler<QS: AsyncQueryStore + ?Sized + 'static>`;
  `fn save(&self) -> Result<(), StorageError>` → `async fn save(&self)`;
  `fn load(&mut self) -> Result<(), StorageError>` → `async fn load(&mut self)`.
- `load` stops ignoring errors: a failed row read or column decode is
  returned (was `if let Next(Ok(row))` + `unwrap_or(0)`).
- Tests (`tests/scheduler_tests.rs`) use `MemoryStorage` directly, async.

### 3.6 foundation_db — `MigrationRunner`, `StorageCleanup`

- `MigrationRunner::run(&self, &dyn QueryStore) -> StorageResult<usize>` —
  **deleted**. `run_async(&self, &dyn AsyncQueryStore)` stays (rename question
  §9 Q1).
- `pub trait StorageCleanup: QueryStore` + blanket impl → deleted, replaced by
  ```rust
  #[async_trait]
  pub trait AsyncStorageCleanup: AsyncQueryStore {
      async fn cleanup_expired_sessions(&self, now: i64, max_inactive_seconds: Option<i64>) -> StorageResult<u64>;
      async fn cleanup_expired_jwt_tokens(&self, now: i64) -> StorageResult<u64>;
      async fn cleanup_expired_verification_tokens(&self, now: i64) -> StorageResult<u64>;
      async fn cleanup_expired_oauth_states(&self, now: i64) -> StorageResult<u64>;
      async fn cleanup_expired_magic_links(&self, now: i64) -> StorageResult<u64>;
      async fn cleanup_expired_email_otps(&self, now: i64) -> StorageResult<u64>;
      async fn cleanup_old_rate_limits(&self, now: i64, window_seconds: i64) -> StorageResult<u64>;
      async fn run_full_cleanup(&self, config: &CleanupConfig) -> StorageResult<CleanupStats>;
  }
  impl<T: AsyncQueryStore + ?Sized> AsyncStorageCleanup for T {}
  ```
- New `CleanupConfig` (CLAUDE.md rule 8: limits are config): fields
  `session_max_inactive`, `jwt_token_retention`, `verification_token_ttl`,
  `oauth_state_ttl`, `magic_link_ttl`, `email_otp_ttl`, `rate_limit_window`
  (all `Duration`), builder methods, `Default` = today's hard-coded values
  (24 h, 30 d, 1 h, 10 min, 1 h, 10 min, 1 h) with each documented.
- `cleanup_old_rate_limits` takes `now` instead of reading `SystemTime`
  (which panics on `wasm32-unknown-unknown` and `unwrap`s); `run_full_cleanup`
  takes the clock from the wasm-safe helper the in-memory stores use.

### 3.7 foundation_nativeapis (`vfs-d1`, `vfs-r2`, `vfs-fjall`)

- `D1Delta { store: Blocking<D1Store> }` → `D1Delta { store: Arc<D1Store> }`;
  `R2Delta { store: Blocking<R2Store> }` → `R2Delta { store: Arc<R2Store> }`.
- The **async** VFS impls (`AsyncVfsFileSystem`, `AsyncDeltaStore`,
  `AsyncVfsFile`, `AsyncVfsDirectory` for `AsyncD1File`/`AsyncR2File`/…)
  become the real implementations, awaiting `AsyncBlobStore` /
  `AsyncKeyValueStore` (today they call the sync impls, i.e. block inside
  async code).
- The sync VFS impls (`VfsFileSystem`, `DeltaStore`, `VfsFile`, `VfsDirectory`,
  `SeekableVfsFile` for the D1/R2 types) are **deleted**; sync callers wrap the
  delta in the existing `SyncFs<…>` bridge (`shared/vfs/sync_bridge.rs`), which
  waits with `exec_async` — the VFS's established sync boundary (native only).
  Tests `d1_delta_vfs`/`r2_delta_vfs` assert the async traits and go through
  `SyncFs` for sync checks.
- `FjallDocumentStore<V>`: `impl DocumentStore` → `impl AsyncDocumentStore`
  (bodies are local fjall/VFS calls, ready at once).
- The stubbed async methods there (`list_async` returning `[]`,
  `seek_async` returning 0, …) are recorded in backlog §13; they are not
  introduced by this feature.

### 3.8 foundation_db — async `StateStore` and its callers

```rust
/// A lazily pulled stream of state results (Send natively, !Send on wasm, like
/// every async storage stream).
pub type StateStoreStream<T> = AsyncStorageItemStream<'static, T>;

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn init(&self) -> StorageResult<()>;
    async fn list(&self) -> StorageResult<StateStoreStream<String>>;
    async fn count(&self) -> StorageResult<usize>;
    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>>;
    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>>;
    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>>;
    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()>;
    async fn delete(&self, resource_id: &str) -> StorageResult<()>;
    async fn sync_remote(&self) -> StorageResult<()> { Ok(()) }
    async fn list_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<String>>;   // default: filter list()
    async fn count_by_prefix(&self, prefix: &str) -> StorageResult<usize>;                   // default: count list_by_prefix()
    async fn all_by_prefix(&self, prefix: &str) -> StorageResult<StateStoreStream<ResourceState>>;
    async fn find_by_kind(&self, kind: &str) -> StorageResult<StateStoreStream<ResourceState>>;
    async fn find_by_status(&self, status: &str) -> StorageResult<StateStoreStream<ResourceState>>;
    async fn find_by_provider(&self, provider: &str) -> StorageResult<StateStoreStream<ResourceState>>;
    async fn delete_by_prefix(&self, prefix: &str) -> StorageResult<usize>;
}
```

Single-value results (`get`, `count`, `set`, `delete`) return the value, not a
one-item stream. Multi-row results stay streams (never collected).

Implementations:

- `FileStateStore` (`core/state/file.rs`): async impl over its existing file
  logic (no sync side door; decision Q3).
- `NamespacedStore<S: StateStore>`: async impl; `store_typed`, `get_typed`,
  `remove` become `async fn`.
- `LibsqlStore`: `init` / `sync_remote` await libsql (the `.block()`s go).
- `D1Store`: every `StateStore` method uses `execute_sql_async`; the blocking
  `execute_sql` (sync `send()`) is deleted.
- `R2Store`: every `StateStore` method uses the `*_object_async` helpers; the
  sync `send()` calls go.
- `native::create_state_store(project, project_dir, provider, stage) -> Result<Box<dyn StateStore>, _>`
  → `async fn create_state_store(..)` (awaits `LibsqlStore::from_env_state`);
  `D1Store::from_env_state` / `R2Store::from_env_state` unchanged (no I/O).
- `StoreStateTask` / `StoreStateIdentifierTask`: after the inner task is
  `Ready`, they hold the store write as a
  `FutureTask<Pin<Box<dyn Future<Output = StorageResult<()>> + Send>>>` in
  their state and poll it from `next_status` (the `AgentLoop` pattern from
  #132): `Ready` finishes, `Pending`/`Depends` pass through to the executor so
  a waiting store parks the task. The future owns an `Arc<St>` clone.

Callers:

- foundation_deployment: `Deployable::Store: StateStore` (bound unchanged,
  trait now async); `ProviderClient` unchanged (holds `Arc<S>`); doc examples
  updated.
- foundation_deployment_docker / _hetzner / ewe_deployables: in `deploy` /
  `destroy` (already `BoxFuture`s), `store.store_typed(..)` →
  `store.store_typed(..).await`, same for `get_typed`, `remove`.
- foundation_proxy (decision Q3: async startup): `ProxyStateStore::{load,
  save, save_backend_state}` become `async fn` awaiting `StateStore`;
  `restore_persisted_state` becomes an async step of startup, and
  `ProxyState::persist_backend_state` an `async fn` awaited by the admin
  control path (`src/control.rs`). Every function on those paths that now
  awaits becomes async up to the process entry point; only that entry point
  (a sync `main` / `ProxyServer::start` called from sync code) waits, with
  `block_on_future` / `.block()` behind `blocking_boundary!`. The exact
  boundary is found while implementing T8 and recorded here.
- Tests: `state_store_tests`, `state_namespaced_tests`, `store_state_task`,
  the deployable tests, `cloudflare_worker_integration`, proxy
  `persistence_tests` become async (`#[valtron_test]`).

### 3.9 foundation_ai — agent tool store factory; `builder_for`

```rust
/// Builds a fresh document store and memory store for one sub-agent.
pub trait SessionStoreFactory<D, M>: Send + Sync + 'static {
    fn create(&self) -> Pin<Box<dyn Future<Output = StorageResult<(D, M)>> + Send + 'static>>;
}

/// Factory for stores whose `Default` doesn't block (the in-memory ones).
pub struct DefaultStores;
impl<D: Default + Send + 'static, M: Default + Send + 'static> SessionStoreFactory<D, M> for DefaultStores { … }

/// Any `Fn() -> impl Future<Output = StorageResult<(D, M)>>` is a factory.
impl<D, M, F, Fut> SessionStoreFactory<D, M> for F where F: Fn() -> Fut + Send + Sync + 'static, Fut: Future<Output = StorageResult<(D, M)>> + Send + 'static { … }
```

- `AgentTool<D, M>` gains `stores: Arc<dyn SessionStoreFactory<D, M>>`.
  `AgentTool::new(router, depth, max_depth, default_model, output_base, user, child_tools)`
  keeps its signature but requires `D: Default, M: Default` (uses
  `DefaultStores`); new `AgentTool::with_store_factory(router, …, child_tools, stores)`
  has no `Default` bound. `start` awaits `self.stores.create()` and maps a
  failure to `ToolError::Execution`. The `Default` bounds on the `impl`
  blocks at `agent.rs:124/525/672` move to `new` only.
  `register_agent_tool` and `harness/tools.rs` follow the same split.
- `AgentSession::builder_for` (deprecated) — **deleted**.
- Then `impl Default for TursoStorage` and `impl Default for LibsqlStore` —
  **deleted** (their docs point to `TursoStorage::new(":memory:")` +
  `init_schema()`, and `LibsqlStore::new_kv(":memory:", None)` + migrations).

### 3.10 foundation_db — what remains of `Blocking`

Once §3.1–§3.9 land, nothing in the workspace uses `Blocking<S>`:
foundation_auth (§3.1/§3.2), cedar/cronjobs/vectors tests (§3.3–§3.5),
nativeapis (§3.7). It is deleted with the sync traits (§4).

## 4. Deleting the sync traits

Order (each step compiles the whole workspace before the next):

1. After §3.1–§3.7: delete from `foundation_db`:
   - `KeyValueStore`, `BlobStore`, `QueryStore`, `RateLimiterStore`,
     `DocumentStore` (`core/storage_provider.rs`) and their re-exports in
     `lib.rs`;
   - `StorageItemStream` if nothing else uses it (grep first; it is the sync
     valtron stream alias);
   - `StorageCleanup` (replaced in §3.6);
   - `PasskeyStore`, `TosStore`, `AuthStore` (`core/auth_store.rs`);
   - `MigrationRunner::run`;
   - `Blocking<S>`, `blocking.rs`, `HAS_BLOCKING` and the compile-time
     asserts on it in `lib.rs` (keep `BLOCKING_BOUNDARY` usage only if
     something else in the crate needs it);
   - tests `blocking_tests.rs`, `blocking_executor_tests.rs`; every other
     foundation_db test that went through `Blocking` awaits the store.
2. After §3.8: the old `StateStoreStream` (iterator of `ThreadedValue`) alias.
3. After §3.9: the two blocking `Default` impls.

If any caller can't move, stop and ask the owner (with the reason and options)
instead of keeping a trait.

**Rename `Async*` → plain names (owner decision, §9 Q1).** Options:

- **A. Rename now, in this PR.** `AsyncKeyValueStore` → `KeyValueStore`,
  methods `get_async` → `get`, `AsyncQueryStore` → `QueryStore`
  (`query_async` → `query`), etc., plus `MigrationRunner::run_async` → `run`,
  `AsyncPasskeyStore` → `PasskeyStore`, `AsyncStorageCleanup` →
  `StorageCleanup`, `AsyncCredentialStore` → `CredentialStore`. One very large
  mechanical diff (foundation_db, foundation_ai, foundation_auth, keychain,
  nativeapis, vectors, cedar, cronjobs, examples). Reviewers see the semantic
  and the mechanical change together.
- **B. Rename in a follow-up PR** right after this one merges: this PR stays
  reviewable (semantic changes only); the rename PR is purely mechanical and
  can be reviewed by diff stats. Short window with `Async*` names and no sync
  counterpart.
- **C. Keep the `Async*` names.** No churn; the names stay longer than they
  need to and suggest a sync twin exists.

**Decided (owner, 2026-10-21): B.** The rename, including the
`SessionManager::*_async` suffix, is a separate mechanical PR right after this
one merges. Not in this PR.

## 5. Breaking public API changes

foundation_db:
- Removed: `KeyValueStore`, `BlobStore`, `QueryStore`, `RateLimiterStore`,
  `DocumentStore`, `StorageCleanup`, `PasskeyStore`, `TosStore`, `AuthStore`,
  `Blocking`, `HAS_BLOCKING`, `MigrationRunner::run`,
  `impl Default for TursoStorage`, `impl Default for LibsqlStore`, the sync
  `StateStoreStream` alias.
- Added: `AsyncStorageCleanup`, `CleanupConfig`.
- Changed: `StateStore` is async (signatures in §3.8);
  `native::create_state_store` is `async`; `NamespacedStore::{store_typed,
  get_typed, remove}` are `async`; `StoreStateTask`/`StoreStateIdentifierTask`
  bounds follow the async trait.

foundation_auth:
- Removed: `CredentialStore`, `AsyncHandlerStorage`, `AsyncPowService`, the
  sync `SessionManager::{create_session, get_session, revoke_session,
  revoke_all_sessions}`, every `server::storage::*_async` fn.
- Changed: `server::storage::*` fns async over `&dyn AsyncQueryStore`;
  `HandlerStorage<KV: AsyncKeyValueStore>` with `Arc<dyn AsyncQueryStore>`;
  `PowService`, `TosService`, `WebAuthnService`, `ProviderService`,
  `ProvisioningService`, `SessionService`, `provider_admin::*` async;
  `TosService::needs_acceptance` returns `Result<bool, _>`;
  `IdpHandlerCore`/`IdpServer` default `KV = MemoryStorage`;
  `OAuthTokenStore: AsyncCredentialStore` with async helpers;
  `AsyncCredentialStore` futures `Send` natively;
  `AuthManager::init_from_store` async; `AuthManager` no longer gated on
  `blocking_boundary!` (it now exists on JS-hosted wasm).

foundation_cedar: `PolicyStore` async; `load_from_store` async;
`KvPolicyStore<S: AsyncKeyValueStore>`; `SqlPolicyStore` over
`&dyn AsyncQueryStore`, `ensure_table` async.

foundation_vectors: `SqlVectorStore`/`LibsqlVectorStore` bounds
`Q: AsyncQueryStore`, `new` async, implement `AsyncVectorStore` (not
`VectorStore`); `schema::run_migrations` async.

foundation_cronjobs: `CronScheduler<QS: AsyncQueryStore>`, `save`/`load` async.

foundation_nativeapis: `D1Delta`/`R2Delta` implement only the async VFS
traits (sync via `SyncFs`); `FjallDocumentStore` implements
`AsyncDocumentStore` (not `DocumentStore`).

foundation_ai: `AgentSession::builder_for` removed; `AgentTool` gains
`with_store_factory`; `SessionStoreFactory`, `DefaultStores` added.

foundation_deployment & deployables: `Deployable::store()` returns a
`NamespacedStore` whose helpers are async.

## 6. Backlog §11 small items and CI

- **In-memory rate limiters never expire a busy window**
  (`MemoryStorage::record_rate_limit_async`, `MemoryJsonStore::record_rate_limit_async`,
  `src/core/backends/memory{,_json}.rs`): start a new window (count 1,
  `window_start = now`) only when `now - window_start >= window`; otherwise
  increment and keep `window_start`. The window length is a field on the
  store's config with a documented default (rule 8), not a constant. Tests in
  `tests/memory_storage_tests.rs` / `memory_json_store_tests.rs`: a key hit
  every half-window resets after one full window.
- **`worker` unused natively:** move `worker` to
  `[target.'cfg(target_family = "wasm")'.dependencies]` in
  `backends/foundation_db/Cargo.toml`. Done when
  `cargo check -p foundation_db --all-features` has no unused-dependency
  warning for it.
- **`async-stream` / `foundation_netio` unused on wasm:** move both to
  `[target.'cfg(not(target_family = "wasm"))'.dependencies]` (keeping their
  feature wiring). Done when `cargo check --target wasm32-unknown-unknown -p
  foundation_db --no-default-features --features wasm-bindgen-storage` has no
  unused-dependency warning.
- **CI runs** (`ci/packages.toml`):
  - `[packages.foundation_vectors] runs = ["", "sql-store"]`;
  - `[packages.foundation_db] runs = ["", "libsql"]`;
  - `[packages.foundation_nativeapis] runs = ["", "vfs-native,vfs-d1,vfs-r2"]`
    (merged with any existing entries; the D1/R2 tests skip without the
    emulator);
  - `[packages.foundation_auth] runs = ["", "server-test"]` — the suite
    passes after T1 (138/138).
  - An examples job is out of scope here (it needs its own workflow); it stays
    in the backlog.
- New backlog entries recorded while surveying (not fixed here):
  - foundation_auth `PowService::verify_solution` compares leading zero
    **bits** with `difficulty / 8` (bytes), so the effective difficulty is
    ~8× lower than configured (§12);
  - foundation_auth `header_name_eq` matches headers by `Debug` formatting
    (§12);
  - foundation_auth `WasmCredentialStore` / `wasm::session` in foundation_db
    duplicate `CredentialStorage` now the sync trait is gone (§12);
  - foundation_vectors `AsyncVectorStore::len_async` can't report errors (§9 Q4);
  - foundation_nativeapis D1/R2 async VFS stubs (`list_async`, `seek_async`,
    `get_entry_async`) (§13);
  - foundation_cronjobs `CronJob::next_fire` is a 5-minute stub (§14, if not
    already there).

## 7. Task list

Each task ends with: build + clippy + tests of the touched crates, commit,
push, update `progress.md`/`learnings.md` and the PR checklist.

- **T1 — foundation_auth IdP server on the async traits.**
  Files: `src/server/{storage.rs, idp_server.rs, mod.rs}`,
  `src/server/services/{pow,tos,webauthn,provider,provisioning}_service.rs`,
  `src/server/handlers/{core,provider_admin,serve_adapter}.rs`; tests
  `tests/integration/{mod,e2e,idp_server,provider_admin,provider_service}`,
  `tests/units/{pow_service,server_services/provisioning_service}`.
  Done when: no `QueryStore`/`KeyValueStore`/`Blocking` in `src/server`;
  `ci.py run foundation_auth` passes; `--features server-test` run recorded
  (pass count and the IdP failures, if any remain).
- **T2 — foundation_auth credential/session layer async-only.**
  Files: `src/shared/{credential_store,session,auth_manager}.rs`,
  `src/server/services/session_service.rs`, `src/lib.rs`; tests
  `tests/{credential_store_tests,oauth_tests}.rs`, `tests/units/shared/session.rs`.
  Done when: `CredentialStore` is gone, no `Blocking` in foundation_auth, the
  wasm runs (`wasm`, `wasm-bindgen-session`, `wasm-bindgen-oauth`) build, and
  `examples/cf-login-app` builds for wasm32.
- **T3 — foundation_cedar async `PolicyStore`.** Files: `src/storage/*`,
  `src/lib.rs`, `Cargo.toml`; tests `tests/*.rs`. Done when
  `ci.py run foundation_cedar` and `cargo test -p foundation_cedar --features db`
  pass.
- **T4 — foundation_vectors SQL stores on `AsyncVectorStore`.** Files:
  `src/{sql_store,libsql_vector_store}.rs`, `src/schema/mod.rs`; tests
  `tests/{sql_vector_store_tests,libsql_vector_store_tests}.rs`. Done when
  `ci.py run foundation_vectors` and `cargo test --features sql-store` pass.
- **T5 — foundation_cronjobs async `CronScheduler`.** Files: `src/scheduler.rs`;
  tests `tests/scheduler_tests.rs` (+ a load-error test). Done when
  `ci.py run foundation_cronjobs` passes.
- **T6 — `MigrationRunner::run` removed; `AsyncStorageCleanup` + `CleanupConfig`.**
  Files: `foundation_db/src/core/{cleanup.rs, schema/migrations.rs}`; tests
  `tests/cleanup_tests.rs`, migration tests. Done when nothing calls `run`
  and the cleanup tests pass against `MemoryStorage` and Turso.
- **T7 — foundation_nativeapis off the sync traits.** Files:
  `src/native/vfs/{d1_delta,r2_delta}/mod.rs`,
  `src/shared/vfs/fjall_document_store.rs`; tests `tests/{d1_delta_vfs,
  r2_delta_vfs, fjall_document_store}`. Done when `ci.py run
  foundation_nativeapis` and `cargo test --features vfs-native,vfs-d1,vfs-r2`
  and `--features vfs-fjall` pass.
- **T8 — async `StateStore`.** Files: `foundation_db/src/core/state/{traits,
  file, namespaced, store_state_task}.rs`, `src/native/{libsql_store,
  d1_kvstore, r2_blobstore, mod}.rs`; callers in foundation_deployment,
  foundation_deployment_docker, foundation_deployment_hetzner, ewe_deployables;
  foundation_proxy's persistence, startup (`server.rs`) and admin control path
  (`state.rs`, `control.rs`) made async up to the entry point; their tests. Done when no `.block()` remains in
  foundation_db `src/`, and `ci.py run` passes for foundation_db,
  foundation_deployment, foundation_deployment_docker,
  foundation_deployment_hetzner, ewe_deployables, foundation_proxy.
- **T9 — agent tool store factory; remove `builder_for` and the blocking
  `Default`s.** Files: `foundation_ai/src/agentic/{tools/agent.rs, session.rs}`,
  `foundation_ai/src/harness/tools.rs`,
  `foundation_db/src/native/{turso_backend,libsql_store}.rs`; tests
  `foundation_ai/tests/agentic/agent_tool_tests.rs` (+ a factory test, + a
  factory-error test), foundation_db `default_stores_tests.rs`. Done when
  `ci.py run foundation_ai` (steps one at a time, see §8) passes and
  cf-agent-loop/cf-cloud-provider build for wasm32.
- **T10 — delete the sync traits and `Blocking`.** Files:
  `foundation_db/src/{lib.rs, blocking.rs, core/storage_provider.rs,
  core/auth_store.rs, core/cleanup.rs}`, every foundation_db test using them,
  foundation_db docs. Done when the workspace and the five example
  workspaces build, and the grep in §2 finds no sync storage trait.
- **T11 — §11 small items:** rate-limiter window fix (+ tests),
  `worker` / `async-stream` / `foundation_netio` target dependencies.
  Done when the tests pass and the two `cargo check` runs in §6 are clean.
- **T12 — CI runs** in `ci/packages.toml` (§6). Done when each new run passes
  locally with `ci.py run <crate> --step test`.
- **T13 — docs and records:** foundation_db/foundation_auth docs and READMEs,
  `docs/bug-backlog.md` (delete fixed §11/§12 entries, add the new ones),
  spec 50 `progress.md`/`learnings.md`, this feature's status.
- **T14 — moved out:** the `Async*` → plain-name rename (and the
  `SessionManager::*_async` suffix) is a follow-up PR after this one merges
  (decision Q1/Q2).

## 8. Verification

Per touched crate (with `CARGO_INCREMENTAL=0`, one crate at a time, pruning
stale `target/uat/build/*/` hashes when free disk drops below ~4 GB):

```
PROTOC=~/.local/share/mise/installs/protoc/36.2/bin/protoc \
python3 scripts/ci/ci.py run <crate> --step build --step clippy --step test --step wasm
```

Crates: foundation_db, foundation_auth, foundation_cedar, foundation_vectors,
foundation_cronjobs, foundation_nativeapis, foundation_ai (each step run on
its own with `target/uat` wiped in between, for disk), foundation_keychain,
foundation_deployment, foundation_deployment_docker,
foundation_deployment_hetzner, ewe_deployables, foundation_proxy.

Extra feature runs: `cargo test -p foundation_auth --features server-test`,
`cargo test -p foundation_cedar --features db`,
`cargo test -p foundation_vectors --features sql-store`,
`cargo test -p foundation_db --features libsql`,
`cargo test -p foundation_nativeapis --features vfs-native,vfs-d1,vfs-r2`.

Examples (each in its own `CARGO_TARGET_DIR`, deleted afterwards):
`apps/answerme-agent` (native), `examples/cf-agent-loop`,
`examples/cf-cloud-provider`, `examples/cf-login-app`
(`cargo build --target wasm32-unknown-unknown`).

Known pre-existing failures (not caused by this feature):
- foundation_auth `server-test`: 17 IdP HTTP tests failed on master (backlog
  §12) — fixed in T1 (body reading and error mapping, §3.1);
- `--features server-native` doesn't link on this host (OpenSSL 3 symbols).

## 9. Risks and decisions

Owner decisions (2026-10-21):

- **Q1 — rename `Async*` to the plain names: decided B.** A separate,
  mechanical follow-up PR right after this one merges; not in this PR.
- **Q2 — drop the `SessionManager::*_async` suffix: decided,** in that same
  follow-up rename PR.
- **Q3 — foundation_proxy state: decided, async startup.** The proxy's
  startup and admin paths that touch state become async and `.await` the
  async `StateStore`; only the true process entry point waits, through
  `block_on_future` / `.block()` behind `blocking_boundary!` (§3.8). No sync
  file methods on `FileStateStore`.
- **Q4 — `AsyncVectorStore::len_async -> usize` can't report errors:
  decided,** record in the backlog and leave it.

Risks:

- **Risk: `FileStateStore` / `FilePolicyStore` do blocking `std::fs` inside
  `async fn`.** Files are small and local; acceptable for now, recorded as a
  backlog item (an async fs seam would fix it).
- **Risk: `#[async_trait]` futures are `Send`.** `AsyncStorageItemStream` is
  `!Send` on wasm; methods holding one across an `.await` need
  `foundation_compact::SendWrapper` on wasm (learned in #132). The state store
  and the auth server aren't built for wasm today; the cedar/vectors/
  cronjobs crates are checked with their wasm runs where they have one.
- **Risk: `StoreStateTask` rewrite** changes when the store write happens
  relative to the task's output; the existing `store_state_task` tests pin
  the order and must keep passing unchanged.
- **Risk: size of the PR.** Commits are per crate (T1…T13), each building on
  its own, so review can go commit by commit.
