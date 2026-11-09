---
feature: "Merge ContextBag into Ctx, rename connectrpc Ctx to RpcCtx"
status: "planned"
priority: "high"
model: "sonnet"
depends_on: ["01-foundation-context-crate"]
crates: ["foundation_context", "foundation_http", "foundation_connectrpc", "foundation_connectrpc_codegen", "foundation_macros", "foundation_auth", "foundation_browser", "foundation_db", "foundation_keychain", "foundation_proxy", "foundation_toolings", "foundation_wasm_ui", "foundation_deployment_docker", "foundation_core"]
---

# Feature 02: Merge `ContextBag` into `Ctx`, and rename the RPC `Ctx` to `RpcCtx`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); remote procedure call
(RPC); random number generator (RNG); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Remove `foundation_http::ContextBag`. Every user takes a `foundation_context::Ctx` instead.
Rename `foundation_connectrpc::Ctx` to `RpcCtx` and change its `bag: Arc<ContextBag>` field to
`ctx: Ctx`. Both changes land in one PR, with no alias or deprecated shim left behind.

## 2. Measured facts

- `ContextBag` is defined in `backends/foundation_http/src/shared/context/mod.rs`. It has
  358 uses in 81 files, in these crates: foundation_http (43 files), foundation_connectrpc
  (13), foundation_proxy (10), foundation_auth (3), foundation_browser (3), foundation_db (3),
  foundation_toolings (2), foundation_keychain (1) and foundation_wasm_ui (1).
- `Arc<ContextBag>` appears 110 times. `Ctx` is already an `Arc` handle, so these become `Ctx`.
- `Serve::create(bag: &ContextBag)` (`foundation_http/src/shared/serve/mod.rs:136`) becomes
  `Serve::create(ctx: &Ctx)`.
- `ContextBag` has `unsafe impl Send` and `unsafe impl Sync`. The compiler derives both for
  the field types, so the `unsafe` blocks only hide a future mistake. `Ctx` has no `unsafe`.
- `foundation_connectrpc::Ctx` is emitted by two generators. Root rule 5 says: fix the
  generator, then regenerate.
  - `backends/foundation_macros/src/connectrpc_service.rs` (about 10 sites emit
    `foundation_connectrpc::Ctx`);
  - `backends/foundation_connectrpc_codegen/src/generator.rs`, with its
    `tests/generator_tests.rs`;
  - regenerate every checked-in generated file that names it (`grep -rn
    "foundation_connectrpc::Ctx"`).
- Method mapping from `ContextBag` to `Ctx`:

| `ContextBag` | `Ctx` |
|---|---|
| `ContextBag::new(clock)` | `Ctx::new(clock)` |
| `ContextBag::build(clock, \|b\| ..)` | `Ctx::build(clock, \|c\| ..)` (new; the closure takes `&Ctx`) |
| `ContextBag::from_ctx(&ctx)` | `ctx.with_empty_map()` (new) |
| `bag.with_ctx(&ctx)` | `bag.rebase(&ctx)` (new: copy this map, take `ctx`'s clock, config and RNG) |
| `bag.fork()` | `ctx.fork()` |
| `bag.store(v)` | `ctx.store(v)` (new: `insert(Arc::new(v))`, prior value dropped) |
| `bag.get::<T>()` | `ctx.get::<T>()` |
| `bag.get_cloned::<T>()` | `ctx.get_cloned::<T>()` (new) |
| `bag.remove::<T>()` (returns `Arc<dyn Any>`) | `ctx.remove::<T>()` (returns `Arc<T>`); fix the callers |
| `bag.contains::<T>()` | `ctx.contains::<T>()` |
| `bag.clock()`, `bag.rng()`, `bag.assert_minimum()` | same names |

## 3. Work

1. In `foundation_context`, add `build`, `store`, `get_cloned`, `with_empty_map` and `rebase`
   with doc comments. Change the map lock from `Mutex` to `RwLock` (the HTTP request path
   reads the map on every request, and it rarely writes). Use the same native/wasm switch
   that feature 01 added. Keep the poisoned-lock messages.
2. Delete `foundation_http/src/shared/context/mod.rs`, and its re-exports. Re-export
   `foundation_context::Ctx` from `foundation_http` at the same public path the bag had, so
   users import one name. Add the `foundation_context` dependency to each crate that names
   `Ctx` directly.
3. Replace every use, crate by crate, with the table above. `Arc<ContextBag>` becomes `Ctx`.
   `&ContextBag` becomes `&Ctx`. Variables and fields named `bag` become `ctx`, unless that
   name is already taken in the same scope.
4. In `foundation_connectrpc`, rename `Ctx` to `RpcCtx`. The field `bag` becomes `ctx: Ctx`.
   `RpcCtx::client(ctx: Ctx)` and `RpcCtx::background()` keep their meaning.
   `clock_from_bag` goes away; read `ctx.clock()`.
   `RpcCtx::background()` builds `Ctx::host()`, and its doc comment says it is a host
   entry point.
5. Update both generators to emit `foundation_connectrpc::RpcCtx`, update their tests, and
   regenerate the checked-in output.
6. Move `foundation_http/tests/context_bag_tests.rs` to `foundation_context/tests/` as tests
   of the new `Ctx` methods. Delete the tests that only tested the `unsafe impl`.
7. Fix GitHub issue #448 (medium). `Ctx::new` and `Ctx::host` seed the shared RNG with the
   constant `0`, so every host draws the same stream (WASI `random_get` included).
   `Ctx::host()` must seed the RNG from `foundation_compact`'s entropy backend.
   `Ctx::new(clock)` keeps a fixed seed for tests, and a new `Ctx::with_seed(clock, seed)`
   lets a deterministic simulation test choose the seed. `foundation_platform::host_session_ctx`
   then uses `Ctx::host()` and drops its own reseed. Close #448 in this PR.
8. Search docs, READMEs, `.agents` and the specs for `ContextBag` and `connectrpc::Ctx`, and
   update them (`grep -rn "ContextBag" --include=*.md .`).

## 4. Tests

- `foundation_context/tests/`: `rebase` keeps the map and takes the other context's clock
  and RNG (`Arc::ptr_eq`); `with_empty_map` shares the sources and has an empty map; `store`
  then `get_cloned` returns the value; `remove::<T>` returns `Arc<T>`. The `rebase` and
  `with_empty_map` tests do not compile on the old code.
- The existing foundation_http, connectrpc, proxy, auth and browser tests pass with no change
  to what they assert.
- `foundation_connectrpc_codegen/tests/generator_tests.rs` asserts `RpcCtx` in the output.
- Two `Ctx::host()` handles draw different first values from `rng()`. Two `Ctx::with_seed`
  handles with the same seed draw the same values. The first test fails on the old code.

## 5. Verify

```bash
cargo fmt --all --check
for p in foundation_context foundation_http foundation_connectrpc foundation_connectrpc_codegen \
         foundation_macros foundation_proxy foundation_auth foundation_browser foundation_db \
         foundation_keychain foundation_toolings foundation_wasm_ui foundation_deployment_docker; do
  cargo clippy -p $p --all-targets --profile uat -- -D warnings || echo "CLIPPY FAIL $p"
  cargo test -p $p --profile uat || echo "TEST FAIL $p"
done
cargo check --workspace --profile uat
grep -rn "ContextBag" --include=*.rs . | grep -v target   # no output
```

Where `ci/packages.toml` does not deny warnings for a crate (13 packages are exempt, see
issue #441), add no new warnings.

## 6. Done when

- No `ContextBag` remains in Rust code, docs or generated code.
- `foundation_connectrpc::RpcCtx { ctx: Ctx, request: RequestContext }` replaces the old
  `Ctx`, and both generators emit it.
- The crates in the frontmatter pass clippy and tests, and `cargo check --workspace` passes.
