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
- E2 (typed D1 `read_replication` update bodies):
  - `foundation_openapi` gained `reuse_identical_schemas` (and its helper `reuse_in_props`), a
    transform that walks `components/schemas` and replaces an inline object property with a
    `$ref` when it is byte-identical to a named schema. The pipeline runs it after
    `canonicalize_operations`. (`src/transform.rs`, `src/pipeline.rs`, `src/lib.rs`.)
  - Two unit tests in `foundation_openapi/tests/transform.rs`:
    `an_inline_object_identical_to_a_named_schema_reuses_it` and
    `an_inline_object_without_a_duplicate_is_untouched`.
  - `foundation_deployment_cloudflare` regenerated. `D1DatabaseUpdateRequestBody.read_replication`
    and `D1DatabaseUpdatePartialRequestBody.read_replication` are now `D1ReadReplicationDetails`
    (was `HashMap<String, serde_json::Value>`). The same transform also typed two more fields:
    `AbuseReportsMitigationAppealResult.result_info` and
    `ImagesImageVariantPublicRequest.hero`.
  - Test: `foundation_deployment_cloudflare/tests/types_tests.rs`
    `d1_update_body_uses_typed_read_replication` (builds both update bodies, serialises and
    deserialises). All 17 cloudflare types tests pass.
  - Validation under `--profile uat`: openapi tests (47 transform + 12 type_resolver + 5 doc)
    green; cloudflare types tests green. No new clippy warnings from the new transform.
  - Backlog §8 `read_replication` entry deleted.
- E3 (inline tests moved out, rule 7):
  - `unified/mod.rs` exposes `pub mod emit;` and `pub mod analyzer;`. The `emit` IR became
    public: `Field`, `TypeBody`, `TypeDef`, `ModuleFile`, `SCHEMA_DERIVES`, `ARGS_DERIVES`,
    `hoist_envelopes`, `hoist_shared_shapes`, `dedupe_within_modules`, plus the constructors
    `TypeDef::structure` / `TypeDef::alias` / `TypeDef::render` and `ModuleFile::new` /
    `ModuleFile::push` / `ModuleFile::types`. `ModuleFile.items` became private.
    `analyzer::extract_group_name` became public.
  - The seven emit tests and one analyzer test moved to `tests/unified_emit.rs` and
    `tests/unified_analyzer.rs`. The test count did not drop (8 tests, all green).
  - `#[must_use]` added to `structure`, `alias`, `extract_group_name`; a `# Errors` doc section
    added to `TypeDef::render`. No new clippy warnings.
  - New public API: `foundation_openapi` 0.0.3 → 0.0.4; `foundation_codegentools` requirement
    0.0.3 → 0.0.4.
  - Validation under `--profile uat`: openapi tests green (incl. the 8 moved tests); clippy
    shows no new warnings from the change.
  - Backlog §8 "Inline tests remain" entry deleted.
- E4 (`genapi` honours its flags):
  - `genapi generate` gained a `--profile <PROFILE>` flag, passed through to the
    `cargo fix` post-step (was hard-coded to the dev profile). The post-step is now
    built by the public `cli::gen_api::cargo_fix_args`, so its argument list is a
    testable unit.
  - `genapi generate` gained `--selection-file <PATH>` (a JSON `{paths,tags,operations}`
    allowlist), and a split-out provider's committed `codegen.json` is honoured when the
    user names no `--include-*` flag. A plain `generate hetzner` now regenerates its six
    declared operations (3 paths, 2 groups) instead of the full 151 endpoints; `--check`
    reports the committed tree up to date. `backends/foundation_deployment_hetzner/codegen.json`
    is the machine-readable twin of its `build.rs` declaration.
  - Public API: `codegen::selection_from_file` (re-exported), `cli::gen_api::cargo_fix_args`.
    `foundation_codegentools` 0.1.2 → 0.1.3; the three split-out provider crates' build-dep
    requirement 0.1.2 → 0.1.3; `BUILD_DEP_VERSION` 0.1.0 → 0.1.3 (a stale const, fixed in passing).
  - Tests: `tests/codegen.rs` `a_selection_file_limits_what_generate_emits`; new
    `tests/gen_api_tests.rs` (`cargo_fix_passes_the_profile_through`,
    `generate_declares_a_profile_flag`, `generate_declares_a_selection_file_flag`).
  - Validation under `--profile uat`: codegentools tests green (22+7+3+8+3+1);
    `generate hetzner --check` reports "Up to date"; `cargo check` of hetzner green. No new
    clippy warnings from the change.
  - Backlog §8 "genapi's flags are ignored in two places" entry deleted; the pre-existing
    clippy-warnings entry now also names `foundation_codegentools` (~67).

## Next

- Feature complete. Recorded (backlog §8): a dedicated clippy-cleanup pass for
  `foundation_openapi` (~112 lints), `foundation_deployment_docker` (~231 lints) and
  `foundation_codegentools` (~67 lints), none from this feature.
