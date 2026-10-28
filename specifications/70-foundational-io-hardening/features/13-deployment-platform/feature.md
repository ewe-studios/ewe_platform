---
feature: "foundation_deployment_platform: log waits, Docker-gated tests, Vagrant lookup, inline tests, warnings"
status: "pending"
priority: "medium"
depends_on: ["12-docker-deployment"]
branch: "fix/io-13-deployment-platform"
crates: ["foundation_deployment_platform", "foundation_deployment", "foundation_proxy"]
backlog: "docs/bug-backlog.md §7 (6 entries); §9 Docker-gated tests (shared helper)"
---

# Feature 13: `foundation_deployment_platform`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); JavaScript Object Notation
(JSON); pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the
owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. One
commit per entry.

## T0. Check that every entry is still open

As in feature 12: a table in the PR body, entry → open / fixed by `<commit>` / partly fixed.

## E1. `wait_for_stdout` re-reads the whole log, misses split messages, scans stderr

- **Fix:** follow the log stream (`follow=true`, stdout only) and match across frame
  boundaries with a rolling buffer whose size is the pattern length plus one frame (config).
- **Tests:** `wait_for_stdout_matches_across_frames` (the pattern split across two frames);
  `wait_for_stdout_ignores_stderr`; `wait_for_stdout_reads_each_byte_once` (count bytes the mock
  sends: total equals the log size, not a multiple).

## E2. A shared "Docker daemon is reachable" test helper

- **Problem:** `docker_available()` in `tests/container_integration.rs`, `ssh_backend_tests.rs`,
  `runner_integration_tests.rs`, `image_build_integration.rs` (and
  `foundation_proxy/tests/proxy_integration_tests.rs`) is true when `/var/run/docker.sock`
  exists, even with no daemon; `docker_container_macro_supports_async_functions` fails instead of
  skipping.
- **Fix:** one helper (in the crate's `testing` feature or `foundation_testing`) that connects
  with `DockerClient::connect_with_defaults` and pings. All Docker-gated tests use it. With no
  daemon they skip with a printed reason (they stay real tests when a daemon is there).
- **Test:** run the gated tests on this machine (no daemon): they report skipped, not failed.

## E3. Vagrant box lookup runs `curl`

- **Where:** `src/import/mod.rs` near line 512.
- **Fix:** use the `foundation_netio` HTTP client (async, `collect_bytes_async` with a size
  cap from config), parse into a typed struct, structured errors. Delete the stale "tokio
  runtime conflicts" comment.
- **Test:** `vagrant_lookup_parses_metadata` against a local HTTP server; `vagrant_lookup_reports_http_error`.

## E4. Inline tests in `docker/wireguard.rs` (rule 7)

- **Fix:** move to `tests/`; count before and after (must not drop).

## E5. Warnings

- Deprecated `SimpleHttpClient` in `wait_for.rs` and `foundation_deployment/src/provider_client.rs`
  → `HttpClientBuilder`. Unused import `PortMapping` (`docker/wireguard.rs:21`) → remove.
- **Verify:** `cargo check -p foundation_deployment_platform -p foundation_deployment --all-targets 2>&1 | rg warning` is empty.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_deployment_platform foundation_deployment foundation_proxy; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted (§7 and the §9 Docker-helper entry).
- [ ] New tests fail on the base code and pass now; 20-run flake check passes.
- [ ] No warnings in the two crates. CI green.
