# Spec 75 progress

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. One entry per batch or feature PR: date, scope, result._

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
