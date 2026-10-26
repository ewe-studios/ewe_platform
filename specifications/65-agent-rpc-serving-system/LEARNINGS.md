# LEARNINGS — Agent RPC Serving System

_No implementation yet — this file is seeded at spec creation and updated after
each milestone/task. See `.agents/skills/specifications-management/skill.md`._

## Key finding (creation)

This system is **four single-mission components composed by a manifest and one
live handshake — not a layered stack.** The instinct to make the proxy
"understand" agent-rpc, or to have agent-rpc register itself, or to share a
manifest/secret across components, all reintroduce the coupling the design
explicitly forbids. The two hard seams are: (1) the manifest, read *only* by
installer + registrar, and (2) the registration handshake, between registrar and
proxy. Everything else is independent.

## Decision (2026-10-19)

- **agent-rpc is a per-machine control plane, not a process-per-instance.**
  Load a model once, run many threaded inference sessions, share the loaded
  model across all instances. Instances are lightweight logical state
  (an `AgentSession` + memory + provider config).
- **Two independent auth channels** — client↔agent-rpc (secret token → mTLS,
  owned by agent-rpc) and registrar↔proxy (token → mTLS, owned by registrar +
  proxy). The proxy does not see agent-rpc's client auth.
- **Only installer + registrar read the manifest.** agent-rpc and the proxy do
  not know it exists.
- **Docker delegates to `foundation_deployment_docker`** (pull-by-tag preferred,
  build as fallback). The sidecar is a deployment-time choice (1/bare-metal host
  vs per-container), not an architecture change.

## Load-bearing assumption to verify first (2026-10-19)

The whole "one model, many threads" memory model rests on `foundation_ai`'s
loaded backend (llamacpp GGUF via `gguf_provider`/`infrastructure_llama_cpp`, or
Candle via `candle_llama`) being `Send + Sync` and handable to many
`AgentSession`s. This is **not assumed** — it is the first task of `01-agent-rpc`
(a spike) and, if it fails, the fallback is a per-backend `Arc<Mutex<…>>`
serialization point rather than per-instance copies.

## Inherited gap (2026-10-19)

gRPC `grpc-status` is not yet observable end-to-end because
`foundation_connectrpc`'s H2 transport does not forward trailing HEADERS
(spec 63 T1). agent-rpc's gRPC status assertions are gated on that landing; until
then, Connect and gRPC-Web round-trips are the verifiable RPC paths.

## Corrections absorbed during design (2026-10-19)

- Do **not** frame agent-rpc as process-per-instance (defeats memory sharing).
- Do **not** say components "share" a manifest or secret — only installer +
  registrar read the manifest; the secret is per-channel, not per-system.
- The registrar owns proxy communication; agent-rpc's auth is client-facing only.

## Cloudflare research (2026-10-19, validated against the repo)

"agent-rpc on Cloudflare" is **two artifacts, not one**, and the split is forced
by the platform, not by taste:

- **Full `foundation_ai` cannot compile to a Worker.** `llamacpp`/`candle`
  backends are already `not(target_family = "wasm")`-gated
  (`src/backends/mod.rs`) and are infeasible anyway (C FFI, threads, mmap, ~128 MB,
  no raw TCP). Do not attempt to port them.
- **The agentic layer + cloud providers are nearly wasm-clean, but the blocker
  is `getrandom 0.3.4`, pulled by two chains — not just `ahash`.** Re-verified
  2026-10-20: the red gate fires on `getrandom 0.3.4`, reached via (a)
  `foundation_ai`'s direct `ahash = "0.8"` (`runtime-rng` feature) and (b)
  `foundation_auth`'s non-optional `rand 0.9` (`os_rng` → `rand_core 0.9`).
  `foundation_auth` itself fails the wasm check today (regression since spec 28):
  its `wasm` feature only enables `getrandom 0.2/js`, but getrandom 0.3+ moved
  wasm support to a `wasm_js` *feature* that neither `rand_core 0.9` nor `ahash
  0.8` forwards. (getrandom 0.4.3 via `jwt-simple`/`ed25519-compact` is already
  fine — `ed25519-compact` forwards `wasm_js`.) Fix: enable `getrandom 0.3`'s
  `wasm_js` in `foundation_auth` (target-gated alias dep) + `ahash`
  `default-features = false` in `foundation_ai` (its 3 `AHasher::default()` sites
  are deterministic content-hashing). Note: getrandom 0.3.4's `compile_error!`
  misleadingly names a `wasm_js` *configuration flag* (`--cfg
  getrandom_backend="wasm_js"`), but its README says that cfg "is no longer
  required and does nothing" — the `wasm_js` **feature flag** is what's needed.
- **The providers already hold `Arc<dyn HttpClient>`.** The three cloud providers
  already declare `http_client: Option<Arc<dyn HttpClient>>` + a
  `with_http_client(Arc<dyn HttpClient>)` builder, and the native
  `foundation_netio::http::default_http_client()` default is already
  `#[cfg(not(target_family = "wasm"))]`-gated (`openai_provider.rs:198,390`,
  `openai_responses_provider.rs:354,536`, `anthropic_messages_provider.rs:483`).
  Remaining runtime-readiness (not compile blockers): `foundation_ai` never
  enables `foundation_netio/wasm-fetch`, and `std::thread::sleep` backoff compiles
  on wasm but panics at runtime.
- **The repo already has the hard parts.** `foundation_netio::wasm::client::
  FetchHttpClient` (behind `wasm-fetch`), `foundation_http::wasm::{serve_cf,
  dispatch_cf}`, and `foundation_connectrpc::shared::Router` (wasm-compatible
  Connect/gRPC-Web handlers; H2/H3 + `ConnectRpcServe` are native-only). The one
  gap on the RPC side is `foundation_connectrpc::shared::transport::wasm` — a
  capabilities stub ("lands in F24").
- **Deployment gaps.** `foundation_deployment_cloudflare` has Workers + API v4 +
  `generated/ai` (Workers AI control plane) but **no Sandboxes/Containers**;
  `ewe_deployables::cloudflare::CloudflareWorker` uploads Worker scripts.

**Decision:** thin WASM agent orchestrator on Workers (model via Workers AI or a
remote agent-rpc); full native model-server agent-rpc on Sandboxes/Containers.
All cross-crate changes are enumerated in `00-foundation/feature.md` §"Changes
required in existing crates" (readiness) and `05-cloudflare/feature.md` (the
target itself).

## Corrections — foundation-first, not deployment-first (2026-10-19)

Two oversteps corrected during re-scoping:

- **Do not fold wasm readiness into a single "Cloudflare deployment" feature.**
  The readiness work (make `foundation_ai` + deps compile/run on wasm) is a
  **foundation** that precedes agent-rpc. It is now `00-foundation`
  (`has_fundamentals: true`), grounded in the completed
  `28-cloudflare-workers-readiness` spec (cf-valtron-counter example,
  ci-wasm-checks, miniflare harness), verified with miniflare locally and on
  Cloudflare before anything else is built.
- **`foundation_installer` must not care about Cloudflare.** Its deploy targets
  are systemd and Docker. Cloudflare deployment (Worker upload, Sandboxes/
  Containers) lives in `foundation_deployment_cloudflare` / `ewe_deployables`.
  The `cloudflare` deploy-target adapter that was added to the installer was
  removed.

## Cross-platform seams (2026-10-19)

Two substrate homes already exist — reuse them, do not invent new types:

- **HTTP substrate — `foundation_netio`.** `DynNetClient` (`Arc<dyn NetClient>`,
  combined `HttpClient + WebSocketConnector`) and the singular `HttpClient` trait
  (`Arc<dyn HttpClient>`). Use the singular trait for HTTP-only providers, the
  combined one only when WebSocket is needed. **No new `DynHttpClient` type.**
- **Non-I/O substrate — `foundation_compact`.** The central home for
  cross-platform primitives: `time` (JS `Performance.now`/`Date.now` on wasm),
  `entropy` (vendored, all targets), `rng` (ChaCha CSPRNG, no external
  rand/getrandom), `ids` (scru128), `crypto`, `send_wrapper`. See
  `00-foundation/feature.md` §Changes for the current getrandom/ahash resolution.

## WASM T1 resolution (2026-10-20) — getrandom/ahash removed, not patched

The original plan (above) was to *patch* getrandom 0.3's `wasm_js` feature and
`ahash`'s `runtime-rng`. The actual fix is **stronger**: it removes `getrandom 0.3`
from the wasm graph entirely, by routing both call sites through `foundation_compact`.

- **`foundation_auth` drops `rand = "0.9"`** (the `getrandom 0.3` source) and now
  uses `foundation_compact::rng::rng()` / `Rng` (ChaCha CSPRNG, no getrandom).
  8 files: `server/models/code.rs`, `server/services/{client,provider,pow,token}_service.rs`,
  `shared/{oauth,session,two_factor}.rs`. `user_service.rs` keeps
  `password_hash::rand_core::OsRng` (argon2 → `getrandom 0.2`, already handled).
- **`foundation_ai` drops `ahash = "0.8"`** (the other `getrandom 0.3` edge) and
  routes its 3 `AHasher::default()` content-hash sites through a new
  `foundation_compact::hash::Hasher` — a deterministic newtype over
  **foldhash `quality`** (`foldhash 0.2`, `default-features = false`, `no_std`,
  no getrandom/rand, no AES — fast on wasm). Sites: `agentic/embedding.rs:274`,
  `agentic/loop_detection.rs:103,189`. `quality` over `fast` because SimHash
  relies on per-token hash statistics (foldhash's own docs steer that way).
- **Result:** `getrandom 0.3.4` is gone from the wasm graph (verified via
  `cargo tree -i getrandom@0.3.4`). This is better than the old plan: no
  target-gated `getrandom` alias dep, no `wasm_js` feature plumbing, and the
  hasher is deterministic-by-default.

**But `getrandom 0.3` was only the *first* of six stacked, pre-existing wasm
breakages** in `foundation_ai`'s wasm build — each fix unmasks the next:

1. **`foundation_netio` pulled `multi` into wasm.** `foundation_ai` declared
   `foundation_netio` in its *unconditional* `[dependencies]` with default
   features (→ `multi` → `foundation_core/multi`), so valtron's `TaskIteratorExt`
   and `send` resolved to their `+ Send` variants — but the wasm websocket/
   body-reader code is deliberately non-`Send` (single-threaded). Also valtron
   removed the `Iterator → TaskIterator` blanket impl (`task.rs:619`), which
   `websocket/wasm/browser.rs`'s `StreamToTaskAdapter` relied on. **Fix (done):**
   target-gate `foundation_netio` in `foundation_ai` (native = defaults, wasm =
   `default-features = false, features = ["wasm-fetch"]`, mirroring
   `foundation_auth`), and add an explicit `impl TaskIterator for
   StreamToTaskAdapter` (forwards `next()` → `next_status`, drops the unused `Ctx`).
2. **`foundation_auth` `password_hash.rs` wasm code gated by target, not
   feature.** Its WebCrypto PBKDF2 paths were `#[cfg(target_family = "wasm")]`
   but use `js_sys`/`web_sys`/`wasm_bindgen`/`wasm_bindgen_futures`, which only
   exist under `wasm-pbkdf2`/`wasm-bindgen-oauth`. **Fix (done):** gate them by
   `#[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]`, matching the
   `wasm-bindgen-oauth` gating in `lib.rs`.
3. **`foundation_ai`'s file/search tools reference `foundation_nativeapis`
   (native-only) on wasm — fixed with a 3-line change, not a relocation.**
   `agentic/tools/files.rs` + `agentic/tools/search.rs` + `harness/tools.rs`
   import the VFS traits/searcher from `foundation_nativeapis`, declared
   `[target.'cfg(not(target_family = "wasm"))'.dependencies]`. The `shared/vfs`
   module was **already wasm-safe** (native searchers are already
   `cfg(not(target_family = "wasm"))`-gated in its `lib.rs`, and `vfs`+`vfs-search`
   only pull `async-trait`+`regex`). **Fix (done):** declare it on the wasm target
   with `default-features = false, features = ["vfs", "vfs-search"]` (no
   `vfs-search-fff`). Verified `foundation_nativeapis` itself checks clean on
   wasm with that feature set. The "relocate VFS to `foundation_vectors`" plan was
   unnecessary — no public API moved.
4. **`foundation_ai` used `std::time::SystemTime` where
   `foundation_compact::SystemTime` is required — fixed.** On wasm the latter is
   a polyfill, so `std::time::SystemTime::now()` no longer matches the struct
   fields. Sites: `agentic/tools/memory.rs` (import + 2 calls) and
   `agentic/agent_loop.rs:1028` (inconsistent with `:473`/`:1254`, which already
   used the compact type).
5. **`foundation_ai`'s agentic tool loop is `!Send` on single-threaded wasm —
   fixed with `SendWrapper`.** `ToolImpl: Send + Sync` (`tool_impl.rs:349`) +
   `async_trait`'s `+ Send` future require `AgentTool<D, M>` →
   `Arc<Mutex<HashMap<String, AgentRun<D, M>>>>` → `TurnStream` →
   `DrivenStreamIterator<AgentLoop<D, M>>` to be `Send`, but `AgentLoop`'s valtron
   handles are `!Send` without `multi`. **Fix (done):** wrap `TurnStream`'s
   iterator in `foundation_compact::SendWrapper` (asserts `Send`+`Sync` on
   single-threaded wasm, transparent on native), and add `Deref`/`DerefMut` to
   `SendWrapper` so `TurnStream`'s forwarding methods keep working
   (`foundation_compact/src/send_wrapper.rs`, `agentic/turn.rs`).

Native `foundation_ai`/`foundation_auth`/`foundation_compact` still compile after
these changes (verified `cargo check`), and the wasm gate is now **green**:
`cargo check --target wasm32-unknown-unknown -p foundation_ai --no-default-features`
exits 0 (only pre-existing dead-code warnings remain). T1 is complete.

## WASM T2 resolution (2026-10-20) — the `multi` leak was transitive

The T2 dependency-crate audit (`cargo check --target wasm32-unknown-unknown` on
`foundation_netio`/`foundation_vectors`/`foundation_compact`/`foundation_arrow`/
`foundation_connectrpc`) found **one real failure**, and it was *not* the crate the
spec predicted (`foundation_arrow`):

- **`foundation_vectors`, `foundation_compact`, `foundation_arrow` all pass
  unchanged.** `foundation_arrow`'s `arrow-array → ahash/compile-time-rng` edge is
  build-time `const-random` (host-only) and never reaches the wasm runtime graph.
- **`foundation_connectrpc` failed** — but the error was inside `foundation_netio`
  (`body_reader.rs:3370`, `map_ready` on a `!Send` boxed continuation). Root cause:
  the same `multi` leak T1 fixed in `foundation_ai`, but **transitive** and deeper:

  1. `foundation_connectrpc` declared `foundation_netio` and `foundation_http`
     unconditionally with default features → `multi` on wasm.
  2. `foundation_http` declared `foundation_netio` unconditionally with default
     features → another `multi` edge.
  3. `foundation_auth`'s **default feature `turso`** (`foundation_db/turso` →
     `foundation_db/multi` → `foundation_core/multi` + `foundation_netio/multi`) is a
     native-only backend that leaks `multi` on wasm. connectrpc's `auth` feature pulled
     foundation_auth with default features, but its `shared/auth.rs` only needs JWT
     verification (core), not Turso storage.

  **Fix (done):** target-gate `foundation_netio` + `foundation_http` in both
  `foundation_connectrpc` and `foundation_http` (native = defaults, wasm =
  `wasm-fetch`/`wasm` — mirroring `foundation_auth`/`foundation_ai`), and pull
  `foundation_auth` with `default-features = false` (`wasm` feature on the wasm target).

**The rule generalised:** `foundation_netio`'s `multi` feature must never be enabled on
wasm — netio's wasm code (`ExchangeContinuation`, `WasmWebSocketBridge`, the browser
body-reader) is deliberately `!Send` for the single-threaded target, and `multi` flips
valtron's `TaskIteratorExt` to its `+ Send` variant, which then no longer applies to
those types. The leak is **always transitive**: any crate that depends on netio (or on
`foundation_http`/`foundation_auth`/`foundation_db`, whose defaults reach netio/`multi`)
must target-gate that dep and drop `default-features` on wasm. `foundation_db`'s
`turso`/`libsql`/`d1`/`r2` features all pull `multi` — note `d1`/`r2` are Cloudflare
(wasm) features that nevertheless enable `foundation_netio/multi`, a pre-existing smell
to revisit when the DB layer is ported.

## WASM T3 resolution (2026-10-20) — the `wasm-check` job was already gone

The spec-28 feature 07 `wasm-check` job that change #12 set out to "extend" **no
longer exists**. The CI refactor to the per-package model (`.github/workflows/
check.yaml` + `scripts/ci/ci.py` + `ci/packages.toml`) dropped it, so **no** crate
was being wasm-checked on any branch — which is precisely how `foundation_auth`
regressed on wasm (see the T1 note) without CI noticing.

The fix re-introduces wasm checking in the current model rather than bolting a
standalone job back on:

- **New `wasm` step in `ci.py`.** `STEPS` gains `"wasm"`; `Settings` gains `wasm`
  (mode) + `wasm_runs` (feature-flag specs, same syntax as `runs`). `commands()`
  emits `cargo check --target wasm32-unknown-unknown -p <name> <flags>` per entry
  — no `--all-targets`, no `--profile uat` (wasm has neither test binaries nor a
  uat-tuned profile; a plain `check` is the gate). The `wasm32-unknown-unknown`
  target is already installed in the CI image (`check.yaml`'s `rustup toolchain
  install ... --target wasm32-unknown-unknown`).
- **`ci/packages.toml` defaults `wasm = "off"`** (most crates aren't wasm targets);
  ten foundation crates opt in with `wasm = "enforce"` + a `wasm-runs` entry.
- **Both CI jobs** (`package` and `package-artifact`) run `--step wasm`, so a fork/
  artifact-image path checks the same thing.

The ten gates (all verified green locally, wasm-relevant feature sets):

| crate | wasm-runs |
|-------|-----------|
| foundation_core | `--no-default-features --features std` |
| foundation_auth | `--no-default-features --features wasm` |
| foundation_http | `--no-default-features --features wasm` |
| foundation_db | `--no-default-features --features wasm` |
| foundation_netio | `--no-default-features --features wasm-fetch` |
| foundation_vectors | `""` (defaults) |
| foundation_compact | `""` (defaults) |
| foundation_arrow | `""` (defaults) |
| foundation_connectrpc | `--no-default-features --features rpc` |
| foundation_ai | `--no-default-features` |

The spec-28 commands are **stale** — `ssl-rustls-awsrc` moved to `foundation_netio`,
`foundation_core`'s wasm surface is now just `std`/`alloc` (+ `js-wasmbindgen`/
`js-foundation-wasm` event-loop backends), and the wasm-relevant features are the
`wasm`/`wasm-fetch`/`rpc` flags above. `--no-default-features` on the `wasm-runs` is
deliberate: several defaults (netio `multi`, auth `turso`, db `turso`/`d1`/`r2`) are
native-only and would re-break the wasm build if enabled.

## WASM T4 resolution (2026-10-20) — Workers AI SSE has no `[DONE]` sentinel

T4 added `WorkersAiProvider`/`WorkersAiModel` (`backends/workers_ai.rs`, always
compiled — no feature flag), a REST provider over the singular `HttpClient` seam:
`POST /client/v4/accounts/{account_id}/ai/run/{model}` with a Bearer token,
non-stream `{"result":{"response":"…"}}` via `send`, and SSE `data:
{"response":"<cumulative text>"}` via `send_sse` (`WorkersAiStream`). Two findings
worth recording:

- **Workers AI SSE streams carry no `[DONE]` sentinel** (unlike OpenAI/Anthropic).
  Each `response` field is the *cumulative* text so far, and the stream ends only
  when the server closes the connection (EOF). `WorkersAiStream` therefore
  terminates by the inner SSE iterator returning `None` on EOF, not by a sentinel;
  it still tolerates a `[DONE]` if one ever appears.
- **The streaming test must use `SseTestServer`, not `TestHttpServer::with_response`.**
  The first test version used `TestHttpServer::with_response` with a fixed
  `Content-Length` body and *hung forever* (~1.5 h before it was caught): the SSE
  body reader reads until EOF, not until `Content-Length`, so after consuming the
  whole body it blocks on the next read because `TestHttpServer` keeps the
  connection open. OpenAI/Anthropic's `[DONE]` sentinel masks this in their tests —
  their `*Stream::next()` stops iterating at `[DONE]` and never asks the event
  source for another byte. Workers AI has no such sentinel, so the test server must
  actually close the connection. `SseTestServer` (`SseConnectionResult::clean_close()`
  → `shutdown(Write)` → EOF) is the purpose-built harness for exactly this; switching
  the test to it fixed the hang (`test_workers_ai_stream` passes in ~0.1 s).

## WASM T6 hang (2026-10-20) — the sync `Model::stream` path blocks the isolate

Local `wrangler dev --local` runs the Worker fine, but `POST /generate` hangs
with 0 bytes **and blocks the whole isolate** (`GET /` stops answering too). The
root cause is *not* the executor selection — the wasm single-threaded executor
correctly selects `JSThreadYielder` (`single/mod.rs:77-88`), which hooks
`setTimeout` to drive the engine. The bug is one layer up, in the **synchronous**
streaming path:

- `AgentLoop::transition_start_generate` (agent_loop.rs:798) calls the sync
  `Model::stream(interaction, params)`.
- `OpenAIModel::stream` calls `HttpClient::send_sse(req)` (sync).
- `FetchHttpClient::send_sse` (wasm/client/client.rs:160) wraps its async
  `do_fetch` future in `run_future(..)`.
- `run_future` (non_sendables.rs:408) is `ReadyValues::new(iter).collect()` — a
  synchronous loop that drives the future **inline** and never returns control to
  JS. The `fetch` completion is scheduled on the JS microtask queue; `run_future`
  re-polls the still-pending future in a tight loop, and the `setTimeout` callback
  that would advance the engine never fires → deadlock.

The transport-level async seam already exists and is the fix's foundation:
`HttpClient::send_sse_async` (wasm/client/client.rs:118) is a real `async fn`
that `.await`s `do_fetch`, genuinely yielding to JS. But its result
(`BoxedSseFutureStream`, built by `into_future_stream()`) is a **raw lens** — it
forwards `Stream::Wait`/`Stream::Pending` as ordinary `Ready(Some(..))` items and
never parks — so it cannot be consumed with `while let Some(x) = stream.next().await`
(that would busy-spin on `Wait`). The missing piece is a parking adapter that turns
`Wait`/`Pending` into `Poll::Pending` (yielding to JS), which `00-foundation`
never built because T5 was only ever compile-checked, never run against a live
model.

Resolution → `06-wasm-async-streaming` (corrected — **zero API change**): the
`Model` trait is unchanged — `Model::stream()` already returns a `StreamIterator`
of `Stream<ParseResult, SseProgress>`. The fix has two parts, both "make the
sync path not block", not "add an async seam":

1. **Make the bridge park.** `into_future_stream()` is rebuilt as a parking
   full-enum lens (`StreamParkingFutureStream`) that registers a
   `WakeableStream::wake` callback and returns `Poll::Pending` on
   `Wait`/`Pending`/`Delayed`/`Init`; the old raw pass-through lens is renamed
   `into_pure_future_stream()` (`StreamAsFutureStream`, no `WakeableStream` bound).
2. **Remove `run_future` from the wasm sync path.** wasm `send_sse` becomes a lazy
   `LazySseIterator` (spawns `do_fetch` via `spawn_local` on first poll, flattens a
   `WasmSseIterator`), and `run_future` is renamed `block_on_collect` + documented
   wasm-unsafe (native-only). Fetch failures surface as `SseProgress::Failed(msg)`,
   mapped to `ModelState::Error` by the OpenAI/Workers-AI streams.

T1 lands the parking bridge; T2 removes `run_future`; T3 makes `send_sse` lazy;
T4 maps the error; T5 verifies `cf-cloud-provider` streams live without blocking
the isolate.

## 06 T1 (2026-10-20) — the parking bridge lands (`into_future_stream` parks)

- `into_future_stream()` now returns `StreamParkingFutureStream`, an await-friendly
  full-enum lens (`Item = Stream<D, P>`) that parks on `Wait`/`Pending`/`Delayed`/
  `Init` via `WakeableStream::wake` (register-then-recheck; `NotSupported` falls
  back to `cx.waker().wake_by_ref()`), yields `Next(v)`/`Spread(items)` as-is, and
  skips `Ignore`. It carries `where Self: WakeableStream`.
- The raw lens is renamed `into_pure_future_stream()` → `StreamAsFutureStream`
  (unchanged: `Poll::Ready(iter.next())`, no `WakeableStream` bound). All existing
  raw-lens callers were moved to it: `http_client_impl.rs` `send_sse_async`,
  `wasm/client/client.rs` `send_sse_async`, `cf-valtron-counter`, and the
  `stream_future`/`spread` tests.
- Six new parking tests in `tests/valtron/bridge_park.rs` pin the new lens:
  park-then-wake-on-push, extension entry point, close-wakes-to-`None`,
  `NotSupported` self-wake, finished-producer-through-`Delayed`, and
  `Spread`-yielded-as-is. All 24 `bridge_park` tests pass under `--profile uat`.
- Native `cargo check -p foundation_core --tests` and wasm
  `cargo check --target wasm32-unknown-unknown -p foundation_core --no-default-features
  --features std` both pass; `foundation_netio` (native + `wasm-fetch`) compiles.
- Fixed two pre-existing broken tests (`test_into_ready_future_via_ext` and
  `test_tokio_await_ready_future`) that asserted `remaining.next().is_none()`
  after `into_ready_future()`, though the bridge returns the *remaining* iterator
  (which still holds the unconsumed item). Corrected to assert the remaining item
  then `None`. Now the whole foundation_core suite is green.

## 06 T2 (2026-10-20) — `run_future` → `block_on_collect` + the single-threaded drop-on-`Pending` fix

- The audit found **two** pre-existing blocking helpers, not one: `block_on_future`
  (returns a single `F::Output`, drives to completion; used by the `#[valtron]`
  macro, tests, `deployment_platform`, `keychain`) and `run_future` (returns
  `GenericResult<Vec<F::Output>>`, the sync `collect()` wasm trap). The feature
  doc's "rename `run_future` → `block_on_future`" would have collided with the
  existing single-output `block_on_future`, so the collect helper was renamed
  **`block_on_collect`** instead, keeping the two blocking helpers distinct
  (`block_on_future` = single output, `block_on_collect` = all outputs).
- `block_on_collect` (both `non_sendables.rs` and `sendables.rs`) is now
  documented `# WASM-unsafe: native-only`: the `collect()` is inherent (a future
  has no stream to return), so it never yields to the JS event loop and deadlocks
  the single-threaded wasm isolate. Its three remaining call sites (netio
  `send`/`send_sse`, ai `backend_utils::sleep`) are the traps T3/T4 remove.
- **Fixed** the single-threaded `FutureIterator` (`future_task.rs`,
  `not(multi)`) that rebuilt + dropped the future on every `next()`, so a future
  that returned `Pending` once could never make progress. It now holds
  `factory: Option<F>` (runs once) + `future: Option<Pin<Box<Fut>>>` and re-polls
  the retained future on each `next()`, yielding `ThreadedValue::Waiting` on
  `Pending` without dropping it.
- Added `test_threaded_future_survives_pending` (a `PendingNTimes` future that is
  `Pending` twice before resolving) asserting two `Waiting`s then the values flow —
  the old code would have yielded `Waiting` then `None`, losing the values.
- Gates green: `cargo check -p foundation_core` (default + `--features multi`) and
  `cargo check -p foundation_core --target wasm32-unknown-unknown`; full
  `cargo test -p foundation_core --profile uat` (742 passed, 0 failed).

## 06 T3 (2026-10-20) — wasm `send_sse` goes lazy; the deferred-error channel lands

- `SseProgress` gained `Failed(String)` (so `Clone, Debug, Eq, PartialEq`, but the
  `Copy` derive is gone — `String` isn't `Copy`). This was safe because no code
  matches `SseProgress` exhaustively: the native `map_progress` only *constructs*
  it, and the model layer maps `Stream::Pending(_)` / `Stream::Pending(p)` by
  wildcard/bind, never by enumerating variants.
- Wasm `send_sse` no longer wraps `fetch` in `block_on_collect`. It returns a
  `LazySseIterator` immediately; the first `next()` spawns the fetch via
  `spawn_local` and yields `Pending(Connecting)`, then `Wait` until the fetch
  resolves, then flattens the body `WasmSseIterator`. A fetch failure surfaces as
  `Pending(Failed(msg))` then `None` (the `failed: Option<String>` terminal guard),
  because a lazy iterator can no longer carry the error on its `Result`.
- The fetch work is a free `async fn fetch_sse(req, max_redirects) -> Result<WasmSseIterator, _>`,
  shared by the async `send_sse_async` seam and the lazy `spawn_local` path. It holds
  `web_sys::Request`/`Response` across `.await`s, so it is `!Send`; `send_sse_async`
  wraps it in `SendWrapper`, `spawn_local` needs no `Send`.
- `impl WakeableStream for WasmSseIterator` registers on a `WakeRegistry` that the
  `spawn_local` reader fires on every queue push and on done/close. `impl
  WakeableStream for LazySseIterator` delegates to the body iterator once the fetch
  resolved, else registers on its own fetch-stage registry. Both types carry
  `unsafe impl Send` (wasm is single-threaded; the `!Send` registry/callbacks are
  only touched on that thread).
- `send_sse_async` switched from `into_pure_future_stream()` (raw lens, forwards
  `Wait`/`Pending` and busy-polls) to `into_future_stream()` (parks via
  `WakeableStream`), which is what makes the async seam genuinely await-friendly.
- Gates green: `cargo check -p foundation_netio` (native), wasm `--no-default-features
  --features wasm-fetch`, and `cargo test -p foundation_netio --profile uat`
  (979 passed, 1 pre-existing macOS unix-socket failure `EINVAL`, already in the
  backlog). `cargo check -p foundation_ai` (native + wasm) also green after the
  `SseProgress` change.

## 06 T4 (2026-10-20) — model-layer error mapping + the last `block_on_collect` backoff is retired

- `OpenAIStream::next` and `WorkersAiStream::next` now split the `SseProgress::Failed`
  arm out of their `Stream::Pending(_)`/`StreamSpread::Pending(_)` matches and map it
  to `ModelState::Error(msg)`; every other `Pending` still maps to
  `GeneratingTokens(None)`. This is the in-band path a lazy `send_sse` uses to reach a
  sync `Model::stream` consumer with a deferred fetch failure.
- Wasm `backend_utils::sleep` was the third and last wasm `block_on_collect` caller.
  It became a documented **no-op**: there is no synchronous sleep on wasm
  (`std::thread::sleep` panics "time not implemented", and `block_on_collect` deadlocks
  the isolate), and the retry backoff that calls it is unreachable on wasm anyway —
  the non-streaming `generate` path deadlocks earlier at `HttpClient::send`, which
  still wraps `fetch` in `block_on_collect`. Making backoff genuinely yield needs an
  async `generate` seam (a `Model` API change) that feature 06 deliberately avoids;
  the non-streaming wasm path is tracked in `docs/bug-backlog.md` under a new
  `foundation_ai` section.
- Two new tests (`tests/providers/sse_error_mapping_tests.rs`) drive the public
  `Model::stream` path through a mock `HttpClient` whose `send_sse` yields a single
  `Pending(Failed("boom"))`, asserting `OpenAIStream` and `WorkersAiStream` both map
  it to `ModelState::Error("boom")` and then terminate. Gates green: `cargo test -p
  foundation_ai --profile uat` (all suites pass, provider_tests 243→245) and wasm
  `cargo check --target wasm32-unknown-unknown -p foundation_ai --no-default-features`.

## 06 T5 (2026-10-20) — the live smoke test: park, don't poll; wake from JS

- **Reproduce before theorising.** `wrangler dev --local` plus a 40-line Python
  mock of `/chat/completions` (SSE chunks 300 ms apart, wired in through
  `OPENROUTER_BASE_URL`) reproduced the hang in seconds and showed the mock
  received **zero** requests, so the isolate was frozen before the fetch, not
  waiting on it.
- **Hang 1: a hidden sync HTTP call.** `OpenAIProvider::get_model` does a sync
  `GET models/{name}` on a cache miss (the first turn). On wasm, sync `send` was
  `block_on_collect`, which runs a nested `run_until` *inside the running agent
  task*. The running task's slot counts as active, so the nested loop spins on
  `CanProgress(None)` forever. Lesson: no sync "block on" helper may be reachable
  from a task on wasm. Wasm `send` now errors (`use send_async`), and `get_model`
  skips the optional lookup on wasm.
- **Hang 2: `TaskStatus::Wait` is not a park.** `Wait` keeps the task active
  *and* pushes a `Stream::Wait` item to the output queue; every push wakes the
  consumer, whose next poll drives the executor and arms another `setTimeout`
  chain. Chains multiplied (~1,700 timer callbacks/s). Use
  `TaskStatus::Depends(signal)` for I/O waits.
- **Make the boxed streams wakeable.** `BoxedSseIterator`/`ModelStreamBox` became
  `dyn WakeableStream` (the trait's default `wake` = `NotSupported`, so in-process
  streams need only an empty impl). The valtron adapters (`MapPending`, …) and
  `NotifyQueue` already forward `wake`, so native SSE gets real wakes too.
  `NotifyQueue::register_wake` fires immediately if data is already queued; the
  wasm SSE iterators now do the same (register-then-check), so a wake is never lost.
- **Event-driven resume instead of timers.** JS wasm `run_until` now returns to
  the event loop when every task is parked on readiness. Whoever flips the signal
  calls `request_executor_resume()` (`WakeSignal::callback` does), which queues
  one coalesced executor run as a microtask: `spawn_local` on wasm-bindgen, and a
  new `schedule_microtask` host import (`queueMicrotask`) on foundation_wasm.
  `setTimeout` stays only for real timers (`Delayed`/`SpinWait` deadlines).
- **foundation_wasm schedule registry:** callbacks ran under the registry mutex
  and fired timeouts were never removed. A callback that reschedules (valtron's
  yield loop, `testing::run_async`) re-locked the same std `Mutex`. Callbacks are
  now taken out (one-shot) and run after the lock drops; cancelling an
  already-fired one is a no-op.
- **Unrelated bugs the smoke test exposed:** `OPENROUTER_BASE_URL` ended in `/v1`
  while `build_url` appends `api_version`, so every OpenRouter call hit
  `/api/v1/v1/...` (fixed). And the agent loop ran `LoopDetector` on the raw
  streamed chunks: HTTP backends send snapshots (the text so far), so every
  answer looked like a loop (4 model calls, then `LoopDetected`). The loop already
  had the right joiner (`join_text_chunks`); detection and `Turn::text`/`Answer`
  just weren't using it. Fixed; `session_mock_tests::snapshot_stream_*` pin it.
- **`TaskStatus::Wait` on native single-threaded** mapped to `State::Pending(None)`,
  which no driver stop-check sees, so a forever-`Wait` task held the caller
  (`park_busy_loop` was red). It is now `State::Wait` everywhere; the native stream
  driver hands back on it, other native loops keep re-polling.
- **`cargo test -p foundation_ai` skips the agentic suite**: `agentic_tests`
  needs `--features testing`. Run it explicitly.

## "Zero response" root cause (2026-10-24) — a failed turn streamed nothing

Both deployed examples (`cf-cloud-provider`, `cf-agent-loop`) answered a failed
turn with an empty 200 response. The failure was dropped at four layers, so the
terminal `Failed` event never reached the caller.

- **The providers dropped the partial text and the error.** On an in-band error
  after some text had streamed (a bad Server-Sent Events (SSE) chunk, a vendor
  `error` event, or a transport failure), each provider emitted a bare
  `Stream::Pending(ModelState::Error(..))`. The text already streamed was lost.
  Fix: `partial_then_error` emits `Stream::Spread([Done(partial),
  Pending(ModelState::Error(..))])` — the accumulated text first, then the failure.
- **`lift_model_item` dropped the error inside a spread.** Its `Stream::Spread`
  arm mapped `StreamSpread::Pending(ModelState::Error(..))` through the general
  progress arm, which returned `Initializing`. The failure became a progress event
  and was never reported. Fix: the spread arm maps `Pending(ModelState::Error(..))`
  to `Done(FailedAction)`.
- **The agent loop dropped all but the first spread item.**
  `transition_inner_generate` emitted only the first element of a `Stream::Spread`.
  A `[partial, error]` spread lost the error. Fix: it fans the whole spread out as
  `TaskStatus::Spread`.
- **`into_text_stream` dropped the terminal `Failed`.** It yielded only text
  chunks; `TurnEvent::Failed` fell into a catch-all and was discarded. Fix: it now
  yields `TurnTextEvent` (`Text` / `Done` / `Failed`), so the terminal outcome
  always reaches the caller.

The mock substrate gained `MockModelProvider::stream_fails_after_partial(..)`
(a `MockSpreadStream` that emits the in-band error shape). This scripts the spread
path in tests, not only the `Err`-returning path.

Tests prove each layer:

- `tests/providers/{openai_provider, anthropic_messages_provider,
  workers_ai_provider}.rs`: each has a `*_stream_error_emits_partial_then_failure`
  test. Partial text, then the failure in one spread, for all three providers.
- `tests/agentic/progress_tests.rs`:
  `lift_spread_error_becomes_failed_action_not_progress` — the lift maps the
  spread error to a `FailedAction`.
- `tests/agentic/agent_loop_generation_tests.rs`:
  `stream_error_after_partial_emits_partial_then_failed_action` — the loop fans
  the spread out (partial text plus `FailedAction`).
- `tests/agentic/session_mock_tests.rs`:
  `run_turn_keeps_partial_text_from_an_in_band_stream_error` and
  `text_stream_reports_an_in_band_stream_error_after_partial` — end to end through
  `Turn` and `into_text_stream`, partial text then `Failed`.

All suites pass under `--profile uat --features testing`.

## Text overlapping and the empty-queue EOF bug (2026-10-24)

After the zero-response fix, the deployed `cf-cloud-provider` still showed two
problems. The chat text repeated itself, and the worker log showed
"Failed to parse SSE chunk".

### Overlapping text: snapshots, not deltas

`chat()` mapped `turn.events()`. Each `TurnEvent::Text` carries the full text
so far (a snapshot), because HTTP backends re-send the whole reply each chunk.
The client appends every `content`, so it repeated the whole prefix each time.

The fix: `chat()` now maps `turn.into_text_stream()`. That stream yields
`TurnTextEvent::Text` deltas. It reduces each snapshot to its new suffix. The
client appends each delta once. `event_frame` became `text_frame`.

### The empty-queue EOF bug: a split `data:` line parsed early

The wasm `QueueReader` adapted a `ConcurrentQueue<Vec<u8>>` to `std::io::Read`
for `SseParser`. When the queue was empty for a moment, `read` returned `Ok(0)`,
the end-of-stream signal. A `data:` line split across two JS chunks then parsed
as a complete line. The parser emitted a partial JSON payload and failed with
"Failed to parse SSE chunk".

The fix: `QueueReader` now holds the `done` flag. When the queue is empty and
the stream is not done, `read` returns `ErrorKind::WouldBlock`. `SseParser` parks
and resumes when more data arrives. `read` returns `Ok(0)` only when the queue
is empty and the stream ended.

### The swallowed error kind

`EventSourceError::ReadError` held a `String`, so the `io::ErrorKind` was lost.
`WasmSseIterator::next` could not tell `WouldBlock` from a real error, and mapped
every error to `Wait`. `ReadError` now holds the `io::Error`. `next` logs real
errors with `tracing::error!` and treats only `WouldBlock` as "wait".

### The tracing-wasm hang

The worker used `tracing-wasm`. Its span hooks call
`performance.mark` and `performance.measure`, the User Timing API. Workerd does
not implement that API, so the first span froze the isolate (error 1101). A
custom `tracing_subscriber::fmt` writer that forwards lines to
`web_sys::console::log_1` replaces it. That writer needs only the console API,
which workerd supports.

Deployed and checked: `cf-cloud-provider` streams the poem as five deltas with
no overlap. `cf-agent-loop` answers with text. Both workers run under
`wrangler tail`.

Green under `--profile uat`: `foundation_netio` event-source tests (65 passed),
`foundation_ai` full suite (agentic 324, tools/memory/models 148, doc-tests 8).

## Reusable `#[wasm_logs]` console backend (2026-10-24)

Each example hand-wrote the same tracing subscriber + `console.log` writer in an
`init_logs` function. That is now one reusable attribute macro.

- `#[wasm_logs(console, tracing = "…")]` on a wasm entry point installs the
  console subscriber. `foundation_macros` emits it; `foundation_core` re-exports
  it. The macro injects one idempotent init call into the function body.
- The default backend is `console`. An early `#[derive(Default)]` made the
  default `stdout`, a no-op on wasm, so `#[wasm_logs]` with no arguments logged
  nothing. A manual `Default` impl fixes it to `console`.
- `foundation_compact`'s `host-abi` backend declares a `host_console_log` `abi`
  import (like `host_fill_random`) and a `ConsoleWriter`. This lets the
  no-wasm-bindgen path log too. `foundation-wasm.js` supplies `host_console_log`
  in its `web_abi`.
- Both examples use the macro. Their `init_logs` functions and the unused
  `console_log`/`tracing`/`tracing-subscriber` dependencies are removed.

Verified: `foundation_compact` checks on native, `host-abi`, and `wasm-bindgen`;
both examples check on `wasm32-unknown-unknown`; `foundation_macros` unit tests
(33) pass.

## no_std tracing subscriber — what it takes (investigation)

There is no ready-made no_std formatting subscriber on crates.io.
`tracing-subscriber`'s no_std support (PR #1648) covers only the
`Layer`/`LookupSpan` traits, not `fmt` or `EnvFilter`. `tracing-wasm` uses the
User Timing API and froze workerd. So a no_std subscriber must be hand-rolled.

What a hand-rolled subscriber needs:

- `tracing = { version = "0.1", default-features = false }` in
  `foundation_nostd`. This drops `std` and `attributes`; the `info!`/`debug!`
  macros still work because they are `macro_rules!` inside the crate.
- Seven required `Subscriber` methods: `enabled`, `new_span`, `record`,
  `record_follows_from`, `event`, `enter`, `exit`. A `field::Visit` impl needs
  only `record_debug`; the typed methods default into it. `record_error` is
  `#[cfg(feature = "std")]`, so it is absent in no_std.
- A sink. no_std has no stdout. Two options: (A) a host-injected `fn(&str)`
  set before install, forwarded by `foundation_wasm`'s `host_console_log` seam;
  (B) drop events. Option A is ~90 lines; Option B is ~15 lines.

Implemented Option A in `foundation_nostd::trace` (feature `trace`):
`NoStdSubscriber` (7 methods + a `field::Visit` formatter), `set_sink(fn(&str))`,
and `try_init(Level)`. `set_global_default` is not `std`-gated (only the
thread-local `set_default`/`with_default` are), so install works in no_std.

Two things the first pass got wrong, now fixed:

- `tracing = { workspace = true, default-features = false }` is ignored: the
  workspace dep sets no `default-features`, so the override needs the 2024
  edition. It silently compiled `tracing` with `std` + `attributes`. The fix is
  `tracing = { version = "0.1", default-features = false, optional = true }`,
  which compiles only `tracing` + `tracing-core` in no_std.
- `trace.rs` must declare `extern crate alloc;` itself, like `embeddable.rs` and
  `raw_parts.rs`. The crate root's `extern crate alloc` is `#[cfg(not(feature =
  "std"))]`, so a dev-dependency that turns on `std` removes it and breaks
  `use alloc::…` in modules that do not declare their own.

Verified: `--no-default-features --features trace` (no_std) and
`--features "std trace"` both check; the integration test passes. The message
field prints unquoted (`fmt::Arguments`), while `&str` fields print quoted.


