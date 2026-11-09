# Learnings — medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. Updated after each batch. See
`.agents/skills/specifications-management/skill.md`._

## 2026-11-09, issue #257: the reset is gone, the stop is not

- `quinn-proto` frees a recv stream's state when it reads FIN and drains. A later
  `RESET_STREAM` is dropped with "received RESET_STREAM on closed stream". Do not build
  cancellation on a reset after the body ends. Use `SendStream::stopped()`, which reports
  the peer's `STOP_SENDING` (RFC 9114 section 4.1.2).
- A client that sends `STOP_SENDING` frees its own receive state too. It then reads the
  end of its stream as an error, not as `Terminated`. A test must not demand the reset
  code on that side; assert that no response starts.
- `std::pin::pin!` keeps the borrow alive until the enclosing block ends, not until the
  `match`. Put the probe in an inner block and return an owned outcome, or the later use
  of the borrowed value fails to compile.
- An eager `select` between a handler and a probe needs one test per dispatch path: unary
  and streaming code are separate. Each test must time out on the old code; both did.

## 2026-11-09, issue #409: the move found hidden must-use discards

- A feature-gated CI lint (`--all-features`) reported eight `unused_must_use` warnings
  that the default build never showed. Run the gate the merge target runs.
- A worktree agent branched from master, not from the integration branch. Its three
  merge conflicts were comments and one visibility change. Read each conflict; do not
  take one side whole.

## 2026-11-09, issue #290: a soak must keep the panic text

- A flake at 2% needs about 150 runs to fail once with high probability, and it can fail
  once in 50 and then not in 200. A negative soak is weak evidence, and only a positive
  one proves the test can fail.
- The first soak script kept only the `test result` line. The second kept the full
  output but did not fail. Print the test's own stderr in every soak iteration; a lone
  `error: test failed` wastes the whole run.
- Never compare a soak that ran under a different load with one that did not. The h3
  timeout is a 15 s wait, so a busy machine is part of the input.

## 2026-11-09, issue #428: an optional feature needs a compiled check

- Code behind an optional feature is invisible to every default build. The macOS watcher
  did not compile, and no job noticed, because no job named `watcher-macos`. A feature
  without a check rots. Add the `cargo check` to CI in the same change that touches it.
- The backend reached into `Registry::selector` while `Registry` already had public
  `register_vnode` and `deregister_vnode` calls. Read the public surface before a private
  access. A private access ties the backend to the reactor's internals.
- `event.ident()` is a `usize`. Do not cast it to `i32`; a value that does not fit would
  name a different fd and report the wrong path. Use `i32::try_from` and skip the event.
- A single kqueue event can carry several flags, and a delete also sets the write flag.
  Test `NOTE_DELETE | NOTE_RENAME | NOTE_REVOKE` first, so a delete does not report a
  modification.
- The signal module treats iOS as "not macOS": every kqueue gate names `target_os =
  "macos"` alone. The whole delivery path is then dead code on iOS (issue #431). A target
  family list belongs in one place, not in every `cfg`.

## 2026-11-09, issue #430: a false registration means the wait cannot end

- `FdReadiness::register_waker` returns `false` when the reactor no longer holds the fd.
  In that state no wake can arrive. A park site that ignores the value parks forever; a
  timeout-bounded site reports a timeout for work that finished. Map `false` to a
  terminal `BrokenPipe` error. This is the same defect that issue #424 fixed in
  `foundation_connectrpc`.
- A `#[must_use]` result is not always live data. The accept loop snapshots the event
  generation again before each `accept`, so the value from `wait_for_events` has no later
  use. Bind it and write the reason. A silent discard leaves the reader to guess.
- Local clippy warnings do not fail a crate unless `ci/packages.toml` sets
  `clippy-deny-warnings` for it. `foundation_proxy` has no such entry, so three
  `unused_must_use` warnings lived in the tree. Check the CI entry before a crate's
  warning count is called clean.

## 2026-11-09, issue #411: the shared tree must not hold host-process code

- A folder named `shared` is a promise. A `#[cfg(not(target_family = "wasm"))]` marker
  inside it hides a native dependency; it does not remove it. Move the code to `native/`,
  and the marker becomes unnecessary.
- A dead cfg marker survives because no supported build takes the other branch. The proof
  of the fix is a real wasm build, not a comment in the source.
- `native::vfs` was declared only for `vfs-native`, but the CLI searcher needs only
  `vfs-search`. Widen the parent gate to `any(vfs-native, vfs-search)` and add
  `vfs-native` to each existing child. The two trees then stay independent.
- The crate root needs the gate too. `native` and `signal` call operating-system APIs.
  Without a crate-root gate, `pub use native::...` fails on wasm even when the module is
  absent. Gate every re-export of a gated module.
- `--all-features` is not a build that the crate supports. It fails on master because
  `native/watcher/unix.rs` calls the private `foundation_reactor::poll::Registry::selector`
  (#428) and several feature-gated modules have `unreadable_literal` warnings (#429). Lint
  the two feature trees that CI enforces instead.
- `EXCLUDED_DIRS` is shared policy: the in-code walk and the CLI searcher must skip the
  same directories. Keep one `pub(crate)` constant in `shared`, not a copy in `native`.
- A crate version bump also stales every example lockfile that reaches the crate. Refresh
  `devbinary`, `cf-agent-loop`, `cf-cloud-provider` and `web/wello` offline with
  `cargo metadata --offline`, and check each with `--locked`.

## Issue #412 (2026-11-09)

### A native-only type must leave `shared/`, not gain a target gate

- `Serve` names `foundation_netio::netcap::RawStream`. netio exports `RawStream` on
  native targets only. A `#[cfg(not(target_family = "wasm"))]` marker in `shared/`
  hides the dependency. It does not remove it.
- Move the type to `native/`. `lib.rs` already gates the whole `native` module, so the
  item leaves the wasm build without a second marker. `shared/` then has no target gate
  for it.
- An inherent `impl HttpApp<...>` can live in another module of the same crate. The
  generic `HttpApp<S>` stays in `shared/app/mod.rs`; the native impls move to
  `native/app.rs`.

### A `pub(crate)` re-export of a native-only helper warns on wasm

- `pub(crate) use health::health_body;` in `shared/handlers/mod.rs` is unused when the
  `native` module is not compiled. The wasm check reports an unused import.
- Make the module `pub(crate) mod health;` and import the helper by its full path from
  `native/handlers/health.rs`. Keep `shared/` free of target gates.

### A wasm test needs a target that can build it

- `http_app_generic_tests` used both `CfServe` and `WebServe`. `CfServe` needs
  `wasm-bindgen-http`. On the host, `wasm-bindgen` does not define `JsError`. On
  wasm32, the crate's dev-dependencies (socket2 and others) do not build. So the file
  did not compile in any run.
- The browser `WebServe` path needs no wasm-bindgen. Its tests now live in
  `tests/wasm_web_serve_tests.rs`. They run on the host in the default test run and
  pass.
- The wasm32 library check with `--features wasm-bindgen-http` is the compile proof for
  `CfServe`. A runnable `CfServe` test needs the wasm test target from issue #364.

### A template manifest pins the workspace version too

- The `bin/platform/templates/*/Cargo.toml.template` files name workspace crates by
  version. The template build test maps them to local paths with `[patch.crates-io]`.
- For 0.0.x versions, cargo treats each patch release as incompatible: `^0.0.18` does not
  accept 0.0.19. A stale pin fails with "failed to select a version for the requirement".
- A crate bump must update the template requirement too. The three templates now name
  `foundation_core` 0.0.19.

### Version bumps still need the example lockfiles

- The `foundation_http` bump updated the workspace lockfile. The refresh also removed a
  stale `uuid` entry from the example lockfiles. The manifest dropped `uuid` in an
  earlier change, but those lockfiles were not refreshed then.

## Batch 2, issue #426: a WASI context is part of the side-effect boundary

- `WasiCtxBuilder` has defaults for every side effect: the host wall clock, the host
  monotonic clock and the process RNG. An empty standard stream, environment and argument set
  is not an injected context. Pass `wall_clock`, `monotonic_clock` and `secure_random` from
  the `Ctx`.
- `HostWallClock::now` returns `std::time::Duration` since the UNIX epoch.
  `HostMonotonicClock::now` returns nanoseconds. `Clock::wall_now` and `Clock::elapsed` are
  the two sources.
- `cap-rand` and `foundation_compact` use different `rand_core` versions. A newtype
  implements `cap_rand::RngCore` over `ChaCha8Rng`; one type cannot implement both traits
  with one set of methods.
- `wasmtime-wasi` fills a `random_get` request with `sample_iter(Standard)`. For a byte, the
  standard distribution takes the low byte of one `next_u32` draw. A test that expects eight
  stream bytes fails after the first byte. Compare against eight low bytes.
- Make the proof exact. A `SimulatedClock` with a known wall time and a fixed RNG seed gives
  one expected value for each call. The old code fails every assertion, so the test names the
  defect and not a side effect.
- Seed the bundle at the host entry point. `PlatformSession::new` fills a 32-byte seed from
  `foundation_compact::entropy::fill`; `new_test` keeps seed `0`. A test helper must not read
  the operating system entropy source.

## Batch 2, issue #416: a `std` guest imports WASI, a `no_std` guest does not

- A guest built for `wasm32-wasip1` with `std` imports functions from `wasi_snapshot_preview1`
  even when the application does not call them. Wasmtime resolves every import at
  instantiation, so a host that links only its own namespace fails with a link error.
- Add the preview 1 context through `preview1::add_to_linker_sync`. The store value becomes a
  private `HostState` that holds `WasiP1Ctx`. `WasiCtxBuilder::new().build_p1()` gives an
  empty context: no streams, no environment and no arguments.
- `wasmtime-wasi` 26 lists `preview1` as its only and default feature, and it depends on the
  same `wasmtime 26.0.1`. Build it with `default-features = false, features = ["preview1"]`
  to keep the feature set small and the version unified.
- Test a link defect with a WAT module. Declare the import and also call it once, then
  discard the errno. That proves the import is served, not only named. The old code fails at
  `Linker::instantiate`, so the test names the defect directly.

## Batch 2 (2026-11-09)

### A target-gated backend needs its crate declared for that target (issue #427)

- The `wasi_p2_3` backend splits on `target_env`: `p2` imports `wasip2` and `p3` imports
  `wasip3`. Neither crate was in the manifest, so the target did not build.
- A target-specific dependency is resolved into every lockfile that takes the crate by path,
  even when that target is not built. Add the dependency under the same `cfg` as the code,
  then refresh the example lockfiles.
- `wasip2` 1.0.4 and `wasip3` 0.9 both default to `std`. Use `default-features = false` for a
  `no_std` substrate; the `random` module needs neither `std` nor `bitflags`.
- `wasip3` 0.9 requires Rust 1.87. That equals this workspace's minimum, so the dependency
  stays inside the supported range.
- A WASI check does not prove a WASI target. Install the target, then check it. The pinned
  nightly lists `wasm32-wasip3`, and `rustup target add` installs it.

### A `macro_rules!` cannot emit a match arm; a cfg alias can carry the rule

- A macro call in match-arm position is parsed as a pattern. The compiler reports "expected
  pattern, found `#`" and "this macro call doesn't expand to a pattern". One macro cannot emit
  `pattern => body` into an enclosing `match`.
- A named cfg alias keeps one rule behind many arms. Add a `build.rs` that prints
  `cargo::rustc-check-cfg=cfg(<name>)` and, when the rule holds, `cargo::rustc-cfg=<name>`.
  This repository already does this for `cranelift_backend` in `foundation_buildtools`.
- Build scripts see `CARGO_CFG_TARGET_FAMILY` and `CARGO_CFG_TARGET_OS`. `wasm32-wasip1` and
  `wasm32-wasip2` both have `target_family = "wasm"` and `target_os = "wasi"`. WASI preopens
  give them `std::fs`.
- Check the build script's choice by reading
  `target/<target>/<profile>/build/<crate>/<hash>/run/stdout`. It shows the cfg that the target
  got.
- `wasm32-wasip2` has `target_env = "p2"`. `foundation_compact`'s `wasi_p2_3` entropy backend
  imports an undeclared `wasip2` crate. So a `foundation_db` wasip2 check stops in that
  dependency (issue #427). Do not read a wasip1 pass as a wasip2 pass.

### A `Waker` is `Send` by type, so its data must be thread-safe by construction

- `std::task::Waker` is `Send + Sync` whatever its vtable holds. The non-`multi`
  build put a `WakeCallback` with no `Send` bound behind `Waker::from_raw`, so safe
  code could run or drop the callback on another thread (issue #423).
- Making `ReadinessWaker` `!Send` with `PhantomData<Rc<()>>` does not work here: the
  reactor shares an `Arc<ReadinessNotifier>` with its drain thread, so the notifier
  must stay `Send`.
- The fix keeps the waker `Send` and confines the callback instead. `ThreadBound<T>`
  is `Send + Sync` for every `T` because no other thread reads or drops the value:
  its accessor returns the value only on its owner thread, and `Drop` skips the value
  on any other thread (a leak is safe). A wake on another thread is dropped.
- The trait bound did not change. Only the private `from_callback` path uses this.
- Test the defect through public API: the split observer's `QueueReadiness` is `Send`
  when its item type is `Send`, so a test can push from another thread.

### A fix with no regression test lets the tracker go stale

- Seven of the nine issues in this batch were already fixed at `4ca007301`. The open
  tracker did not show that. Each fix had its commit, but only some had a test that names
  the defect.
- Before a fix is called done, run its regression test against the code without the fix.
  Record that result in the commit or the issue. A tracker entry then has a fact behind it.
- #291 had no direct test at first. The bridge test that showed the defect
  (`fast_producers_never_strand_a_consumer`) timed out at 60 s on the old code and takes
  under 2 s with the fix. That test is the regression test.

### A `Waker` built from a non-`Send` callback must stay on one thread

- `Waker` is `Send` and `Sync` by type, but `Waker::from_raw` expects thread-safe data.
  A non-`Send` callback inside the data breaks that promise if the waker crosses a thread.
- The non-`multi` build has no `Send` bound on `WakeCallback`. The single-threaded executor
  owns the waker, so the callback stays on that one thread. The type must not move.
- Use `#[cfg_attr(not(feature = "multi"), expect(clippy::arc_with_non_send_sync, ...))]`
  and state the single-thread rule in the doc comment.
- The type cannot show the rule. `ReadinessWaker` is still `Send` and `Sync`, so safe code
  can move it. Issue #423 records this risk and its likely fix.

### A no-progress requeue is not urgent work

- `TaskStatus::Ignore` maps to `State::Pending(None)`. The executor pushed that entry to the
  front of its queue, so a task that self-wakes and makes no progress ran again before any
  other runnable task. One such task could starve its worker.
- A requeue with no progress goes to the back of the queue, like other runnable work. Do not
  add a fairness counter until a real case needs one.

### Test a queue-order defect with a bounded number of ticks

- The no-progress task never ends, so `run_until(NoWork)` cannot drive the test.
- Schedule the no-progress task first and a finite sibling second. Run a fixed number of
  `run_once` ticks and assert that the sibling finished. The old order never runs the
  sibling. The new order gives it a turn after every no-progress poll.

### A CPU bound in a test must include its own bookkeeping (issue #425)

- The idle-CPU test measured every `valtron-worker` thread and required under 2 ms of CPU
  in one second. The Linux container measured 2.3 to 2.7 ms. The workers made 8 to 9
  wake-ups, which is the documented idle rate, so they did not spin.
- One wake costs about 0.25 ms of CPU in the unoptimised `uat` build. The 2 ms bound had no
  room for the wake cost and the container scheduler.
- Raise the bound to 10 ms. This is 2.7 times below the old defect (about 27 ms) and 3.5
  times above the measured steady state. Keep the wake-up bound at 30, because the defect
  made about 900 wake-ups.
- Measure a bound against the defect it must catch, not against a best-case run.

### Read the status before you read the body (issue #410)

- The unary client read the whole response body, then checked the HTTP status. A 256 KiB
  error body then delayed the error and used memory for data that becomes one message.
- The fix reads the response head, then branches on the status. For a non-OK status it reads
  only `error_body_max_bytes` (default 8 KiB) and drops the stream. The error returns at
  once, and the body closes early.
- The regression test writes 64 chunks of 4 KiB through a pipe. It asserts that the message
  length is the cap and that the sender did not finish every chunk. A timer is not needed,
  because the dropped stream is the proof.
- The cap is a field of `SizeLimits` with a builder method. The default follows nginx's
  error-page buffer and `foundation_db`'s error-body default.

### A hard-coded socket timeout is configuration (issue #420)

- `Connection::connect_unix` set a read timeout of 5 seconds and ignored the result of
  `set_read_timeout`. A caller could not change the timeout, and a failed set was silent.
- The timeout is now a `ClientConfig` field with a builder method. `connect_unix` takes it
  as an argument and returns the error when the platform cannot set it.
- The test reads the timeout back from the kernel socket option. It proves that the
  configured value reaches the socket without a sleep.

### An ignored `must_use` result can be a lost wake (issues #390, #424)

- `PipeSender::close` returns a bool that only reports who closed the pipe. A dropped result
  is correct there, and `let _ =` records that choice.
- `FdReadiness::register_waker` is different: `false` means the reactor no longer holds the
  fd, so the waker will never fire. Ignoring it parks the task forever. The fix returns
  `BrokenPipe` so the poll fails instead.
- The per-crate clippy command shows residual warnings from dependency crates. CI uses
  `--no-deps -- -D warnings`, so only the crate's own warnings fail the run.

## Batch 1 (2026-11-09)

### One reactor registration per fd, not one per token (issue #421)

- The reactor keys its selector registration by file descriptor (fd). `Reactor::register`
  overwrites the fd-to-token entry. `Reactor::deregister(fd, token)` removes the selector
  filters only when `token` is the live token, but it still drops the entry of the fd.
- The HTTP idle handler cached one `FdReadiness` for the life of a connection. The RPC
  connection I/O and the response writer also register the same fd, then drop their
  registration. Such a drop removes the fd from the selector. The handler then parked on an
  fd that nothing watched, and the next request never woke it.
- The fix registers the fd at each park and lets the registration drop when the task runs
  again. The live registration then always belongs to the parked task.
- The long-term shape is one `FdReadiness` per fd, owned by `RawStream` and shared by every
  user. That is a larger change; the current fix keeps the invariant with a short lifetime.
- The failure was invisible per branch. The `foundation_http` parking test passed alone, and
  the `foundation_connectrpc` keep-alive test passed against master's `foundation_http`.
  Only the merged tree failed. After each merge, run the tests of every crate that depends
  on the changed crate.

### A task must reuse one waker across polls (issue #329)

- `ReadinessNotifier` removes duplicate registrations by waker identity. A task that builds
  a fresh waker on every poll defeats that removal, so the registration list grows with the
  poll count.
- The fix gives each future and each state task one stable waker for all its polls. The
  split observers register a one-shot callback waker on their queue
  (`ReadinessWaker::from_callback`), so they park until the producer pushes a value.

### A non-blocking connect and a TLS handshake are state machines (issues #277, #279, #418)

- `connect(2)` and a TLS handshake can block a valtron worker. Start both on a non-blocking
  socket, then finish them from socket readiness. A valtron task parks with
  `TaskStatus::Depends`; a synchronous boundary uses `SocketReadiness::wait_blocking` with a
  clock-bounded deadline.
- Each TLS backend (rustls, openssl, native-tls) reports the direction its handshake waits
  on. One task drives all three.
- The openssl and native-tls backends did not compile before this change. A feature that CI
  never builds hides this class of defect. Add every backend to a CI run.

### The clippy gate must cover the feature that holds the code (issues #386, #387, #419)

- `foundation_netio` had 74 pedantic warnings in code that only compiles with `quic`. The
  default clippy run never saw them.
- A `clippy-deny-warnings = true` entry fails CI only when a `clippy-runs` entry compiles the
  affected code. Add both the feature run and the deny entry together.

### Identifiers come from `foundation_compact::uuid`, not from a `uuid` backend

- `foundation_compact` already owns the version-4 UUID path: `uuid::new_v4()` fills 16 bytes
  from `entropy::fill` and sets the version and variant bits with `uuid`'s builder. The
  entropy backend is selected for the target (the operating system source on native, the
  `abi` host module on wasm32-unknown-unknown). This is the spec 70 rule 11 path.
- `foundation_http`'s request-ID middleware called `uuid::new_v4()` directly and enabled
  `uuid`'s `v4` and `js` features. It now calls `foundation_compact::uuid::new_v4()` and
  warns if the entropy source fails. The `uuid` dependency is gone from the crate.
- `uuid` 1.20 refuses to compile on wasm32-unknown-unknown unless one of `js`,
  `rng-getrandom` or `rng-rand` is on, even when the crate only uses the value API. The old
  `uuid`/`js` feature hid this for the whole graph. With it gone, no crate in the
  `foundation_http` wasm graph enables `uuid`'s `v4` feature, so `uuid` compiles without its
  `rng` module. Keep `foundation_compact`'s `uuid` dependency at
  `default-features = false`; do not add a randomness feature to silence a compile error.
- `#[must_use]` on `FdReadiness::register_waker` and `wait_for_events` exposed two ignored
  return values in `foundation_http`. The writer now returns `NotConnected` when the reactor
  no longer holds its registration (parking would hang). The accept loop names the ignored
  generation with a comment, because the next iteration takes a fresh snapshot.

### A crate version bump must update the standalone example lockfiles

- The plan job runs `cargo metadata --locked` for `examples/devbinary`,
  `examples/web/wello` and `examples/platform_android/src-tauri`. These manifests are outside
  the workspace and take the crates by path, so a crate version bump makes their lockfiles
  stale and the job fails.
- The `foundation_wasmtime` bump to 0.2.0 updated the workspace lockfile but missed
  `examples/platform_android/src-tauri/Cargo.lock`, which still named 0.1.1. Regenerate each
  one with `cargo metadata --offline` and check it with `cargo metadata --locked`.

### Version lines conflict on every merge; keep the highest version

- Each branch changed a crate version and its consumer requirements. The merges conflicted
  on the same manifest and lock-file lines. Resolve with one rule: keep the highest version
  of every crate, line by line. Then run `cargo metadata --offline` for the workspace and
  each example lock file.

### Restore a path from `HEAD`, not from the index

- `git checkout <rev> -- <path>` writes the file into the index as well as the worktree.
  A later `git checkout -- <path>` restores from the index, not from `HEAD`, so the old
  tree comes back.
- Use `git restore --source=HEAD --staged --worktree -- <path>` to revert a file fully.

### The `shared/` tree is target-agnostic (skills change, `bac23cf3c`)

- Nothing in `shared/` is "wasm unsafe". A `cfg` that deletes a code path on wasm means the
  code is native-only and belongs in `native/`. Do not gate it.
- Issues #409, #411, #412 and #413 record the remaining places to move.
