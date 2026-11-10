---
feature: "Ctx in the remaining crates"
status: "planned"
priority: "medium"
model: "haiku"
depends_on: ["02-merge-contextbag-into-ctx"]
crates: ["foundation_deployment_platform", "foundation_nativeapis", "foundation_platform", "foundation_toolings", "foundation_wireguard", "foundation_core", "foundation_deployment", "foundation_repl", "foundation_testing", "foundation_testbed", "foundation_deployment_cloudflare", "foundation_cronjobs", "foundation_sshkit", "foundation_wasmtime"]
---

# Feature 05: Ctx in the remaining crates

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); remote procedure call
(RPC); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## Goal

Replace each `Arc<dyn Clock>` parameter and field with a `Ctx` in these crates. The counts
were measured on 2026-11-09 (`dyn Clock` outside `tests/`):

| Crate | Sites |
|---|---|
| `foundation_deployment_platform` | 81 |
| `foundation_nativeapis` | 73 |
| `foundation_platform` | 21 |
| `foundation_toolings` | 18 |
| `foundation_wireguard` | 17 |
| `foundation_core` | 13 |
| `foundation_deployment` | 11 |
| `foundation_repl` | 8 |
| `foundation_testing` | 6 |
| `foundation_testbed` | 6 |
| `foundation_deployment_cloudflare` | 5 |
| `foundation_cronjobs` | 5 |
| `foundation_sshkit` | 3 |
| `foundation_wasmtime` | 2 |
Note for `foundation_core`: `synca/sleepers.rs` and the executor internals may keep a clock when they build or own it (requirements section 2.3). Change only the public API that callers pass a clock into.


## Migration recipe (the same in features 03, 04 and 05)

1. Find the sites: `grep -rn "dyn Clock" --include=*.rs backends/<crate>/src`.
2. A function or constructor parameter `clock: Arc<dyn Clock>` becomes `ctx: Ctx`. Inside,
   read `ctx.clock()` where the clock is used.
   A borrowed `clock: &dyn Clock` or `&Arc<dyn Clock>` becomes `ctx: &Ctx`, and an
   `Option<Arc<dyn Clock>>` becomes `Option<Ctx>`. The review on 2026-11-09 counted 373
   `Arc<dyn Clock>`, 208 `&dyn Clock`, 12 `&Arc<dyn Clock>` and 5 `Option<Arc<dyn Clock>>`
   sites in all crates.
3. A struct field `clock: Arc<dyn Clock>` becomes `ctx: Ctx`. Read `self.ctx.clock()` at each
   use. Do not cache a second clock next to the `Ctx`.
4. A caller that has a `Ctx` passes it (`ctx.clone()`). A caller that has only a clock moves
   up one level, and so on, until it reaches the code that already has a `Ctx`: a valtron
   task, an HTTP or RPC handler, or a host entry point.
5. Library code that builds `MonotonicClock::new()` inline (GitHub issue #463 lists the
   sites) takes the caller's `Ctx` instead. Close #463 when features 03 to 05 remove all of them.
6. A host entry point (`main`, a command-line command, a worker entry) builds `Ctx::host()`
   once and passes it down. Library code never builds `Ctx::host()` or `MonotonicClock::new()`.
7. A test that used `Arc::new(SimulatedClock::new(..))` builds
   `Ctx::fixed(clock.clone())` and keeps the `clock` handle to advance it.
8. A trait method that took a clock takes `ctx: &Ctx` (or `Ctx` by value, if it moves into a
   future). Change every implementation in the same PR.
9. Keep `Arc<dyn Clock>` only where the requirements section 2.3 allows it. Each kept site
   gets a one-line comment that says why.

Tests: the crate's existing tests pass, and they assert the same values. Add one test per
crate that builds the main public type from a `Ctx` with a `SimulatedClock`, advances the
clock, and sees the change through the type. This test does not compile on the old
signatures.

Verify, for each crate in the frontmatter and each direct dependent
(`cargo tree -i <crate> --workspace --depth 1`):

```bash
cargo fmt --all --check
cargo clippy -p <crate> --all-targets --profile uat -- -D warnings
cargo test -p <crate> --profile uat
cargo check --workspace --profile uat
grep -rn "dyn Clock" --include=*.rs backends/<crate>/src   # only the allowed, commented sites
```

## Done when

- Each crate above has no `Arc<dyn Clock>` outside the allowed, commented sites.
- Each crate and its direct dependents pass clippy, tests and `cargo check --workspace`.
