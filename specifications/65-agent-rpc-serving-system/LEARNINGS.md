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

## 01 T1 (2026-10-20) — shareability spike: shared weights, per-generation KV

- **The assumption was false for Candle, and the test proved it.** `CandleModels`
  was `Arc<Mutex<state>>` with the KV cache *inside* the shared state, and the
  stream took the lock per token. So it was `Send + Sync`, but two generations on
  one loaded model reset and overwrote each other's KV cache between tokens.
  `interleaved_streams_on_one_gemma2_model_do_not_cross_talk` (two greedy streams
  polled A, B, A, B) was red: their tokens differed from the same streams run
  alone. Greedy decoding makes "same as alone" an exact cross-talk check.
- **Fix (in `foundation_ai/src/backends/candle.rs`), not the `Arc<Mutex>`
  fallback:** split the model into read-only `CandleWeights` (in an `Arc`,
  shared by every clone) and per-generation `CandleGeneration` (its KV cache).
  Llama's cache is external, so a generation is just a fresh `Cache`; Gemma2's is
  internal, so a generation clones the model (candle tensors are refcounted, so
  no weights are copied) and clears the cache. No lock is held across a forward
  pass, so sessions on one model now run in parallel, not serialized.
- **Found on the way: Llama ran with the KV cache off** (`Cache::new(false, ..)`)
  while the stream fed one token per step, so every token after the first
  attended only to itself. Now `true`.
- **Stream errors were swallowed**: a failed forward/sampling ended the stream as
  `Finished`. It now yields `Pending(ModelState::Error(..))` (the loop maps it to
  a `FailedAction`).
- **`CandleModels::shares_weights_with`** (`Arc::ptr_eq`) is the observable proof
  that two handles are one load; agent-rpc's `ModelHandle` builds on it.
- **llama.cpp (`LlamaModels`) is unverified**: building it needs the
  `tools/llama.cpp` sources, which this environment does not have. Backlog §10.
- **Build notes:** candle's `pulp` does not link under the default cranelift dev
  backend; build and test with `--profile uat` (LLVM). `foundation_ai`'s own test
  targets cannot build without llama.cpp (unconditional dev-dependency), so the
  spike's tests live in `foundation_agent_rpc/tests/model_share_tests.rs`.

## 01 T2–T3 (2026-10-20) — scaffold and `ModelHandle`

- **Scaffold:** one `AgentRpcError` enum (derive_more, not thiserror — the
  rust-clean-code rule) with `code()` as the only Connect/gRPC status table;
  modules are added when their task lands rather than as empty files.
- **No `Arc<Mutex<…>>` fallback was needed** (T1 made candle concurrent), so
  `ModelHandle` is `Arc<{name, ModelId, ProviderRouter}>`. A local model is
  wrapped in `PreloadedProvider`, whose `get_model` hands every session a clone
  of the same `CandleModels`. The handle holds no loader, so nothing downstream
  can reload; `load_with(name, loader)` runs the loader exactly once.
- **`from_router(name, model_id, router)`** serves any router (cloud providers,
  `MockModelProvider` in tests). `MockModel` is not `Clone`, so it cannot go
  through `PreloadedProvider`.
- **GGUF:** there is no `ModelSource::Gguf` arm: the llama.cpp backend cannot be
  compiled in this environment, and writing an arm that was never built would be
  an untested stub. A GGUF `LlamaModels` (a cheap `Arc` clone) is served with
  `ModelHandle::load_with`/`from_model`; the `gguf` feature enables
  `foundation_ai/llamacpp`. Adding the arm is tracked with the llama.cpp
  concurrency check in backlog §10.

## 01 T4 (2026-10-20) — `InstanceRegistry`

- An instance is `Arc<{name, ModelHandle, AgentSession, busy flag}>` in a
  `RwLock<BTreeMap>`; `create` builds the session on the model's router outside
  the write lock and never loads (ten instances, loader count 1).
- **One turn per instance at a time:** an `AgentSession` shares its steering
  queues across turns, so two concurrent turns would read each other's prompts.
  `begin_turn()` returns a `TurnGuard` or `InstanceBusy` (→ `aborted`).
- Names: 1..=63 of `[a-z0-9._-]`, starting alphanumeric (RFC 1123 label). Limits
  (`max_instances` 1024 = nginx `worker_connections`, `max_name_len` 63) are
  `RegistryConfig` fields.
- `destroy` aborts a running turn and calls `AgentSession::end()` (flush); an
  `end()` error is returned, not dropped, after the instance is removed.

## 01 T5 (2026-10-20) — generated `AgentService`

- **Contract:** `proto/ewe/agent_rpc/v1/agent.proto` — `AgentService` (Health,
  Create/List/DestroyInstance, Chat, ChatStream) + `BootstrapService` (Enroll)
  + `BootstrapPayload`. `build.rs` = the docker BuildKit pattern: `buffa-build`
  for messages, `foundation_connectrpc_codegen::generate_services` once per
  service (each service's `pub mod procedure` needs its own module). Needs
  `protoc` (`PROTOC`; installed under mise at `~/.local/share/mise/installs/protoc`).
- **`buffa-build` must match the `buffa` runtime.** 0.9.2 codegen calls 0.9-only
  APIs (`saturate_size`, `EncodeSink`) and does not compile against connectrpc's
  0.8.1 runtime; pinned `=0.8.1`. The docker crate pairs 0.9.2 with 0.8.1 too
  (backlog §6). Views are off (`generate_views(false)`), unused here.
- **Generator fixes (not local `allow`s):** default trait bodies named their
  unused params `ctx`/`request` (unused-variable warnings in every consumer), and
  the client mocking trait used `async fn` in a public trait without allowing
  `async_fn_in_trait`. Both fixed in `foundation_connectrpc_codegen`.
- **Async turn consumption:** `TurnStream` did not implement `WakeableStream`,
  so it could not use `into_future_stream()`; it now forwards `wake` to its
  `DrivenStreamIterator`. And `TurnEvent::from_stream_item` (public) is the
  record→event mapping `TurnEvents` uses, so an async handler can map raw items
  without the blocking `TurnEvents` iterator.
- **Boundary note:** `DrivenStreamIterator::next` waits up to its park duration
  on the result queue before yielding `Wait`, so a poll of the bridged stream can
  hold a worker briefly. That is valtron's existing boundary behaviour, not new.
- `ChatStream` holds the instance's `TurnGuard` until the stream ends; dropping
  it mid-turn (client gone) or a cancelled `Ctx` aborts the session's turn.

## 01 T6 (2026-10-20) — unary over Connect on loopback

- **Server shape:** one listener, `ServerApp::Both` (HTTP/1.1 for Connect and
  gRPC-Web, h2c for gRPC), every `AgentService` procedure routed to a
  connectrpc handler. `ConnectRpcHandler` is not `Clone`, so the h1 and h2 apps
  each get their own `agent_router` over the same `Arc<AgentRpcService>`. The
  serve loop runs on a named std thread; `RunningServer::graceful_shutdown`
  turns the `OnSignal` and joins it (also on drop). The builder refuses to build
  until auth is chosen explicitly (`insecure_plaintext()` for local use).
- **Worker starvation (important for operators):** the h1 connection driver
  blocks its valtron worker in `poll(2)` for the whole call, and a turn also
  needs workers for the client tasks and the agent loop. With 4 workers a
  `Chat` never ran (client hit its deadline); with 8 it completes, and 10
  sequential calls before it show no per-call leak. The same `chat` future
  completes under `futures::executor::block_on` off the pool, so the wake path
  is fine; it is pure pool exhaustion. Run agent-rpc with ≥ 8 workers plus one
  per concurrent HTTP/1.1 call (backlog §1 has the root cause).
- **connectrpc client bug fixed:** a non-200 Connect unary response was decoded
  from the HTTP status only and the raw JSON body became the message, so
  `aborted`/`already_exists` (both 409) and `invalid_argument`/
  `failed_precondition` (both 400) were indistinguishable, and `is_wire_error()`
  was false. `do_unary_round_trip` now reads the `WireError` body's code and
  message first. connectrpc's connect/error/conformance/grpc-web/client/grpc
  suites still pass.

## 01 T7 (2026-10-20) — `ChatStream`, proven unbuffered

- **Proof design:** a gated model emits token `k+1` only once the *client* has
  received token `k`; any buffering between model and client keeps the gate shut
  and the turn fails. The gate yields `Stream::Delayed(2ms)` instead of blocking
  (the agent loop turns that into `TaskStatus::Delayed`). An in-process variant
  drives `AgentService::chat_stream` directly.
- **The first red was not buffering.** The first gated model blocked on a
  condvar, and the stream "buffered" over both h1 and h2c. Bisecting: a raw
  socket got head + frame 0 from a bare connectrpc server in 8 ms; `H1Transport`
  alone delivered head (63 ms) and frame 0 (103 ms); but the connectrpc client's
  `server_stream` only returned after the call's deadline. Cause: valtron
  re-polls readiness-parked tasks only on the worker that parked them, so the
  client's reader task (a `from_future` task parked on a waker) is stranded
  when that worker blocks — on my condvar, or in the h1 server's
  `ConnIo::block_on` `poll(2)`. Backlog §4 (valtron) and §1 (h1 serve).
- **Consequence for tests:** the unbuffered proof runs Connect over **h2c**
  (`H2Transport`; the server's h2 path never blocks a worker). Connect over
  HTTP/1.1 streaming is round-tripped with an ungated producer; it passes but can
  stall ~30 s when the client's reader lands on the server call's worker. This
  only affects a client in the *same process* as the server (tests); a remote
  client is unaffected, though server-side tasks parked on a worker that then
  blocks in an h1 call are affected the same way.

## 01 T8 (2026-10-20) — bootstrap token → client certificate → mTLS

- **Two listeners, no handler-level auth.** The agent listener is mutual TLS
  (rustls `WebPkiClientVerifier` over our CA): a call without a CA-issued client
  certificate fails in the handshake and never reaches a handler, so the router
  has no auth logic (the spec's "auth gates the router"). A second, server-auth
  TLS listener serves only `BootstrapService/Enroll`. One listener with optional
  client auth would need the verified peer certificate plumbed into `Ctx`, which
  `foundation_http` does not do.
- **Enrollment = token + CSR.** The client generates its key locally and sends
  a CSR; the CA copies only the CSR's public key (subject = validated client
  name, non-CA, `clientAuth` EKU, our lifetime). The token is 256 bits of
  `foundation_compact::entropy`, base64url, compared in constant time,
  single-use by default (`MtlsConfig::with_max_enrollments`), rotatable
  (`EnrollmentService::rotate`), zeroed on drop, redacted in `Debug`/`Display`.
- **Before it has the CA, the client pins the server certificate's SHA-256**
  (the fingerprint the bootstrap payload carries); handshake signatures are
  still verified.
- **Bugs fixed on the way (both made "https" silently not work):**
  - `foundation_connectrpc` `H1Transport` rebuilt every URL as `http://`, so an
    `https://` base URL sent plaintext to a TLS port (the client then read the
    TLS alert as an HTTP status line).
  - `foundation_netio` `NativeHttpClient::open_exchange` (the path H1Transport
    uses) built a fresh connection pool, discarding the client's TLS connector
    and custom `Connector` — custom roots and client certificates never applied
    (`UnknownIssuer` against our CA). It now uses the client's pool.
  - Added `RustlsAcceptor::with_config` (server twin of
    `RustlsConnector::with_config`) so a server can use an mTLS `ServerConfig`.
- **Log hygiene test:** `#[traced_test]` cannot coexist with `#[valtron_test]`
  (both set the global subscriber), so `auth_log_tests` is its own binary that
  installs a TRACE-level capturing subscriber before starting the pool and runs
  the whole lifecycle — including logging the token's `Debug` on purpose.
- **Not covered:** gRPC over TLS has no client (`H2Transport` is h2c only), so
  gRPC is exercised in plaintext mode (T10). Proxy forwarding of the mTLS
  channel is out of this feature (the spec's proxy terminates TLS; end-to-end
  client certificates through it need passthrough — spec 65 F3).

