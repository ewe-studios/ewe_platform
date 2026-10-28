# Bug and limitation backlog

This is the single list of known bugs and limitations that are still open.

CLAUDE.md rule 9 says every bug found gets tracked and scheduled. This file is where they are tracked. Each entry names the crate and file, says what is wrong, and gives the likely fix.

How to use it:
- When you find a new bug, add it here in the same change, even if you are not fixing it.
- When you fix one, delete its entry in the PR that fixes it and name the PR in the commit message.
- Group entries by crate. Mark limits that are still hard-coded constants with **(rule 8)**.

Last refreshed: 2026-10-27, on `fix/foundational-backlog`: sections 1–5 checked entry by entry against master `38398e928` (fixed entries removed, partly fixed ones rewritten; the table is in that branch's PR). Before that: 2026-10-22, on `nativeapis/async-vfs` (spec 50 F05 T2, the VFS async-first); 2026-10-21, on `ai/async-stores` and `db/async-stores`; 2026-10-20, after PR #96. In flight on branches:
- `fix/foundational-backlog`: sections 1–5 (foundation_connectrpc, netio, http, core/valtron, nostd/wasm/iogate).
---

## 1. foundation_connectrpc

- **HTTP/1.1 RPC holds a worker for its whole life.**
  - `foundation_http`'s `Serve` trait (`shared/serve/mod.rs:72`) is
    synchronous, so a long-lived h1 stream occupies a valtron worker until it
    ends. Enough of them use up the pool.
  - Needs an async or pending `Serve` contract.
- **Transports with no pollable fd fail.** `native/conn_io.rs` readiness waits
  (`fd()`, `gone_now`, `ready_now`) return `Unsupported` for WireGuard
  overlay, io_uring completion sockets and openssl/native-tls.
  - With rustls, the fd is polled in the application's direction, so a TLS
    session that needs the opposite direction mid-stream waits until the
    timeout.
- **Conformance coverage.** Every conformance test runs with `threads = 4`
  (`tests/conformance_tests.rs`). Add an 8-thread variant (task #27).

## 2. foundation_netio

- **QUIC driver falls back to a 1 ms poll.** `quic/quinn_driver.rs`
  `QuicDriver::next_status` returns `Delayed(IDLE_POLL)` (1 ms) when it has no
  `fd_readiness`. The `with_fd_readiness` hook exists but nothing calls it: the
  H3 server (`foundation_http` `h3_server.rs` `serve_h3`) and the connectrpc H3
  transport build the driver without one. Also, `EventReadiness` has no `clear`,
  so the driver cannot clear edge-triggered readiness before `pump_recv`. Tracked
  as feature 28.
- **`FramedRecv` buffers whole DATA frames.** *(Partly fixed:
  `http3/stream.rs` `FramedRecv::with_max_frame_size` exists.)* What is left:
  `H3Connection` / `H3Request` never expose it, so every connection uses
  `DEFAULT_MAX_FRAME_SIZE` (16 MiB, `http3/frame.rs:233`) **(rule 8)**, and a
  DATA frame is still buffered whole before it is yielded. Stream within a
  frame, and put the cap on the connection config.
- **`AsyncSendSafeBody` polls sync body iterators inline**
  (`shared/client/body_reader.rs` `poll_next`). `poll_next` calls the inner
  iterator's `next()`. Unless that iterator returns a "not ready" item
  (`Data::Retry`), a read that waits for the socket waits inside `poll_next`
  and holds the worker; a `Retry` self-wakes and busy-polls. Every async caller
  that reads a streamed body has this. Fix: give the body iterators a
  readiness-based async form that returns `Poll::Pending` and wakes on socket
  readiness.
- **`SendSafeBodyBytesIterator` reports would-block inconsistently.** It
  returns `Ignore` for a sized body's `Data::Retry` but passes a chunked body's
  `WouldBlock` through as `StreamError`, so its users can't tell "not ready"
  apart from bookkeeping items or failures.
- **Non-blocking work not done yet:**
  - non-blocking connect and TLS handshake (task #23):
    `HttpClientConnection::connect` blocks in `TcpStream::connect_timeout`
    (`native/connection.rs` `Connection::with_timeout`) and the TLS upgrade
    handshakes in place;
  - a pushed request body should wait on pipe readiness (task #24);
  - the remaining transports (task #25).
- **Ignored tests.**
  - 17 network-dependent or never-finished tests are ignored:
    `tcp_and_tls/http_external_validation.rs` (8),
    `tcp_and_tls/http_server_integration.rs` (6),
    `tcp_and_tls/tls_integration.rs` (3). Make them hermetic (task #31).
  - 37 doc examples are `ignore`/`no_run` (task #33).
- **A wasm SSE read error ends the stream as if it finished cleanly.**
  `wasm/client/stream.rs` `spawn_stream_reader` sets `done` on a JS
  `ReadableStream` read error, so the consumer sees a normal end. Fix: record
  the error next to `done` and have `QueueSseEvents` end with
  `Pending(SseProgress::Failed(..))`.
- **`poll_accept_webtransport` finishes the CONNECT stream after the `200`**
  (`http3/connection.rs`). In WebTransport over HTTP/3 the session lives as
  long as its CONNECT stream; closing it (FIN) ends the session. Check
  against the draft and keep the stream open, handing it to `WtSession`.

## 3. foundation_http

- **The accept loop polls a non-blocking listener** (`native/server/mod.rs`,
  `serve_with_listener`): on `WouldBlock` it sleeps `would_block_sleep` and
  tries again, instead of waiting for the listener to be readable.

## 4. foundation_core / valtron

- **Hard-coded valtron limits (rule 8):** `MAX_YIELD_DURATION`
  (`executors/local.rs`), `DEFAULT_YIELD_WAIT_TIME` (4 s) and `BACK_OFF_*`
  (`executors/constants.rs`), and `ThreadedIterFuture::new`'s queue size of 16
  and 10 ms back-pressure sleep (`executors/future_task.rs`; builder methods
  exist, but the defaults are literals and undocumented).
- **A single-thread `DrivenStreamIterator` cannot park an async consumer**
  (`executors/non_sendables.rs`, `wake`). The iterator drives the engine in
  `next`, and a parked task becomes runnable when a foreign thread flips its
  readiness (`WakeSignal`, a future's waker, a socket) - which pushes nothing
  and fires no queue wake. So its `wake` only registers on the queue and the
  inner pull still blocks once (`max_spins` timed waits); an async consumer
  (`into_collect_future` under smol/tokio) whose task stays parked for more
  than about two of those waits returns `Pending` with nothing to wake it.
  Likely fix: let `EventReadiness` sources register a wake (a `Send` callback;
  `WakeCallback` is not `Send` without `multi`), and have this `wake` register
  on the sleepers of the engine as well as on the queue.
- **`BackgroundJobRegistry`** (`executors/background.rs`): the queue is
  unbounded (it needs a capacity setting), and `new` panics if it can't spawn
  a thread instead of returning an error.
- **`BytesPool` has no bounds** (`io/buffer_pool.rs`).
  - It is unbounded and keeps returned buffers of any size, so it needs a
    capacity and a maximum buffer size **(rule 8)**.
  - `acquire_with_capacity` counts both a hit and an allocation when the
    pooled buffer is too small.
  - `PooledBuffer::owned` builds a throwaway pool for every buffer.
  - `PooledBuffer` has no `Debug`.
- **Release coupling.** `foundation_macros` calls
  `foundation_core::valtron::initialize_current_thread`, so the two crates
  need a coordinated version bump when published.
- **`BatchReader` logs every `WouldBlock` at `error!`**
  (`io/readers/mod.rs`, the `WouldBlock`/`TimedOut` arm). It is the ordinary
  "not ready" signal on a non-blocking socket.
- **`SleepIterator` treats a reached zero-duration deadline as still pending.**
  `delayed_iterators.rs` `SleepIterator::next` matches
  `completed_at.checked_duration_since(now)` with `Some(diff) => Pending`, so
  when `now == completed_at` (a zero-duration sleep, or the two `Instant::now`
  calls landing on one clock tick) it returns `Pending(0ns)` instead of `Done`.
  This makes `tests/valtron/delayed_iterators.rs:10`
  `zero_duration_sleep_iterator_finishes_immediately` flake under parallel load
  (`Instant::now` does not always advance between the constructor and `next`).
  Likely fix: return `Done` when `diff` is zero (only `Pending` when
  `diff > Duration::ZERO`), and make the test not depend on `Instant::now`
  advancing.

## 5. foundation_nostd / foundation_wasm / foundation_iogate

- **`SpinWaiter`** (foundation_nostd `primitives/spin_waiter.rs`, still used on
  wasm) rounds sub-millisecond waits down to zero, and its spin calibration is
  arbitrary.

## 6. foundation_deployment_docker / foundation_deployment

- **Seven client methods build requests by hand.**
  - The methods: `upload_to_container`, `get_container_archive_info`, `exec_inspect`, `system_info`, `system_ping`, `system_data_usage`, `network_inspect`.
  - They read success bodies whole with no cap, throw away error bodies, ignore `DockerLimits`, and carry stale docs.
  - `upload_to_container` takes the whole tar as a `Vec<u8>`; it should stream.
  - `system_data_usage` returns untyped JSON. Route all seven through the generated functions, fixing the spec conversion where needed (rule 5).
- **`send_json` has no body cap.** It reads a successful JSON body whole. Add `max_json_body` to `BodyLimits` **(rule 8)**.
- **The `integration-tests` targets are never compiled in CI.**
  `foundation_deployment_docker`'s CI runs are `""` and `docker`, so
  `tests/deployable_tests.rs` (gated on `integration-tests`) rotted: it
  still called the now-async `FileStateStore::init` synchronously until
  #140's verification fixed it. Fix: a CI step that at least builds the
  `docker,integration-tests` targets (`cargo test --no-run`), with the
  daemon-backed runs kept manual.
- **`LogFrameDecoder::decode` returns `None` for two cases:** "need more bytes" and "skipped an unknown frame type". Callers have to compare buffer lengths to tell them apart.

## 7. foundation_deployment_platform

- **`wait_for_stdout`:** re-reads the whole log on every poll, misses a message split across frames, and scans stderr too.
- **`docker_container_macro_supports_async_functions`** fails instead of skipping when no Docker daemon is running.
- **Docker-gated tests check only that the socket file exists.** `docker_available()` in `tests/container_integration.rs`, `tests/ssh_backend_tests.rs`, `tests/runner_integration_tests.rs` and `tests/image_build_integration.rs` returns true when `/var/run/docker.sock` exists, also when no daemon listens on it. Then `--ignored` runs fail with "Docker daemon not reachable" instead of skipping. Fix: add one shared test helper that connects with `DockerClient::connect_with_defaults` and pings the daemon, and use it in all Docker-gated tests (foundation_proxy has the same helper, see §9).
- **Vagrant box lookup runs a `curl` subprocess** (`src/import/mod.rs`, near line 512). The comment says this avoids "tokio runtime conflicts", but no tokio runtime is used now. The subprocess needs `curl` installed and returns no structured errors. Fix: use the valtron-based HTTP client from `foundation_netio` with a bounded, streamed read of the JSON response, and remove the comment.
- **Inline tests remain** in `docker/wireguard.rs` (rule 7).
- **Warnings:**
  - deprecated `SimpleHttpClient`, used in `wait_for.rs` and in `foundation_deployment/src/provider_client.rs`;
  - unused import `PortMapping` in `docker/wireguard.rs:21`.

## 8. foundation_openapi / foundation_codegentools

- **Boolean query parameters are generated as `Option<String>`** (for example `ContainerStatsArgs.stream` / `one_shot`), probably for every endpoint.
  - Generate `Option<bool>` and serialise it.
  - Related: Docker `image_build` can only send one tag, because `t` isn't treated as an array.
- **Inline tests remain** in `unified/emit.rs` and `unified/analyzer.rs` (rule 7).
- **genapi's flags are ignored in two places.**
  - Its post-step `cargo fix` always runs in the dev profile, ignoring `--profile`.
  - A plain `generate hetzner` ignores the crate's declared operation selection and generates the full API.

## 9. foundation_proxy

- **gRPC health probe leaks a thread + H2 connection when its inner timeout hangs.**
  - `health.rs::probe_grpc_health` drives `grpc_health_check` on a throwaway thread and bounds it with `recv_timeout` as a backstop, because the inner `select` + valtron `sleep_async` timer could fail to fire (fixed on `fix/foundational-backlog`: `sleep_async` now runs on valtron's timer thread) and `H2Transport`'s pump parks on the socket read with no read timeout.
  - When the backstop trips, the inner `block_on_future` (and its H2 pump task) is abandoned — a per-occurrence thread/connection leak. Rare (the timer only fails under pool contention, e.g. proxy shutdown), but a flapping or non-responsive backend could repeat it every probe interval.
  - Likely fix: give `H2Transport` a real read/keepalive timeout so the pump self-terminates, then drop the backstop thread (the timer half of the cause is gone).
- **Backend-state persistence can lose an update.** Each control-socket
  connection runs on its own thread, and `ProxyStateStore::save_backend_state`
  / `save_tls_cert` (`src/persistence.rs`) load the whole persisted state,
  change one entry and save it back. Two admin commands at once can each save
  over the other's change. Fix: serialise the read-modify-write behind a
  mutex in `ProxyStateStore` (or store each backend's state as its own record).
- **Docker-gated tests check only that the socket file exists**
  (`tests/proxy_integration_tests.rs`, `docker_available()`). When the socket
  exists but no daemon listens on it, the `--ignored` runs fail instead of
  skipping. Fix: use a shared helper that pings the daemon (see §7).
- **The ACME client does not follow RFC 8555's account flow** (`src/acme.rs`).
  `create_account` returns the newAccount directory URL instead of the
  account URL from the `Location` header (it only checks that the answer has
  an `orders` field), and `provision` discards it, so `key_id` is never set
  and every later request is signed with the `jwk` instead of the `kid`
  (§6.2 requires the `kid` once the account exists). The nonce is fetched
  fresh for every request instead of taken from each answer's
  `Replay-Nonce`. `provision` waits for DNS with `std::thread::sleep(5s)`
  inside an `async fn` (it blocks the executor; the 5 s is a constant, not a
  setting). The mock CA in `tests/acme_provisioning_tests.rs` sends no
  `Location`, so the tests miss all of this. Fix: return and use the account
  URL as the `kid`, carry the nonce from each answer, wait with an async
  sleep (or poll the challenge status) under a configured timeout, and make
  the mock CA strict.
- **The SSL redirect listener hard-codes its limits** (`src/server.rs`,
  `ssl_redirect_loop`): a 4096-byte head buffer, a 2 s read timeout and a
  10 ms accept poll are constants, and a head longer than one read is cut
  (rule 8). Fix: fields on the redirect config with documented defaults.
- **`tests/diag_client_tap.rs` is a "TEMPORARY diagnostic", not a test.**
  It is `#[ignore]`d, only `eprintln!`s what the client sent and received, and
  asserts nothing, so it can never fail. Fix: turn it into a real test of
  `forward_http`'s outgoing request (assert the `Host`, `X-Forwarded-*` and
  `Connection` headers the fake upstream receives and the 200 status) and run
  it un-ignored, or delete it if `proxy_integration_tests.rs` already covers it.
- **The `mut` on `h3_local_addr` warns without the `h3` feature**
  (`src/server.rs`, `ProxyServer::start`). Fix: build it from a `cfg`-gated
  expression instead of assigning inside the `cfg` block.

## 10. foundation_ai

- **Non-streaming wasm `generate` is unsupported.** The providers' sync
  `generate` path calls `HttpClient::send`, which on wasm now returns an error
  (spec 65 feature 06 T5: a sync send cannot wait for `fetch` without freezing the
  isolate), and its retry backoff `backend_utils::sleep` is a no-op on wasm (T4).
  So one-shot generation fails fast on wasm instead of deadlocking, but does not
  work. Likely fix: an async `generate` seam (e.g. `generate_async`) over
  `send_async`, or route wasm `generate` through the lazy `send_sse`/`stream`
  machinery and collect its result async.
- **The `agent` tool busy-loops on `Stream::Wait`.**
  `src/agentic/tools/agent.rs` (~l. 492) drains a sub-agent's stream
  synchronously and `continue`s on `Init | Ignore | Wait | Delayed`, so a
  sub-agent waiting on I/O spins the caller's thread (and on wasm never yields
  to the event loop, so the I/O can never complete). Likely fix: drive the
  sub-agent as its own task and park on it (`valtron::park_on`), or report
  `Working` and return on `Wait` the way it already does on `Pending`.
- **HTTP providers read whole error bodies.** Every provider's non-2xx path
  (`openai_provider`, `openai_responses_provider`,
  `anthropic_messages_provider`, `backends::model_info`) collects the full
  body to extract the vendor message. Likely fix: read at most a configured
  number of bytes (a field on each provider config, default 64 KiB like Go
  `net/http`'s error-drain limit) before parsing.
- **`ResponsesProvider::fetch_model_info` has no success test.**
  `tests/models/provider_model_info_tests.rs` defines `GPT_JSON` (a model
  object) but no test uses it, so only the error and placeholder paths of the
  Responses provider are tested. Fix: add the success test (a `GET` with the
  bearer token that parses `GPT_JSON`), as the Anthropic one does.

- **Embeddings are generated synchronously inside async code.**
  `EmbeddingProvider::embed` is sync, and `CachedEmbeddingProvider` calls the
  model's sync `generate` on a cache miss (`src/agentic/embedding.rs`,
  `embed_single`). `ContextProvider::search` (semantic recall, the
  `search_context` tool) and the `shed` tool's embedding search call it from
  async code, so an HTTP embedder blocks the worker, and on wasm the sync
  `generate` fails (see the first entry). Fix: an async `embed` /
  `embed_batch` over an async generate, awaited by the callers.
- **A flush that finds another flush running returns `Ok(0)` at once.**
  `MessageApi::flush` (`src/agentic/message_api.rs`) guards with an
  `AtomicBool`; a second caller doesn't wait. The agent loop's `Flushing`
  state then resumes with the buffer still at the threshold, so it re-enters
  `Flushing` every step until the other flush (e.g. a concurrent
  `AgentSession::end`) finishes. Fix: let the second caller await the running
  flush (an async lock or a completion signal) instead of returning early.
- **A cancelled flush can write a record twice.** A record stays in
  `MessageApi`'s `retry` slot until its write returns, so a flush future
  dropped after the store accepted the write but before it returned writes the
  record again on the next flush. Fix: make the writes idempotent (upsert on
  the document id for `append_with_id`, and give memory records a stable id),
  or clear the slot through a store acknowledgement.
- **`MemoryCoordinator` keeps `_async` method names.** `record_async` and
  `hydrate_async` (`src/agentic/memory_coordinator.rs`) are inherent, so spec
  50 F05 T3 (storage trait methods) left them; next to `MemoryStore::hydrate`
  the suffix now reads as a different operation. Fix: rename to `record` /
  `hydrate` (no clash: `MemoryCoordinator` implements no store trait).
- **The test targets need llama.cpp even without `llamacpp`.**
  `infrastructure_llama_cpp` is an unconditional native dev-dependency
  (`Cargo.toml`, for the `errors/llama.rs` conversion tests), so
  `--no-default-features --features testing` still builds llama.cpp and fails
  without the `tools/llama.cpp` submodule. Fix: drop the dev-dependency (a
  dev-dependency cannot be optional) and reach the llama.cpp error types
  through a `llamacpp`-gated re-export, with those tests gated on the feature.

## 11. foundation_db and its sync callers

Follow-ups from making the I/O backends async-only (branch `db/async-stores`).
- **`StateStore::set` ignores its `resource_id`.** Every backend (file,
  libsql, D1, R2) keys the write by `state.id`, so `set("a", &state_b)` writes
  `b` (`src/core/state/*.rs`, `src/native/{libsql_store,d1_kvstore,r2_blobstore}.rs`).
  `NamespacedStore` now rewrites `state.id` itself (fixed in F04 T8). Fix:
  drop the parameter, or key by it and set `state.id` from it in one place.
- **`R2Store` parses the R2 REST JSON by hand** (`src/native/r2_blobstore.rs`).
  The generated Cloudflare types live in `foundation_deployment_cloudflare`,
  which depends on `foundation_db`, so they can't be used here (rule 5). Fix:
  move the generated R2 types to a crate both can depend on. Related:
  `worker-stub.js` has no object-listing endpoint, so the R2 listing is only
  tested against the in-process mock in `tests/r2_state_store_tests.rs`, not
  against the wrangler stub.
- **R2 state listings hold every matching ID before streaming.** The listing
  pages through R2's cursor, sorts the IDs, then fetches each state lazily;
  the ID list itself is in memory. Fine for deployment state; a store with
  very many resources would want a cursor-driven stream (R2 already returns
  keys in order).
- **D1's state `init` sends several statements in one `/query` call**
  (`src/native/d1_kvstore.rs`). It (and libsql's) was missing the `;` after
  `CREATE TABLE`, so `init` always failed; fixed in F04 T8 and covered for
  libsql by `tests/libsql_state_store_tests.rs`, but the D1 path is only
  exercised by the wrangler integration tests (`CF_INTEGRATION_TEST=1`).
- **Auth tables mix timestamp units.** The `created_at` defaults in the core
  migrations are milliseconds (`strftime('%s','now') * 1000`), while
  `AsyncStorageCleanup` compares `expires_at` with Unix seconds and the IdP
  server writes milliseconds for its own rows. Fix: pick one unit per column,
  document it in the migrations, and make cleanup follow it.
- **`KVWasmStorage`'s rate limiter isn't atomic**
  (`src/wasm/wasm_storage/kv_wasm.rs`): a record reads the entry, then
  writes it back, so two concurrent hits can count once (and KV is
  eventually consistent across locations). Fix: use a Durable Object or D1
  for rate limits on Workers, and document KV as best-effort.
- **Databases created by the old rate-limit code have the wrong table.**
  Before F04 T11 a store that wasn't migrated created
  `rate_limits (key PRIMARY KEY, count, window_start)`; the store now uses the
  migration 014 shape (`id`, `key UNIQUE`, `count`, `window_start`,
  `window_end`), which `CREATE TABLE IF NOT EXISTS` won't add to an existing
  old-shape table. Fix: a migration that rebuilds an old-shape table (rate
  limit rows are disposable), or drop it on detection.
- **The wasm stores keep a second, inherent `*_async` API.** `D1WasmStorage`,
  `KVWasmStorage` and `R2WasmStorage` (`src/wasm/wasm_storage/*.rs`) have
  public inherent methods with the old trait names (`get_async`,
  `query_async`, `put_blob_async`, ...). They return `!Send` futures, and
  `D1WasmStorage::list_keys_async` / `KVWasmStorage::list_keys_async` return
  a `Vec` instead of a stream. The trait impls, `StorageProvider`,
  `WasmCredentialStore`, `foundation_auth`'s `CredentialStore for
  D1WasmStorage` and `examples/cf-login-app` call these. Spec 50 F05 T3
  dropped the suffix from the trait methods, so the two APIs no longer share
  a name, but a caller can still pick either by accident. Fix: make the
  inherent methods private helpers (as the native stores' `*_internal` ones
  are) and route every caller through the traits (`list_keys` +
  `collect_all` where a `Vec` is needed).
- **No CI job builds the example workspaces** (`examples/cf-*`,
  `apps/answerme-agent`); `examples/cf-login-app`'s manifest had stopped
  loading unnoticed. Fix: an examples job (wasm32 for `cf-*`). The feature
  runs that had rotted (foundation_db `libsql`, foundation_nativeapis
  `vfs-native,vfs-d1,vfs-r2`, foundation_vectors `sql-store`, foundation_auth
  `server-test`) are in `ci/packages.toml` since F04 T12.

## 12. foundation_auth

- **`PowService` checks leading zero bits against a byte count.**
  `verify_solution` (`src/server/services/pow_service.rs`) compares
  `count_leading_zero_bytes(&hash)` — which counts *bits* — with
  `difficulty / 8`, so a difficulty of 22 bits only needs 2 zero bits. Its
  `solve_validity_secs` field is never read either. Fix: compare bits with
  `difficulty`, rename the helper, and either enforce the solve window or
  drop the field (update the tests, which solve at difficulty 8).
- **`register` answers a duplicate email with `200 {"error": "user_exists"}`**
  (`src/server/handlers/core.rs`); spec 48 F03 says `409`. Fix: return
  `IdpError::Conflict`.
- **`header_name_eq` compares header names through `Debug` formatting**
  (`src/server/handlers/core.rs`), so it depends on `SimpleHeader`'s `Debug`
  output. Fix: compare with the header's canonical name.
- **IdP integer columns are cast with `as`** (`interval_secs`, `counter`,
  `failed_login_attempts` in `src/server/storage.rs`): a negative or oversized
  value wraps silently. Fix: `u32::try_from` and report a `Parse` error.
- **`--features server-native` tests don't link here.** `webauthn-rs` needs
  OpenSSL 3 symbols (`EVP_Q_digest`, `OPENSSL_sk_*`) that the build host
  doesn't provide. Fix: vendor OpenSSL (`openssl/vendored`) or document the
  system dependency.
- **Unused dependencies on wasm.** `cargo check --target wasm32-unknown-unknown
  -p foundation_auth --no-default-features --features wasm` warns that
  `argon2`, `foundation_errstacks`, `pbkdf2`, `time` and `uuid` are unused
  there. Fix: make the native-only ones target-specific dependencies (or gate
  them behind the features that use them). The same build warns that
  `constant_time_eq` (`src/shared/password_hash.rs`) is never used on wasm32;
  gate it with the native code that calls it.
- **`foundation_db::wasm::{WasmCredentialStore, session}` duplicate
  `CredentialStorage`.** Their docs say they exist because the sync
  `CredentialStore` routed D1 through blocking calls; that trait is gone (spec
  50 F04), and `CredentialStorage` over a D1 `StorageProvider` already serves
  `CredentialStore` on Workers. Fix: fold callers onto
  `CredentialStorage` and delete the duplicates. `wasm_credential_store.rs`
  also has an inline `#[cfg(test)] mod tests` (against rule 7); deleting the
  file removes it, otherwise move the tests to `tests/`.
- **`NativeOAuth` and `PasswordAuthClient` read an empty body.**
  `src/native/oauth.rs` (`extract_body_text`, `error_body_text`) and
  `src/native/password_auth.rs` (`extract_body`) read the response through
  `get_body_ref()`. The client gives a lazy `SendSafeBody::Stream`, which
  these helpers read as an empty string, so a token or login answer fails to
  parse and an error answer has no message. Fix: take the body and read it as
  a stream (status first; an error body only up to a configured limit), as the
  `shared` clients do.
- **The `shared` HTTP clients read bodies synchronously and whole.**
  `discovery`, `jwks`, `introspection`, `userinfo` and `upstream_client`
  (`src/shared/`) call the sync `try_collect_bytes` inside `async fn`s (a
  blocking read on the executor; on wasm it cannot wait for `fetch`), and
  read an error body whole before they use the status. Fix: read with
  `collect_bytes_async` (`AsyncSendSafeBody`), check the status first, and
  read an error body only up to a limit on a config (64 KiB, as
  `foundation_deployment`'s `BodyLimits`).
- **`NativeOAuth` keeps sync twins** (`exchange_code`, `client_credentials`,
  `refresh_token`) next to the `*_async` methods (`src/native/oauth.rs`),
  against the async-only API rule. Fix: keep only the async methods (as the
  wasm client now does) and let callers `.block()`.

## 13. foundation_nativeapis

- **The shared-reactor tests use Linux-only calls.** `tests/reactor_shared_selector_tests.rs`
  calls `libc::pipe2` (not present in `libc` on macOS) and reads `/proc/self/fd` (Linux only),
  so `cargo check --workspace --all-targets` fails on macOS. The `Reactor` itself has a kqueue
  backend. Fix: build the nonblocking pipe with `libc::pipe` then `fcntl` (portable), and gate
  the `/proc/self/fd` count test behind `#[cfg(target_os = "linux")]`.
- **The blob-backed D1/R2 deltas can't list.** `BlobDelta`
  (`src/native/vfs/blob_delta.rs`, behind `D1Delta`/`R2Delta`) keeps each
  file, its metadata and its whiteout as separate blobs, and a blob store has
  no key listing: `list_async` and `list_whiteouts_async` return nothing and
  `reset_async` is unsupported. Fix: keep a per-directory index blob (or use
  D1's key-value listing, `list_keys`, for the D1 delta).
- **A blob-delta write rewrites the whole file.** Each `write_at_async` reads
  the blob, splices the bytes in and writes it back, so a large file written
  in small pieces is O(n²). Fix: chunked storage (the unused
  `d1_delta::chunking` module sketches it) with a configurable chunk size.
- **The VFS shim never uses the D1/R2 deltas.** `src/native/vfs/shim/mod.rs`
  logs "impls pending" and falls back to the memory delta for
  `DeltaBackend::D1` / `R2`. The overlay is async now, so the deltas plug in
  directly (`DynFs::from_async(OverlayFileSystem::new(base, d1_delta))`).
  Fix: build them from the shim config.
- **`FjallDocumentStore` scans buffer their rows.** `scan`,
  `scan_all` and `scan_from` collect the matching rows into a
  `Vec` before streaming them (`src/shared/vfs/fjall_document_store.rs`);
  `scan_all` is unbounded. Fix: walk the fjall index range lazily.

- **No delta store on the local Turso engine.** `TursoDelta` never compiled and was
  deleted in spec 50 F05 T2 (with its example, the `vfs-turso` feature and the
  shim's `turso` backend). A Turso cloud database still works through
  `LibsqlDelta` with `TursoRemoteConfig`. Fix: rebuild it on the `turso` crate as
  an async-only store; tracked in ewe-studios/ewe_platform#142.
- **The `LD_PRELOAD` shim does not compile.** With `vfs-preload`,
  `src/native/vfs/shim/mod.rs` defines `open` with a fixed argument list, and
  the current compiler refuses it ("invalid definition of the runtime `open`
  symbol"): libc's `open` is variadic. The `DeltaBackend::Sqlite` arm also
  calls `initialize_pool` without an import. Fix: declare `open` / `openat`
  as variadic (`c_variadic`) or export them from a small C file, and import
  `initialize_pool`.
- **The FUSE adapter cannot build in CI.** `vfs-fuse` needs the system
  `libfuse` (`fuse.pc`); the build script of `fuser` panics without it, so
  `FuseMount` is never compiled here. Fix: install `libfuse-dev` in the CI
  image and add a `vfs-fuse` run, or use `fuser` without `libfuse`.
- **CI does not build most VFS features.** `ci/packages.toml` runs
  `foundation_nativeapis` with `""` and `vfs-native,vfs-d1,vfs-r2` only.
  `vfs-sqlite`, `vfs-ipc`, `vfs-nfs`, `vfs-fjall`, `vfs-search-fff` and
  `vfs-ptrace` are not built, which is how `vfs-sqlite` stopped compiling
  (fixed in F05 T2). Fix: add a run with
  `vfs-native,vfs-search-fff,vfs-sqlite,vfs-ipc,vfs-fjall,vfs-nfs,vfs-ptrace,vfs-d1,vfs-r2,vfs-arrow`.
- **`NativeFs` and `DirectoryDelta` block for each system call.** They
  implement the async traits, but call `std::fs` inline: each call blocks the
  valtron worker for one system call. Fix: an async file API (the crate's
  `uring` layer on Linux, or a bounded offload thread elsewhere).
- **The IPC bus is sync.** `ClientTransport::request`
  (`src/native/vfs/ipc_bus.rs`) waits on the bus receive on the calling
  thread, and `run_daemon_loop` is a thread that waits for each dispatch
  through `exec_async`. The join timeout (30 s) and the receive poll (1 s)
  are constants **(rule 8)**. Fix: an async receive on the bus, and the two
  timeouts in a transport config.
- **The VFS IPC protocol has no request ids.** Responses are matched to
  requests by order only, so `ClientTransport` holds one lock across a send
  and its receive, and counts released (closed) handles to skip their
  responses. `ReadAt` sizes are `u32`. Fix: a request id in `VfsRequest` and
  `VfsResponse`.
- **The CLI and fff searchers block.** `CliSearcher` waits for the `rg` /
  `grep` process on the calling thread; `FffSearcher::new` waits up to 30 s
  for its scan, and the grep limits (50 per file, 200 per page, 5 s budget)
  are constants **(rule 8)** (`src/shared/vfs/search.rs`). Fix: an async
  process wait, and a `SearchConfig` for the limits.
- **`InodeFs` / `InodeDelta` ignore their path.** `src/shared/vfs/fjall_fs/`
  keeps everything in memory; `InodeFs::open(path)` and `open_with_config`
  ignore the path and the config, and the `FjallFs` / `FjallDelta` aliases
  suggest a disk store. They implement only the sync traits. Fix: rename the
  constructors (`InodeFs::new()`), drop the aliases, and add the async face
  like `MemoryFs`.
- **`FjallDocumentStore` uses the sync VFS traits.** `FjallDocumentStore<V:
  VfsFileSystem>` (`src/shared/vfs/fjall_document_store.rs`) calls its VFS
  synchronously inside the async `DocumentStore` methods. With an I/O-backed
  VFS (wrapped in `SyncFs`) each call blocks. Fix: make `V:
  AsyncVfsFileSystem` and await the calls.
- **NFS `readdir` ignores `start_after`.** `VfsNfs::readdir`
  (`src/native/vfs/nfs/mod.rs`) always starts at the first entry, so a
  directory larger than one page repeats; the attribute times are always 0.
  Fix: skip entries up to `start_after`, and map `VfsMetadata` times.
- **Whole-file reads in the VFS defaults.** `read_file`, `copy` and
  `copy_async` on the VFS traits read the whole file into one `Vec`
  **(rule 4)**. Fix: copy in chunks of a configured size.
- **`SyncFs` is not gated as a blocking boundary.** `exec_async`
  (`src/shared/vfs/exec_async.rs`) uses `collect_one` when a future is
  pending, without `blocking_boundary!`, so on JS-hosted wasm it would spin.
  Fix: gate `SyncFs` and `exec_async` with `blocking_boundary!`.
- **ptrace warnings.** `src/native/vfs/ptrace/` has unused imports, unused
  constants and an unreachable statement after `execvp`. Fix: remove them.

## 14. foundation_cronjobs

- **`CronJob::next_fire` ignores the cron expression.** It always returns five
  minutes from now (`src/scheduler.rs`, marked TODO), so every job runs on the
  same fixed interval. Fix: parse `cron_expr` with a cron crate (`cron` or
  `saffron`) and compute the real next fire time; reject an invalid
  expression at `register`.
- **`CronScheduler` never runs a job.** The crate docs (`src/lib.rs`)
  describe tasks parked on `Delayed` until each fire time, execution, result
  recording and missed-job catch-up (`MissedJobPolicy`); none of it exists —
  the scheduler only registers jobs and saves/loads their state. Fix:
  implement the run loop (one valtron task per job) and the missed-job
  policy, or cut the docs down to what exists.

## 15. foundation_cedar

- **`SqlPolicyStore` reads its row once per field.** `load_from_store` calls
  `load_schema`, `load_policies` and `load_entities`, and each runs the same
  `SELECT` (`src/storage/sql_store.rs`). Fix: a `load_all` on `PolicyStore`
  (default: the three calls) that the SQL store overrides with one query.
- **`derive_more` is an unused dependency** (`cargo` warns). Fix: drop it
  from `Cargo.toml`.
- **`unreachable call` warnings in `src/core/request.rs`.** The three
  `EntityId::from_str(id).map_err(..)` calls map an error type that can't
  occur (`Infallible`), so the compiler warns on each `map_err`. Fix: drop the
  `map_err` (or use `EntityId::new`) so the build is warning-free.

## 16. Workspace / process

- **Shared target dir clobbering.** When parallel worktrees share one `CARGO_TARGET_DIR`, crates such as foundation_core and foundation_macros build with the same hashes and overwrite each other's test binaries and proc-macro artifacts mid-run. That gives stale results and `E0460` doctest failures.
  - Give each worktree its own target dir, or `RUSTFLAGS=-Cmetadata=<id>`. Note that the latter rebuilds all dependencies.
- **`crates/ewe_deployables` doesn't build.** It isn't a workspace member, so
  nothing compiles it, and it still implements an older `Deployable` shape
  (sync `TaskIterator`s, sync `store(&client).get_typed(..)`), which no longer
  exists. Fix: port it to the current async `Deployable` / `StateStore` and
  add it to the workspace (or delete it if superseded by the per-provider
  deployment crates).
  Its HTTP client references were moved to `HttpClientBuilder` /
  `ProviderClient::with_client` for #140, but the rest is still stale: it
  imports `foundation_netio::simple_http`, `ProviderClient<S, R>` and
  `foundation_deployment::providers::cloudflare` (now
  `foundation_deployment_cloudflare`). `CloudflareWorker::deploy` also reads
  the script from disk into `_script` and never uploads it, so a deploy would
  push an empty module.
- **Cloudflare crate size.** Split the generated Cloudflare groups into per-group crates (task #28).
- **Wasm runtime tests never run in CI** (spec 68 T4). CI's wasm step is a
  compile gate only (`scripts/ci/ci.py --step wasm` → `cargo check --target
  wasm32-unknown-unknown`). The runtime suites (`foundation_core/integration/wasm_tests`
  via `wasm-testbed deno`/`web`, the bindgen-deno path, `cf-cloud-provider` under
  miniflare) are local mise tasks, and `integration/wasm_tests` is a standalone
  crate outside the root workspace, so not even its compile is gated. That is how
  the feature 06 T5 isolate freeze shipped behind green CI, and how the
  `wasm-embedded-js` testbed stopped building (deno_v8 had no JS engine selected)
  unnoticed. Add a CI job that runs `mise run test:wasm-testbed:deno` (embedded
  Deno; no browser needed) and `mise run test:wasm-bindgen:deno`.
- **`wasm-testbed` warns on `Command::Bindgen(args)`** (`foundation_testbed/src/bin/wasm-testbed.rs:37`):
  `args` is unused unless the `wasm-bindgen-test` feature is on (e.g. under
  `--no-default-features --features wasm-embedded-js`). Gate the binding with the
  same `cfg` as its use, or prefix it with `_` in the other arm.
- **Running a wasm mise task installs every tool in `mise.toml`.** `mise exec`/`mise run`
  first installs all pinned tools (bacon, cargo-nextest, cargo-audit, python…),
  building several from source. On a small disk that fills it, and where the
  GitHub attestation API is blocked the python install fails, so
  `test:wasm-bindgen:deno` cannot start even though it only needs deno and
  wasm-bindgen. Split the wasm tools into their own mise env/profile, or have the
  task invoke the two binaries via `mise which`.

- **Wasm runtime tests never run in CI.** `foundation_core`'s
  `tests/valtron/wasm_js_yield_integration.rs` (and the other
  `wasm_bindgen_test` suites) need `wasm-testbed` + Deno; the CI wasm step
  only runs `cargo check --target wasm32` on the library, so these tests are
  not even compiled there. Fix: a CI step that at least builds them
  (`cargo check --tests --target wasm32-unknown-unknown`) and, where a
  runner is available, runs them.
- **`foundation_db` dead-code warnings in test builds**: `core/rate_limit.rs`
  `CREATE_TABLE_SQL`, `RECORD_SQL`, `SELECT_SQL`, `sql_check`, `sql_record`,
  `sql_reset` are unused in the feature set the connectrpc test builds pull
  in. Fix: gate them on the features that use them.

## 17. foundation_vectors

- **`FjallVectorStore` runs fjall inline in its async methods**
  (`src/fjall_store.rs`). fjall has only a sync API. The calls read and write
  the page cache without `fsync`, but the first use of a namespace scans all
  of its vectors to rebuild the index, and `flush` (`fsync`) is sync. Both can
  hold a worker for a long time. Fix: run them on a blocking-work seam when
  valtron has one (or in a task of their own), and make `flush` async.
- **`SqliteVecVectorStore` ships only a linux-x86_64 `vec0.so`**
  (`vendor/vec0.so`). `open` fails on every other platform. Fix: vendor the
  extension for each supported target, or build sqlite-vec from source.
- **`SqliteVecVectorStore` tables from before spec 50 F05 T1 do not work.**
  T1 changed the `vec0` schema (text primary key, `metadata` column, distance
  metric). An old `vec0_{ns}` table gives SQL errors. Fix: detect the old
  schema on `open` and migrate it, or fail with a clear message.
- **`TurboPufferVectorStore` is tested only against a mock.** The mock
  follows the request and response types of the official Python client
  (`turbopuffer` 2.14.0), because the documentation site was not reachable.
  Two details are not proven against the live service: that `deleted_ids`
  lists only ids that existed (`delete` reports `NotFound` from it), and
  that `include_attributes: ["vector"]` returns the vector. Fix: an opt-in
  test against a real namespace (API key from the environment).
- **`TurboPufferVectorStore` does not retry HTTP 429.** `TurboPuffer` throttles
  writes with 429 (backpressure). The store reports it as an error. Fix: retry
  with backoff, with the attempts and delays on a config.
- **The `sql-store` feature does not build for wasm32.**
  `cargo check --target wasm32-unknown-unknown -p foundation_vectors
  --features sql-store` fails in `foundation_core` (`idle_wake` missing, a
  `NoopMutex` `PoisonError` mismatch). The crate takes `foundation_db` with
  its default features on every target, so `foundation_core/multi` is on for
  wasm. This was so before spec 50 F05 T1. CI builds `sql-store` only for
  native, so it does not see it. Fix: take `foundation_db` without default
  features on wasm32, then add `sql-store` to the crate's `wasm-runs` in
  `ci/packages.toml`.
- **`hybrid_search` gives the reranker no texts** (`src/fusion.rs`). It calls
  `Reranker::rerank(query, fused, &[])`, so a reranker cannot read the
  candidate documents. Fix: take a text lookup (id → text) as an argument, or
  change `Reranker` to fetch the texts it needs.
- **The SQL-backed stores scan a whole namespace per search**
  (`SqlVectorStore::search`): memory is bounded (`TopK`), but time is
  O(n·d) per query. Expected for the portable fallback; large namespaces
  should use `LibsqlVectorStore` (native index) or a dedicated vector backend.

## 18. foundation_testbed

- **The embedded JavaScript runner uses a tokio runtime** (`src/wasm/embedded_js.rs`, `tokio_runtime()`, and the optional `tokio` dependency behind `wasm-embedded-js` in `Cargo.toml`). The event loop and timers of `deno_core` (`reactor_tokio.rs`) need a tokio runtime, and `deno_websocket` uses tokio for its TCP connection. This is the only tokio runtime in `backends` outside the cross-runtime tests of valtron. The project rule is "valtron always". Options:
  - keep it as a documented third-party boundary: a current-thread runtime, local to this module and behind the feature flag;
  - or drive the event loop of `deno_core` another way (poll `JsRuntime::poll_event_loop` from a valtron task with a valtron timer), if `deno_core` lets us replace its reactor.

## 19. Apps (tokio and axum)

- **The apps still use tokio and axum, not valtron and `foundation_http`.** This is `bin/platform` (and its templates `api-app`, `static-html-app`, `web-app`), `examples/devbinary`, `examples/web/wello`, `examples/todo`, `crates/channels` and `crates/domain`: tokio/axum servers and tokio tests. `crates/channels` also has inline `#[cfg(test)]` modules (`src/broadcast.rs`, `src/mspc.rs`, `src/executor.rs`), which go against rule 7. Tracked in [#137](https://github.com/ewe-studios/ewe_platform/issues/137).

## 20. foundation_deployment_huggingface

- **The Hub client API is sync.** `client::{whoami, list_*, create_repo, …}`
  and `repository::*` (`src/client.rs`, `src/repository.rs`) block on the
  native client's sync `HttpClient::send`, so they freeze a valtron worker
  and cannot run on wasm. Fix: async functions over `send_async`
  (`collect_bytes_async` for the downloads), with `.block()` at the callers
  (`foundation_ai`'s candle/gguf providers, `foundation_testing`).
- **Uploads are buffered whole.** `repo_create_commit` reads each added file
  into memory (`std::fs::read`) and builds the whole multipart body before
  sending it. Fix: a streamed multipart body (`SendSafeBody::Stream` over the
  file chunks) with the length computed up front.
- **`list_*` and `repo_list_tree` read one page only.** They ignore the Hub's
  `Link: rel="next"` pagination header, so a listing stops at the first page
  (the docs say "with pagination"). Fix: follow `Link` lazily as the stream
  is pulled.

## 21. Wasm-only warnings (seen in `cargo check --target wasm32-unknown-unknown -p foundation_keychain`)

- **`foundation_keychain`**: `rmpv` is an unused dependency on wasm32
  (`Cargo.toml`; only native code uses it), and `src/server/signalr_do.rs:29`
  imports `notifications::self` without using it, and binds `mut resp` at
  line 74 that is never mutated. Fix: move `rmpv` to the native target
  section, drop the `self` import and the `mut`.
- **`foundation_deployment_cloudflare`**: `src/workers/durable_object.rs:24`
  imports `worker::Error` without using it. Fix: drop it.

## Paused

- Windows/macOS VM images (#69).
