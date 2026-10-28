---
feature: "Remove the last tokio runtimes: the testbed embedded JS runner and the tokio/axum apps (#137)"
status: "pending"
priority: "medium"
depends_on: ["08-async-serve"]
branch: "fix/io-24-tokio-apps"
crates: ["foundation_testbed", "bin/platform", "examples/devbinary", "examples/web/wello", "examples/todo", "crates/channels", "crates/domain"]
backlog: "docs/bug-backlog.md §18 (1 entry), §19 (1 entry); issue ewe-studios/ewe_platform#137"
---

# Feature 24: tokio boundaries and apps

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** JavaScript (JS); Hypertext Transfer Protocol (HTTP); Transmission
Control Protocol (TCP); continuous integration (CI); pull request (PR); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. The
project rule is "valtron always": no tokio outside valtron's cross-runtime tests. This feature
needs feature 08 (async `Serve`) so that `foundation_http` can replace axum. Do one app per PR
(E2–E7); E1 and E8 may share a PR.

## T0. Check that every entry is still open (table in the PR body)

Also run `rg -n '\btokio\b|\baxum\b' --glob 'Cargo.toml'` and list every hit; each one is in an
entry below or is a valtron cross-runtime test.

## E1. The testbed embedded JS runner uses a tokio runtime

- **Where:** `foundation_testbed/src/wasm/embedded_js.rs` (`tokio_runtime()`), the optional
  `tokio` dependency behind `wasm-embedded-js`. `deno_core`'s event loop and timers need tokio;
  `deno_websocket` uses tokio for TCP.
- **Step 1 (research, write it in the PR):** can `JsRuntime::poll_event_loop` be polled from a
  valtron task, with timers on valtron's clock? Read the `deno_core` version in use.
- **Fix:** if yes, drive the event loop from a valtron task and drop tokio. If no, keep it as a
  documented third-party boundary: a current-thread runtime local to this module, behind the
  feature flag, with a module doc that says why and links the research; drop `deno_websocket` if
  no test needs it.
- **Verify:** `mise run test:wasm-testbed:deno` passes (feature 22 E4 runs it in CI).

## E2. `bin/platform` and its templates (`api-app`, `static-html-app`, `web-app`)

- **Fix:** port the servers to `foundation_http` on valtron; the templates generate valtron code.
  Replace tokio tests with valtron tests in `tests/`.
- **Verify:** `cargo build -p ewe_platform` (the bin crate name); each template generates a
  project that builds (`tests/templates_build.rs`); `cargo tree -p <crate> | rg tokio` is empty.

## E3. `examples/devbinary`
## E4. `examples/web/wello`
## E5. `examples/todo`

- **Fix (each):** port to `foundation_http` and valtron; keep the behaviour (same routes and
  answers). Add one test in `tests/` per example that starts it on a free port and checks one
  route through the netio client.
- **Verify (each):** it builds; its test passes; `cargo tree | rg tokio` is empty.

## E6. `crates/channels`

- **Fix:** replace tokio with valtron primitives; move the inline `#[cfg(test)]` modules
  (`src/broadcast.rs`, `src/mspc.rs`, `src/executor.rs`) to `tests/` (rule 7), with valtron tests.
- **Verify:** `rg -n 'cfg\(test\)' crates/channels/src` is empty; tests pass.

## E7. `crates/domain`

- **Fix:** as E6.

## E8. Close #137

- When E2–E7 are merged, delete §19 from the backlog and say in the last PR that #137 can be
  closed.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo check --workspace --all-targets --profile dev
rg -n '\btokio\b|\baxum\b' --glob 'Cargo.toml'   # only valtron cross-runtime tests (and E1 if kept)
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted; #137 can be closed.
- [ ] No tokio or axum dependency outside valtron's cross-runtime tests (and E1's documented
      boundary, if research says it must stay).
- [ ] Each ported app has a test that runs it. CI green.
