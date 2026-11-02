# Feature 14 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** OpenAPI Specification (OpenAPI); pull request (PR); AeroSpace
and Defence Industries Association of Europe (ASD), the owner of the STE standard.

## Landed

- E1 (generator + Docker + platform + Cloudflare):
  - `foundation_openapi` emits `Option<bool>` for `type: boolean` query parameters and
    serialises `true`/`false`; array query parameters stay `Vec<String>` serialised as repeated
    keys. (`src/endpoint.rs`, `src/extractor.rs`, `src/unified/generator.rs`.)
  - `foundation_deployment_docker` `ImageBuildOptions.tag` became `tags: Vec<String>`; the
    hand-written client passes boolean args as `bool`; version 0.1.4 → 0.1.5.
  - `foundation_deployment_platform` passes `tags` and requires docker 0.1.5.
  - `foundation_deployment_cloudflare` regenerated (114 boolean query parameters now
    `Option<bool>`); version 0.1.3 → 0.1.4; consumers `foundation_proxy` and
    `foundation_keychain` bumped to 0.1.4.
  - Tests: `foundation_openapi/tests/query_params.rs` (two golden-output tests) and
    `foundation_deployment_docker/tests/images_tests.rs` (`image_build_sends_two_tags`).
  - Test-infra fix folded in: the mock daemon now binds its Unix socket under a short `/tmp`
    path (not the deep `std::env::temp_dir()`), so the mock tests run on macOS (SUN_LEN).
  - Validation under `--profile uat`: openapi tests green; docker tests green; cloudflare
    `--all-features` check green; platform check green. No new clippy warnings from this change.

## Next

- E2 — typed D1 `read_replication` update bodies (backlog §8 entry still open).
- E3 — move inline tests out of `unified/emit.rs` and `unified/analyzer.rs`.
- E4 — `genapi` `--profile` pass-through and `generate hetzner` selection honour.
- Recorded (backlog §8): a dedicated clippy-cleanup pass for `foundation_openapi` (~112 lints)
  and `foundation_deployment_docker` (~231 lints), none from this feature.
