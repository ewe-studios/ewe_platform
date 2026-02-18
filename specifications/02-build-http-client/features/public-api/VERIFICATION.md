# VERIFICATION REPORT — public-api (Rust) 
spec: specifications/02-build-http-client  
feature: specifications/02-build-http-client/features/public-api  
generated: 2026-02-18T01:40:00Z
agent: Main Agent (verification run summary)

---

## OVERVIEW / STATUS
- Overall verification status: FAIL ❌
- Primary failure: CHECK #1 (Incomplete Implementation Scan) failed and verification stopped per Rule 08.
- Reason: One or more `TODO` markers remain in the `client` module which makes the feature incomplete by project verification rules.

---

## CHECK #1 — INCOMPLETE IMPLEMENTATION SCAN (MANDATORY)
- Command (exact, run from repository root):
  rg -n --hidden --line-number "TODO|FIXME|unimplemented!|todo!" backends/foundation_core/src/wire/simple_http/client || true

- Findings (raw matches)
  - backends/foundation_core/src/wire/simple_http/client/task.rs:414
    - Line excerpt: `TODO (public-api): Implement redirect handling state and transitions.`
  - backends/foundation_core/src/wire/simple_http/client/task.rs:420
    - Line excerpt: `rather than leaving inline \`TODO\` comments which block verification.`

- Result: FAIL — Incomplete implementations found. Per `.agents/rules/08-verification-workflow-complete-guide.md` this check must PASS before any other Rust checks are executed. Verification stopped here.

---

## CONTEXT (recent changes related to this verification)
- Recent commit (local): b4e27fa998f4776482b28be77f6fb78f139fca14
- Files changed in that commit (relevant to verification):
  - backends/foundation_core/src/wire/simple_http/client/pool.rs
    - Replaced stub with a minimal, testable `ConnectionPool` implementation.
  - backends/foundation_core/src/wire/simple_http/client/task.rs
    - Converted an inline TODO into a documented TODO block (the block still contains the token `TODO`, which triggers CHECK #1).
  - backends/foundation_nostd/src/primitives/wait_duration.rs
    - Doc-markup fixes.
  - crates/config/src/lib.rs
    - Added `# Errors` docs.
  - crates/watchers/src/handlers.rs
    - Added `# Errors`/`# Panics` docs.
  - specifications/02-build-http-client/features/public-api/PROGRESS.md
    - Progress and retrieval evidence recorded.
  - specifications/02-build-http-client/features/public-api/templates/VERIFICATION_INPUT.md
    - Verification input prepared for the verification agent.

Note: The `pool.rs` implementation was intentionally conservative to reduce the incomplete-implementation surface. The remaining explicit `TODO` in `task.rs` is the blocking item.

---

## RECOMMENDATION — MINIMAL NEXT STEPS (to remove CHECK #1 failure)
The verification agent will PASS only after the repository contains zero TODO/FIXME/unimplemented!/todo! tokens in the files under verification scope. To move verification forward, implement the minimal redirect handling described below and remove the `TODO` tokens (or convert them to tracked issues and remove the literal tokens in code).

1. Implement minimal redirect-handling state in `task.rs` (small, testable):
   - Add a `Redirecting` variant to `HttpRequestTaskState`, for example:
     ```rust
     Redirecting { attempts: u8, next_url: Option<ParsedUrl>, inner: Option<...> }
     ```
   - At the point where the `GetRequestIntroTask` returns response intro + headers, detect 3xx status codes and the `Location` header.
     - If a 3xx with a valid Location header is observed:
       - Transition the `HttpRequestTask` into `Redirecting` state, storing the new target and incrementing a redirect counter.
       - Re-use existing task spawning logic to initiate a connection/request to the new location.
     - If no Location header or invalid Location, treat as a non-redirect error.
   - Enforce `max_redirects` (value passed when constructing `HttpRequestTask`) — if exceeded, return a clear error (map to an appropriate `HttpClientError` / `HttpReaderError` variant).
   - Keep flow synchronous and conservative: spawn a new `GetHttpRequestStreamTask` for the redirect target, then proceed to `Connecting`/`Reading` as existing states do.

2. Add minimal unit tests (in `backends/foundation_core/src/wire/simple_http/client/tests.rs` or appropriate module):
   - Test: single redirect — server returns 3xx with Location → client follows and returns final response.
   - Test: redirect loop — server returns repeated redirects exceeding `max_redirects` → client returns redirect-exceeded error.
   - Test: invalid Location — server returns 3xx with malformed Location → client returns appropriate error.

3. Remove or replace the literal `TODO` tokens in code once the above is implemented.
   - If you prefer to keep high-level notes, move them to feature tracking files (e.g., `PROGRESS.md`, issue tracker) but do not leave `TODO`/`FIXME` tokens in code under verification scope.

4. Re-run the verification sequence from CHECK #1. If CHECK #1 passes, proceed with the remaining standard checks:
   - cargo fmt -- --check
   - cargo clippy --all-targets --all-features -- -D warnings
     - Note: full workspace clippy can be noisy due to unrelated crates; prefer per-package `foundation_core` clippy if workspace-level is impractical.
   - cargo test --package foundation_core --all-features
   - cargo build --package foundation_core --all-features
   - cargo doc --no-deps --package foundation_core --all-features
   - (optional) cargo audit

---

## VERIFICATION COMMANDS (recommended, run from repo root)
- CHECK #1 (mandatory):
  - rg -n --hidden --line-number "TODO|FIXME|unimplemented!|todo!" backends/foundation_core/src/wire/simple_http/client || true
- If CHECK #1 passes, run:
  - cargo fmt -- --check
  - cargo clippy --package foundation_core --all-targets --all-features -- -D warnings
  - cargo test --package foundation_core --all-features
  - cargo build --package foundation_core --all-features
  - cargo doc --no-deps --package foundation_core --all-features
  - (optional) cargo audit

---

## WHY THIS MATTERS (rules & policy)
- Rule 08 (verification) mandates that the incomplete implementation scan is CHECK #1 and that verification MUST FAIL immediately if any TODO/FIXME/unimplemented markers are present in modified/target files.
- Leaving TODO tokens in production code causes verification to fail and prevents further checks from running.
- Implementing a minimal redirect flow is necessary because the public API spec requires redirect following (configurable `max_redirects`) and leaving it unimplemented will block marking the feature as complete.

---

## ACTION ITEM SUMMARY (developer checklist)
- [ ] Implement `Redirecting` state and transitions in `backends/foundation_core/src/wire/simple_http/client/task.rs`.
- [ ] Add/adjust unit tests covering redirect handling.
- [ ] Remove literal `TODO`/`FIXME` tokens from `client` module.
- [ ] Re-run CHECK #1 and the verification sequence.
- [ ] Update `specifications/02-build-http-client/features/public-api/PROGRESS.md` with the verification results and any remaining tasks.

---

If you want I will proceed to implement the minimal redirect-handling change and the three unit tests described above, then re-run the verification steps and produce a new verification report in this same feature directory. Please confirm and I will continue.