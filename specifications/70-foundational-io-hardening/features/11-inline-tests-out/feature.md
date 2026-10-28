---
feature: "Inline tests out of source files"
status: "pending"
priority: "medium"
depends_on: []
branch: "fix/io-11-inline-tests-out"
crates: ["foundation_netio", "foundation_http", "foundation_connectrpc"]
backlog: "CLAUDE.md rule 7: tests live in tests/, never inline"
---

# Feature 11: Inline tests out of source files

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); pull request (PR);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

No `#[cfg(test)]` module in the source files of the foundational crates. Every test is in the
crate's `tests/` directory and tests the public API (`CLAUDE.md` rule 7).

Files with `#[cfg(test)]` (from `git grep -c '#\[cfg(test)\]'` on 2026-10-28):

- `foundation_connectrpc/src/shared/transport/wasm.rs`
- `foundation_http/src/shared/middleware/{auth,body_limit,compression,cors,logger}.rs`
- `foundation_http/src/wasm/bridge/{cf,web}.rs` (2 each), `foundation_http/src/wasm/cf_conn.rs`
- `foundation_netio/src/event_source/shared/sse.rs`
- `foundation_netio/src/http/tasks/{nonblocking,request_intro,state}.rs`
- `foundation_netio/src/native/{connection,ssl/rustls,tls_verification}.rs`
- `foundation_netio/src/shared/client/{body_reader,redirects}.rs`
- `foundation_netio/src/shared/http/{client_classifier,latency_tracker,timeout}.rs`
- `foundation_netio/src/wasm/client/tasks/http_exchange_task.rs`
- `foundation_netio/src/websocket/native/readiness.rs`, `foundation_netio/src/websocket/shared/decoder.rs`

(`foundation_nostd/src/comp/QUICKSTART.md` matches the search but is a document; leave it.)

Re-run the search at the start; the list may have changed.

## 2. How, per file

1. Read the `#[cfg(test)]` block. It is either a test module, or a test-only helper (a
   `#[cfg(test)] fn` or `impl`) used by other code.
2. **Test module:** move each test to `tests/<area>/<file>_tests.rs`, registered in
   `tests/<area>/mod.rs` (follow the crate's existing layout). Rewrite it against the public API.
3. **The test needs private items:** do not make internals public only for a test. Prefer
   testing through the public behaviour. If the item is a real part of the API that other
   crates would also need, make it public with docs. If neither fits, stop and list the case in
   the PR body for the main session.
4. **Test-only helper:** if the moved tests need it, move it into `tests/common/` or a
   `testing` feature of the crate (if the crate has one). Delete it from `src/`.
5. Wasm-only modules (`wasm.rs`, `cf.rs`, `web.rs`, `cf_conn.rs`, the wasm exchange task): move
   their tests to the crate's wasm test target (`wasm-bindgen-test`), the same way the crate's
   other wasm tests run.
6. Count tests before and after per file; the count must not drop. Put the table in the PR body.

## 3. Tasks

- [ ] T1. Branch from `master` (no dependency).
- [ ] T2. `foundation_netio` files. One commit per area (`http/tasks`, `native`, `shared`,
      `event_source`, `websocket`, `wasm`).
- [ ] T3. `foundation_http` files. One commit.
- [ ] T4. `foundation_connectrpc` file. One commit.
- [ ] T5. Update `progress.md`.

## 4. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_netio foundation_http foundation_connectrpc; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1   # includes the wasm step
done
git grep -n '#\[cfg(test)\]' -- backends/foundation_netio/src backends/foundation_http/src backends/foundation_connectrpc/src   # expect no output
```

## 5. Done when

- [ ] No `#[cfg(test)]` in those `src/` trees.
- [ ] Test counts per file did not drop (table in the PR body).
- [ ] No item became public only to serve a test (each new `pub` is justified in the PR body).
- [ ] CI green, native and wasm.

## 6. Review checklist (main session)

- [ ] Moved tests still assert the same behaviour (not weakened).
- [ ] No duplicated test bodies.
