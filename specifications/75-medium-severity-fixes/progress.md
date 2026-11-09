# Spec 75 progress

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. One entry per batch or feature PR: date, scope, result._

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

### Verification

| Check | Result |
| --- | --- |
| `cargo test --profile uat -p foundation_core` | Pass, 0 failed |
| `cargo test --profile uat -p foundation_core --features multi` | Pass, 0 failed |
| `cargo clippy --profile uat -p foundation_core --all-targets` | Zero warnings |
| `cargo clippy --profile uat -p foundation_core --features multi --all-targets` | Zero warnings |
| wasm32 check, std | Pass |
| wasm32 check, std and js-foundation-wasm | Pass |
| wasm32 check, std and js-wasmbindgen | Pass |

### Next

- Close the nine issues when the branch merges. Seven already have their fix commit.
- Batch 2 still lists foundation_connectrpc #410, foundation_wasmtime #416 and
  foundation_netio #420.

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
