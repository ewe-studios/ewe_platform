# Verification Input — public-api (Rust Verification Agent)

purpose: Provide the Rust Verification Agent with a concise, reproducible checklist and file list to verify the current work for the `public-api` feature of `02-build-http-client`.

SUMMARY
- Scope: Verify the current `public-api` work inside `backends/foundation_core/src/wire/simple_http/client/*` and this feature folder.
- Primary goal: Run the mandatory "Incomplete Implementation Scan" first. If that check passes, run the standard Rust verification sequence (formatting, lints, tests, build, docs, audit, standards).
- Important: This input is for the Rust Verification Agent only. Follow .agents rules and `verification.md` (and Rule 08) — CHECK #1 (incomplete implementations) is mandatory and must fail fast.

FILES TO VERIFY (exact paths)
- backends/foundation_core/src/wire/simple_http/client/api.rs
- backends/foundation_core/src/wire/simple_http/client/client.rs
- backends/foundation_core/src/wire/simple_http/client/pool.rs
- backends/foundation_core/src/wire/simple_http/client/task.rs
- backends/foundation_core/src/wire/simple_http/client/actions.rs
- backends/foundation_core/src/wire/simple_http/client/mod.rs
- backends/foundation_core/src/wire/simple_http/mod.rs
- backends/foundation_core/src/wire/simple_http/impls.rs
- backends/foundation_nostd/src/primitives/wait_duration.rs
- crates/config/src/lib.rs
- crates/watchers/src/handlers.rs
- specifications/02-build-http-client/features/public-api/feature.md
- specifications/02-build-http-client/features/public-api/PROGRESS.md
- specifications/02-build-http-client/features/public-api/COMPACT_CONTEXT.md
- specifications/02-build-http-client/features/public-api/machine_prompt.md

MANDATORY CHECK #1 — INCOMPLETE IMPLEMENTATION SCAN (FAIL-FAST)
1. Run (from repo root):
   - grep -rn --line-number "TODO\\|FIXME\\|unimplemented!\\|todo!" backends/foundation_core/src/wire/simple_http/client || true
   - grep -rn --line-number "TODO\\|FIXME\\|unimplemented!\\|todo!" specifications/02-build-http-client/features/public-api || true
2. If ANY matches are found → STOP further checks and report FAIL.
   - Report must list each file, line number, and the exact matching line.
   - If any TODOs are present, verification must mark CHECK #1 as FAIL per Rule 08.
3. If ZERO matches → proceed to the Rust standard checks below.

RUST STANDARD CHECKS (run only if CHECK #1 passes)
Run these in order and record full outputs. If any check fails, stop and record details.

A. Formatting
- Command:
  - cargo fmt -- --check
- PASS: No formatting diffs.
- FAIL: List files that need formatting.

B. Linting (Clippy)
- Preferred (workspace): cargo clippy --all-targets --all-features -- -D warnings
- If workspace clippy impractical due to unrelated crates, run per-package:
  - cargo clippy --package foundation_core --all-targets --all-features -- -D warnings
- Record:
  - PASS: 0 warnings
  - FAIL: List warnings/errors and files. Note if failures are outside `foundation_core` (report as contextual, but do not block `public-api` verification if CHECK #1 passed — still report).

C. Tests
- Commands:
  - cargo test --package foundation_core --all-features
  - If package tests take long, record summary of results.
- PASS: All tests pass for the `foundation_core` package.
- FAIL: Provide failing test names and output.

D. Build
- Command:
  - cargo build --package foundation_core --all-features
  - Optionally: cargo build --all-features (workspace) — record failures if any
- PASS: Compiles successfully.
- FAIL: Provide compiler errors.

E. Documentation
- Command:
  - cargo doc --no-deps --package foundation_core --all-features
- PASS / FAIL: Provide output.

F. Security Audit (optional)
- Command (if available):
  - cargo audit
- If tool not available, note skipped.

G. Standards / Forbidden Patterns
- Run these grep commands and list findings:
  - grep -r --line-number "unwrap()" backends/foundation_core/src | grep -v "tests/" || true
  - grep -r --line-number "expect(" backends/foundation_core/src || true
  - grep -r --line-number "panic!(" backends/foundation_core/src | grep -v "tests/" || true
- PASS: No forbidden patterns (or only allowed in tests).
- FAIL: List occurrences and suggest fixes.

CHECK OUTPUT & REPORT FORMAT
Produce a Markdown report with the following sections:
- Status: PASS ✅ / FAIL ❌
- Files verified: list
- Check #1: Incomplete Implementation Scan: PASS/FAIL with raw grep output
- Format: command and PASS/FAIL with output
- Lint: command and PASS/FAIL with output (include whether workspace or per-package clippy was used)
- Tests: summary (total, passed, failed) with failing output if any
- Build: success / errors
- Docs: success / errors
- Audit: skipped / vulnerabilities
- Standards: list of forbidden-pattern occurrences
- Recommendations: next actions (explicit, small steps), e.g. which TODOs to remove or convert to tracked issues, which files need edits, and whether changes are safe to commit.
- Exit code: 0 for PASS, non-zero for FAIL

SPECIAL NOTES / CONTEXT (evidence-based)
- Evidence gathered by the implementer (do not assume):
  - A minimal `ConnectionPool` implementation was added to `backends/foundation_core/src/wire/simple_http/client/pool.rs`.
  - A documented TODO block for redirect handling remains in `task.rs` (this still contains the token `TODO` and therefore may trigger CHECK #1).
  - Small doc fixes were applied to `backends/foundation_nostd/src/primitives/wait_duration.rs` and `crates/watchers/src/handlers.rs` to address clippy doc warnings.
  - The `public-api` client types (`ClientRequest`, `SimpleHttpClient`) are present and already implement required user-facing methods; the verification should focus on incomplete markers and lints/tests for `foundation_core`.
- If CHECK #1 finds ANY TODOs in the `client` directory (including task.rs), mark FAIL and list the occurrences. The presence of TODOs means the feature cannot be considered complete.

VERIFICATION AGENT INSTRUCTIONS (what to do next)
1. Perform CHECK #1 exactly as specified. If it fails, prepare the report (see format) and STOP.
2. If CHECK #1 passes, run formatting, clippy (prefer per-package if workspace clippy fails due to unrelated crates), tests, build, docs, audit, and standards checks in the given order.
3. Upload the full report and attach the raw command outputs (or paste them into the report).
4. If the verification FAILs due to TODOs or stubbed functions inside the `client` module, include an actionable list of the minimal fixes required to pass (file path + line excerpt + suggested change).
5. If verification PASSes, mark the specification `public-api` as verified-ready and include the list of files changed and the commit hash to be used for the commit.

VERIFICATION TIMEBOX
- Attempt full CHECK #1 and per-package checks. If workspace clippy or docs takes longer than 10 minutes, report partial results and indicate remaining items to be completed in a follow-up verification run.

CONTACT / HANDOFF
- After producing the report, notify the main agent (or the user) with:
  - The verification report file path
  - PASS/FAIL status
  - The exact commands used and their outputs (or a summarized excerpt plus full attachments)

END OF VERIFICATION_INPUT