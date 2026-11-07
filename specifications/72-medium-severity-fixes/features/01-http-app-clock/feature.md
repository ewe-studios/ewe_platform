---
feature: "HTTP app constructors take their context and clock from the caller"
status: "pending"
priority: "medium"
depends_on: []
crates: ["foundation_http"]
issues: [234]
model: "sonnet"
branch: "spec72/f01-http-app-clock"
---

# Feature 01: HTTP app clock injection

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); random number
generator (RNG); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## 1. Goal

Let a deterministic host build an `HttpApp` with its own clock and RNG. A plain constructor
still uses the host clock, but only at the host entry point. Close issue
[#234](https://github.com/ewe-studios/ewe_platform/issues/234).

## 2. Problem

`HttpApp::new*`, `HttpApp::new_web()` and the wasm bridges call
`ContextBag::from_ctx(&Ctx::host())`. The initial app clock and RNG are therefore the real host
ones. A deterministic host must replace `app.ctx` after construction. That step is easy to
forget, and a forgotten step fails silently.

Files: `backends/foundation_http/src/shared/app/mod.rs`,
`backends/foundation_http/src/shared/serve_web.rs`, `backends/foundation_http/src/wasm/`.

## 3. Requirements

1. Add `HttpApp::with_ctx(ctx: Ctx)` (or an equivalent constructor that takes the bundle) and
   `HttpApp::with_clock(clock: Arc<dyn Clock>)` that builds on it.
2. The plain constructors keep their signature. They call `with_ctx(Ctx::host())` and nothing
   else. The `Ctx::host()` call stays at the host boundary.
3. The wasm bridges take the bundle the same way. No wasm path builds a host bundle of its
   own.
4. The new constructor is documented as the deterministic-test entry point.
5. If the existing `ContextBag::from_ctx` signature differs from the sketch here, follow the
   real signature and record it in `progress.md`.

## 4. Design

```text
HttpApp::new(...)        -> HttpApp::with_ctx(Ctx::host())     // host boundary only
HttpApp::new_web(...)    -> HttpApp::with_ctx(Ctx::host())     // host boundary only
HttpApp::with_ctx(ctx)   -> builds ContextBag::from_ctx(&ctx)
HttpApp::with_clock(c)   -> HttpApp::with_ctx(Ctx::host_with_clock(c))
```

- `with_clock` exists so a test does not build a whole bundle by hand.
- A deterministic host passes its simulated clock, seeded RNG and in-memory transport through
  `with_ctx`.
- No global fallback is added. A missing value stays a typed error.

## 5. Tasks

- [ ] Read the three files and the `ContextBag` constructors. Record the real signatures.
- [ ] Add `with_ctx` and `with_clock`. Route every plain constructor through `with_ctx`.
- [ ] Update the wasm bridges.
- [ ] Add the regression test.
- [ ] Run the verify commands. Record the results.
- [ ] Close #234 in the PR body.

## 6. Tests

In `backends/foundation_http/tests/`:

1. `with_clock_keeps_the_injected_clock`: build an app with a simulated clock, advance the
   simulated clock, and read the app's clock through its public accessor. The value is the
   simulated one. On the old code there is no such constructor, so the test is new by
   necessity; add a companion test that the plain constructor still builds.
2. `with_ctx_keeps_the_injected_rng`: build with a seeded RNG bundle and assert two apps with
   the same seed produce the same value.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_http
cargo clippy --profile uat -p foundation_http --all-targets -- -D warnings
cargo test --profile uat -p foundation_http
```

## 8. Done when

- No constructor other than a host entry point calls `Ctx::host()`.
- The new tests pass, and the plain-constructor test still passes.
- Issue #234 is closed by the PR.
- `progress.md` and `learnings.md` record the change.
