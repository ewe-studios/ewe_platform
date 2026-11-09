# Spec 75 progress

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. One entry per batch or feature PR: date, scope, result._

## 2026-11-09: the branch's first full CI run, two gates and two issues

The first full audit of the branch ran on 2026-11-09. The image build failed first
with `curl: (22) ... error: 504` in the `setup-nu` step, a GitHub release download.
The Rust `Analyze` job passed. A re-run built the image and then ran the package jobs.

Two package jobs failed:

| Job | Cause | Action |
| --- | --- | --- |
| `foundation_core` | `scripts/ci/check_time_sources.py` counted a doc comment that names a clock call as a direct read (`native/readiness.rs:271`, from commit 1a774aff8) | Reworded the doc line; the guard passes; issue #435 records that the guard should skip comment lines |
| `foundation_reactor` | 61 clippy errors in Linux-only poll code, which macOS clippy never compiles | Issue #434, fixed now |

`cargo clippy --profile uat -p foundation_reactor --target x86_64-unknown-linux-gnu
--all-targets --no-deps -- -D warnings` reproduces the Linux list on macOS because the
Linux target is installed. Use it for any crate with `target_os = "linux"` code.

The debt is pre-existing, not new. The four Linux-only files are byte-identical on
`master` (`176dd9a18`) and on this branch before the fix. `master`'s audit run 42596013845
did run the `foundation_reactor` job, and its clippy step printed the same 61 warnings but
ended with `Finished` and conclusion `success`, because the package entry did not set
`clippy-deny-warnings` then. This branch set it (`ci/packages.toml`, the #336 fix), so the
next run made the debt visible as errors. Issue #436 records the same gap for the other
24 packages that still do not set the flag.

Issue #433 (low) records the `setup-nu` download: it fetches nushell from GitHub releases
on every run with no cache, and a 504 burst fails the whole audit.

The #434 fix is commit `206f9d84a`, in `backends/foundation_reactor/` only. It replaces
the lossy casts with `try_from`, `std::ptr::from_ref` and `.cast::<T>()`, adds the missing
`# Errors` and `# Panics` sections and `#[must_use]` markers, and turns three module docs
into `//!` docs. The Linux test target had one further error, which the saved log did not
show. One new `#[allow]` carries a reason: `clippy::unused_self` on `Poll::rearm_readable`,
whose Linux body is empty.

| Check | Result |
| --- | --- |
| Linux-target clippy, all targets, `-D warnings` | 0 warnings |
| macOS clippy, all targets, `-D warnings` | 0 warnings |
| `cargo test --profile uat -p foundation_reactor` | all pass |
| `cargo fmt -p foundation_reactor --check` | pass |

Result: head `608877411` is green. The `Audit: Checks & Tests` run (42606247964), the
`Platform images (publish)` run (42606248008) and all five `CodeQL` jobs (42606242705)
succeeded. The `foundation_core` and `foundation_reactor` jobs pass with the new
`clippy-deny-warnings` gate on.

Next: turning on `clippy-deny-warnings` for the other 24 packages (issue #436) is the
lasting fix for the class of defect that #434 found.

## 2026-11-09: issue #257, a peer stop reaches the h3 handler

A client that cancels a Connect call after its request body ends stayed invisible to
`dispatch_h3`. The unary handler ran to completion, and the streaming exchange waited on
a `std::future::pending()` that never resolved.

Cause: `quinn-proto` frees a stream's receive state when the body reads FIN and drains.
A `RESET_STREAM` that arrives after that point is dropped with
`received RESET_STREAM on closed stream`. The one frame that still arrives is
`STOP_SENDING` on our send half (RFC 9114 section 4.1.2).

Changes:

| Crate | Change |
| --- | --- |
| `foundation_netio` | `QuicSendStream::stopped()` returns the peer's `STOP_SENDING` code, default `None`. `QuinnSendStream` reads `SendStream::stopped()`; `QuinnBidiStream` delegates to its send half. `H3Request::peer_stopped()` exposes it. |
| `foundation_connectrpc` | Unary races the handler against a probe. A stop cancels the call's `Ctx` and resets the response half with `H3_REQUEST_CANCELLED`. Streaming passes the probe as the `gone` future of `exchange_until_aborted`, and the `PeerGone` arm resets the response half. |

`foundation_netio` goes from 0.0.11 to 0.0.12. Every consumer requirement and every
committed lockfile names 0.0.12.

Tests:

| Test | Proves |
| --- | --- |
| `foundation_netio`, `peer_stop_sending_surfaces_on_the_send_half` | The backend reports the stop code and latches it |
| `foundation_connectrpc`, `peer_stop_sending_after_unary_body_cancels_the_call` | A unary call ends when the peer stops after its body ends; the `Ctx` is cancelled |
| `foundation_connectrpc`, `peer_stop_sending_after_request_end_cancels_a_stream` | The same for a streaming call |

Both connectrpc tests were run against the old code. Each timed out while waiting for
`dispatch_h3` to complete, so each proves the fix.

Verification:

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_netio --features quic --test quic` | 9 passed |
| `cargo test --profile uat -p foundation_connectrpc --features h3` | 308 passed |
| `cargo test --profile uat -p foundation_http --features quic` | Pass |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --all-features --no-deps -- -D warnings` | Zero warnings |
| `cargo clippy --profile uat -p foundation_netio --all-targets --features quic --no-deps -- -D warnings` | Zero warnings |
| `cargo fmt -p foundation_netio -p foundation_connectrpc --check` | Pass |

## 2026-11-09: issue #409, the h2 server path leaves the shared router

`shared/router/dispatch.rs` held nineteen `cfg(not(target_family = "wasm"))` gates. The
HTTP/2 server path names `foundation_netio::http2` types and valtron pipes, so it is
native-only, but it sat in the target-agnostic tree.

The path now lives in `native/dispatch_h2.rs`: `run_unary`, `send_whole_response`,
`H2Body`, `collect_h2_body`, `dispatch_h2`, `run_streaming_h2`, `grpc_status_trailers`
and the h2 header helpers. `h3_serve.rs` imports them from there.
`shared/router/dispatch.rs` has no wasm target gate. Three items became `pub(crate)`:
`ConnectRpcHandler::router`, `deadline_response` and `is_framing_header`.

The agent also discarded the `#[must_use]` result of `PipeSender::close` in the h3
server and transport. The all-features lint gate rejected the eight existing discards.

Verification:

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_connectrpc` | 273 passed |
| `cargo test --profile uat -p foundation_connectrpc --features h3` | 308 passed after the merge |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --all-features --no-deps -- -D warnings` | Zero warnings |
| wasm check, `--no-default-features --features rpc` | Zero warnings |

## 2026-11-09: issue #290, the HTTP/3 echo test does not reproduce

`http3_server_echoes_a_request_over_quic` fails about 1 to 3 times in 100 runs. The
failure is `poll_headers` with no response headers after 15 s (`tests/http3_server_tests.rs:209`).

Two soak runs:

| Run | Command | Result |
| --- | --- | --- |
| 50 runs | `cargo test --profile uat -p foundation_http --features quic --test http3_server_tests http3_server_echoes_a_request_over_quic -- --exact` | 1 failure at run 30 |
| 200 runs | same command, the failing output copied to `/tmp/290-fail.log` | 0 failures |

The first failure is real: `error: test failed` at run 30, and run 31 passed at once. The
soak script printed only the test result, so the panic text is lost. The second soak
captured no panic at all.

The working tree holds the #257 probe on the send half. That probe gives a future run a
new observable: a `STOP_SENDING` that arrives while the handler waits. It does not change
the handshake path, so it does not explain the negative result.

Issue #290 stays open with this evidence. The next pickup must print the test's own
stderr on failure, and it must record the driver state (connection, stream and packet
counters) at the moment of the timeout.

## 2026-11-09: issue #428, the macOS watcher compiles again

`foundation_nativeapis` did not compile with `--features watcher-macos`. The error was
`error[E0624]: method 'selector' is private` at
`backends/foundation_nativeapis/src/native/watcher/unix.rs:76` and line 108.

`KqueueWatcher` called `poll.registry().selector()` to register and deregister its
kqueue fd. `foundation_reactor` keeps `Registry::selector` private. The public calls are
`Registry::register_vnode` and `Registry::deregister_vnode`, and they already exist.

The fix:

- `add_watch_inner` calls `register_vnode(raw_fd, vnode_token)`.
- `close_fd` calls `deregister_vnode(fd)`.
- The unused `next_token` field and the `Registry` import are deleted.
- The kqueue event reader converts `event.ident()` with `i32::try_from` and skips an
  event whose fd has no path, instead of silently wrapping a value that cannot be a
  file descriptor.
- One event with several flags reports `Removed` before `Modified`, because a delete
  also sets the write flag.
- The doc comments wrap `KqueueWatcher` and the `EVFILT_*` names in backticks. The
  `new` function documents its errors.

The macOS watcher had no CI check. `.github/workflows/platform-vms.yaml` now runs

```
cargo check -p foundation_nativeapis --target aarch64-apple-ios --features watcher-macos --profile uat
```

in the `macos-ssh` job, next to the iOS checks of `foundation_wireguard` and
`foundation_platform`. The check compiles the shared `watcher/unix.rs` file. Issue #432
records that the job is `workflow_dispatch` only.

Verification:

| Check | Result |
| --- | --- |
| `cargo check --profile uat -p foundation_nativeapis --features native-macos` | Pass, no warnings |
| `cargo check --profile uat -p foundation_nativeapis --target aarch64-apple-ios --features watcher-macos` | Pass, 3 warnings from issue #431 |
| `cargo clippy --profile uat -p foundation_nativeapis --features watcher-macos --no-deps -- -D warnings` | Zero warnings |
| `cargo test --profile uat -p foundation_nativeapis` | Pass |
| `cargo fmt --check` | Pass |

## 2026-11-09: issue #430, `foundation_proxy` checks the reactor results

`foundation_proxy` ignored the two `#[must_use]` results of the reactor readiness
interface. Clippy reported three `unused_must_use` warnings.

| Site | Before | After |
| --- | --- | --- |
| `src/aio.rs:109`, `AsyncFd::poll_op` | `register_waker` result dropped, then `Poll::Pending` | A `false` result fails the poll with `BrokenPipe` |
| `src/aio.rs:181`, `dial_tcp` | `register_waker` result dropped | A `false` result fails the connect with `BrokenPipe` |
| `src/server.rs:547`, SSL redirect accept loop | `wait_for_events` generation dropped | The generation is bound and the comment says why the next loop turn re-reads it |

A `false` from `register_waker` means the reactor no longer holds the fd. Nothing can
wake the task after that. `poll_op` has no timeout, so it would park forever. The
`dial_tcp` wait has the connect timeout, so a connected socket could report a timeout.

The accept loop is different. It snapshots the generation again before each `accept`, so
the value returned by `wait_for_events` has no later use. The discard is correct, and the
comment records the reason.

Verification:

| Check | Result |
| --- | --- |
| `cargo clippy --profile uat -p foundation_proxy --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo test --profile uat -p foundation_proxy` | Pass |
| `cargo fmt -p foundation_proxy --check` | Pass |

## 2026-11-09: issue #411, the `foundation_nativeapis` shared tree is target-agnostic again

`backends/foundation_nativeapis/src/shared` claimed to be target-agnostic, but it held
host-process code. The crate did not build for wasm and `ci/packages.toml` had no wasm
entry for it.

The native CLI searcher moved from `shared/vfs/search.rs` to a new
`native/vfs/search.rs`:

| Item | New home |
| --- | --- |
| `CliSearcher`, `SearchConfig`, `native_vfs_searcher` | `native::vfs::search` |
| `FffSearcher` (feature `vfs-search-fff`) | `native::vfs::search` |
| `detect_tool`, `probe_tool`, `run_cli_streaming`, `run_and_collect`, `parse_grep_line` | `native::vfs::search` (private) |

`VfsSearcher`, `VfsSearchKind`, `VfsSearchMatch`, `InCodeVfsSearcher`,
`CascadingVfsSearcher` and `EXCLUDED_DIRS` stay in `shared/vfs/search.rs`.
`EXCLUDED_DIRS` is now `pub(crate)`, so the native CLI searcher can use it.

All 22 dead `#[cfg(not(target_family = "wasm"))]` markers are deleted: 21 in
`shared/vfs/search.rs` and 1 in `shared/vfs/mod.rs`. The `shared/vfs/mod.rs` re-export
now covers the target-agnostic items only. `lib.rs` re-exports `CliSearcher`,
`SearchConfig` and `native_vfs_searcher` from `native::vfs::search`, under `vfs-search`.
`native::vfs` is declared for `vfs-native` or `vfs-search`, and each existing child keeps
its `vfs-native` gate.

`foundation_nativeapis` is 0.0.11. Every consumer requirement moves with it, and the
workspace lockfile and four example lockfiles are refreshed.

### Wasm verdict

The crate builds for `wasm32-unknown-unknown`. `native` and `signal` are gated at the
crate root, as `foundation_http` does.

| Command | Result |
| --- | --- |
| `cargo check -p foundation_nativeapis --target wasm32-unknown-unknown --no-default-features` | Pass |
| `cargo check -p foundation_nativeapis --target wasm32-unknown-unknown` | Pass |
| `cargo check -p foundation_nativeapis --target wasm32-unknown-unknown --no-default-features --features vfs,vfs-search` | Pass |

`ci/packages.toml` now sets `wasm = "enforce"` and
`wasm-runs = ["", "--no-default-features --features vfs,vfs-search"]`.

## 2026-11-09: issue #412, native serving types leave `shared/`

Branch `fix/412-native-serving-types`, based on `fix/medium-integration` at `8811a029f`.
The change bumps `foundation_http` from 0.0.9 to 0.0.10 and every consumer requirement.

- `Serve` and `ServeFactory` moved to `src/native/serve.rs`, next to `H2Serve`.
- The native `HttpApp` impls, `H1App`, `H2App`, `H3App`, `ServerApp` and its impls moved
  to `src/native/app.rs`. The generic `HttpApp<S>` and the `ServeWriter` impl stay in
  `shared/app/mod.rs`.
- The `Serve` and `ServeFactory` impls for `HealthHandler` moved to
  `native/handlers/health.rs`. `HealthHandler`, `ServeWriter` and `ServeWriterFactory`
  stay in `shared/handlers/health.rs`.
- `shared/` has no `#[cfg(not(target_family = "wasm"))]` marker for these items.
- All callers use `foundation_http::native::serve::{Serve, ServeFactory}` and
  `foundation_http::native::app::{ServerApp, H1App, H2App, H3App}`.
- `tests/wasm_web_serve_tests.rs` proves the browser `WebServe` dispatch path on the
  host. The `CfServe` dispatch test needs a wasm test target (issue #364), so the wasm32
  library check is its compile proof.
- The `static-html-app` and `web-app` templates use the new public path and the
  `foundation_http` 0.0.10 requirement. All three template manifests now name
  `foundation_core` 0.0.19. The old 0.0.18 pin does not match the local 0.0.19 crate,
  so the template build test failed before the refresh.

Verification:

| Check | Result |
| --- | --- |
| `cargo fmt -p foundation_http --check` | Pass |
| `cargo clippy --profile uat -p foundation_http --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo clippy --profile uat -p foundation_http --all-targets --features quic --no-deps -- -D warnings` | Zero warnings |
| `cargo test --profile uat -p foundation_http` | Pass |
| `cargo test --profile uat -p foundation_http --features "multi quic"` | Pass |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_http --no-default-features --features wasm` | Pass, no warnings |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_http --features wasm-bindgen-http` | Pass, no warnings |
| `cargo test --profile uat -p foundation_http --test wasm_web_serve_tests` | Pass, 7 tests |
| `cargo test --profile uat -p foundation_proxy` | Pass |
| `cargo test --profile uat -p foundation_proxy --features quic --test h3_integration_tests` | Pass, 1 test |
| `cargo test --profile uat -p foundation_deployment_docker --features buildkit` | Pass |
| `cargo test --profile uat -p foundation_connectrpc` | Pass |
| `cargo test --profile uat -p ewe_platform --test templates_build` | Pass |
| `cargo check --profile uat -p foundation_auth --features server-test --all-targets` | Pass |
| `cargo metadata --offline`, then `--locked`, for the workspace and each example | Pass |

## 2026-11-09: issue #426, the WASI context uses the injected clock and randomness

`foundation_wasmtime::WasmtimeBuilder::build` built the Preview 1 (`WASIp1`) context with
`WasiCtxBuilder::new().build_p1()`. The context kept the crate defaults: the host wall clock,
the host monotonic clock and the process RNG. A guest that calls `clock_time_get` or
`random_get` then read the host globals. That breaks spec 70 rule 11, and a test cannot
control the value.

| Change | Detail |
| --- | --- |
| `build` | Takes `&Ctx`. It sets `wall_clock`, `monotonic_clock` and `secure_random` from `ctx.clock()` and `ctx.rng()` before `build_p1()`. The compiler now rejects a build with no bundle. |
| Adapters | Private `InjectedWallClock`, `InjectedMonotonicClock` and `InjectedRng` implement the `wasmtime-wasi` host traits over the bundle. `InjectedRng` is a newtype because `cap-rand` and `foundation_compact` use different `rand_core` versions. |
| Session bundle | `PlatformSession` holds a `Ctx` and gives it to `WasmtimeShell`. `PlatformSession::new` seeds the RNG from `foundation_compact::entropy::fill`, so a host guest gets unpredictable randomness. `PlatformSession::new_test` keeps the deterministic seed. |
| Public API | `foundation_wasmtime` 0.2.0 → 0.3.0 and `foundation_platform` 0.0.4 → 0.0.5. `WasmtimeResponder::new`, `WasmtimeShell::new` and `WasmtimeBuilder::build` take the bundle. `WasmtimeShell` no longer implements `Default`: a default would be a silent host bundle. |
| Tests | `tests/wasi_injected_env_tests.rs` has three tests: the realtime clock, the monotonic clock and the random stream. `foundation_platform` adds a session-boundary test that the session clock reaches the guest. |

The three `foundation_wasmtime` tests fail on the old code. The guest returned the real wall
clock, the real elapsed time and a different random stream. See `learnings.md` for the
`random_get` sampling detail.

### Verification

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_nativeapis` | Pass: 42 tests |
| `cargo test --profile uat -p foundation_nativeapis --test vfs_search --features vfs-search-fff` | Pass: 15 tests |
| `cargo test --profile uat -p foundation_ai` | Pass: 496 tests |
| `cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo clippy --profile uat -p foundation_nativeapis --all-targets --features vfs-search-fff --no-deps -- -D warnings` | Zero warnings |
| `cargo fmt -p foundation_nativeapis -p foundation_ai -- --check` | Pass |
| `cargo metadata --offline --format-version 1` | Pass |

The command with `--all-features` does not run. The crate already fails on master because
`native/watcher/unix.rs` calls the private `foundation_reactor::poll::Registry::selector`
(#428), and several feature-gated modules have `unreadable_literal` warnings (#429). Both
are separate defects and not caused by this change.

| `cargo test --profile uat -p foundation_wasmtime` | Pass, 4 test binaries |
| `cargo test --profile uat -p foundation_wasmtime --test wasi_injected_env_tests` against the old builder | Fail, 0 passed and 3 failed; this is the regression proof |
| `cargo test --profile uat -p foundation_platform` | Pass, 19 test binaries |
| `cargo clippy --profile uat -p foundation_wasmtime --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo clippy --profile uat -p foundation_platform --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo fmt -p foundation_wasmtime -p foundation_platform -- --check` | Pass |
| `cargo metadata --locked --offline` for the workspace, `examples/platform_ios/src-tauri` and `examples/platform_android/src-tauri` | Pass |

## 2026-11-09: issue #427, `foundation_compact` builds for WASI preview 2 and 3

`foundation_compact` selects the `wasi_p2_3` entropy backend for every WASI target that is
not preview 1. That backend imports `wasip2` when `target_env` is `p2`, and `wasip3` when it
is `p3`. The manifest declared neither crate. `cargo check --target wasm32-wasip2` failed in
this crate with `E0432: unresolved import wasip2`, so every crate in the graph stopped there.

| Change | Detail |
| --- | --- |
| `wasip2` | `= "1.0.4"`, `default-features = false`, for `target_env = "p2"`. The version matches the lockfile entry that another crate already pulls. |
| `wasip3` | `= "0.9"`, `default-features = false`, for `target_env = "p3"`. It requires Rust 1.87, which equals the workspace minimum. |
| Example lockfiles | The three standalone lockfiles take `foundation_compact` by path, so they hold the new target dependencies. |

The `random` module of each crate needs no `std` or `bitflags` feature, so the `no_std`
substrate keeps its feature set.

### Verification

| Check | Result |
| --- | --- |
| `cargo check --profile uat -p foundation_compact --target wasm32-wasip2 --no-default-features` | Pass |
| `cargo check --profile uat -p foundation_compact --target wasm32-wasip3 --no-default-features` | Pass |
| `cargo check --profile uat --target wasm32-wasip2 -p foundation_db --no-default-features --features wasm` | Pass; this is the check that failed while #413 was verified |
| `cargo test --profile uat -p foundation_compact` | Pass, all test binaries |
| `cargo clippy --profile uat -p foundation_compact --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo fmt -p foundation_compact -- --check` | Pass |
| `cargo metadata --locked --offline` for the workspace and the three examples | Pass |

## 2026-11-09: #413, `JsonFile` stays on WASI, on `fix/413-jsonfile-wasi`

The branch starts from `fix/medium-integration` at `8811a029f`. The issue's first text named a
`shared/` layout defect. The issue comment of the same date corrects the scope: the file is
`backends/foundation_db/src/storage_provider.rs` at the crate root. The defect is the bare
`#[cfg(not(target_family = "wasm"))]` gate. That gate also removes `JsonFile` on
`wasm32-wasip1` and `wasm32-wasip2`. The WebAssembly System Interface (WASI) gives those
targets filesystem access through preopened directories.

| Change | Detail |
| --- | --- |
| Capability cfg | New `backends/foundation_db/build.rs` sets `json_file_backend` when the target is not wasm, or when `target_os` is `wasi`. |
| 22 markers | The 22 bare gates in `storage_provider.rs` use `#[cfg(json_file_backend)]`. `lib.rs` gates the `native` module and its re-export the same way. |
| Public API | No change on non-WASI targets. `StorageBackend::JsonFile`, `StorageProvider::json_file` and `JsonFileStorage` are now present on WASI targets. |

A `macro_rules!` cannot emit a match arm. A macro call in match-arm position is parsed as a
pattern. A named cfg alias puts the rule in one place, as the issue comment asks. The
`cranelift_backend` cfg in `foundation_buildtools` is the same pattern in this repository.

### Verification

| Check | Result |
| --- | --- |
| `cargo fmt -p foundation_db --check` | Pass |
| `cargo clippy --profile uat -p foundation_db --all-targets --no-deps -- -D warnings` | Pass, zero warnings |
| `cargo clippy --profile uat -p foundation_db --all-targets --no-deps --no-default-features --features wasm -- -D warnings` | Pass, zero warnings |
| `cargo test --profile uat -p foundation_db` | Pass, 210 tests in 45 binaries |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_db --no-default-features --features wasm` | Pass. The build script does not set `json_file_backend`. |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_db --no-default-features --features wasm-bindgen-storage` | Pass. |
| `cargo check --profile uat --target wasm32-wasip1 -p foundation_db --no-default-features --features wasm` | Pass. The build script sets `json_file_backend`. |

The build-script stdout at
`target/<target>/<profile>/build/foundation_db/<hash>/run/stdout` shows the cfg that each
target gets.

No run-time test can prove a target cfg on the host. The build-script output and the cross
checks are the proof. `wasm32-wasip2` is not verified: `foundation_compact` does not compile on
that target. Issue #427 records that defect with `severity: medium`.

## 2026-11-09: issue #423, a cross-thread wake cannot run a non-`Send` callback

The non-`multi` build stores a `WakeCallback` (no `Send` bound) in a
`std::task::Waker`, which is `Send` and `Sync` by type. Safe code could move the
waker to another thread, and then a push on that thread ran the callback there.

The fix confines the callback to its own thread (`confined::ThreadBound` in
`foundation_core/src/valtron/readiness_wake.rs`):

- `ThreadBound::get` returns the value only on the thread that made it. A wake
  on any other thread takes no callback, runs nothing and clears the liveness
  token.
- `ThreadBound` drops the value only on its own thread. A last drop on another
  thread leaks the value. A leak is safe; a cross-thread drop can break the
  value's thread affinity.
- `Send` and `Sync` for `ThreadBound<T>` are sound for every `T`, because no
  other thread reads or drops the value.
- The `multi` build keeps the plain `Mutex`; its callback is already `Send`.

`ReadinessWaker` stays `Send + Sync`, so `ReadinessNotifier` stays `Send` and the
reactor keeps sharing a notifier with its drain thread.

Tests: `tests/valtron/callback_waker_thread_confinement.rs` (non-`multi` only).

| Test | Old code | New code |
| --- | --- | --- |
| `a_cross_thread_wake_does_not_run_a_non_send_callback` | Fails: the callback ran on the other thread | Passes |
| `a_cross_thread_drop_keeps_a_non_send_callback_away_from_that_thread` | Fails: the callback dropped there | Passes |

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_core --test mod` | Pass |
| `cargo clippy -p foundation_core --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo clippy -p foundation_core --features multi --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo fmt -p foundation_core -- --check` | Pass |

The crate version does not change: no public item is added.

## 2026-11-09: `foundation_wasmtime` WASI imports (issue #416), branch `fix/wasmtime-wasi-imports`

The host linker now registers the Preview 1 (`wasm32-wasip1`) imports, so a guest that a
`std` build produces can instantiate. The WASI context is empty: no standard streams, no
environment variables and no arguments. A `no_std` guest imports nothing and still works.

| Item | Change |
| --- | --- |
| Dependency | `wasmtime-wasi = { version = "26", default-features = false, features = ["preview1"] }`. It resolves to 26.0.1 and shares one `wasmtime` package with the crate. |
| State | A private `HostState { wasi: WasiP1Ctx }` replaces the unit state in `Linker` and `Store`. |
| Linker | `preview1::add_to_linker_sync` registers `wasi_snapshot_preview1`. `ewe.log` stays. |
| Guest ABI | Unchanged: `memory`, `ewe_alloc`, `handle_request`, and every public method signature. |
| Test | `tests/wasmtime_wasi_imports_tests.rs`: a WAT module imports `fd_write`, calls it, returns a fixed body, and asserts the response. |

The new test fails on the old code with
`failed to link WASM module: unknown import: wasi_snapshot_preview1::fd_write has not been defined`.

### Verification

| Check | Result |
| --- | --- |
| `cargo fmt -p foundation_wasmtime --check` | Pass |
| `cargo clippy --profile uat -p foundation_wasmtime --all-targets --no-deps -- -D warnings` | Zero warnings |
| `cargo test --profile uat -p foundation_wasmtime` | Pass, 15 tests and one ignored doc test |
| `cargo check --profile uat -p foundation_platform` | Pass |
| `cargo tree -i wasmtime -p foundation_wasmtime` | One package: wasmtime 26.0.1 |

## 2026-11-09: batch 2, `foundation_core` fixes on `spec75/core-medium`

The branch is based on `fix/medium-integration` at `4ca007301`. Seven of the nine listed
issues were already fixed at that commit. The branch fixes the other two and adds the
regression coverage for all nine.

| Issue | State at `4ca007301` | Fix or test |
| --- | --- | --- |
| #291 | Fixed by `a7eeaa390` | `idle_wake` interrupts the park of an idle executor. Test: `bridge_park_executor::fast_producers_never_strand_a_consumer`. The old code timed out at 60 s. |
| #293 | Fixed by `8849434a2` | `ThreadedIterFuture` uses a real waker and parks. Test: `threaded_future::test_threaded_future_parks_while_pending`. It counts the polls. |
| #294 | Fixed by `8849434a2` | `push_waiting` handles `Closed` and `Full`. Test: `threaded_future::threaded_iter_future_error_reaches_consumer_when_queue_full`. |
| #296 | Fixed by `5e14275b4` | `BackgroundJobRegistry::new` and `with_config` return `BackgroundJobError::SpawnFailed`. Test: `background_jobs::background_registry_spawn_failure_is_an_error`. |
| #300 | Fixed by `d9f393a1e` | One acquire counts one outcome. Test: `buffer_pool_tests::bytes_pool_counts_one_outcome_per_acquire`. |
| #303 | Fixed by `69ca21afc` | A zero-length pooled read is not EOF. Test: `stream_ext_tests::read_pooled_buffer_with_zero_max_len_is_not_eof`. |
| #311 | Fixed by `c192c6e2d` | `block_on_future` re-raises the task panic payload. Test: `worker_panic_accounting::block_on_future_reraises_the_task_panic_payload`. |
| #407 | Open | Fixed on this branch. The non-sendable split observers register a non-`Send` callback waker. Test: `non_sendable_split_observer_wake::a_non_sendable_split_observer_accepts_a_wake_callback_and_fires_it_on_push`. |
| #408 | Open | Fixed on this branch. A no-progress requeue goes to the back of the queue. Test: `self_wake_fairness::a_no_progress_task_does_not_starve_a_scheduled_sibling`. |

The two new tests fail when their fix is reverted. The old code returns `NotSupported` for
#407 and re-polls the first task for #408.

### The rpc and netio checks (branch `spec75/rpc-netio-medium`)

This branch starts from commit `4ca007301` and fixes two medium-severity issues.

| Issue | Crate | Fix |
| --- | --- | --- |
| #410 | foundation_connectrpc | A unary call checks the HTTP status first. A non-OK body gives a bounded error message. |
| #420 | foundation_netio | `Connection::connect_unix` takes the read timeout as configuration. |
| #390 | foundation_connectrpc | The dropped pipe `close` results now use `let _ =`. |
| #424 | foundation_connectrpc | A deregistered socket returns an error, not a lost wake. |

#424 is a new low-severity issue. It records the two `FdReadiness::register_waker` results
that `conn_io` ignored. The same branch fixes it.

### Verification

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_core` | Pass, 0 failed |
| `cargo test --profile uat -p foundation_core --features multi` | Pass after `fff848fba`. The first Linux run failed the idle-CPU test (#425) |
| `cargo clippy --profile uat -p foundation_core --all-targets` | Zero warnings |
| `cargo clippy --profile uat -p foundation_core --features multi --all-targets` | Zero warnings |
| wasm32 check, std | Pass |
| wasm32 check, std and js-foundation-wasm | Pass |
| wasm32 check, std and js-wasmbindgen | Pass |
| `cargo test --profile uat -p foundation_connectrpc` | Pass, 34 test binaries |
| `cargo test --profile uat -p foundation_connectrpc --features h3` | Pass, 37 test binaries |
| `cargo test --profile uat -p foundation_netio` | Pass, 6 test binaries |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --all-features --no-deps -- -D warnings` | Zero warnings |
| `cargo clippy --profile uat -p foundation_netio --features quic --all-targets --no-deps -- -D warnings` | Zero warnings |
| wasm32 check, foundation_connectrpc `--no-default-features --features rpc` | Pass |
| wasm32 check, foundation_netio `--no-default-features --features wasm-fetch` | Pass |

### Next

- Close the nine core issues when the branch merges. Seven already have their fix commit.
- Issue #423 records the remaining risk: in the non-`multi` build the callback waker can
  cross a thread, and it breaks the `Waker` contract. It is `severity: medium`.
- Issue #425 records the idle-CPU bound. It is fixed on the branch.
- foundation_wasmtime #416 (WASI imports) is still open.

## 2026-11-09: batch 1 is complete on `fix/medium-integration`

The branch collects the fixes for 29 medium-severity issues. Each fix has its own commits
and its own regression test. The branch is one pull request (PR).

| Issue | Crate | Fix |
| --- | --- | --- |
| #262 | foundation_connectrpc | `is_method_not_allowed` reads the stored HTTP status, not the message text. |
| #263 | foundation_connectrpc | `build_request_descriptor` returns a typed `Internal` error. |
| #264 | foundation_connectrpc | `percent_decode` collects bytes and decodes UTF-8 once at the end. |
| #266 | foundation_connectrpc | The gRPC-Web text writer flushes the pending base64 tail on every exit. |
| #270 | foundation_connectrpc | The H2 Unix dial is non-blocking, and the pump settles the connect. |
| #277 | foundation_netio | TCP connect runs on a non-blocking socket and finishes from readiness. |
| #279 | foundation_netio | Every TLS handshake is a resumable state machine bounded by its timeout. |
| #283 | foundation_http | An idle HTTP/1.1 connection parks on its socket readiness. |
| #305 | foundation_core | `SimulatedClock::advance` wakes parked executors after it releases the clock lock. |
| #309 | foundation_core | Pool workers are named `valtron-worker`, so the idle CPU tests match only their own threads. |
| #310 | foundation_core | A pipe pop makes no wake; the close makes one. The test hook is `executor_wake_count`. |
| #315 | foundation_nostd | The zero-duration wait test allows 1 ms, not a tighter bound. |
| #328 | infrastructure_llama_bindings | A worktree restores `tools/llama.cpp` from the main checkout's submodule clone. |
| #329 | foundation_core | Split observers park on their queue through a one-shot callback waker. |
| #336 | foundation_reactor | Every clippy warning in the crate and its tests is gone. |
| #366 | foundation_nativeapis | The ptrace loop maps every `PTRACE_SYSCALL` failure to a backend error. |
| #371 | foundation_wasmtime | `respond` calls the guest `handle_request` export and returns the guest body. |
| #375 | foundation_netio | The WebSocket connect functions use their `sleep_between` argument. |
| #376 | foundation_http | The protocol-detect refusal write logs its outcome and the peer. |
| #380 | foundation_connectrpc | A streaming writer error reaches the caller as the stream's terminal error. |
| #381 | foundation_connectrpc | A unary request applies `compress_min_bytes` and `send_max_bytes`. |
| #386 | foundation_netio | Every clippy diagnostic in the quic, http3 and webtransport code is gone. |
| #387 | foundation_netio | CI runs the quic feature for tests and clippy, and fails on a new warning. |
| #389 | foundation_connectrpc | A handler cannot override framing headers on h1, h2 and h3. |
| #402 | CI | The pinned-toolchain step skips the install when rustup already lists the channel. |
| #403 | foundation_nativeapis | A failed `ptrace::getevent` returns `VfsError::Backend`. |
| #418 | foundation_netio | The Unix socket connect is non-blocking, and the redirect task accepts all three TLS backends. |
| #419 | foundation_netio | The quic clippy gate is at zero warnings, and `clippy-deny-warnings` is on. |
| #421 | foundation_http | The idle handler registers the socket at each park, so a stale drop cannot cancel it. |

The branch also holds these changes:

- `bac23cf3c`: the rust-clean-code skill states that `shared/` is target-agnostic and that a
  deleting wasm gate means the code belongs in `native/`. The skill submodule commit is
  `91c03b3`, and the pointer bump is part of the same commit.
- `d64d2106f`: the macOS image comment records why the guest stays on macOS 15 (issue #373).
- `2aee52cd0`: the time-source gate comment records why `foundation_boringtun` keeps its
  clock calls (issue #326, closed).
- The manifest of `foundation_http` drops an unused `getrandom` dependency. The `wasm` feature
  enables only `uuid/js`.

### Verification on the merged tree

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_http --features "multi quic"` | Pass |
| `cargo test --profile uat -p foundation_connectrpc` | Pass, including `server_socket_tests::keep_alive_connection_serves_unary_and_streams_in_turn` (3 of 3 runs) |
| `cargo test --profile uat -p foundation_netio` | Pass |
| `cargo test --profile uat -p foundation_reactor` | Pass |
| `cargo test --profile uat -p foundation_nativeapis` | Pass |
| `cargo test --profile uat -p foundation_nostd` | Pass |
| `cargo clippy --profile uat -p foundation_http --all-targets --features "multi quic ssl"` | Zero warnings |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --all-features` | Zero warnings |
| `cargo clippy --profile uat -p foundation_netio --features quic --all-targets --no-deps` | Zero warnings |
| wasm32 check, foundation_core (std, js-foundation-wasm, js-wasmbindgen) | Pass |
| wasm32 check, foundation_http `--no-default-features --features wasm` | Pass, no warnings |
| wasm32 check, foundation_netio `wasm-fetch` | Pass |
| wasm32 check, foundation_connectrpc | Pass |

### Next

- Batch 2 must fix these open medium-severity issues:
  - foundation_core: #291, #293, #294, #296, #300, #303, #311, #407, #408.
  - foundation_connectrpc: #410.
  - foundation_wasmtime: #416.
  - foundation_netio: #420.
- The `shared/` layout moves are separate work. Issues #409, #411, #412 and #413 record the
  places that must move to `native/`.
- #257 needs the reset probe in `foundation_netio`; the connectrpc change records the
  limitation only.
- #373 waits for an upstream `dockurr/macos` fix.
- #290 needs a flake soak of `http3_server_echoes_a_request_over_quic`. The test passed in the
  `multi quic` run of batch 1.
