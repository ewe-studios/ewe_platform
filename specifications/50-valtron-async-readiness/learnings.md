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
- A shared rule beats per-backend copies. The rate limiter's window bug was
  in all eight implementations (memory, KV, Turso, libsql, D1 REST, D1
  Workers), each with its own SQL or arithmetic. Putting the rule in one
  place (`core/rate_limit.rs`, generic over `AsyncQueryStore`) fixed them
  together and made one test file cover every local backend.
- Test against the migrated schema, not only a fresh table. The SQL rate
  limiter created its own `rate_limits` shape on demand and had only ever run
  against that; on a migrated database (migration 014, different columns)
  every record failed. `TursoStorage::in_memory()` (migrated) caught it.
- `std::time::SystemTime::now()` panics on wasm32-unknown-unknown; crate code
  that can run in a Worker reads the clock through `foundation_compact`
  (`unix_now_secs` in foundation_db).
- When a disk fills mid-build, `ci.py`'s failure is a link error or a
  "failed to write fingerprint", not a test failure. Read the error before
  assuming the code broke, free space, and rerun the step.

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

## Renaming after removing the sync traits (PR #134)

- **Check for name clashes before a mechanical rename.** Two `Async*` traits (`AsyncDeltaStore`,
  `AsyncVectorStore`) still had sync twins in other crates, so the plain names were taken. A grep
  for `trait|struct|enum <PlainName>` across the workspace found them before any edit.
- **A word-boundary rename is safe here because no longer identifier contains the old names**
  (checked first: no `AsyncQueryStoreExt` or similar). Use `\bName\b`.
- **Check the example workspaces and wasm builds, not only `--workspace`.** `examples/cf-*` and
  `apps/answerme-agent` are separate workspace roots; `cargo check --workspace` does not see them.
- **Pushing several small commits in a row cancels the earlier CI runs.** The "Checks passed"
  summary job then reports the cancelled run as a failure. Only the run on the newest commit counts.
- **Agents can stop on account spend or rate limits.** Commit and push at every stable point, so
  a stop loses nothing; the owner prefers everything pushed.

## Feature 05 task T1: vector stores async-only

- **A sync trait hides wrong answers.** The sync `TurboPufferVectorStore::len` always returned
  0, and `get` searched with a zero vector and kept the first row. The SQLite-vec `get` always
  returned `None`, and its insert added a second row for an existing id. Fjall's `delete` of an
  unknown id was `Ok`. A shared contract test (`tests/contract`) found these differences at once.
  Each store now runs the same checks.
- **Write the contract before you convert the stores.** Then each store conversion has a clear
  "done" check, and a store that cannot obey a rule must say so (for example, `Unsupported` for
  the dot-product metric on `TurboPuffer` and SQLite-vec).
- **A macro that defines a macro cannot reuse `$name` metavariables.** The outer
  `macro_rules!` reads the inner `$name:ident` as its own. Write each generated test out in full
  in the outer macro.
- **Check the external API before you keep an adapter.** The `TurboPuffer` adapter used
  `/v1/vectors/...`, which the current official client no longer has. The documentation site was
  blocked here, but the Python client on PyPI (Python Package Index) has typed request and
  response definitions. Those were enough to move to the v2 API.
- **A sync registry can feed an async index through a queue.** `ToolCallManager::register`
  has about 20 sync callers. Instead of making them all async, `register` queues the index change
  (last change per tool name wins, so the queue is bounded by the tool count). The async search
  applies the queue first and returns the failures in `ShedResult::index_failures`. The caller
  and the model see each failure; the log is not the only record.
- **Disk:** a test run of `foundation_vectors` with all store features links about eleven test
  binaries, each with Turso and libSQL. With debug information they filled the disk.
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` keeps the same checks. In the
  `build/<crate>/<hash>/` layout, removing hash directories older than the current run is safe
  between runs.
- **A new worktree has no submodules.** `foundation_ai` builds `llama.cpp` from the
  `tools/llama.cpp` submodule. Run `git submodule update --init tools/llama.cpp` in a new
  worktree before you build it.

## Feature 05 task T2: the VFS async-first

- **Build every feature of a crate before you change it.** `foundation_nativeapis` CI builds only
  `""` and `vfs-native,vfs-d1,vfs-r2`. A check of each other VFS feature found `vfs-sqlite` broken
  (a missing dependency), `vfs-preload` broken (the `open` symbol), and `TursoDelta` never
  compiled. Find these first, so a new failure is not taken for your own.
- **Poll once before you go to the pool.** `exec_async` now polls the future once on the calling
  thread. Memory, native and Fjall futures are ready at once, so `SyncFs` over them needs no
  valtron pool and uses no worker. This also keeps the sync boundaries (the shim, ptrace, sync
  tests) free of a pool where the store does not need one.
- **A `Drop` impl cannot await.** Remote file handles close themselves in `Drop`. The transport got
  a sync `release` for requests whose response is not needed. Over a bus with ordered responses,
  the client counts the released requests and skips their responses before it reads its own.
- **A handle that borrows its parent must share it.** `OverlayDirectory` held raw pointers to the
  layers. With an async trait the handle can live across awaits, so the layers are now in an
  `Arc` that the file system and its directory handles share.
- **`#[traced_test]` and `#[valtron_test]` in one test binary race.** Both try to install the
  global tracing subscriber; the loser panics at random. Do not mix them in one binary.
- **A shared target directory is not yours.** Other sessions built and pruned the shared target
  at the same time; deleted dependency outputs gave "can't find crate" errors that `cargo` does
  not repair. A private target in the worktree (with `CARGO_PROFILE_DEV_DEBUG=0`) was reliable.
