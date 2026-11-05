# Spec 70 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-05: feature 48 done on `spec70/f47-f50-lane-a-mechanical`. Both
  `#[ignore = "Requires threading support"]` markers are removed from
  `tests/primitives_condvar_tests.rs`. The reason contradicted the `#[cfg(feature = "std")]`
  guard. Both tests pass 20 of 20 with the `std` feature, and the full `foundation_nostd`
  test run with `std` is green. No condvar bug. Backlog: new entry for the 19 ignored doc
  examples in `foundation_nostd`.
- 2026-11-05: feature 47 partial on `spec70/f47-f50-lane-a-mechanical`. Measured cause of
  the seven ignored ptrace tests: (1) ptrace requests came from a thread that did not attach
  to the tracee, so they failed `ESRCH` and the tracee stayed stopped; (2) `argv` lacked the
  program name, so the `/bin/sh -c` tests ran a wrong command. The tracer thread now forks
  and traces the child, the child uses `PTRACE_TRACEME` plus `SIGSTOP`, every `waitpid` uses
  `__WALL | __WNOTHREAD`, the first child stop is consumed, and the spawn tests have a 60 s
  timeout. All seven ignores are removed; `fork_child_first_stop_is_consumed` is added.
  Local run in the Linux CI image: 17 of 25 `ptrace_vfs` tests pass; the 8 spawn tests fail
  with `PTRACE_GETREGS ... EIO` because OrbStack amd64 emulation does not expose amd64
  registers (a C probe fails the same way). Native amd64 GitHub CI must confirm 20 of 20.
  Backlog: new entries for nativeapis clippy debt and the emulation limit.
- 2026-11-05: feature 80 done on `fix/io-41-50-lane-a` (PR #173). CI repairs: (1) the executor
  lost a wake between the readiness check and `register`, so a finished DNS lookup waited for
  the 30 s timeout and the netio and proxy jobs stalled (see learnings); the fix registers
  first and checks again, and a new test fails on the old code (505 ms) and passes now (0 ms)
  on one thread and on a pool. (2) The SSE reconnect replay test reset its first connection
  before macOS sent the buffered event; a 100 ms wait in the handler makes it pass 20 of 20.
  (3) The `lib.rs` comment named the private `idle_wake` module, so the CI guard failed;
  reworded. Checks: `foundation_core --test mod` 785/785 (`multi`) and 745/745 (default) in
  5 runs, the new test 10 runs, the idle-CPU test 10 of 10 green natively, `foundation_netio`
  Linux CI green in both feature sets (with the three tests that failed on the PR) and
  `foundation_proxy` green (with `connect_unary_via_h1_proxy` and
  `grpc_web_unary_via_grpc_backend`); wasm checks green; the executor-wake guard and its unit
  tests green. Backlog: three new entries.
  The GitHub CI image job then failed before any package job ran: master commit 2512754fa
  removed the `tools/whisper.cpp` gitlink, the PR merge drops it, and
  `ci/ewe-platform-image/config.toml` lists it, so `images.nu prepare` stopped with
  "submodule tools/whisper.cpp is not in HEAD". The branch now merges master and re-adds the
  `tools/whisper.cpp` gitlink at 35cb684, so the merge keeps the submodule and the image bakes
  it again. The config keeps its three submodule entries.
  Then the `foundation_core` job failed on one test: `valtron::multi_workers::can_queue_and_complete_task`
  panicked with "Thread pool not initialized". Cause: `PoolGuard::shutdown` cleared the global
  pool slots even for a private guard from `PoolGuard::new`, so the test's own private-guard
  tests (running in parallel in the same binary) wiped the live pool between `initialize_pool`
  and `get_pool`. Each slot now clears only when it holds the guard's own registry. The new test
  `private_pool_guard_drop_keeps_the_global_pool` fails on the old code and passes now. Checks:
  macOS `foundation_core` green (787 `multi`, 745 default); Linux CI image green in both sets
  except the emulated-amd64 CPU-time test that also fails on the old code there. This also
  explains the open `.expect("execute")` flake, so its backlog entry is deleted.
  Still open: 57 `ignore` doc examples in `foundation_core` never compile or run; a conversion
  to executing doctests is in progress.
  Doctests converted (same day): all 57 now execute and assert real behaviour. Checks: default
  set 70 doctests, `multi` set 73, both "0 ignored"; full suites green (745 + 70 default,
  786 + 73 `multi` on macOS); `cargo fmt --check` clean; clippy errors 0 with no warning on
  an edited file. The backlog entry for the ignored examples is deleted.
  GitHub run 41953248793 (commit 7126b1616): **green**, every job passed, including
  `foundation_core` in both feature sets on native amd64. Feature 80 is done and marked
  `status: "done"`. Next: feature 81 (lane B), branched from this branch because it needs
  feature 80's API and PR #173 is not merged yet.
- 2026-11-04: feature 46 done on `wip/f46-driven-stream-wake` (from `fix/io-41-50-lane-a`). A
  single-thread `DrivenStreamIterator` now wakes its async consumer when a parked task becomes
  ready: through a new `WakeableStream::wake_send` (Send callback), a one-shot resume listener in
  `idle_wake` (fired by `request_executor_resume`), and a timed wake on the shared timer thread
  (`timer::call_at`) at the nearest sleeper deadline or the readiness poll quantum. Six new tests in
  `tests/valtron/driven_stream_async_wake.rs` (smol and tokio; signal, deadline-only and
  needs-polling parks) timed out on the old code and pass now. `multi` is unchanged.
- 2026-11-03: feature 75 done (and feature 72 done through it) on `fix/io-41-50-lane-a`.
  Measured cause: the executor re-checked readiness-parked tasks every 1 ms (idle cost) and the
  idle back-off `SpinWait` sleep in `block_on` could outlast a wake (the feature 72 upload stall).
  Added `EventReadiness::deadline`/`needs_polling`, `ResumeSignal`, deadline plumbing in six
  crates, a kill check after sleep and per-step `idle_wake::checking()`. Idle QUIC listener: 92 ms
  to under 20 ms CPU per second. Feature 72's three H3 tests pass 20 of 20. CI: foundation_core
  1655 tests, foundation_netio 2720 tests, foundation_proxy `--features quic` 137 tests,
  plus reactor, wireguard, nativeapis and http tests, all green. New backlog entries for the
  signals that still poll.

- 2026-11-03: feature 72 partial on `fix/io-41-50-lane-a`. E2 done: the io_uring tests of
  `foundation_iogate` and `foundation_proxy` fail when `EWE_REQUIRE_IO_URING=1` and io_uring is
  missing; the CI variable is not set (Docker/seccomp support unconfirmed, recorded in the backlog).
  E1 not merged: the streamed H3 request body works (peak 1420 bytes, 413 over the limit, 0 idle
  polls) but exposes a QUIC driver bug: a parked driver does not wake for its QUIC timers, so a
  16 MiB upload stalls about one run in four. Work is on branch `wip/f72-h3-request-body`; the
  fix (an `EventReadiness` deadline hint) is a new backlog entry.

- 2026-11-03: feature 70 done on `fix/io-41-50-lane-a`. E1 deletes the polling
  `splice_bidirectional` (the io_uring splice test uses `aio::splice`). E2 makes the io_uring
  upstream dial non-blocking (`dial_tcp` parks on write readiness, `iogate::adopt_connected`
  registers the socket) and adds `connect_timeout`. E3 streams the H3 response body with the
  H3 flow control; the root cause was `LimitedBatchStreamReader`/`LimitedEOFStreamReader` in
  `foundation_core` handing out pieces as big as the remaining cap. Tests:
  `completion_dial_does_not_hold_worker`, `h3_forward_streams_large_body`, two reader tests,
  `adopt_connected_registers_an_already_connected_upstream`. Three new backlog entries.

- 2026-11-02: feature 69 (unsolicited 1xx interim responses skipped with `Expect` off) done on
  `fix/io-41-50-lane-a`. T0: open (no client bug; the skip logic in `request_redirect.rs` was
  already correct). Six new tests, all passed on the first run, so no client change was made:
  five in `foundation_netio` (`simple_http/unsolicited_interim_tests.rs`: 100 before 200 over TCP
  and Unix socket, 103 with `Link` skipped, 100 then 102 then 200, 100 on GET) and one in
  `foundation_deployment_docker` (`tests/expect_continue_tests.rs`:
  `docker_client_skips_unsolicited_100`). Each test has a hard time limit (`valtron_test`
  timeout, server read limit 10 s). Flake check: 20 of 20 for each of the six. Correction to the
  feature verify commands: the netio test binary is `foundation_netio_tests` (feature `multi`),
  not `simple_http`. `python3 scripts/ci/ci.py run foundation_netio --profile dev` exit 0 (the
  run had two clippy warnings in the new test file; fixed, and the six tests pass again). Two
  older clippy warnings in other files are logged in `docs/bug-backlog.md` section 2. Limitation:
  the tests prove the skip path; they were not shown to fail on the old code because no old code
  path was changed.

- 2026-11-02: feature 65 (the rest of the proxy's threads and polling loops) done on
  `fix/io-41-50-lane-a`. T0: open. E1: no exchange starts a thread. The upgrade splice, the raw
  TCP tunnel and the gRPC-Web and Connect translators run in the `Async` future (new `aio.rs`:
  `AsyncFd`, `splice`, `dial_tcp`); errors before the first byte answer through the async writer.
  E2: `TcpPassthrough` and `UdpPassthrough` are futures parked on socket readiness
  (`PassthroughConfig`). E3: the H2 plain forward is async (no feeder thread, no sleeps); the H3
  waits are timer parks; the remaining thread and sleep sites are justified in comments with config
  intervals (`HealthCheckConfig::shutdown_check_millis`, `ProxyConfig::drain_poll`,
  `redirect_accept_poll`). E4: the control socket is futures (`ControlConfig`). E5: a slow client
  upload used to fail with "body truncated" (502); the body now streams through a pipe, capped by
  `ProxyConfig::max_request_body` (default 1 MiB, 0 = none; 413). E6: `load` takes `LOCK_SH`. Each
  new test failed on the old code (thread counts 13 to 63, 11 to 61, 12 to 62; control socket
  unanswered; upload 502; torn read). E1 and E2 share one commit.

- 2026-11-02: feature 45 (proxy async relay, state lock, unused feature dependency) done on
  `fix/io-41-50-lane-a`. T0: open (all three). E1: `ProxyHandler::serve` returns
  `ConnectionResult::Async` for an ordinary HTTP exchange. The future owns the connection (in a
  `ResponseWriter`), request, backend lease and cloned state, awaits `send_async` upstream, reads the
  upstream body as an `AsyncSendSafeBody` with the upstream socket's read-wake source and writes each
  piece with the feature 61 writer (a `Content-Length` body is kept; any other is re-framed chunk by
  chunk). A failure before the head sends 502 through the writer; after it, only a close. The
  handler's early 404/503 answers use `send_text`/`send_not_found`. Upgrade, raw TCP and the gRPC-Web
  and Connect translators stay on a thread (blocking boundary, backlog §9). New
  `ProxyConfig::relay_send_timeout` (default 60 s, nginx `send_timeout`). Test
  `proxy_relay_does_not_spawn_threads` failed on the old code (12 to 62 threads with 50 open
  relays); `proxy_relay_holds_no_worker` passes before and after (the thread relay did not hold a
  worker either); three streaming tests added. E2: `ProxyStateStore` takes a `flock` on a lock file
  in the state directory for each load-change-save (the state store has no conditional write); test
  `two_stores_on_one_backend_do_not_lose_updates` failed before, passes after. E3: the Cloudflare
  DNS-01 path does not exist (the ACME flow takes a caller-made setter), so the unused dependency was
  removed; the feature name stays (`cloudflare = []`). Backlog §9: two entries deleted, six added;
  §11: one added.

- 2026-11-02: feature 64 (keychain streamed request body) done on `fix/io-41-50-lane-a`. T0:
  open. `ServeAdapter::serve` no longer reads the body; the async future reads it with
  `AsyncSendSafeBody` plus the connection's `read_wake_source`, through `collect_bytes_bounded`
  (cap + 1 bytes, to tell "at the cap" from "over"). New `KeychainServerConfig`
  (`max_request_body`, default 1 MiB, nginx `client_max_body_size`; builders on the config and
  `KeychainServer::with_max_request_body`), stored in the `ContextBag`. Over the cap: 413 with
  `Connection: close`, rest unread, connection closed. Test `slow_request_body_does_not_hold_worker`
  failed on the feature 44 code: `try_collect_bytes` hit `WouldBlock` on the non-blocking socket
  and silently returned a truncated body ("slow body was truncated, prelogin answered:
  kdfIterations 600000" instead of 123456), so the old code was worse than a held worker. Now
  passes; `request_body_over_cap_is_413` passes. 20/20 runs each. New backlog entry (§11): the
  Turso store fails overlapping calls with "concurrent use forbidden" (now reachable through
  concurrent async handlers); the slow test staggers its two uploads. §22 sync-body entry deleted.

- 2026-11-02: feature 44 (keychain async serve) done on `fix/io-41-50-lane-a`. T0: open.
  `ServeAdapter::serve` now returns `ConnectionResult::Async`. The future owns the method,
  path, body, bearer, master key, a clone of `KeychainContext` (all `Arc` fields) and the
  connection; it runs `provisioning_route` / `route` with `.await` and writes the JSON with
  `render_head` + `write_response` (the feature 61 non-blocking writer). `poll_ready`,
  `noop_waker`, `status_code` and the sync `write_json` are deleted. The recursion lint is
  fixed with `#![recursion_limit = "256"]` in `src/bin/keychain.rs` (same cause and fix as
  `foundation_db`); the 12 keychain test files that warned for the same lint got the same
  attribute. New test `tests/async_serve.rs::keychain_handler_that_awaits_does_not_panic`
  (a store that yields `Pending` once with a real wake): before, panic at
  `native.rs:272` "did not complete in a single poll"; after, 200. Two new backlog entries
  (sync `read_body`, keychain clippy debt); both §22 entries deleted.

- 2026-11-02: feature 43 (Docker `100 Continue` stall) done on `fix/io-41-50-lane-a`. T0:
  open. Root cause: `HttpClientBuilder::read_timeout` in
  `foundation_netio/src/network_client.rs` (the line `set_expect_continue_read_timeout(timeout)`,
  old line 183) also set the `Expect: 100-continue` wait. `DockerClient::connect_unix` builds its
  client with `read_timeout(120 s)`, so the wait became 120 s; the state machine
  (`park_for_continue`, `request_redirect.rs` ~1188) was correct and always parks on the
  send-body deadline. Fix: `read_timeout` no longer touches the handshake wait
  (`expect_continue_timeout` is the only setter); `ClientConfig::blocking_body_read_timeout`
  now returns the larger of the handshake wait and `min_read_timeout`, so sync body reads keep
  their long timeout. The Docker client (unix, tcp, tls, ssh) also calls `expect_continue(false)`.
  New tests: netio `expect_continue_timeout_tests` (4: timer over TCP and Unix, and
  `read_timeout` does not lengthen the wait over TCP and Unix); docker `expect_continue_tests`
  (2: no `Expect` header, POST does not wait). The two `read_timeout_...` netio tests failed
  before the fix (body arrived 10.0 s after the head) and pass now; the timer-only tests pass
  on the old code, which proves the state machine is correct. 20/20 runs of each. No Docker
  daemon was available here, so no real-daemon timing.

- 2026-11-02: feature 42 (netio dead wait loops) done on `fix/io-41-50-lane-a`. T0: no
  real caller of `collect_exchange`, `drain_exchange_head` or `QuicListener` anywhere in the
  repo (only definitions, the `pub mod` line, a deprecation note and a comment). Deleted
  `collect_exchange` and `drain_exchange_head` from `shared/client/body_reader.rs` (about 80
  lines, including the `yield_now` loops and the now-unused `BytesMut` and `SimpleResponse`
  imports), and `native/quic_listener.rs` (101 lines, `QuicListener`, `QuicBind`, its 1 ms
  sleep and 30 s constant) with its `pub mod` line. Deprecation note in
  `foundation_deployment/src/providers/common/api_types.rs` now names
  `split_exchange`/`send_and_split`. `split_exchange` and `send_and_split` are kept. No
  `yield_now` remains in netio `src`. Verified: `ci.py run foundation_netio --profile dev`
  EXIT=0 (lib, unit, integration and doc suites ok, 13 test result blocks, no unused-import
  warnings); `ci.py run foundation_deployment --profile dev` EXIT=0 (5 test result blocks ok).
  The `collect_exchange_parks_while_waiting` test is not needed on the delete branch.

- 2026-11-02: feature 61 (shared response writer, idle send timeout) done on
  `fix/io-41-50-lane-a`. New `native::writer`: `ResponseWriter` (offset-tracking write and
  flush that parks on write readiness), `WriterConfig`, and `send_bytes`/`send_text`/
  `send_json`/`send_html`/`send_not_found`/`send_server_error`, which return
  `ConnectionResult::Async`. `StaticFileHandler`, `StaticAssetHandler` (body written from its
  shared slice, no copy) and the native `HealthHandler` use it. `send_timeout` (was
  `write_timeout`, kept as a deprecated builder) now means nginx `send_timeout`: the timer
  restarts after every write that takes at least one byte. The reactor watch registers on
  the first `WouldBlock` and `FdReadiness` deregisters in its `Drop`. The synchronous
  `respond::*` keep their `impl Write` signature (other crates and wasm call them); the
  backlog entry (section 3) stays, narrowed to those callers and the small closing writes.
  New tests: `respond_large_body_is_byte_exact`, `static_asset_large_body_is_byte_exact`,
  `send_timeout_is_idle_not_total`, `send_timeout_closes_stalled_client`.

- 2026-11-02: feature 41 (static file handler streams correctly) done on
  `fix/io-41-50-lane-a`. T0: open on master. `StaticFileHandler` now returns
  `ConnectionResult::Async`. It reads the file in `StaticFileConfig::chunk_size` chunks
  (default 64 KiB), writes each with an offset that tracks what the socket took (the old
  `write_all(&wire)` retry restarted at byte 0 and sent duplicate bytes), and on `WouldBlock`
  parks on write readiness (shared reactor, or the overlay wake callback). It never sleeps a
  thread. A reset, aborted or broken peer ends the response at once; a `write_timeout` (default
  60 s, nginx `send_timeout`) closes a stalled client. `MAX_RETRIES` and every `thread::sleep`
  are gone; the head carries `Content-Length`, and `HEAD` sends no body. New tests in
  `tests/static_file_tests.rs` (all fail on master). New backlog entry: `StaticAssetHandler`
  and `respond::*` have the same partial-write bug (section 3).
- 2026-11-02: feature 66 (auth stubs and session lookup) done on `fix/io-51-60-lane-b`. E1:
  `generate_tos_code` and `verify_tos_code` are deleted. Evidence: no code calls them; the login
  handler never issues a `tos_await_code` (that is only in the spec 48 sketch); `tos_accept`
  records acceptance of the latest version without a code. E2: `IdpConfig::dev_mode` (default
  `false`, `with_dev_mode`); `POST /auth/v1/dev/register` is registered only when it is on
  (with a start-up warning) and the handler returns 404 when it is off. E3: sessions are stored
  under `session:{base64url(SHA-256(token))}`; `get_session` does one `get` (and still verifies
  the signed token, so a key that leaves the ring ends its sessions). The stub cache is
  deleted. Revoke-all uses one index key per session, `user_session:{user_id}:{session_id}`,
  and also revokes old-format keys. Existing sessions (old key `session:{user}:{prefix}`) are
  logged out: they cannot be found by hash; users sign in again. The three section 12 entries
  are deleted.

- 2026-11-02: feature 53 (auth leftovers) done on `fix/io-51-60-lane-b`. E1 deletes the dead
  `foundation_db/src/wasm/session.rs`. E2 renames `NativeOAuth::{exchange_code, client_credentials,
  refresh_token}` (no `_async`); no other crate called the old names. E3 adds `KeyRing` and
  `SigningKey` to `TokenSigner`: tokens are `token.kid.tag`, a ring has one current key and
  previous verify-only keys, and old `token.tag` tokens verify with the current key while
  `allow_legacy_tokens` is on (default). E4 brings `foundation_auth` to 0 clippy warnings
  under `server-native` and `server-test` (was 212 in `src/server` plus test-tree lints),
  moves the inline tests of `two_factor.rs` and `auth_session.rs` to `tests/units/shared/`,
  and adds `clippy-runs` to `ci/packages.toml` / `scripts/ci/ci.py` so CI lints `server-test`.
  The §12 clippy entry is deleted; three newly found bugs are recorded in §12.

- 2026-11-02: features 41–60 added by the main session after PR #172 merged: the PR review
  findings, the open backlog entries and the stale ones (requirements.md section 8). Two lanes
  (A: I/O and runtime, `fix/io-41-50-lane-a`; B: data, auth and hygiene,
  `fix/io-51-60-lane-b`); Sonnet 5.5 for diagnosis and design work, Haiku 5.5 for mechanical
  work. Frontmatter and features table now list all 60.

- 2026-11-02: feature 39 (rustfmt version pin) done on
  `fix/io-35-turso-async-trait-overflow`. `rust-toolchain.toml` names the pinned channel
  `nightly-2026-09-20`. That channel gives rustfmt `1.10.0-nightly
  (feaadeeaca 2026-09-19)`, the same rustfmt the CI image uses. The file also
  adds `clippy` to the components. One repo-wide `cargo fmt --all` reformats
  1334 hand-written Rust files (7292 diff hunks); the standalone
  `crates/channels` workspace is formatted too. The 43 generated
  `foundation_cloudflare_*/src/lib.rs` shims are excluded with a `rustfmt.toml`
  `ignore` list (their build script regenerates them on any difference).
  `cargo fmt --all -- --check` now prints nothing on the host. The §16
  backlog entry is removed; a §8 entry records the generator emitting
  non-fmt-clean shims.

- 2026-11-02: feature 22 (workspace, process and CI) done on
  `fix/io-35-turso-async-trait-overflow`. E1–E8 landed. E1 gives `scripts/ci/ci.py` a
  per-worktree target directory. E2 deletes the unused `crates/ewe_deployables`.
  E3 splits the generated Cloudflare groups into 43 per-group crates
  (`foundation_cloudflare_<group>`); the umbrella re-exports them behind
  `cloudflare_<group>` features, so old callers do not change. The umbrella
  bumps to 0.2.0, `foundation_keychain` to 0.1.4 and `foundation_proxy` to 0.1.6.
  E4 adds a synthetic `wasm-tests` CI package that checks every crate with wasm
  tests and runs the two Deno wasm suites. E5–E8 clear the wasm-only warnings
  and the mise over-install. The §16 and §21 backlog entries are removed; new
  entries record the remaining lints.

- 2026-11-02: feature 23 (foundation_vectors) done on
  `fix/io-35-turso-async-trait-overflow`. E1–E8 landed. E1 offloads `FjallVectorStore`
  blocking work to a valtron pool; E2 builds the vendored sqlite-vec extension;
  E3 migrates old-shape `vec0` tables on open; E4 adds an opt-in live
  TurboPuffer test; E5 retries HTTP 429/503 with backoff; E6 builds the
  `sql-store` feature on wasm32; E7 passes texts to the reranker; E8 documents
  the O(n·d) scan cost. `foundation_vectors` bumps to 0.0.4 and `foundation_ai`'s
  requirement follows. The turbopuffer wasm `Send` failure is fixed with
  `SendWrapper`. Linux CI is green (199 tests plus the three wasm runs).

- 2026-11-02: feature 19 (foundation_nativeapis) E4 done on
  `fix/io-35-turso-async-trait-overflow`. E4 rebuilds `TursoDelta` on the `turso` crate
  0.8.3 (local engine, `default-features = false`). The store is async-only:
  `AsyncVfsFileSystem` plus `AsyncDeltaStore`, with `TursoFile`,
  `SeekableTursoFile` and `TursoDirectory`. It lives under
  `src/native/vfs/turso_delta/` (mod, schema, types, path_resolve, chunking,
  file_handle). The `vfs-turso` feature enables it, the `turso` dependency is
  target-gated to `not(target_arch = "wasm32")`, and the shim gains a `turso`
  backend (`FOUNDATION_VFS_DELTA=turso`, default path `vfs-turso.db`, memory
  fallback). The delta contract suite runs in `tests/turso_delta_vfs/`: 8
  tests, all pass under `--profile uat`. A `#![recursion_limit = "256"]` is
  added to the crate root and the test file (the local turso engine's async
  chain overflows the default limit while proving `Send`, the same root cause
  as feature 35). The crate bumps 0.0.5 to 0.0.6 and the five consumers
  (iogate, toolings, ai, proxy, wireguard) pin 0.0.6. The §13 backlog entry
  is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E10 done on
  `fix/io-35-turso-async-trait-overflow`. E10 gates the VFS sync boundary behind
  `foundation_core::blocking_boundary!`. `exec_async` is wrapped in the
  macro, and the whole `sync_bridge` module (`SyncFile`, `SyncDirectory`,
  `LocalSeekableFile`, `SyncFs`) is compiled only where valtron can block.
  The `exec_async` and sync-bridge re-exports in `shared/vfs/mod.rs`, the
  crate-root `SyncFs` re-export in `lib.rs`, and `DynFs::from_async` in
  `dynfs.rs` are gated the same way. Verified with `cargo check --target
  wasm32-unknown-unknown --no-default-features --features vfs` (plain wasm)
  and with a scratch crate that enables `foundation_core/js-wasmbindgen`
  (JS-hosted wasm): the build passes and `SyncFs`/`exec_async` are absent.
  All VFS tests pass under `--profile uat`. The `SyncFs` §13 backlog entry
  is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E9 done on
  `fix/io-35-turso-async-trait-overflow`. E9 makes the VFS `copy` and `copy_async`
  default methods stream in chunks. The four traits (`VfsFileSystem`,
  `VfsDirectory`, `AsyncVfsFileSystem`, `AsyncVfsDirectory`) gain a
  `copy_chunk_size` method with a documented 32 KiB default (Go `net/http`
  `io.Copy`); an implementation overrides it to read its own config. Each
  copy loops with `read_at`/`read_at_async`, advances by the returned byte
  count, and stops on a short read, so no whole-file `Vec` is built.
  `read_file` and `read_file_async` stay whole-file by contract and are now
  documented as such; `copy` no longer calls them. The `InodeFs::copy`
  override is removed so it uses the chunked default. New tests
  `vfs_copy_streams_in_chunks` (sync) and `vfs_copy_async_streams_in_chunks`
  (async) wrap `MemoryFs` in a counting VFS, set the chunk to 256 bytes, and
  assert a 1000-byte copy issues several reads with none over 256 bytes.
  The crate bumps 0.0.4 to 0.0.5, and the five consumers pin 0.0.5. The
  whole-file-reads §13 backlog entry is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E8 done on
  `fix/io-35-turso-async-trait-overflow`. E8 makes `VfsNfs::readdir` honor the NFS v3
  `start_after` cookie: it resolves the fileid of every entry in list order
  first, then skips up to the entry whose fileid equals the cookie and takes
  the next `max_entries`. A `to_nfstime3` helper maps `VfsMetadata` times to
  `nfstime3`, saturating the 64-bit seconds into the NFS 32-bit field with
  `u32::try_from(...).unwrap_or(u32::MAX)`; `to_fattr3` now sets `atime` from
  `accessed` and `mtime`/`ctime` from `modified`. New tests
  `nfs_readdir_pages_without_repeats` (a 1000-entry directory, page 100,
  every name seen exactly once) and `nfs_attrs_have_mtime` land. All 13 NFS
  tests pass under `--profile uat`. The NFS `readdir` §13 backlog entry is
  removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E7 done on
  `fix/io-35-turso-async-trait-overflow`. E7 renames the constructors to
  `InodeFs::new()` and `InodeDelta::new()` (infallible, no path or config).
  It drops `InodeFsConfig` and the `FjallFs` / `FjallDelta` /
  `FjallVfsConfig` aliases and their re-exports. It adds the async face like
  `MemoryFs`: `AsyncVfsFile` for `InodeFile` and `SeekableInodeFile`,
  `AsyncSeekableVfsFile`, `AsyncVfsDirectory` for `InodeDir` and the
  `AsyncInodeDirectory` wrapper, `AsyncVfsFileSystem` for `InodeFs`, and
  `AsyncVfsFileSystem` plus `AsyncDeltaStore` for `InodeDelta`. The
  never-read `InodeEntry.name` field and the dead `InodeDir::new` are
  removed, so the module compiles with zero warnings. `InodeDelta`'s sync
  `capabilities` now calls `VfsFileSystem::capabilities(&self.inner)`;
  `capabilities` is the only method name shared by the two traits. The 29
  sync tests move from `FjallFs` / `FjallDelta` to `InodeFs` / `InodeDelta`,
  and 6 async contract tests land via `#[valtron_test]`. 35 fjall VFS tests
  and 20 fjall document-store tests pass under `--profile uat`. The §13
  backlog entry is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E6 done on
  `fix/io-35-turso-async-trait-overflow`. E6 switches `FjallDocumentStore` from
  `V: VfsFileSystem` to `V: AsyncVfsFileSystem` and awaits every VFS call, so
  an I/O-backed store (wrapped in `SyncFs`) no longer blocks the calling
  thread inside the async `DocumentStore` methods. To keep the async futures
  `Send`, no `std::sync` lock guard is held across an await: `put` mutates the
  batch and `flush_locked` advances the high water mark under the lock before
  the guard is dropped, and `read_document_at` scopes its read guard to a
  block before the file read. `collect_rows` collects the fjall index entries
  first and then awaits the document reads, because the fjall range iterator
  is not `Send`. The tests move from `InodeFs` to `MemoryFs`; the read-open
  counting wrapper now implements `AsyncVfsFileSystem` and counts
  `open_async(Read)`. 20 fjall document-store tests pass under `--profile
  uat`. The fourth §13 backlog entry (sync VFS traits) is removed.

- 2026-11-02: feature 25 (foundation_deployment_huggingface) E3 done on
  `fix/io-35-turso-async-trait-overflow`. E3 makes `list_models`, `list_datasets`,
  `list_spaces` and `repo_list_tree` follow the Hub's `Link: rel="next"`
  pagination. The first page is fetched eagerly; later pages are fetched
  lazily by a `LazyPageIterator` when the previous page's items are pulled
  past. `send_json_async_with_headers` returns the response headers and
  `next_link` extracts the next URL. Config `max_list_pages` (default
  10 000) guards against a loop. New tests `hub_list_follows_link_pages`
  (three pages, page 2 fetched only after page 1's items are pulled) and
  `hub_list_stops_without_next_link` land. The third §20 backlog entry is
  removed, closing the section.

- 2026-11-02: feature 25 (foundation_deployment_huggingface) E2 done on
  `fix/io-35-turso-async-trait-overflow`. E2 replaces the buffered commit body with a
  streamed multipart body. `repo_create_commit` no longer calls
  `std::fs::read`; it builds a `SendSafeBody::Stream` via
  `repository::commit_multipart`, opening each added file and reading it in
  `upload_chunk_size` chunks (default 32 KiB, cited against Go `net/http`
  `io.Copy`). The total body length is computed up front from the part headers
  and `file.metadata().len()`. New tests `hub_commit_streams_multipart` (a
  64 KiB binary file round-trips byte-exact) and
  `commit_multipart_streams_file_in_chunks` (a 785-byte file is read in three
  full 256-byte chunks) land. The second §20 backlog entry (buffered uploads)
  is removed.

- 2026-11-02: feature 25 (foundation_deployment_huggingface) E1 done on
  `fix/io-35-turso-async-trait-overflow`. E1 turns the Hub client and repository API into
  `async fn`s over `send_async`/`send_raw_async`; the sync `send`/`send_raw`/
  `send_json` helpers and `LimitedWriter` are gone (no sync twins).
  `repo_download_file` streams the body chunk by chunk to a `.part` file and
  renames it into place, never `collect_bytes_async` for files. The crate bumps
  0.1.2 -> 0.1.3, and `foundation_ai`/`foundation_testing` require 0.1.3; their
  sync callers wrap the async Hub calls with `block_on_future` at the boundary
  (the `ModelProvider` trait stays sync). `foundation_testing`'s inline tests
  move to `tests/huggingface_tests.rs` and `TestHarness` gains a public
  `artefacts_dir` accessor. New tests `hub_whoami_awaits_without_blocking` (a
  300 ms delay plus a zipped ticker) and `hub_download_streams_to_file` land,
  and the older tests convert to `#[valtron_test] async fn`. The first §20
  backlog entry (sync Hub API) is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E5 done on
  `fix/io-35-turso-async-trait-overflow`. E5 makes `scan`, `scan_all` and `scan_from`
  lazy: no row `Vec` is collected and `scan_all` no longer buffers the whole
  collection. A producer thread walks the fjall index range (which is not
  `Send`) and pushes decoded offsets through a bounded valtron `Pipe`; the
  stream reads one document per yielded item and the producer parks on a full
  pipe, so an undrained `scan_all` stays bounded. Test
  `fjall_scan_yields_before_reading_all` wraps `InodeFs` in a read-open
  counter and asserts the stream reads zero files when created and one file
  after its first item (not 50). 20 fjall document-store tests pass under
  `--profile uat`. The first §13 backlog entry (scans buffer their rows) is
  removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E3 done on
  `fix/io-35-turso-async-trait-overflow`. E3 makes the shim build the real D1/R2 deltas
  instead of falling back to the memory delta. `DeltaBackend::D1` builds
  `D1Delta` and `DeltaBackend::R2` builds `R2Delta`; each panics when the
  feature is missing or init fails (no memory fallback). Both branches
  initialize a valtron pool so the async HTTP store can be driven from the
  sync libc handlers, and wrap the delta as
  `DynFs::from_async(OverlayFileSystem::new(base, delta))`. A
  `LOCAL_CF_API_BASE` override makes the shim reach a local Cloudflare
  emulator (test credentials are defaulted in that mode). Test
  `shim_config_d1_uses_d1_delta` writes a file through the `LD_PRELOAD` shim
  with `FOUNDATION_VFS_DELTA=d1` and reads it back from the same D1 emulator.
  The third §13 backlog entry (shim never uses D1/R2) is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E2 done on
  `fix/io-35-turso-async-trait-overflow`. E2 replaces the whole-file blob rewrite with
  chunked storage: a file's content lives in `vfs/data/{path}/{chunk}` and
  `replace_chunks` writes only the chunks whose bytes change. The chunk size
  is `BlobDeltaConfig::chunk_size` (default 64 KiB, documented against Go's
  `net/http` 32 KiB copy buffer). The unused `d1_delta::chunking`,
  `d1_delta::types` and `r2_delta::types` modules are removed (dead code from
  the abandoned inode design); their behaviour is covered by the new
  `blob_delta_small_writes_touch_one_chunk` test, which counts store bytes and
  asserts under 16 MiB for a 1 MiB file written in 4 KiB pieces. 15 blob-delta
  tests and the 9-test D1/R2 API suites pass under `--profile uat`. The second
  §13 backlog entry (whole-file rewrite) is removed.

- 2026-11-02: feature 19 (foundation_nativeapis) E1 done on
  `fix/io-35-turso-async-trait-overflow`. E1 adds `BlobStore::list_blobs_after` to
  `D1Store` (delegates to `list_keys` and enforces the literal prefix, because
  SQL `LIKE` widens `%`/`_`). `BlobDelta` now lists a directory from meta keys,
  lists whiteouts from whiteout keys, and resets by deleting every `vfs/` key.
  Two tests land in `tests/blob_delta_vfs/`: `blob_delta_lists_files_and_whiteouts`
  and `blob_delta_reset_clears_everything` (14 tests green under `--profile uat`).
  The `vfs-d1`/`vfs-r2` features gain `dep:futures-lite` so `BlobDelta` can
  drain the listing stream. The first §13 backlog entry (blob deltas cannot
  list) is removed.

- 2026-11-02: bug-backlog §15 (foundation_cedar) clippy cleanup on
  `fix/io-35-turso-async-trait-overflow`. Cleared the 24 pre-existing `pedantic` warnings
  and deleted both §15 backlog entries (the warnings list and the stale
  `recursion_depth_exceeding_limit` note, which no longer reproduces). Nine
  `missing_errors_doc` sites gain `# Errors` sections, the redundant `#[must_use]`
  attributes on `principal_str`/`action_str`/`resource_str` are dropped (one
  added to `is_authorized_with_entities`), two
  `redundant_closure_for_method_calls` sites use
  `std::string::ToString::to_string`, two `missing_fields_in_debug` Debug impls
  are completed, one `struct_field_names` site renames `*_path` fields, three
  `needless_raw_string_hashes` sites drop the extra `#`, and three
  `assert_is_empty` sites use the typed-empty-array form `assert_ne!(x, [] as [T; 0])`
  (a plain `vec![]` fails E0282, and clippy's own `cedar_policy::api::id::PolicyId`
  path is private, so the public `cedar_policy::PolicyId` re-export is used).
  Verified with `cargo clippy --profile uat --all-targets [--features db] --no-deps`
  (zero code warnings, only `fullfp16` noise) and `cargo test --profile uat
  [--features db]` (17 default / 25 `db` tests green).

- 2026-11-02: bug-backlog §7 (foundation_deployment_platform) `vms` clippy
  follow-up on `fix/io-35-turso-async-trait-overflow`. The first pass (commit `ab014f58a`)
  left 28 warnings: the check grep missed colorized clippy output, so it saw
  none. Commit `614de3710` clears all 28. Seven `cast_precision_loss` allows
  sat on `println!`/`eprintln!` macro calls; rustc ignores an allow on a macro
  call, because the cast sits inside the macro's generated code. Each cast now
  moves to a `let` binding that carries the allow. Two `redundant_closure`
  sites use `map_while(std::result::Result::ok)` to dodge the
  `crate::config::Result` alias. Nine `doc_markdown` sites gain backticks.
  `cargo clippy --profile uat --all-targets --features vms --no-deps` and the
  default-feature clippy both report zero code warnings (only the workspace
  `fullfp16` note); 125 non-Docker tests pass.

- 2026-11-02: bug-backlog §7 (foundation_deployment_platform) `vms` clippy
  warnings zeroed on `fix/io-35-turso-async-trait-overflow`. After the first-pass
  `cargo clippy --fix` (commit `abbe0c039`, 625 to 307), three subagents fixed
  the remaining 285 code warnings in disjoint file groups (edit-only), and the
  main session fixed the 3 manifest `unused_crate_dependencies` warnings:
  `clap` moved from the `vms` feature to `cli`, `portpicker` removed,
  `foundation_sshkit` moved to dev-dependencies. `cargo clippy --profile uat
  --all-targets --features vms --no-deps` and the default-feature clippy both
  report zero warnings; 137 non-Docker tests pass. The §7 entry is reduced to
  one remaining item: extend the CI clippy run to the `vms` feature.

- 2026-11-02: bug-backlog §7 (foundation_deployment_platform) cleaned on
  `fix/io-35-turso-async-trait-overflow`. The three stale entries are removed: the
  `docker/wireguard.rs` inline tests already moved to `tests/wireguard_tests.rs`,
  the deprecated `SimpleHttpClient` is gone (`wait_for.rs` now uses
  `HttpClientBuilder`), and `PortMapping` is now used. The lib and every
  default-feature test file reach zero clippy warnings: 21 lints in
  `docker/container.rs`, 17 in `docker/wait_for.rs`, and the rest across
  `docker/` (config, group, image, network, wireguard), `providers/`,
  `lib.rs`, `testing.rs` and six test files (`doc_markdown`,
  `missing_errors_doc`, `must_use_candidate`, `implicit_hasher`,
  `cast_possible_truncation`, `unused_async_trait_impl`, `map_unwrap_or`,
  `match_wildcard_for_single_variants`, and others). The 43 non-Docker tests
  pass. A new §7 entry records the ~625 pedantic warnings in the `vms` feature
  tree (not CI-enforced: CI clippy/build run `runs[0]` = default features). A
  new §6 entry records a Docker client stall: POST requests with a body wait
  ~2 minutes for `100 Continue`, so the daemon-backed integration tests take
  10+ minutes and look hung.

- 2026-11-02: bug-backlog §6 (foundation_deployment_cloudflare) fixed on
  `fix/io-35-turso-async-trait-overflow`. The unused `urlencoding` dependency is removed
  from `foundation_deployment_cloudflare/Cargo.toml`. It appears nowhere in the
  crate's source or tests. `cargo check --profile uat` passes. The full test
  build is skipped: this generated crate's `--all-targets` twin peaks near
  7 GB, and the local disk is 97% full. The backlog entry is removed.

- 2026-11-02: bug-backlog §2 (foundation_netio) fixed on
  `fix/io-35-turso-async-trait-overflow`. The unused `thiserror` dependency is removed from
  `foundation_netio/Cargo.toml`. Nothing in the crate uses it; netio builds
  errors with `derive_more`. The two remaining manifest warnings (`flate2`,
  `rustls-pemfile`) are false positives: both are optional and used under their
  features. The default test suite passes (1270 tests), clippy is clean, and
  the wasm-fetch check passes. The backlog entry is removed.

- 2026-11-02: bug-backlog §12 (foundation_auth) fixed on
  `fix/io-35-turso-async-trait-overflow`. Two entries are cleared. First, the dead
  `WebAuthnService.config` field is dropped from both the `server-native` and
  the non-`server-native` variants; the `config` argument is used only locally
  in `new()`. Second, the five `shared` clients (`discovery`, `jwks`,
  `introspection`, `userinfo`, `upstream_client`) each gain a `max_error_body`
  field and a `with_max_error_body` setter that threads the cap into
  `read_error_text`. The new public setter bumps `foundation_auth` 0.0.4 ->
  0.0.5 and its four versioned consumers follow. Five `body_streaming` tests
  assert a client with `with_max_error_body(128)` cuts a 128 KiB error body at
  128 bytes. Two more `server-native` warnings are cleared in the same file: a
  dead `base64::engine::Engine` import and an unused `wa_passkey`
  deserialization. The backlog entries are removed, and a new entry records
  the 200+ clippy warnings in the `server`/`server-native` feature tree.

- 2026-11-02: bug-backlog §10 gemm-f16 fullfp16 (foundation_ai) fixed on
  `fix/io-35-turso-async-trait-overflow`. `cargo test` with candle failed to assemble
  `gemm-f16` on Apple Silicon because `aarch64-apple-darwin` does not enable
  the `fullfp16` target feature. `.cargo/config.toml` now enables `fullfp16`
  for that target. Apple M-series chips all support it in hardware. The
  `candle_tests` suite (61 tests) passes with no manual `RUSTFLAGS`. The
  backlog entry is removed.

- 2026-11-02: bug-backlog §10 clippy-warnings (foundation_ai) fixed on
  `fix/io-35-turso-async-trait-overflow`. Eight clippy lints are cleared: one
  `needless_pass_by_value` (agent_loop), one `wrong_self_convention` (turn),
  one `arc_with_non_send_sync` (llamacpp), two `cast_possible_truncation`
  (candle), one `must_use_candidate` (testing), and two `large_enum_variant`.
  The two large variants are boxed: `Tool::SingleCommand(Box<ToolDefinition>)`
  and the candle Llama cache (`Box<candle_llama::Cache>`). Boxing `Tool` is a
  breaking API change, so `foundation_ai` bumps 0.0.4 -> 0.0.5 and its two
  versioned consumers (`ewe_platform`, `answerme-agent`) follow. The backlog
  entry is removed. A new issue is recorded: `cargo test` with candle fails to
  assemble `gemm-f16` on Apple Silicon without the `fullfp16` target feature.

- 2026-11-02: bug-backlog §10 flaky-test fix (foundation_ai) done on
  `fix/io-35-turso-async-trait-overflow`. `openai_provider::test_provider_streaming_tool_calls`
  flaked when the mock SSE response stayed open. The SSE client then hit its
  100 ms idle timeout, reconnected, and replayed tool-call deltas. The replayed
  deltas corrupted the accumulated arguments JSON. The fix adds
  `Connection: close` to the shared `sse_response` test helper. The mock now
  closes after the body, and the client sees EOF instead of reconnecting. The
  test passes 25/25 in a loop. The backlog entry is removed.

- 2026-11-02: bug-backlog §10 `recursion_depth_exceeding_limit`
  (foundation_ai) fixed on `fix/io-35-turso-async-trait-overflow`. The `agentic_tests`
  and `foundation_ai_tests` test crates both warned on the deep `Send`
  auto-trait resolution in `tests/agentic/session_mock_tests.rs`
  (`build_and_run_a_turn_on_sql_backed_stores`). The two crate roots are now
  split: `tests/agentic_tests.rs` is the `agentic_tests` crate root with
  `#![recursion_limit = "256"]` and loads the shared module tree via
  `#[path = "agentic/mod.rs"] mod agentic;`; `tests/mod.rs` (the unique
  `foundation_ai_tests` root) gains the same inner attribute.
  `tests/agentic/mod.rs` keeps no inner attribute, so neither crate warns
  `unused_attributes`. The Cargo `[[test]] agentic_tests` path now points at
  the new root. Both crates check clean of the warning, and the `agentic_tests`
  suite still passes 329/329. The backlog entry is removed.

- 2026-11-02: bug-backlog §10 candle-gemm (foundation_ai) fixed on
  `fix/io-35-turso-async-trait-overflow`. Seventeen candle tests panicked in `gemm-f32`
  on the Linux CI. gemm-common picked the `fma` kernel on `fma` alone, but
  that kernel needs AVX2. The emulated linux/amd64 CPU has `fma` and not
  `avx2`, so `V3::try_new()` returned `None` and the `.unwrap()` panicked.
  The fix vendors gemm-common 0.19.0 under `patches/gemm-common` and requires
  `fma` and `avx2` in `init_gemm_fn` and `init_gemm_cplx_fn`. The backlog
  entry is removed.

- 2026-11-02: bug-backlog §10 (foundation_ai) fixed on
  `fix/io-35-turso-async-trait-overflow`. The wasm32 `--no-default-features` check failed
  with three `Send` errors because the cloud providers awaited valtron's
  non-Send `sleep_async` inside their async-trait futures. Each provider now
  wraps the sleep future in `foundation_compact::SendWrapper`. The backlog
  entry is removed, and a flaky
  `openai_provider::test_provider_streaming_tool_calls` is recorded. Commit
  04febe2cf.

- 2026-11-02: feature 16 E8 follow-up (foundation_ai) done on
  `fix/io-35-turso-async-trait-overflow`. The pub/sub `Appended` event id now reuses the
  record's own id for WorkingMemory / Observation / Reflection, matching the
  stored `doc_id`. A test asserts the event id and the append return value equal
  the record id.

- 2026-11-02: feature 16 E8 (foundation_ai) done on
  `fix/io-35-turso-async-trait-overflow`. Memory records (WorkingMemory, Observation,
  Reflection) now write through `append_promotable_with_id` with the record's
  own scru128 id, so a retried write is a no-op. Two idempotency tests are
  added. The conversation arm already used `append_with_id`. Commit 87fe96a67.

- 2026-11-02: bug-backlog §13 (`vfs_search` panic) fixed on
  `fix/io-35-turso-async-trait-overflow`. `detect_tool()` used `Command::status().is_ok()`
  to pick `rg` or `grep`. On the Linux CI nightly toolchain a missing binary
  returns `Ok(127)`, not an error, so the probe picked `rg` though it was
  absent, and the search returned zero matches. The new `probe_tool()` helper
  checks `status.success()`. The `cli_search_streams_results` test passes 15/15
  on macOS and in the Linux CI image. The backlog entry is removed. Commit
  51320ad9a.

- 2026-11-02: bug-backlog §13 (`fuse_vfs` compile) fixed on
  `fix/io-35-turso-async-trait-overflow`. `fuser` 0.15 default features include `libfuse`,
  which probes for a system fuse3 via pkg-config and panics in its build script
  when it is absent. The fix disables `fuser` default features on the workspace
  dependency so it uses the pure-Rust mount implementation. The `fuse_vfs` test
  passes 21/21 in the Linux image (3 mount tests skip without /dev/fuse). The
  backlog entry is removed. Commit d98c9f12a.

- 2026-11-01: feature 13 (foundation_deployment_platform) E1 done on
  `fix/io-35-turso-async-trait-overflow`. E1 rewrites `wait_for_stdout` to follow the log
  stream once (`follow=true`, stdout only, stderr=false), match a message
  across frame boundaries with a rolling buffer sized `message.len() +
  client.limits().max_log_frame`, and race the next frame against the
  remaining timeout with `futures_lite::future::race`. Only stdout bytes
  count; an empty message returns at once. The function is now `pub` and is
  re-exported from the `docker` module. New mock-daemon tests
  (`tests/wait_for_stdout_tests.rs`):
  `wait_for_stdout_matches_across_frames`,
  `wait_for_stdout_ignores_stderr`, `wait_for_stdout_reads_each_byte_once`.
  Verification: `cargo test -p foundation_deployment_platform --test
  wait_for_stdout_tests --profile dev` green (3/3); `cargo clippy -p
  foundation_deployment_platform --all-targets --profile dev` clean. The
  bug-backlog §7 `wait_for_stdout` entry is removed. The local disk was full
  (ENOSPC); the 45 GiB `target/uat` LLVM cache was deleted to free space
  before the build ran.

- 2026-11-01: feature 13 (foundation_deployment_platform) E2 done on
  `fix/io-35-turso-async-trait-overflow`. E2 adds one shared Docker-daemon helper:
  `foundation_deployment_platform::testing::docker_available()`. It connects
  with `DockerClient::connect_with_defaults()` and pings the daemon with
  `negotiate_version()`. The helper is gated on the `docker` feature. Six
  duplicated `/var/run/docker.sock` file checks in the platform tests are
  replaced with the helper, and `docker_container_macro_supports_async_functions`
  skips (prints SKIP and returns) when no daemon answers. The proxy test
  (`foundation_proxy/tests/proxy_integration_tests.rs`) uses the same helper.
  The platform crate is bumped 0.1.2→0.1.3 and `foundation_testbed` pins
  0.1.3. Bug-backlog §7 Docker-gated-tests and
  `docker_container_macro_supports_async_functions` entries and the §9 proxy
  Docker-gated entry are removed. Verification: `cargo check -p
  foundation_deployment_platform --all-targets --profile dev` and
  `--features vms` clean (three pre-existing unused-dependency warnings stay:
  clap, foundation_sshkit, portpicker); clippy clean; the proxy test target
  compiles; the skip path was proven with a dead `DOCKER_HOST`.

- 2026-11-01: feature 13 (foundation_deployment_platform) E3 done on
  `fix/io-35-turso-async-trait-overflow`. E3 rewrites `query_vagrant_cloud` to use the
  `foundation_netio` HTTP client instead of a `curl` subprocess. The lookup is
  now an async function (`send_async` plus `collect_bytes_async`, streamed and
  never buffered whole by hand); its sync caller drives it with
  `block_on_future`. The size cap comes from an `ImportLimits` config (default
  64 KiB), applied through the client `ClientConfig::max_body_size`. The
  response parses into typed structs (`VagrantBoxMetadata`, `VagrantBoxVersion`,
  `VagrantBoxProvider`), not `serde_json::Value`. The function returns
  `Result<Option<String>>`; a malformed document uses the new
  `TestbedError::VagrantMetadataError`, and HTTP errors use the existing
  `DownloadFailed`. The stale "tokio runtime conflicts" comment is deleted.
  New tests in `tests/vms_import_tests.rs`:
  `vagrant_lookup_parses_metadata` (against a local HTTP server) and
  `vagrant_lookup_reports_http_error`. Verification: `cargo test -p
  foundation_deployment_platform --test vms_import_tests --features vms
  --profile dev` green (6/6). The §7 Vagrant backlog entry is removed.

- 2026-11-01: feature 17 (foundation_db) PR 2 of 2 (E7–E12) done on
  `fix/io-35-turso-async-trait-overflow` (committed as `28f951edf`). E7 moves the auth-table cleanup
  to Unix milliseconds (migration 026 converts legacy whole-second rows with an
  idempotent `< 10000000000` guard; `rate_limits` stays in seconds;
  `run_full_cleanup` reads both clocks). E8 makes the SQL rate-limit record one
  atomic `INSERT ... ON CONFLICT ... RETURNING count` statement and keeps
  Workers KV as a documented best-effort path only. E9 adds migration 027 to
  rebuild the old-shape `rate_limits` table. E10 makes the wasm stores'
  inherent `*_async` methods private and routes every caller through the
  traits. E11 adds the synthetic `examples` CI job (wasm32 builds of the
  `examples/cf-*` workspaces plus the native `answerme-agent`). E12
  feature-gates the rate-limit SQL constants and helpers behind
  `turso|d1|libsql|wasm-bindgen-storage`. New tests:
  `cleanup_deletes_expired_rows_by_documented_unit`,
  `old_rate_limit_table_is_rebuilt`, `rate_limit_counts_concurrent_hits`.
  Verification: `cargo test -p foundation_db --profile uat` green (all targets);
  both wasm32 gates (`--features wasm` and `--features wasm-bindgen-storage`)
  clean; `rg` for public `*_async` in `src/wasm` is empty; `ci.py plan` lists
  `examples`. The six-crate CI block and the workspace check are green (dev
  profile) after the disk was freed; the Linux CI gate is the remaining check.

- 2026-11-01: bug-backlog §13 (kqueue waker) fixed on
  `fix/io-35-turso-async-trait-overflow`. The macOS kqueue waker (`EVFILT_USER`) returned
  `Token(0)` because `wake` triggered `NOTE_TRIGGER` with a null `udata` while
  `register_waker` stored the token as the `EV_ADD` `udata`; XNU copies the
  trigger's `udata` onto the knote. Fix: `wake` now passes the stored token as
  the trigger `udata`. The two failing tests (`waker_wakes_from_another_thread`,
  `waker_wakes_from_multiple_threads`) now pass; the full `poll` target is 12/12
  green (dev profile). The backlog §13 entry is removed.

- 2026-11-01: version-bump reconciliation. Commit `a896ff0bb` (foundation_db
  0.0.4 to 0.0.5) missed three consumers that pin `version = "0.0.4"` on the
  line after `path` in a multi-line `[dependencies.foundation_db]` table:
  `foundation_deployment_docker`, `foundation_deployment_hetzner`,
  `foundation_deployment_digitalocean`. `cargo metadata` failed workspace-wide,
  so every CI gate died pre-flight. Feature 16 also added public API but kept
  `foundation_ai` at 0.0.2. Commit `53b02bece` bumps the three consumers to
  0.0.5. This change bumps `foundation_ai` to 0.0.3 and its two consumers
  (`bin/platform`, `apps/answerme-agent`) to 0.0.3. `cargo metadata` resolves
  and a `tomllib` scan reports zero mismatches.

- 2026-11-01: feature 12 (Docker client and foundation_deployment) done on
  `fix/io-35-turso-async-trait-overflow`. E1 routes the seven hand-written client methods
  through the generated request/response types, streams the upload tar, caps
  error bodies, and returns typed `system_data_usage`. E2 adds `max_json_body`
  to `BodyLimits`. E3 adds a CI run that builds the `docker,integration-tests`
  targets. E4 makes `LogFrameDecoder` return an enum so "need more bytes" and
  "skipped" are distinct. The docker crate's version bumps 0.1.3 to 0.1.4 and
  its consumer `foundation_deployment_platform` follows. Four fixed backlog §6
  entries are removed; one new test-infra entry (SUN_LEN) is added.

- 2026-11-01: feature 20 (foundation_nativeapis process side) done on
  `fix/io-35-turso-async-trait-overflow` (commit 869572cd4). E5 makes the IPC client
  receive on an async future; the join and poll timeouts move to
  `VfsIpcConfig` (defaults 30 s join, 1 s poll). E6 adds a request id to
  `VfsRequest`/`VfsResponse`; the client matches responses by id, drops the
  release-count skip logic and the lock across a send and its receive, and
  makes `ReadAt` sizes u64. E7 moves the CLI search wait onto a valtron offload
  pool, streams the child's stdout line by line, and puts the limits on
  `SearchConfig` (50 per file, 200 per page, 5 s budget, 30 s scan wait). The
  macOS mach-port send right is fixed (`MACH_MSG_TYPE_MAKE_SEND` for a port the
  process owns). Eight fixed bug-backlog §13 entries are removed. E1–E4 and E8
  landed earlier (commits 0db0d2734, c37d7cf17, ac61b245f, 52b88e326,
  a80d202dd). Verification: native_vfs 21 green, ipc_vfs 14 green, vfs_search
  15 green, and the combined VFS feature check is clean.

- 2026-11-01: feature 17 (foundation_db) PR 1 of 2 (E1–E6) done on
  `fix/io-35-turso-async-trait-overflow`. E1 makes `StateStore::set` key by `resource_id`
  (the stored state's `id` is set from it, whatever `state.id` holds). E2 moves
  the Cloudflare D1/R2 wire types into a new leaf crate
  `foundation_cloudflare_types` and uses them in `R2Store` (rule 5: the old
  `foundation_deployment_cloudflare` types had a cyclic dependency). E3 makes R2
  state listings a cursor-driven page stream (no sort; R2 returns keys in
  order). E4 streams blob get/put and adds range get, conditional put
  (`if_none_match`/`if_match` returning `PreconditionFailed`) and ordered
  `list_after`. E5 adds `QueryStore::execute_batch_statements` (D1 `batch()`,
  one transaction on libsql and Turso). E6 adds a mock-D1 `init` test.
  Verification: `cargo test -p foundation_db --profile uat` green; clippy clean
  on both crates. E7–E12 stay open for PR 2. The foundation_db 0.0.4→0.0.5
  version bump is deferred until feature 20 lands (it edits
  foundation_nativeapis).

- 2026-11-01: feature 16 (foundation_ai) done in the working tree on
  `fix/io-35-turso-async-trait-overflow` (not committed). E1 adds an async
  `Model::generate_async` (async-first, default falls back to the sync
  `generate`) with async retry back-off over the valtron timer; the HTTP
  providers implement it over `send_async`. E2 makes the `agent` tool report
  `Working` and return on `Stream::Wait` instead of `continue`, so it no longer
  busy-loops. E3 caps non-2xx error bodies at a `max_error_body` config field
  (default 64 KiB from Go `net/http`'s error-drain limit) in the OpenAI,
  Responses, Anthropic, Workers and `model_info` paths; status is read before
  the body. E4 adds the Responses `fetch_model_info` success test over
  `GPT_JSON`. E5 makes `embed`/`embed_batch` async and awaits them from
  `ContextProvider::search` and the `shed` tool. E6 makes a second concurrent
  flush await the running one (a `FlushState` mutex with wakers and a drop
  guard) instead of returning `Ok(0)`. E8 renames `record_async`/`hydrate_async`
  to `record`/`hydrate`. E9 drops the unconditional `infrastructure_llama_cpp`
  dev-dependency, re-exports the llama error types through the `llamacpp`-gated
  module, and gates those tests on the feature. E7 is blocked on `foundation_db`
  (idempotent upsert by document id) and stays in backlog §10. A `Send` bug was
  found and fixed along the way: `GenerationError`/`ModelErrors`/
  `ModelProviderErrors` held a non-`Send` `BoxedError`, which broke `zip` of an
  async generate with a ticker; they now use `SendableBoxedError`. Verification:
  `cargo check -p foundation_ai --all-targets --profile uat` clean (four
  pre-existing candle dead-code warnings); `cargo clippy -p foundation_ai
  --profile uat --no-deps` down to seven pre-existing lib warnings (two new ones
  introduced and fixed); `cargo test -p foundation_ai --profile uat --features
  testing` green (2126 passed, 0 failed); E1/E2/E5/E6 timing tests each pass 20
  runs in a row; E9 builds with `tools/llama.cpp` moved away. PR: see the PR.

- 2026-11-01: feature 24 (tokio boundaries and apps) done on
  `fix/io-35-turso-async-trait-overflow`. Removed tokio and axum from `bin/platform`,
  its templates, the examples and the `ewe_channels` and `ewe_domain` crates. The
  platform binary now uses `#[valtron(seed = 100)]`. Inline tests moved to `tests/`.
  The `deno_core` tokio boundary in `foundation_testbed` is documented. New backlog
  entries record the remaining channels, domain, todo and template debt. The Linux
  CI gate is green (fmt, build, clippy, test, wasm; the templates test passed), so
  issue #137 can close. Commit c18df9490.

- 2026-11-01: feature 21 done on `fix/io-35-turso-async-trait-overflow` (commit
  95838a4a8). `foundation_cronjobs` parses each `cron_expr` with
  `saffron` (five-field Quartz), rejects an invalid expression at `register`
  with a typed error, computes the real next fire, and runs one valtron task per
  job: park on `TaskStatus::Delayed` until the next fire, run the job, record
  the result, apply the missed-job policy (catch up once, skip, or run all). The
  wall clock is a `SharedCronClock` injected by constructor (rule 11); the crate
  bumped to 0.0.4. `foundation_cedar` gained `PolicyStore::load_all` plus
  `PolicyBundle` so `SqlPolicyStore` reads its row in one query, dropped the
  unused `derive_more`, and replaced the infallible
  `EntityId::from_str(..).map_err(..)` with `EntityId::new(..)`; bumped to
  0.0.4. Verification: `cargo check` clean for both crates, clippy adds no new
  warnings (24 pre-existing cedar `pedantic` warnings recorded in backlog §15),
  cronjobs 13 tests green, cedar 17 (default) / 25 (`db`) tests green, timing
  tests stable over 20 runs. Commit 95838a4a8.

- 2026-11-01: bug-backlog §13 #1 (kqueue edge-triggered read) fixed on
  `fix/io-35-turso-async-trait-overflow`. The macOS kqueue selector now records the
  byte count per fd and delivers a readable event only when the count grows; a
  drained fd re-arms on `reactor.clear`. Verification: `registered_fd_edge_triggered`
  passes, the four shared-reactor tests pass, `foundation_reactor` tests are green,
  and clippy adds no new warnings. Commit 0e43720a7. A macOS waker bug
  (`Token(0)` from `EVFILT_USER`) was found and recorded in backlog §13. The
  `fd` CI run was added to `ci/packages.toml`. Linux CI gate queued.

- 2026-11-01: feature 26 done on `fix/io-35-turso-async-trait-overflow`. The
  `foundation_nativeapis` shared-reactor tests now compile and run on macOS. The
  nonblocking pipe was already portable (`pipe` + `fcntl` on macOS, `pipe2` on Linux). The
  `/proc/self/fd` count test and its `open_fd_count` helper are now gated with
  `#[cfg(target_os = "linux")]`, so the four portable tests run on macOS and the Linux-only
  fd-count test stays in CI. Verification: `cargo test -p foundation_nativeapis --test
  reactor_shared_selector_tests --features fd --profile uat` green (4 pass);
  `cargo check -p foundation_nativeapis --all-targets --profile uat` clean. Linux CI gate
  queued. PR: see the PR.

- 2026-11-01: feature 37 done on `fix/io-35-turso-async-trait-overflow`. A
  transport with `conn_readiness() == None` (the completion socket) now falls
  back instead of spinning. `IoReady` sleeps one poll-fallback interval and
  returns `Ok`; `Hangup` re-polls `gone_now` each interval and returns `Ok` only
  on close/error. The raw-fd probe uses `poll(2)` with
  `POLLHUP`/`POLLERR`/`POLLNVAL` (plus `POLLRDHUP` on Linux). The interval is a
  config field (`ServeTimeouts::poll_fallback_interval`, default 10 ms from
  `POLL_DELAY`), never a constant. Tests: `tests/conn_io_fallback_tests.rs`
  (2 tests). Verification: `cargo test -p foundation_connectrpc --profile uat`
  green. PR: see the PR.

- 2026-11-01: feature 36 done on `fix/io-35-turso-async-trait-overflow`.
  `DnsResolver::resolve` became async via `#[async_trait]`; the request task
  awaits it in a new `Resolving` state instead of blocking a valtron worker.
  `SystemDnsResolver` offloads `to_socket_addrs()` to the valtron background
  boundary (`run_background_job` + `Pipe`). The DNS deadline is a config field
  (`dns_timeout`, default 30 s from nginx `resolver_timeout`). A first-pass
  regression — the sync connect path routed `SystemDnsResolver` through
  `run_background_job`, which panics when the pool is uninitialized — was fixed
  by adding a synchronous `DnsResolver::resolve_blocking` seam (default
  park/unpark, overridden pool-free by `SystemDnsResolver`, `CachingDnsResolver`,
  and `Arc<T>`). Tests: `tests/simple_http/dns_async_tests.rs` (3 tests).
  Verification: `cargo test -p foundation_netio --profile uat` green. PR: see
  the PR.

- 2026-11-01: moved UUID generation into `foundation_compact` on
  `fix/io-35-turso-async-trait-overflow`. The earlier keychain wasm fix added
  `uuid = { version = "1", features = ["v4", "js"] }` to the keychain, which is a
  second entropy path that bypasses rule 11 (one injected entropy seam). The
  corrected fix makes `foundation_compact` depend on
  `uuid = { version = "1", default-features = false }` and expose
  `foundation_compact::uuid::new_v4()`, which fills 16 bytes with
  `foundation_compact::entropy::fill` and calls `Builder::from_random_bytes`
  (a `const fn`, no feature). `foundation_keychain` now uses
  `foundation_compact::uuid::{self, Uuid}` through `core::crypto::new_uuid_v4()`
  and no longer depends on `uuid` directly. Verification: native + wasm
  `cargo check` pass, keychain `cargo test --profile uat` green. PR: see the PR.

- 2026-11-01: features 38 and 40 done, and the publish fix landed, on
  `fix/io-35-turso-async-trait-overflow`. Feature 38 deleted the dead
  `foundation_netio/src/native/tls_verification.rs` module (490 lines, never declared)
  and the commented-out `// pub mod tls_verification;` in `native/mod.rs`. Feature 40
  renamed `OverlayStream.owner` to `_owner` (option 1) in `netstack.rs`; the
  `Arc<OverlayStreamOwner>` still keeps the smoltcp socket alive until the last drop.
  The publish fix bumped `foundation_core` and `foundation_macros` to 0.0.7 and every
  consumer dependency to `0.0.7` (43 manifests plus the lock), so `foundation_reactor`
  0.0.1 no longer fails tarball verification on the stale published `foundation_core`
  0.0.6 (E0425/E0407). The `release-plz release-pr` failure was a root-manifest
  `cargo-features = ["codegen-backend"]` already fixed on master by `df86d7e6d`.
  Verification: `cargo check` and `clippy` clean for the touched crates; the netstack
  dataplane tests pass 7/7. A pre-existing `tests/fd/registration.rs` edge-triggered
  failure is tracked in the backlog §13 and is unrelated. PR: see the PR.

- 2026-11-01: feature 35 done (with feature 34) on
  `fix/io-35-turso-async-trait-overflow`. The `recursion_depth_exceeding_limit`
  warning came from the `turso` crate's deep async chain, not from the
  `#[valtron_test]` macro. `TursoStorage::set` (async-trait) overflows the
  `CoerceUnsized`/`Send` check, and `#[valtron_test]` bodies in the turso-backed
  test files force `Send` through the same chain. Fix: `#![recursion_limit = "256"]`
  in `foundation_db/src/lib.rs` and the three test files
  (`foundation_db/tests/turso_storage_tests.rs`,
  `foundation_auth/tests/credential_store_tests.rs`,
  `foundation_auth/tests/auth_manager_tests.rs`). Feature 34's "149 files across 17
  crates" premise was wrong — the warning is confined to turso-backed stores, so
  no macro change is needed. Verification: `cargo check -p foundation_db --tests`
  and `-p foundation_auth --tests` are warning-free; the foundation_db CI gate is
  green and all tests pass. PR: see the PR.

- 2026-10-31: fixed the second native build error on `fix/io-01-valtron-parking`. The
  `pull/163/merge` brought master's incomplete Dependabot bump of `sha2` (0.10 to 0.11).
  `foundation_auth` then used `sha2` 0.11 (`digest` 0.11.3) beside `hmac` 0.12, `pbkdf2` 0.12,
  and `argon2` 0.5 (`digest` 0.10.7). The `CoreProxy` trait moved between these `digest`
  versions, so the build failed with `Sha256: CoreProxy not satisfied`. Fix: revert `sha2` to
  0.10 in the 13 touched crates. Keep `sha2` 0.11.0 in the lock for `rust-embed-utils` (via
  `rust-embed` then `age` in `foundation_keychain`). Verification: `foundation_auth`,
  `foundation_keychain`, and `foundation_connectrpc` compile with the revert. PR: see the PR.

- 2026-10-31: the `ci:linux` task now enforces each package's timeout and captures test output.
  `scripts/ci/ci.py` `run_package` passes the package's `timeout-minutes` as one hard deadline
  to each command, so a looping step stops instead of running forever, matching the GitHub job
  timeout. `cargo test` gets `--nocapture` so a failure leaves its own trace. PR: see the PR.

- 2026-10-31: CI green restored on `fix/io-01-valtron-parking`. The feature 11 PR had 12
  failing CI jobs. Three latent build bugs and one flaky test caused them. `foundation_nativeapis`
  `uring` did not forward `foundation_reactor/uring`, so its uring send path failed to compile
  on Linux. `foundation_reactor`'s `io-uring` dependency sat in the Windows target section, so
  `dep:io-uring` enabled nothing on Linux. `poll::event::Event` had no shell variant for wasm.
  The connectrpc oversized-envelope test asserted a 1 s wall clock that flaked under parallel
  test contention. Fixes: forward the reactor feature; move the dependency to the Linux target
  section; add a shell `Event`; assert the observable outcome instead of elapsed time. Also
  added a `ci:linux` mise task (`scripts/ci/linux.sh`) that mounts the repo into the CI image
  and reuses a per-platform cargo target volume. Verification: the full pipeline (fmt, build,
  clippy, test, wasm) is green for the failing crates. PR: see the PR.

- 2026-10-31: feature 11 done on `fix/io-01-valtron-parking`. Every `#[cfg(test)]` block in
  the three crate source trees is gone (rule 7). foundation_netio moved its tests per area
  (http/tasks, native, shared, event_source, websocket, wasm). foundation_http moved the five
  middleware suites and the `CfConn` wasm suite. foundation_connectrpc moved the wasm-fetch
  capabilities test into `transport_tests.rs`. Test counts did not drop. Four items became
  public with docs because they are real API (`CompressionAlgorithm::from_str`,
  `CompressionMiddleware::select_algorithm`, `CorsConfig::is_origin_allowed`,
  `LoggerMiddleware::level`); the auth and body-limit tests that used private fields or the
  private base64 helper were rewritten through the public `handle()` behaviour. The
  `tls_verification.rs` module is dead code (never declared in `native/mod.rs`), so its five
  former tests never compiled and are removed, not moved. Verification:
  `git grep '#[cfg(test)]'` over the three `src/` trees returns nothing, and the native plus
  wasm CI gates for the three crates are green. PR: see the PR.

- 2026-10-30: feature 33 done on `fix/io-01-valtron-parking`. A stream handle that queued
  outgoing data (a reset, write, finish, stop, close, a datagram, or a flow-control update)
  did not wake the QUIC driver. The driver parked on socket readiness and the next timer
  (feature 28); with no incoming packet and no pending timer, the queued data was never
  transmitted. The flaky `http3_server_rejects_a_request_it_has_no_handler_for` test failed
  about 15 times in 20 runs because its `H3_REQUEST_REJECTED` reset was queued but never sent.
  Fix: the driver holds a `BoolSignal` self-wake and shares a clone into every `ConnState` as
  `driver_wake`. The driver clears it before each pass and composes it into its `Depends`
  readiness (socket edge, next timer, or the latch). Every queued-outgoing-data path calls
  `ConnState::wake_driver()`. A new deterministic test
  (`queued_reset_wakes_a_driver_parked_on_readiness`) parks a server driver on an injected
  readiness, queues a reset, and asserts the parked readiness becomes ready. The H3 reject test
  passes 20 runs in a row (was 15/20 fail); the QUIC suite is 8/8. The §3 flaky-test backlog
  entry is deleted. PR: see the PR.

- 2026-10-30: feature 32 done on `fix/io-01-valtron-parking`. The reactor drain poll timeout
  is configuration, not a constant (rule 8). `const DRAIN_TIMEOUT_MS` is gone; its value lives
  in `pub const DEFAULT_DRAIN_POLL_TIMEOUT: Duration = 100 ms`, referenced only by
  `ReactorConfig::default()`. New `ReactorConfig` holds `drain_poll_timeout` with a builder
  (`with_drain_poll_timeout`), a getter, and a documented default. A new entry point
  `Reactor::init_with_config(preference, config)` threads the config into `init_inner` and the
  drain loop; `get()` and `init()` delegate to it with the default config, so their behaviour is
  unchanged. The reactor stores `drain_poll_timeout` and exposes it via
  `Reactor::drain_poll_timeout()`. Tests: a new `config` test target (its own process, so it
  owns the process-global singleton) with default, builder round-trip, and
  `init_with_config`-stores-the-value assertions. Verification: `foundation_reactor` CI green
  (config 3/3, shutdown_signal 4/4, clippy + fmt clean). The §2 `DRAIN_TIMEOUT_MS` backlog entry
  is deleted. PR: see the PR.

- 2026-10-30: feature 31 done on `fix/io-01-valtron-parking`. The accept loop now wakes at
  once on shutdown. `Reactor::wake_waiters()` (new) bumps `events_gen` and calls `notify_all`,
  so a thread parked in `wait_for_events` returns without a real fd event. `ShutdownSignal`
  (new, `foundation_reactor/src/shutdown_signal.rs`) is a cross-platform `AtomicBool` flag with
  a unix-only `OnceLock<Arc<Reactor>>` wake link; `turn_on` sets the flag then wakes the linked
  reactor on the false-to-true change, and `register_wake` links the shared reactor (best-effort).
  It is re-exported through `foundation_reactor`, `foundation_iogate`, and `foundation_http`.
  The http server's shutdown type changed from `OnSignal` to `ShutdownSignal`, which cascaded to
  every `serve*`/`serve_h3` caller: `foundation_connectrpc`, `foundation_auth`,
  `foundation_proxy` (src + tests, except `passthrough.rs` whose signals never reach `serve*`),
  `foundation_keychain`, `foundation_deployment_docker`, and `foundation_testbed`.
  `serve_loop` calls `shutdown.register_wake()` after the listener fd is registered;
  `accept_shutdown_poll` is now a safety bound only (default unchanged at 10 ms). New
  `foundation_reactor/tests/shutdown_signal/` (4 tests: flag semantics + a wake test that parks
  a thread in `wait_for_events` and asserts `turn_on` returns it promptly). Verification:
  reactor 4/4, http suite green, `cargo check --workspace --all-targets` clean. The §4
  `OnSignal` backlog entry is deleted; a pre-existing flaky H3 test and the reactor
  `DRAIN_TIMEOUT_MS` rule-8 const are recorded. PR: see the PR.

- 2026-10-30: feature 30 done on `fix/io-01-valtron-parking`. The remaining valtron limits
  became configuration (rule 8), one field each: `readiness_poll_quantum` (R1, 1 ms) and
  `wait_check_interval` (R7, 4 ms) on `LocalThreadExecutor`; `op_read_time` (R2, 100 ms) on
  `PoolConfig`; `notify_queue_wait_timeout` (R3, 10 ms) on `ThreadedIterFutureConfig`;
  `max_spins` (R4, 100 native / 1 wasm) on a new `NotifyQueueConfig`; `readiness_wait` (R5,
  10 ms) on the executor state (via `Sleepers`); and `delivery_capacity` (R6, 8) on
  `TaskSpawnConfig`. R8's `DEFAULT_PARK_DURATION` (1 ms) and `DEFAULT_WAIT_CYCLE` (300 µs)
  stay as documented optional-parameter defaults, now read from `StreamConfig::default()` so
  the consts are referenced only by that `Default` impl. New
  `tests/valtron/limits_config_tests.rs` (8 tests) asserts the observable fields round-trip
  through their builders and the non-observable ones match their documented defaults. The §4
  backlog entry is deleted. PR: see the PR.

- 2026-10-30: feature 27 done on `fix/io-01-valtron-parking`. `SleepIterator::next` returned
  `Pending(0 ns)` when its deadline equalled the current time, because
  `Instant::checked_duration_since` gives `Some(Duration::ZERO)` at equality. The reached-deadline
  classification is now a pure `pub fn remaining(completed_at, now) -> Option<Duration>` that returns
  `Some(diff)` only for `diff > Duration::ZERO`, and `next` matches on it — a past, exactly-now or
  overflowed deadline is `Done`. A new boundary test (`Some(now)`, `Some(now ± 1 ms)`, `None`) is
  deterministic and no longer depends on `Instant::now` advancing. The flake test
  `zero_duration_sleep_iterator_finishes_immediately` passed 20 runs in a row in both feature sets;
  `foundation_core` CI green (716 + 753 tests). The backlog §4 `SleepIterator` entry is deleted.
  PR: see the PR.

- 2026-10-30: bug-backlog §3 fixed on `fix/io-01-valtron-parking`. The HTTP/1.1 server's
  `WaitingForBody` state no longer retries `probe()` on fixed 30 ms delays and 408s at ~155 ms.
  It now arms the connection's read waker before each probe and, for a transport that reports
  readiness (the WireGuard overlay), parks on `AnyReadiness::either(read_signal,
  DeadlinePassed(deadline))` — the body wakes it the moment it arrives, and a hung client still
  gets 408 once the new `ExpectContinueConfig.body_timeout` (default 3 s) elapses. Blocking
  transports keep the old `base_delay`/`max_attempts` countdown unchanged (their `register_read_waker`
  returns `false`). The `overlay_connector_http_round_trip` test (spec-55 F11, `Expect: 100-continue`
  enabled) now passes 20/20; the four server-side 100-continue integration tests stay green.
  `foundation_http` CI green (both runs) and `foundation_netio` CI green. The backlog §3 entry is
  deleted. PR: see the PR.

- 2026-10-30: feature 28 done on `fix/io-01-valtron-parking`. The idle H3 endpoint now parks on
  the QUIC socket instead of polling at 1 ms. `EventReadiness` gained a default `clear()` and
  `FdReadiness` overrides it (forwards to its existing inherent `clear`); `QuicDriver::next_status`
  clears edge-triggered readiness before `pump_recv` (the H2 handler's clear-before-read order)
  and, when a socket signal is present, returns `Depends(AnyReadiness::either(fd,
  DeadlinePassed(next_timeout)))` instead of `Delayed(IDLE_POLL)`. The deadline composition is a
  new correctness requirement the spec did not state: a `Depends` sleeper has no deadline, so
  parking only on the fd would starve QUIC loss-detection/idle-timeout/keep-alive timers;
  `next_timeout` is the earliest `poll_timeout()` across connections. `serve_h3` wraps the
  driver's UDP socket in `FdReadiness` via `watch_driver_socket` (falls back to polling on
  non-unix or if the reactor refuses). The flaky process-wide `getrusage(RUSAGE_SELF)` CPU
  assertion is replaced with a deterministic poll count (`CountingDriver`): the idle driver
  parks at ~0 `next_status` calls/s instead of ~1000, asserted over a 1 s idle window, plus a
  direct `driver_parks_on_fd_readiness_not_delayed` test (no signal → `Delayed`, signal →
  `Depends`). Verification: netio default + `quic` green (the `quic` run is 1108/1108), http,
  connectrpc, iogate, core, reactor all green, `cargo check --workspace --all-targets` clean,
  and the idle test passed 20 runs in a row. Two backlog entries deleted (the 1 ms poll and the
  process-CPU measurement). PR: see the PR.

- 2026-10-30: feature 10 done on `fix/io-01-valtron-parking`. The three ignored netio test
  files became hermetic: `http_external_validation.rs` (8) and `http_server_integration.rs`
  (6) drive the local `TestHttpServer` on loopback sockets, and `tls_integration.rs` (3) runs
  a local TLS server with a test CA generated at start (`rcgen`). All 37 netio doc examples
  now compile — `ignore` is gone and `no_run` stays only where the example opens a real
  network connection (`cargo test --doc -p foundation_netio` passes 54/54). Every conformance
  test gained an 8-thread twin through a small `macro_rules!` macro that emits the
  `threads = 4` and `threads = 8` copies from one body (`seed + 1000`); the suite is 36/36.
  The formerly-ignored tests and the 8-thread variants each passed 20 runs in a row.
  Verification: netio default 1267/1267, connectrpc default and `h3` green, the `rg` checks
  return 0; the netio `quic` run still fails only the pre-existing
  `idle_h3_connection_uses_no_cpu` (feature 28, process-CPU measurement) — out of scope.
  `unshare` is not available on macOS, so the no-network check could not run. Backlog entries
  #27, #31 and #33 deleted. PR: see the PR.

- 2026-10-30: feature 09 done on `fix/io-01-valtron-parking`. Limits became
  configuration (rule 8), one item each in its own commit. L1-L3 (valtron): the pool config
  gained `max_yield_duration` (100 ms), `yield_wait_time` (4 s) and four back-off knobs;
  `ThreadedIterFutureConfig` documents the queue size and the back-pressure park timeout. A
  real fix in this feature: `block_on`'s multi-worker `Wait` arm re-polled at
  `no_work_yield`, so `max_yield_duration` governed only `run_until` (single-threaded); it
  now caps the re-poll at `min(no_work_yield, max_yield_duration)`. L4:
  `BackgroundJobRegistry` takes a capacity (default 1024); a full queue returns an error and
  `new` returns `Result`. L5-L8: `BytesPoolConfig` bounds `max_pooled_buffers` (64) and
  `max_buffer_size` (64 KiB); `acquire_with_capacity` counts one outcome; `PooledBuffer::owned`
  holds no pool; `Debug` shows length and capacity only. L9: `BatchReader` logs `WouldBlock`
  at `trace`. L10: `SpinWaiter` keeps nanosecond precision with named defaults. L11: a
  release-plz version group publishes the macros and core crates together. Six backlog
  entries deleted; a new entry records the remaining hard-coded valtron limits that were out
  of scope (`READINESS_POLL_QUANTUM`, `DEFAULT_OP_READ_TIME`, `DEFAULT_NOTIFY_QUEUE_*`,
  `DEFAULT_READINESS_WAIT`, `DEFAULT_DELIVERY_CAPACITY`, `JS_WAIT_CHECK_INTERVAL`). PR: see
  the PR.

- 2026-10-30: feature 08 done on `fix/io-01-valtron-parking`. `Serve` gained a fourth result,
  `ConnectionResult::Async(ServeFuture)` (`ServeFuture = Pin<Box<dyn Future<Output =
  ConnectionResult> + Send + 'static>>`), so a long-lived handler returns its continuation at once
  and stops holding a worker. The connection handler (`connection.rs`) gained
  `HandlerState::Awaiting`, which polls the future once per wake with `queue_waker(queue, 0)` and
  parks on `AsyncReadiness` (the `FutureTask` wake bridge ORed with the shutdown signal); `Keep`/
  `Take`/`Close` resolve to the same states as before, and an `Async`-inside-`Async` is a logged
  programming error that closes. The wasm `serve_writer` path treats `Async` as a clear error.
  `ConnectRpcServe::serve` now returns `Async(Box::pin(fut))` and has no `block_on`. The socket wait
  was rebuilt on the readiness seams: `ConnIo` owns a `ConnReadiness` plus an `FdReadiness`
  (kernel fd), `IoReady` parks a `std::task::Waker` (`FdReadiness::register_waker` or a one-shot
  `ConnWaker`) plus a valtron `timer::Sleep` for the deadline, and `Hangup` reads the reactor's
  latched `READ_CLOSED`/`WRITE_CLOSED`/`ERROR` flags so it does not mistake a pipelined request
  for a close. The deadline timer is the existing `ThreadTimer` as a `static SERVE_TIMER`, and
  `dispatch_to` now takes `Box<dyn ResponseSink>` (owned) so the future is `'static`. Regression
  tests: `h1_streaming_calls_do_not_exhaust_the_pool` (8 server-streaming RPCs open on a 2-worker
  pool; a unary call finishes under 100 ms) and `async_serve_tests` (`keep` serves the next
  request, shutdown drops the future, nested `Async` closes). Both pass 20 runs in a row; the
  connectrpc conformance tests stay green. Backlog §1 entry deleted and a §9 proxy follow-up line
  added. PR: see the PR.

- 2026-10-29: feature 07 done on `fix/io-01-valtron-parking`. Three fixes, one commit each.
  §1 (HTTP/3 DATA frames): `FrameDecoder` gained `State::Data { remaining }` and a configurable
  `read_chunk` (default 16 KiB), so a large DATA frame streams chunk-by-chunk instead of buffering
  whole; `H3Connection::max_frame_size`/`read_chunk` thread both onto `FramedRecv`/`H3Request`, and
  a frame whose length exceeds the cap is refused with `H3_FRAME_ERROR` before any allocation
  (`frame_over_configured_cap_is_rejected`). §2 (WebTransport lifetime): `poll_accept_webtransport`
  no longer FINs the CONNECT stream after the 200; it hands the open stream to
  `WtSession::new_with_connect`, whose `poll_connect` drains capsules then treats a FIN/reset as the
  session end (draft-ietf-webtrans-http3 §5 "Session Termination"); `close` sends a
  `CLOSE_WEBTRANSPORT_SESSION` capsule then FIN. Tests:
  `webtransport_session_stays_open_after_accept`, `closing_connect_stream_ends_session`. §3 (wasm
  SSE read error): `spawn_stream_reader` records the JS `ReadableStream` read error in a shared slot
  beside `done`; `QueueSseEvents::end_or_fail` turns it into `Pending(SseProgress::Failed)` then
  `None` instead of a clean end. Native test `queue_sse_events_surface_a_producer_read_error` and a
  wasm-bindgen test `sse_read_error_surfaces_failed_not_a_clean_end`. A bug found and fixed along the
  way: `foundation_reactor` was an unconditional netio dependency with no wasm32 poll backend, so the
  wasm check failed; it is now gated `not(target_arch = "wasm32")` (used only by `native/raw_stream`).
  PR: see the PR.

- 2026-10-29: feature 06 done on `fix/io-01-valtron-parking`. `Connection` and `RawStream`
  gained `conn_readiness()` → a purpose-built `ConnReadiness` in netio: `Fd(fd,
  Option<Arc<dyn TlsInterest>>)` for kernel sockets and `Overlay(Box<dyn OverlayReadWrite>)`
  for the WireGuard overlay. `TlsInterest::corrected` maps an application direction to the
  wire direction rustls needs (`wants_write` first, then "still handshaking"), so the
  opposite-direction stall cannot park on the wrong fd event. The overlay reports the full
  predicate (`readable` = data or EOF, `writable` = space or surfaced error) and is woken by
  one-shot `ConnWaker` callbacks, not `poll(2)`. `conn_io` (connectrpc's blocking HTTP/1.1
  driver) now parks on `ConnReadiness` through a wake socket pair. Two real bugs fixed along
  the way: `OverlayStream`'s `Clone` removed its smoltcp socket twice on drop (now ref-counted
  via `OverlayStreamOwner`), and `HttpClient::open_exchange` silently dropped a
  `with_connector` connector by building a fresh pool (now reuses `self.pool`). Regression
  tests: `overlay_readiness_tests.rs` (20/20, `expect_continue(false)` to dodge the
  out-of-scope foundation_http 100-continue race) and `rustls_opposite_direction.rs` (20/20,
  uses the new `refresh_traffic_keys()` to set `wants_write` deterministically). Inline
  `#[cfg(test)]` blocks in `connection.rs` and `rustls.rs` moved to `tests/tcp_and_tls/`
  (rule 7); two third-party-behaviour tests dropped. Remaining gap tracked in the backlog:
  io_uring completion socket, openssl, and native-tls still report `Unsupported`. PR: see the PR.

- 2026-10-28: feature 05 done on `fix/io-01-valtron-parking`. Pushed request body: the
  `WritingBody` state in `request_redirect.rs` parks on the pipe via
  `TaskStatus::Depends(readiness)` (register-then-check), and the `producer_stalls` back-off
  plus `MAX_PRODUCER_BACKOFF_MS` are deleted. Accept loop: `serve_loop` registers the
  listener fd with `FdReadiness` and blocks in `wait_for_events`, waking at once on a
  connection; shutdown detection is bounded by the new `accept_shutdown_poll` config (10 ms),
  because `OnSignal` is a passive atomic with no wake primitive (see learnings + backlog).
  `would_block_sleep` stays only on the non-unix fallback. Regression tests:
  `pushed_body_waits_without_polling` (poll count under 20), `pushed_body_producer_drop_ends_the_body`
  (renamed from the spec's `_fails_the_request`: dropping the producer closes the pipe = a
  graceful end), and three `accept_loop_parking_tests`. PR: see the PR.

- 2026-10-28: feature 04 done on `fix/io-01-valtron-parking`. The TCP connect and the
  rustls handshake no longer block a valtron worker. The task path
  (`request_redirect.rs`, not the spec's `request_intro.rs` — see learnings) gained two
  states: `Connecting` (a `PendingConnect`/socket2 non-blocking `connect(2)` that parks on
  `SocketReadiness` write readiness and reads `SO_ERROR`) and `TlsHandshake` (rustls
  `complete_io` driven by `drive_handshake`, parked on the direction rustls asks for). One
  fixed connect deadline covers every address and the handshake, so a partial handshake
  cannot reset it. Error kinds survive (`ConnectionTimeout`, `IoError` with the real
  `ErrorKind`, `TlsHandshakeFailed`). The public sync constructors stay on the blocking
  boundary (`Connection::with_timeout`, `TcpStreamWrapper::connect_timeout`, the SSL
  connectors). `connect_completion` in iogate was read and not reused (it does a blocking
  `TcpStream::connect` first, and iogate sits above netio). Four regression tests in
  `tests/tcp_and_tls/nonblocking_connect_tls.rs` pin the behaviour; the blackhole test
  shows a ticker keeps running on a 2-thread-saturated pool. Backlog task #23 deleted; a
  new DNS entry added (`SystemDnsResolver::resolve` still blocks). PR: see the PR.

- 2026-10-28: feature 03 done on `fix/io-01-valtron-parking`. T1–T2 (the §2 hunks:
  `SendSafeBodyBytesIterator` reports `Stream::Wait` for every body kind, and
  `is_would_block`) landed earlier. T3–T6 landed through feature 29 (the reactor bridge):
  `ReadWakeSource` is built from the connection and attached at every body construction site,
  `poll_next` registers the waker then repolls once before parking, the async downstream
  consumers use `into_async_parts`, and the regression tests pass 20 runs in a row. The one
  `wake_by_ref` left in `body_reader.rs` is the logged fallback. The `quic` run still fails the
  pre-existing `idle_h3_connection_uses_no_cpu` (feature 02/28); it is out of scope. PR: see
  the PR.

- 2026-10-28: feature 29 done on `fix/io-01-valtron-parking`. The I/O reactor moved into the
  new `foundation_reactor` crate below `foundation_netio`, so netio can reach `FdReadiness`
  with no crate cycle. `FdReadiness::register_waker(&Waker)` was added. `RawStream::read_wake_source()`
  builds the body's `ReadWakeSource` for three transports: an overlay registers a one-shot
  `ConnWaker`, a non-blocking kernel socket registers its fd with the shared reactor, and a
  blocking or completion socket returns `None` (a blocking `WouldBlock` is a timeout, not a
  wait). `SimpleResponse` gained a fourth field that carries the source; `into_async_parts()`
  and `take_async_body()` attach it, and the sync `into_parts()`/`take_body()` drop it. The
  async downstream consumers (`foundation_ai`, D1, R2, `turbopuffer`, `acme`,
  `foundation_deployment`) now use `into_async_parts` and `collect_*_async`. The regression
  tests live in `tests/simple_http/async_body_readiness_tests.rs` and measure the polling
  thread's CPU, not process CPU. Verification: `foundation_reactor` green (sourcefd doctest
  fixed), netio 999/999 non-quic and 1068/1069 quic (the one quic failure is the pre-existing
  `idle_h3_connection_uses_no_cpu` process-CPU test, feature 02/28), iogate/db/ai/vectors/
  proxy/deployment/connectrpc green. nativeapis still fails `fd`/`poll`/`reactor_shared_selector`
  on macOS (feature 26, pre-existing). PR: see the PR.

- 2026-10-28: feature 29 added (client body reader reaches a Waker-capable readiness source).
  Feature 03 T1–T2 landed and pass (netio 1206 tests, connectrpc, iogate), but T3–T6 are
  blocked: the client body is built in `foundation_netio`, whose kernel-socket readiness is
  poll-based `SocketReadiness` (no `std::task::Waker`), while the reactor that could wake a
  `Waker` (`FdReadiness`) lives in `foundation_iogate`, which depends on netio. Feature 03 §3.2
  says stop and report rather than invent a global registry. Feature 29 records the bridge
  (move the reactor below netio, add a netio poll reactor, or drive the body through valtron
  `Depends`). Pending, size M, depends on 02; blocks 03 → 04 → 05 → 06.

- 2026-10-28: feature 28 added (QUIC driver parks on its UDP socket). It closes the gap that
  feature 02 left open: `serve_h3` builds the `QuicDriver` without `with_fd_readiness`, so the
  driver still polls at 1 ms (`IDLE_POLL`), and `EventReadiness` has no `clear` for the
  edge-triggered socket. Pending, size M, depends on 02.

- 2026-10-28: feature 02 (HTTP/2 and HTTP/3 servers park on readiness) implemented on
  `fix/io-02-h2-h3-readiness`. The h2 connection parks on its socket through `FdReadiness`
  (readable-only; writable is opt-in via `set_writable` so an idle socket does not spin the
  drain thread). The reactor gained `reregister` and kqueue `reregister_fd` deletes a dropped
  filter before re-adding. The H3 accept task parks on its waker/shutdown; H3 request futures
  park on the QUIC connection. Three backlog entries deleted. The CPU test exposed a
  pre-existing executor busy-spin: a worker hosting only a readiness-parked task returned
  `CanProgress` every loop and `block_on` did not yield on it. Fixed by sleeping the readiness
  quantum in `block_on`'s `CanProgress` arm when nothing is runnable. PR number: see the PR.

- 2026-10-28: two new features added from bugs found during the feature 01 review. Feature 26
  (native reactor tests on macOS, `libc::pipe2`/`/proc/self/fd` portability) and feature 27
  (`SleepIterator` zero-duration deadline). Both are pending, size S, no dependency.

- 2026-10-28: feature 01 review fixes landed on `fix/io-01-valtron-parking`. The two blocking
  findings (per-poll callback growth in `TaskStatus::from_stream`/`prime`; full-queue producer
  callback pile-up) and the minor finding (lock on every pop) are fixed via `WakeableStream::announce`,
  `WakeRegistry::replace`, and a `has_waiters()` fast path in `NotifyQueue::pop`. Three new tests in
  `tests/valtron/wakeable_stream.rs` pin each fix. All valtron tests pass in both feature sets.

- 2026-10-28: feature 01 (valtron parking) done on `fix/io-01-valtron-parking`. `ThreadedIterFuture`
  parks on `Pending` and on a full queue; async stream lenses prime before the first pull; the
  `smol/tokio_drives_parked_future_task` hang (without `multi`) is fixed in
  `DrivenStreamIterator::wake`. PR number: see the PR.

- 2026-10-28: features 12–25 added: every other backlog section (6–21) and the four bugs
  recorded only on the branches of PRs #157 and #158 now have a feature. Spec total: 25.
- 2026-10-28: spec written by the main session. Baseline: PR #160 (backlog sections 1–5
  verified; correctness fixes; CI green). Readiness work in progress is on branch
  `fix/foundational-readiness` (commits `31467ebd4`, `ed13119db`, and a work-in-progress commit
  `fcbf523fa` with one hanging test).
- 2026-11-02: feature 60 E1 (first pass, lane B, branch `fix/io-51-60-lane-b`): checked the four §11 entries against code and deleted them (D1 `RateLimiterStore` is one atomic upsert with `RETURNING`; KV documents best effort; migration 027 rebuilds old `rate_limits`; no public wasm `*_async` left; `[packages.examples]` in `ci/packages.toml`). Kept the §16 `foundation_reactor` publish note: crates.io latest `foundation_core` is 0.0.6, so 0.0.7 is not published. Feature 60 stays pending (E3 at the end of lane B).
- 2026-11-02: feature 51 (lane B, branch `fix/io-51-60-lane-b`) done. T0: open on master (the new tests fail there: "cannot start a transaction within a transaction"; a concurrent write is rolled back with a failed batch). `LibsqlStore` and `TursoStorage` keep one connection behind a shared `async_lock::RwLock` (`native/conn_gate.rs`): reads share it; writes, raw SQL and batches (`BEGIN` to `COMMIT`/`ROLLBACK`) are exclusive; row streams hold their guard until they end or drop. A failed `ROLLBACK` is returned with the statement error and marks the store; the next operation checks `is_autocommit()` (both backends have it) and rolls back before it runs. Six tests per backend in `tests/sql_batch_lock_tests.rs`.
- 2026-11-02: feature 51 review fixes (lane B). The connection lock now has a `lock_timeout` (default 30 s, `with_lock_timeout`) that ends a nested-call mistake with `StorageError::LockTimeout { mode, waited }`. Raw `query` takes the read guard for a single `SELECT`/`EXPLAIN` (text rule: neither `libsql` nor `turso` exposes `sqlite3_stmt_readonly`) and the write guard otherwise. `inject_rollback_failure` is behind the `test-hooks` feature (off by default; the crate's tests turn it on through a self dev-dependency). Caller audit found three streams kept open across a later call on the same store: `MigrationRunner::run_async`, the default `StateStore::delete_by_prefix` and `MemoryCoordinator` audit scan; all three now end the stream first. No caller in a lane A crate needed a change. Five new tests per backend (20 in `sql_batch_lock_tests`).
- 2026-11-02: feature 52 (lane B, branch `fix/io-51-60-lane-b`) done. T0: open on master (`execute_batch_statements` joined the SQL with `"; "` and pooled the parameters; the new test `d1_batch_sends_one_entry_per_statement` fails there: the request had `sql` and no `batch`). The generator now produces the D1 and R2 wire types: `foundation_openapi` gains spec overlays (`overlay.rs`, `Pipeline::overlay`), `oneOf` as an untagged enum for named schemas (`UnifiedGenerator::with_typed_unions`, `TypeBody::Union`) and `AnalysisOptions::hoist_to_shared`; `foundation_codegentools::cloudflare_wire_types` sets the Cloudflare lists, and the CLI and the `foundation_deployment_cloudflare` build script both call it. Overlays: `artefacts/cloud_providers/cloudflare/overlays/r2-list-objects.json` (adds the endpoint) and `d1-query.json` (params accept any JSON scalar; `changes` and `rows_written` are integers). `foundation_cloudflare_types/src/lib.rs` has no hand-written type left. `D1Store` sends `D1BatchQuery::MultipleQueries { batch }` with each statement's own params, and `execute_sql_async` uses the generated request type (no `json!` body). A fix found on the way: `format_files` ran `rustfmt` with the repository `rustfmt.toml`, whose `ignore` list skipped the generated crates, so an in-tree regeneration was unformatted while `Codegen::check` compared formatted code; it now runs with an empty configuration. `worker-stub.js` supports the `batch` shape (`env.DB.batch`), and `d1_batch_rolls_back_on_failure_wrangler` passes against `wrangler dev` 4 (a failed third statement leaves no row). Four backlog entries added (§11): hosted D1 REST params unproven, NaN to NULL, uncapped response read, slow wire-type check.
- 2026-11-02: feature 54 (lane B, branch `fix/io-51-60-lane-b`) done. `patches/boringtun` is now the workspace crate `backends/foundation_boringtun` (package `foundation_boringtun` 0.7.1, `[lib] name = "boringtun"`, BSD-3-Clause kept, `NOTICE` names the wasm clock backend and manifest changes). `foundation_wireguard` depends on it by `path` + `version` (its only change: that one `Cargo.toml` line); the `boringtun` line is gone from `[patch.crates-io]`, which now holds only `gemm-common`. For `cargo package` it has description, repository (workspace), keywords, `include`; `hex`, `ip_network`, `ip_network_table` are optional (`device`, `ffi-bindings`), `libc` is non-wasm32 only, and wasm32 gets `getrandom` with `js`, so the crate builds alone for wasm32 and has no unused-dependency warning. `rustfmt.toml` ignores the crate. `gemm-common` stays patched; `patches/gemm-common/UPSTREAM.md` holds the issue text and the two-line diff; backlog §10 has the entry. fp16: `.cargo/config.toml` now uses `+fp16` (rustc 1.97 `--print target-features` lists `fp16`; `+fullfp16` warns "unknown and unstable"; both set `target_feature="fp16"`). Only the macOS CI job can confirm the candle build. Backlog §10 `fullfp16` entry deleted; §16 gained an entry for the upstream inline tests and nine bare `#[ignore]`s in the boringtun fork. Tell agent A: `foundation_wireguard` manifest line changed.
- 2026-11-02: feature 55 (lane B, branch `fix/io-51-60-lane-b`) partial. E1: group `lib.rs` shims go through rustfmt; the 43 Cloudflare `ignore` entries are gone; output regenerated in its own commit. E2: clippy at zero for `foundation_openapi` (130 to 0) and `foundation_codegentools` (71 to 0, both feature sets). E3: native-only deps moved; the default-feature wasm32 check still fails in `foundation_core` (pre-existing; backlog §21). E4: the wire-type test takes 12.4 to 13.5 s (was 147 s) through a `SchemaIndex` and `check_shared_types`; full `check()` 16.7 s.
- 2026-11-03: feature 68 (lane B, branch `fix/io-51-60-lane-b`) done. T0: 2026-11-02T23:51:58Z. Before the fix 12 of the 13 tests that compile on old code failed (the passing one passed only by accident): `tos_accept_ignores_user_id_in_body` got 200 `{"status":"accepted"}` for an unauthenticated accept with a victim `user_id` (the vulnerability), and `login_with_unaccepted_tos_returns_206_and_no_session` and `tos_code_completes_the_login` got 400 "Missing client_id". **Security fixes found on the way and fixed here:** (1) `POST /auth/v1/mfa` accepted any `challenge_id` and any code and answered "authenticated" (authentication bypass); it now needs a real single-use challenge issued after the password, and a wrong code counts as a failed login (lockout). (2) `login` was unreachable over HTTP: `dispatch` matched `ends_with("/authorize")` first, so POST `/auth/v1/oidc/authorize` ran the OIDC `authorize` handler (400 "Missing client_id"); the F02 branch is now first. Also: no path created a session (they answered 202 with no cookie); now every path ends in `IdpHandlerCore::complete_login`, which applies the ToS gate and then creates the session (202, `Set-Cookie`, `Location`). Gated: password login, MFA, WebAuthn finish, passkey finish, social callback, and `tos/accept` by code (device code has no approve handler). ToS codes: 256 bits base64url, only SHA-256 stored (`tos:code:<hex>` in the cache), bound to user, version and pending login, `IdpConfig::tos_code_ttl` default 10 min, deleted before use. ToS versions moved from a per-route in-memory list to the shared cache. `foundation_db` migration 028 creates `tos_acceptances` (the table did not exist anywhere; every acceptance query failed with "no such table"); the migrations test counts were updated (and the stale `26` fixed to `27`). Login checks `redirect_uri` against the client. 15 tests in `backends/foundation_auth/tests/integration/tos_login_gate/`. Eleven backlog entries added (§25).
- 2026-11-03: feature 73 (lane B, branch `fix/io-51-60-lane-b`) done: six security follow-ups of feature 68, one commit each (E5 and E6 share one commit). E1: `KeyValueStore::take` (foundation_db): one lock for memory, memory-json and json-file; one `DELETE ... RETURNING` for Turso, libsql, D1 REST and D1 wasm; Workers KV has no conditional delete and keeps the documented non-atomic default (backlog §25). `TosService::take_code` and `MfaService::take_challenge` use it; test with a slow-`get` cache: 4 of 50 concurrent takers got the code before, 1 after. E2: no backend has expiry, so every cache entry carries `expires_at`; `HandlerStorage::sweep_expired` / `IdpServer::sweep_expired` delete the expired ones (to be run on a timer, backlog §25). E3: TOTP enrollment endpoints (`/auth/v1/mfa/totp/enroll/start` and `/confirm`, session required; a replacement needs `current_code`); secrets encrypted with ChaCha20-Poly1305, key ring from `IdpConfig::totp_keys` (`TotpKeyRing`, key id stored with the ciphertext, retired keys for rotation, no key means enrollment fails closed); the last accepted time step is kept per user, so a code works once. E4: `social_authorize` needs `client_id` and an exact registered `redirect_uri`; `IdpServer::with_social_login`. E5: `logout` revokes the cookie session and clears the cookies. E6: failed attempts reset after a full login. Backlog: six entries deleted, six new ones added (§25). Regression evidence: E1 4 vs 1 (above); E2 and E3 and E4 tests needed new API (compile error before); E3 replay test fails when the step check is removed (202 instead of 401); E4 test fails when the registered-URI check is removed (404 instead of 400); E5 and E6 tests failed on the old code (attempts_remaining 2 instead of 4).
- 2026-11-03: feature 54 E4 and E5 (lane B, branch `fix/io-51-60-lane-b`). E4: the library is `foundation_boringtun` (`[lib] name` changed; 12 `boringtun::` paths in `foundation_wireguard` and `foundation_nativeapis` renamed, path-only). NOTICE and README updated. E5: `patches/gemm-common/src/gemm.rs` now picks the x86 kernels with `V3::try_new()` / `V4::try_new()` (the upstream change, gemm-common part; blob `fd37141`). `UPSTREAM.md` says the upstream PR is prepared on branch `fix-fma-without-avx2`, link pending. `gemm-f16` 0.19.0 is in the graph through candle-core and is unpatched; backlog §10 records it. Warnings seen in the verify runs are in backlog §2 and §16.
- 2026-11-03: feature 54 E6 (lane B, branch `fix/io-51-60-lane-b`). `gemm-f16` 0.19.0 is vendored as `patches/gemm-f16` (copied unchanged from the crates.io release, except `src/gemm.rs`) and added to the root `[patch.crates-io]`. Its `init_gemm_fn` now dispatches on `gemm_common::simd::V3::try_new()` / `V4::try_new()`, the gemm-f16 part of the upstream change (blob `e8200e9`). Checks: `cargo check -p gemm-f16@0.19.0 --profile dev` exit 0, no warnings; the `x86-v4` build in a scratch copy with the patched gemm-common, exit 0. `Cargo.lock` now has no registry source for `gemm-f16` 0.19.0 (path patch). Backlog §10 keeps only the upstream link. Feature 54 stays `done`.
- 2026-11-03: feature 71 E2 to E4 done (lane B, branch `fix/io-51-60-lane-b`; E1 was done earlier). E2: `derive_more`, `foundation_macros`, `tracing` are native-only in `foundation_deployment_cloudflare` (the `workers` module uses none of them); `js-sys`, `wasm-bindgen`, `serde`, `serde_json` are optional wasm32 entries enabled by `workers`. The wasm32 check passes with no warning for default features and for `--no-default-features --features workers`; the native check passes. E3: `clippy-deny-warnings = true` in `ci/packages.toml` runs clippy with `--no-deps -- -D warnings` (`--no-deps` because `-D warnings` otherwise reaches workspace dependencies with debt, for example `foundation_nostd`). On for `foundation_auth` (both clippy runs), `foundation_openapi`, `foundation_codegentools`. `foundation_db` has 12 (default) and 25 (`libsql`) errors, so it stays off (backlog §11). E4: `genapi generate cloudflare` took 56 s warm (46 s of it `cargo fix`, which changed nothing: generated output was already clean, also for hetzner and docker); `cargo fix` is now opt-in with `--fix`; without it the run takes 9 s and the output is byte-identical to the committed files.
- 2026-11-03: feature 56 E1 and E2 (lane B, branch `fix/io-51-60-lane-b`). E1: feature 17 E7 chose Unix milliseconds. Checked every table: all `created_at`/`updated_at` defaults are `strftime('%s','now') * 1000`; the IdP writes `timestamp_millis()` (the seconds in `token_service` are JWT claims, not table columns); `AsyncStorageCleanup` compares in milliseconds; migration 026 converts legacy seconds and is idempotent; `rate_limits` is documented seconds. Nothing disagrees, so no migration; the existing test `cleanup_deletes_expired_rows_by_documented_unit` stays. Entry deleted. E2: the SQL helpers in `core/rate_limit.rs` were gated on `wasm-bindgen-storage` on any target, but the only wasm user is compiled for `target_family = "wasm"`; a native `--features wasm-bindgen-storage` build warned "never used" (6 items). The gate now says `all(target_family = "wasm", feature = "wasm-bindgen-storage")`; the build is silent. `cargo check -p foundation_connectrpc --tests` had no such warning before or after. Entry deleted.
- 2026-11-03: feature 56 E3 and E4 (lane B, branch `fix/io-51-60-lane-b`). E3: `#![recursion_limit = "256"]` with the `foundation_db` comment in `sql_vector_store_tests`, `libsql_vector_store_tests`, `sqlite_vec_store_tests`; the test runs no longer print the future-incompatible warning. E4: `ts_raw_calls_extraction` asserted `len() >= 0` (always true). It now asserts the extracted callees contain `findAll` and `json`. The real assertion on the old TS source failed with `got []`: method ids have no class in them, so the second `handle` is dropped (new backlog §17 entry; the test uses a one-class source). `foundation_vectors` clippy with all targets and all features: 131 lines (warnings) to 0; `clippy --fix` plus manual: docs `# Errors`, `# Panics` (an `unwrap` became a `let ... else`), a `FileContext` struct for an 8-argument function, an `ExtraWalkFn` alias, unused parameters removed from the Leiden helpers, `u16` loop counters with `f32::from` in the tests. `ci/packages.toml`: `foundation_vectors` runs and clippy-runs now include `code-graph`, and `clippy-deny-warnings = true`.
- 2026-11-03: feature 56 E5 (lane B). `data_value_to_json` returns `Result`; a NaN or infinite `Real` gives `StorageError::SqlConversion("non-finite REAL parameter ...")` (no new variant needed) in `query`, `execute` (which had a second copy of the mapping; it now uses `params_to_json`) and batches, before any request. Test `d1_rejects_non_finite_real` (`tests/d1_response_tests.rs`, mock counts requests). Before: `execute` with `Real(NaN)` returned `Ok(0)` (`called Result::unwrap_err() on an Ok value: 0`, line 129). After: passes.
- 2026-11-03: feature 56 E6 (lane B). `D1Store::post_query` checks the status first. A non-200 reads at most `max_error_body` bytes (default 8 KiB, builder `with_max_error_body`) into the error message. A 200 body is read chunk by chunk and ends with `StorageError::Backend("... larger than max_response_body ...")` as soon as the total passes `max_response_body` (default 16 MiB, builder `with_max_response_body`; D1 documents a 2 MB row maximum and no result maximum, so the default is eight rows; I could not re-fetch the limits page, the session blocks developers.cloudflare.com). Tests in `tests/d1_response_tests.rs`: `d1_response_over_cap_is_an_error`, `d1_response_cap_is_inclusive`, `d1_error_body_is_read_up_to_its_cap_only`. Before: the old code has no cap or builder, so the test file does not compile (`no method named with_max_response_body`); the old path read the whole body with `collect_string_async`.
- 2026-11-03: feature 56 E7 (lane B). `FileStateStore::set` writes `.{name}.{pid}.{n}.tmp` in the target directory (`create_new`, `write_all`, `sync_all`), renames it over the target, then fsyncs the directory (Unix); a failed write removes the temp file; the temp name does not end in `.json`, so `list` and `count` never show it. Test `file_state_store_write_is_atomic` (`tests/state_store_tests.rs`): a reader thread calls `get` and `count` in a loop while 200 `set`s replace a 512 KB resource. Before: `after 16 reads: ["torn read: Serialization error: EOF while parsing a value at line 1 column 0", ...]`. After: passes. The missing conditional write is recorded as a new backlog §11 entry (not built here).
- 2026-11-03: feature 56 E8 (lane B). Wrangler 4.156.2 (installed outside the repo in `/tmp/claude-0/wr`) runs `wrangler dev --local` with the stub here. Probe of the D1 binding (workerd): `bind(7)` gives `typeof(?1)` = real (a JS Number is always REAL; integers become INTEGER only through column affinity), `bind(7n)` throws `D1_TYPE_ERROR: Type 'bigint' not supported`, `bind(1.5)` real, string text, null null, `true` real 1. So the old stub stored `7` as REAL. `worker-stub.js` now reads the request with a `JSON.parse` reviver that keeps the literal's source text (`7` is an integer, `2.0` a real), rewrites the placeholder of a JSON integer to `CAST(?N AS INTEGER)` (an integer outside the double-safe range is bound as its decimal text), and cites the D1 "Type conversion" table (Workers Binding API) in a comment; the doc page could not be fetched here (network), the mapping is recorded from the doc as I know it and from the probe. Tests (`tests/d1_param_types_wrangler.rs`, columns without a type so no affinity): `d1_param_types_wrangler` (`[7, 1.5, null, "x"]` gives integer, real, null, text; `2.0` stays real; bare `?`), `d1_batch_with_mixed_types_wrangler`, `d1_i64_extremes_round_trip_wrangler` (`i64::MAX`, `i64::MIN`, 2^53+1 read back as exact text, `typeof` integer). Before (old stub): `left: ["real", "real", "null", "text"] right: ["integer", "real", "null", "text"]`, all three fail. After: 20 of 20 runs pass; `d1_batch_wrangler_tests`, `d1_kvstore_tests`, `d1_state_store_tests`, `d1r2_conformance_tests` still pass. `d1r2_document_store_tests` fail 2 of 4 with the old and the new stub alike (new backlog §11 entry). Backlog §11 "hosted D1 REST unproven" deleted; the `d1-query.json` overlay note now says verified with local wrangler 2026-11-03 and states the limit.
- 2026-11-03: feature 56 E9 and close (lane B). `foundation_db` clippy with all targets: default 34 lines (12 in the library, the rest in tests) to 0; `libsql` to 0 (library errors counted by feature 71: 12 and 25). Fixes: `# Errors` sections, backticks, a `SpreadItems` type alias, `turso_row_to_sql_row` takes `usize` (no casts), `i32::try_from` for the libsql column index, `write!` instead of `push_str(&format!)`, one `#[allow(clippy::too_many_lines)]` with a reason on a mock request handler. `ci/packages.toml`: `foundation_db` has `clippy-runs = ["", "libsql"]` and `clippy-deny-warnings = true`; `ci.py run foundation_db --step clippy` exits 0. `ci.py run foundation_db --profile dev` exit 0 (fmt report-only), `ci.py run foundation_vectors --profile dev` exit 0. Flake runs (20 each, 0 failures): `d1_response_tests`, `file_state_store_write_is_atomic`, `cleanup_deletes_expired_rows_by_documented_unit`, `d1_param_types_wrangler` (with `CF_INTEGRATION_TEST=1`). Feature 56 is done; backlog §11 clippy entry deleted.
- 2026-11-03: feature 74 (lane B, branch `fix/io-51-60-lane-b`) code done, one commit per item, pushed after each. E8 session cookies are `Secure`, `HttpOnly`, `SameSite=Lax` (`IdpConfig::secure_cookies`, default on; the `session_data` cookie value was raw JSON that header parsers split and is now base64url). E7 failed TOTP enrollment codes count toward the lockout (423) and `enroll/start` has its own TTL. E6 TOTP is SHA-1 by default (RFC 6238 appendix B vectors verify for SHA-1 and SHA-256), SHA-256 optional, the algorithm is stored with the secret and named in the `otpauth_uri`; old records stay SHA-256. E10 the social authorization code carries its `client_id` (the token endpoint also looked the user up by email instead of id; fixed). E9 `IdpServer::start_sweeper(shutdown)` runs `sweep_expired` as a valtron task every `sweep_interval` (default 5 min). E1 a missing username is stored as NULL. E5 `023_create_vectors.sql` deleted (foundation_vectors owns that table with an identical file and its own runner); a test checks every SQL file is registered. E11 `KeyValueStore::take_is_atomic`; `IdpServer::try_new` refuses a cache without it unless `allow_non_atomic_cache`. E4 a social login that needs the ToS redirects to `IdpConfig::tos_page_url` with the code in the fragment (206 JSON without it). E3 `handlers/routes.rs`: one static (method, pattern) table, whole-path matching, 404 otherwise. E2 sessions are stored in the IdP cache through `KvCredentialStore`, signed with `IdpConfig::session_key`, so all routes share them and they survive a restart with a persistent cache and a stable key. All eleven backlog entries deleted.
- 2026-11-04: feature 76 partial (lane B, branch `fix/io-51-60-lane-b`): E3, E4, E1b and E2 done; E0 and E1 not started (feature 76 stays pending). E3: a method id is built from its parent (class, impl, trait, Go receiver), so two classes with a method of one name keep two nodes and the second body is walked; `ts_raw_calls_extraction` has its original two-class source back (before: `expected a raw call to findAll, got []`); Go methods now attach to the receiver type (the type was read from the wrong node). E4: `StateStore::set_if_version(id, state, expected_updated_at)`; atomic on the file store (flock around read, compare, rename), libsql, D1 (one statement) and namespaced; `R2Store` keeps the documented non-atomic default; 20 racing writers, one wins. E1b: `D1Store` sends string parameters only and types them in SQL (`CAST(? AS INTEGER)`, `CAST(? AS REAL)`, the `NULL` literal, `unhex(?)`; `?N` renumbered when a NULL is inlined), reads through `/raw`, and takes the whole API prefix (`hosted_api_prefix(account)` or wrangler's `/cdn-cgi/local/explorer/api`); R2 keys are one percent-encoded segment and the explorer's string `is_truncated` is accepted; the generator hoists `d1_raw_result_response` and the overlay no longer widens `d1_params`; `worker-stub.js` is deleted; the CF tests (foundation_db and foundation_nativeapis `d1_delta_vfs`, `r2_delta_vfs`) run on the explorer API with the pinned `WRANGLER_VERSION` 4.156.2 in `mise.toml` (the `package.json` wrangler stays 4.81.1, which has no explorer API). E2: the cause was column order lost in JSON row objects (fixed by `/raw`) plus a test that read a NULL `r2_key` as `String` (NULL reads as ""); the test now reads `Option<String>`. 20 of 20 runs for each new or changed test file. `foundation_nativeapis` `vfs-preload` does not compile (pre-existing, backlog §11), so `vfs_shim_vfs` was not run. R2 range read and conditional put are not served by the explorer: they belong to the E0 harness on the real binding.

