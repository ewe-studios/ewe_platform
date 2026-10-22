# Learnings: Valtron Async Readiness

_Updated: 2026-10-21. Add to this after every task and before stopping work._

## Waking and idle executors

- A parked task becoming ready must wake an idle executor thread directly.
  Relying on the idle back-off added up to 100 ms per wake.
- Only skip an idle sleep for a wake that arrived during the work check. A
  thread's first idle sleep must not be skipped, or tests race the wake.

## Never block a worker from async code

- Anything that waits synchronously inside a task blocks a pool worker. Two
  real hangs came from this: `H2Transport` doing its handshake in `open()`
  (moved into the pump) and `foundation_proxy` tests calling a blocking
  shutdown from async (`ProxyServer::shutdown_async` added). With every worker
  blocked, nothing is left to make progress and the suite hangs.

## Sync only at the boundary

- `.block()` / `.blocking()` exist only natively. On JS-hosted wasm there is no
  thread to block, so they are compiled out; a blocking call there is a compile
  error instead of a spin loop.
- A crate that can't see `foundation_core`'s JS features (e.g. foundation_db)
  gates its own sync adapters with a `foundation_core` macro that expands only
  where `.block()` exists. A plain `not(target_family = "wasm")` is wrong: it
  also drops them from non-JS wasm.

## Converting a crate to async-only traits

- Check the JS-wasm builds of every dependent before removing sync impls.
  foundation_ai's `AgentSession` defaults to in-memory stores through the sync
  traits, and `examples/cf-agent-loop` runs that on Workers, where `Blocking`
  can't exist. Pure in-memory backends never wait, so keeping their sync impls
  is safe until the dependent moves to the async traits.

## `Blocking<S>` design notes (foundation_db)

- `.block()` needs a `'static` future, but the async store methods borrow
  `&self` and `&str`. The adapter holds the store in an `Arc` and moves owned
  copies of the arguments into each future.
- The document scans return `AsyncStorageItemStream<'_, V>`, which borrows the
  store. To give the sync iterator a `'static` stream, one generator owns the
  `Arc`, opens the scan and drains it. It yields an "opened" or "open failed"
  event first, so the sync trait still reports open errors as `Err`.
- Poll once with a no-op waker before going to the executor. The in-memory
  stores are ready at once, so they need no pool and never tie up a worker.
  Only a future that is really `Pending` goes to valtron.
- `.block()` panics when no pool is running (`block_on_future` `expect`s). The
  adapter uses `sync_collect_one(from_future(..))` instead, the same path but
  returning an error, which becomes a `StorageError`.
- Turso's `query_async` is lazy: a bad statement errors on the first row
  poll, not when the query opens. Tests must look for the error as the first
  stream item.
- Dead end: gating `Blocking` on `not(target_family = "wasm")` would also break
  foundation_ai's wasm gate. The CI run is `--no-default-features` on wasm32,
  but foundation_netio's `wasm-fetch` turns on `foundation_core/js-wasmbindgen`
  there. Only foundation_core can evaluate the real gate.

## Converting callers (foundation_db PR 1)

- Feature-gated code nobody builds rots. Three places didn't compile, or had
  tests that never ran, before this change:
  - foundation_nativeapis `vfs-native,vfs-d1,vfs-r2` (written against an old
    streaming storage API);
  - `examples/cf-login-app` (its manifest inherited workspace fields while
    excluded from the workspace);
  - two foundation_db libsql tests with no `#[test]`.

  `cargo check --tests` with default features misses all of these. Grep for
  the changed APIs, then build every feature set that uses them.
- Before blaming your change for a failure, run the same test on the base
  commit. A temporary `git worktree` at the base, sharing the target dir
  (`CARGO_TARGET_DIR`), keeps the rebuild small. That is how the 17 failing
  foundation_auth IdP tests were shown to fail on master too.
- `SessionManager<S>` put every method under `impl<S: CredentialStore>`, so its
  async methods needed the sync trait for no reason. Moving the store-free
  helpers into an unbounded `impl<S>` let the async half run on JS wasm with
  only `AsyncCredentialStore`. Look for over-bounded impl blocks like this
  when a sync trait goes away on a target.
- libsql runs fine on valtron. The old comment saying it needs tokio was wrong;
  the vector-store and libsql KV tests pass on a valtron pool.
- On this host, `--features server-native` (webauthn-rs) tests don't link:
  OpenSSL 3 symbols are missing. Use `server-test` to run the auth server
  suite.

## Moving a sync state machine onto async stores (foundation_ai PR 2)

- A valtron `TaskIterator` can await a store without becoming a future: keep
  a `FutureTask<Pin<Box<dyn Future + Send>>>` in the state and call its
  `next_status` from the loop's own. `Ready` is the result; `Depends` /
  `Ignore` / `Delayed` pass straight back to the executor, so a waiting store
  parks the task.
- Poll the new future in the same step that creates it. In-memory stores are
  ready at once, so the loop's step sequence (and every test that steps it)
  stays the same.
- A write-behind buffer whose flush is now async needs an owner that awaits
  it. `append` only buffers; the loop flushes at a configured threshold
  (checked before each step), at every context assembly and when the turn
  ends. Re-dispatch the interrupted state after the flush without
  re-checking the threshold, or a flush that finds another one running loops
  forever.
- On wasm, `AsyncStorageItemStream` is `!Send`. Holding one across an
  `.await` inside an `async_trait` (`Send`) method fails only in the wasm
  build: wrap both the stream and each `next()` future in
  `foundation_compact::SendWrapper`. Native builds never show this, so
  build `examples/cf-agent-loop` for wasm32 after touching store code.
- Turning a sync `build()` async ripples into every helper that builds a
  session. Scripted `.await` insertion is fine for the call sites, but check
  each converted test: closures (`.map(|n| ... .await)`) and tests that drive
  a stream synchronously must be rewritten by hand, and a test that awaits
  nothing must stay `#[test]`.

- An `async fn` method that borrows `&self` can't be driven by
  `block_on_future`, which needs a `'static` future. Session methods that
  sync callers wrap (`run_turn`, `ask`, `end`) return
  `impl Future + 'static` and clone the `Arc`-backed handle into it.

## Async-only callers (F04, `db/async-callers`)

- A failing test suite that "fails on master too" still deserves a look
  before it is filed away. The 17 foundation_auth IdP failures had two plain
  causes: the sync `Serve` adapter turned every handler error into a 500,
  and the handlers read only `Text`/`Bytes` bodies while the server hands
  over a lazy `Stream`. A debug print of the 500's message found both.
- `ContextBag::store(value)` keys by the value's exact type and wraps it in
  its own `Arc`. Storing an `Arc<T>` means looking up `Arc<T>` (and getting
  `Arc<Arc<T>>` back); a lookup of `T` silently misses. Prefer a panic on a
  missing entry over an empty fallback store, which hides the mistake.
- A thread-local RNG (`foundation_compact::rng::rng()`) is `!Send`; holding
  it across an `.await` makes the whole handler future `!Send`. Fill the
  bytes in a statement of their own before the first await.
- `#[valtron_test]` takes `async fn` (F03), so converting a test module to
  async is mechanical: add `.await` to the converted calls and mark only the
  fns that now await as `async`. A small script that matches parentheses
  does the first part reliably.

- Don't mix pool styles in one test binary. A file whose tests share a static
  `initialize_pool` guard hangs when one test in it is `#[valtron_test]`
  (which starts a second pool in the same process): the new
  `migration_runner_applies_all_then_is_idempotent` hung until it was
  rewritten to the file's shared-pool style. Convert a whole file to
  `#[valtron_test]`, or none of it.
- A sync bridge that a hidden test fallback "rescues" can hide real bugs:
  the IdP `NoOpStore` fallback, `collect_stream` helpers that drop `Err`
  items, and `let _ =` on writes all turned up while converting callers.
  Converting is a good moment to make every error reach the caller.
- Code nothing exercises has usually rotted. The libsql/D1 state stores had
  never had a test: their `init` was a SQL syntax error and `find_by_status`
  could never match. R2's listing read one page and ignored filters. Adding
  the first behavioural test of a backend is worth it even when the task is
  "only make it async".
- An in-process mock HTTP server (`TcpListener` on a thread) is enough to
  test a REST-backed store, but it must answer `Expect: 100-continue` (the
  netio client sends it on bodies and otherwise waits 3s per request).
- To commit one task while the next one's edits are already in the tree,
  stage an exact file version without touching the worktree:
  `git hash-object -w <file>` then `git update-index --cacheinfo`.

## Pool teardown in tests

- `shutdown_within` detaches workers stuck inside a task. They are threads in
  the same process, so nothing is orphaned or left as a zombie: they exit on
  their own once they reach the kill check, or with the process.

## Working practice

- Local CI (`scripts/ci/ci.py run <crate> ...`) can exhaust the session's
  disk. Run one crate at a time and prune old `target/uat/build/*/` hashes
  between crates; keep logs inside the repo's target dir.
- `ci.py run foundation_db` with full debuginfo filled ~15 GB (its ~40
  test binaries each link Turso, libsql and the HTTP stack); the linker then
  died with a bus error. `CARGO_PROFILE_UAT_DEBUG=0` keeps the same checks
  and fits easily. Watch free space with a monitor during long runs.
- `ci.py run foundation_ai` with all steps needs more than 14 GB free:
  `build` with llamacpp + candle leaves ~14 GB in `target/uat`, then `clippy`
  adds more. Run the steps one at a time and `rm -rf target/uat` between them;
  pruning old hashes mid-run isn't safe in the `build/<crate>/<hash>/` layout,
  where each hash is a live unit.
- Long agent runs can be stopped by usage limits. Commit at stable points so a
  stop loses nothing.
