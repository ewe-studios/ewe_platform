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

## Pool teardown in tests

- `shutdown_within` detaches workers stuck inside a task. They are threads in
  the same process, so nothing is orphaned or left as a zombie: they exit on
  their own once they reach the kill check, or with the process.

## Working practice

- Local CI (`scripts/ci/ci.py run <crate> ...`) can exhaust the session's
  disk. Run one crate at a time and prune old `target/uat/build/*/` hashes
  between crates; keep logs inside the repo's target dir.
- Long agent runs can be stopped by usage limits. Commit at stable points so a
  stop loses nothing.
