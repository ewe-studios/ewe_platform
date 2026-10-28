---
feature: "Docker client and foundation_deployment: generated requests, streamed bodies, CI for integration targets"
status: "pending"
priority: "high"
depends_on: ["03-client-body-readiness"]
branch: "fix/io-12-docker-deployment"
crates: ["foundation_deployment_docker", "foundation_deployment", "foundation_openapi"]
backlog: "docs/bug-backlog.md §6 (4 entries)"
---

# Feature 12: Docker client and `foundation_deployment`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** JavaScript Object Notation (JSON); continuous integration (CI);
application programming interface (API); pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4 (rules,
regression test first, 20-run flake check). Each entry below is one commit.

## T0. Check that every entry is still open

For each entry: find the code at the named place on current `master`. If it is already fixed,
delete the backlog entry, name the commit that fixed it in the PR body, and skip the entry.
Write a table in the PR body: entry → open / fixed by `<commit>` / partly fixed.

## E1. Seven Docker client methods build requests by hand

- **Where:** `foundation_deployment_docker`: `upload_to_container`, `get_container_archive_info`,
  `exec_inspect`, `system_info`, `system_ping`, `system_data_usage`, `network_inspect`.
- **Problem:** they read success bodies whole with no cap, throw away error bodies, ignore
  `DockerLimits`, carry stale docs; `upload_to_container` takes the whole tar as a `Vec<u8>`;
  `system_data_usage` returns untyped JSON (rule 5).
- **Fix:** route all seven through the generated request/response functions and types. Where a
  generated type is missing or wrong, fix the generator (`foundation_openapi`) and regenerate;
  do not hand-write the type. `upload_to_container` takes a body stream. Error bodies are read up
  to `DockerLimits`' error-body cap (status first). Docs say what the method does now.
- **Tests** (`foundation_deployment_docker/tests/`, against the existing mock daemon):
  `system_data_usage_returns_typed_struct`; `upload_to_container_streams_without_buffering`
  (a 64 MiB generated tar; assert peak extra memory under 8 MiB — measure with a counting body
  source, not RSS); `exec_inspect_error_body_is_capped_and_reported`; one success test per
  method.

## E2. `send_json` has no body cap

- **Where:** `foundation_deployment` `send_json`.
- **Fix:** `max_json_body` on `BodyLimits`, default 1 MiB (nginx's `client_max_body_size`
  default; if `BodyLimits` already has a JSON-sized field, reuse its value and say so). Over the cap: an error that names the
  limit; the body is not read past the cap.
- **Test:** `send_json_rejects_body_over_cap` (cap 1 KiB, 4 KiB body).

## E3. The `integration-tests` targets never compile in CI

- **Where:** `ci/packages.toml`, `foundation_deployment_docker`.
- **Fix:** a CI run that builds `--features docker,integration-tests` tests with
  `cargo test --no-run`. Daemon-backed runs stay manual.
- **Verify:** `python3 scripts/ci/ci.py plan` shows the new run; the run passes in CI.

## E4. `LogFrameDecoder::decode` returns `None` for two different cases

- **Problem:** "need more bytes" and "skipped an unknown frame type" both return `None`.
- **Fix:** return an enum (`NeedMore`, `Skipped`, `Frame(..)`), update callers.
- **Test:** `log_frame_decoder_distinguishes_need_more_from_skip`.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_deployment_docker foundation_deployment foundation_openapi foundation_deployment_platform; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo test -p foundation_deployment_docker --features docker,integration-tests --no-run --profile dev
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table in the PR body. Every open entry fixed and its backlog entry deleted.
- [ ] New tests fail on the base code and pass now; 20-run flake check passes.
- [ ] No hand-written request in the seven methods (`rg -n 'Request::new|PreparedRequest' src` in
      the crate shows none in them).
- [ ] CI green, including the new `integration-tests` build.

## Review checklist (main session)

- [ ] Generator fixes are in the generator, and the generated code is regenerated, not edited.
- [ ] Upload streaming is real (body stream end to end).
