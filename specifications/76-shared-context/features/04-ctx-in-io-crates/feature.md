---
feature: "Ctx in the I/O crates"
status: "planned"
priority: "medium"
model: "haiku"
depends_on: ["02-merge-contextbag-into-ctx"]
crates: ["foundation_auth", "foundation_proxy", "foundation_netio", "foundation_db", "foundation_browser", "foundation_connectrpc", "foundation_http", "foundation_keychain"]
---

# Feature 04: Ctx in the I/O crates

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
| `foundation_auth` | 80 |
| `foundation_proxy` | 32 |
| `foundation_netio` | 28 |
| `foundation_db` | 28 |
| `foundation_browser` | 19 |
| `foundation_connectrpc` | 9 |
| `foundation_http` | 8 |
| `foundation_keychain` | 8 |

## Migration recipe (the same in features 03, 04 and 05)

1. Find the sites: `grep -rn "dyn Clock" --include=*.rs backends/<crate>/src`.
2. A function or constructor parameter `clock: Arc<dyn Clock>` becomes `ctx: Ctx`. Inside,
   read `ctx.clock()` where the clock is used.
3. A struct field `clock: Arc<dyn Clock>` becomes `ctx: Ctx`. Read `self.ctx.clock()` at each
   use. Do not cache a second clock next to the `Ctx`.
4. A caller that has a `Ctx` passes it (`ctx.clone()`). A caller that has only a clock moves
   up one level, and so on, until it reaches the code that already has a `Ctx`: a valtron
   task, an HTTP or RPC handler, or a host entry point.
5. A host entry point (`main`, a command-line command, a worker entry) builds `Ctx::host()`
   once and passes it down. Library code never builds `Ctx::host()` or `MonotonicClock::new()`.
6. A test that used `Arc::new(SimulatedClock::new(..))` builds
   `Ctx::new(clock.clone())` and keeps the `clock` handle to advance it.
7. A trait method that took a clock takes `ctx: &Ctx` (or `Ctx` by value, if it moves into a
   future). Change every implementation in the same PR.
8. Keep `Arc<dyn Clock>` only where the requirements section 2.3 allows it. Each kept site
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
