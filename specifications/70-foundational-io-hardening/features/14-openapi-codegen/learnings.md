# Feature 14 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** OpenAPI Specification (OpenAPI); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard.

## E1. Typed booleans and arrays in query parameters

- The generator typed every query parameter as `Option<String>` in one place. There was no
  branch for the parameter's `schema.type`. The fix adds one field to the request args struct
  per parameter kind:
  - `type: boolean` becomes `Option<bool>`. It serialises as `true`/`false`, not a string.
  - `type: array` becomes `Vec<String>`. It serialises as one repeated `key=value` pair per
    value.
  - Everything else stays `Option<String>`.
- The array path already existed. Only the boolean path was missing. The spec said both, but
  the array case (`image_build` `t`) was already emitted as repeated keys. The real gap was
  that Docker's `image_build` exposed a single `tag` field, so a caller could not send several
  tags even after the generator supported arrays.
- Three files in `foundation_openapi` carry the change:
  - `src/endpoint.rs` — `EndpointInfo.boolean_query_params` and
    `extract_boolean_query_params`.
  - `src/extractor.rs` — fills `boolean_query_params` in both extractor paths (OpenAPI and
    Google Cloud Platform (GCP)).
  - `src/unified/generator.rs` — emits the `Option<bool>` field and the `true`/`false`
    serialisation. The serialisation line uses doubled braces (`{{`/`}}`) because it is a
    format string.
- Docker's `image_build` `t` parameter is the only `type: array` query parameter in the Docker
  spec. The `/containers/{id}/stop` and `/restart` `t` parameters are `type: integer`, not
  array. Only `/build` `t` changed to `{"type": "array", "items": {"type": "string"}}`.
- `ImageBuildOptions.tag` (one tag) became `tags: Vec<String>` (several tags). This is a
  breaking public-API change, so `foundation_deployment_docker` went from 0.1.4 to 0.1.5, and
  `foundation_deployment_platform`'s requirement went from 0.1.4 to 0.1.5 in the same change.
  The platform's `docker/image.rs` now passes `tags: vec![self.tag.clone()]`.
- 28 compile errors appeared in the Docker hand-written client after regeneration, because the
  generated boolean args became `Option<bool>` but the hand-written code still passed
  `Option<String>`. The fix was to remove the `.to_string()`/`.map(|v| v.to_string())` wrappers
  on boolean fields. The Docker spec declares `pull` as `type: string`, so
  `pull: Some(opts.pull.to_string())` stayed unchanged.
- Provider regeneration is not one command. Each provider's `lib.rs` documents its command:
  - `digitalocean` and `hetzner` need an `--include-operation` selection. A naive full-spec
    generation of digitalocean produced 243 untracked files (447 endpoints). The correct
    selection produces no diff, because the selected operations have no boolean or array query
    parameters.
  - `cloudflare` and `docker` generate the full spec.
- `genapi` needs `--features cli`. Without it the binary refuses to start.
- The `genapi` post-step runs `cargo fix --lib -p <crate> --allow-dirty --features <feature>`.
  It has no `--profile` (see E4, still open).
- The mock-daemon test had two gotchas, both fixed and re-testable:
  - The request target is versioned after negotiation (`/v1.53/build?...`), not `/build`, so a
    mock route must match `contains("/build")` rather than `starts_with("/build")`.
  - The macOS temp directory is deep, so a Unix socket path there can exceed `SUN_LEN` (104
    bytes). `std::env::temp_dir()` returns a deep path on macOS; the kernel rejects the bind.
    The fix (backlog §6, deleted in this change) is to bind the mock socket under a short
    `/tmp` path. Eight docker test files each had a local `temp_socket_path` helper; all now
    use `std::path::PathBuf::from("/tmp")`. Linux CI is not affected because its `temp_dir()`
    is already `/tmp`.
- `cloudflare` regenerates cleanly with the full spec (no `--include-operation` selection).
  114 boolean query parameters flipped `Option<String>` → `Option<bool>`; no array query
  parameters changed. The hand-written `client.rs` / `dns_ops.rs` / `types.rs` / `workers/`
  files needed no change: their `.to_string()` calls are on real string fields, not booleans.
  The `cargo fix` post-step compiled the crate successfully.
- `foundation_deployment_cloudflare` is a public crate, so its generated `pub` field type
  change (`Option<String>` → `Option<bool>`) is a breaking API change. It went 0.1.3 → 0.1.4,
  and both consumers (`foundation_proxy`, `foundation_keychain`) were bumped in the same change.
- `cargo clippy` on the touched crates reports a large pre-existing warning set
  (`foundation_openapi` ~112, `foundation_deployment_docker` ~231). None come from this change;
  the new code (`boolean_query_params`, `extract_boolean_query_params`, the `true`/`false`
  serialisation) is clean. The debt is recorded in backlog §8 for a dedicated cleanup pass.

## E2. Typed D1 `read_replication` update bodies

- The root cause is not a `$ref` in `allOf`/`oneOf`. It is an inline object. Cloudflare declares
  the named schema `d1_read_replication_details` (an object with one required `mode` field). The
  two update bodies (`d1_database_update_request_body` and
  `d1_database_update_partial_request_body`) write the same object inline as the value of
  `read_replication` instead of a `$ref`. They are byte-identical, but a `$ref` is what the
  generator names a type from. An inline object generates as an untyped
  `HashMap<String, serde_json::Value>`.
- `canonicalize_operations` does not fix it. It hoists only operation-level schemas (request and
  response bodies under `paths`), not objects already inline inside `components/schemas`. The
  update bodies are already in `components/schemas`, so their inline `read_replication` object
  was never hoisted.
- The fix is a new transform, `reuse_identical_schemas`. It walks every named schema in
  `components/schemas`. When a property is an inline object byte-identical to a named schema, it
  replaces the object with a `$ref` to that schema. An inline object that matches nothing is
  left as it was. The pipeline runs it after `canonicalize_operations`, before the second prune.
- The byte comparison is `serde_json::Value` equality. The two objects match only when their
  JSON is the same, including `description`, `required` and the nested `$ref` on `mode`. A
  conservative match means the transform rewrites only true duplicates.
- The same transform caught three more untyped fields in the Cloudflare spec, all correct:
  - `AbuseReportsMitigationAppealResult.result_info` now reuses the shared
    `ListAbuseReportsResponseResultInfoShape` (the shape's doc comment changed from "4 generated
    types share" to "5 generated types share").
  - `ImagesImageVariantPublicRequest.hero` now uses `ImagesImageVariantDefinition`.
  - The two D1 update bodies now use `D1ReadReplicationDetails`.
- `D1ReadReplicationMode` is a `String` alias (`pub type D1ReadReplicationMode = String`), not a
  Rust enum. The generator does not emit string enums. So the test builds
  `D1ReadReplicationDetails { mode: "auto".to_string() }`, not `mode: Auto`. The feature's test
  line wrote `Auto` as if the mode were an enum; the generated type is a plain string.
- The compile-level test lives in `foundation_deployment_cloudflare/tests/types_tests.rs` as
  `d1_update_body_uses_typed_read_replication`. It builds both update bodies, serialises them to
  `{"read_replication":{"mode":"auto"}}` / `{"read_replication":{"mode":"manual"}}`, and
  deserialises one back.
- Validation under `--profile uat` hit "No space left on device" (OS error 28). The LLVM `uat`
  build of `foundation_deployment_cloudflare` (plus the heavy `turso` dependency tree) needed
  more space than was free. Deleting `target/uat/incremental` (8.2 GB) freed enough to finish.
  The disk is a recurring constraint on this machine; see the io-hardening swarm memory.

## E3. Inline tests moved out of `unified/emit.rs` and `unified/analyzer.rs`

- Rule 7 moves every test into `tests/`, and tests only public API there. The two inline test
  modules tested internal generator machinery, so the right thing to make public was the
  generator's intermediate representation (IR).
- `unified/mod.rs` changed `mod emit;` / `mod analyzer;` to `pub mod emit;` / `pub mod analyzer;`.
  `unified` was already `pub`, so the new paths are `foundation_openapi::unified::emit` and
  `foundation_openapi::unified::analyzer`.
- The `emit` IR became public: `Field`, `TypeBody`, `TypeDef`, `ModuleFile`, `SCHEMA_DERIVES`,
  `ARGS_DERIVES`, and the three shrink passes `hoist_envelopes`, `hoist_shared_shapes`,
  `dedupe_within_modules`. `TypeDef::structure`, `TypeDef::alias`, `TypeDef::render`,
  `ModuleFile::new`, `ModuleFile::push`, and `ModuleFile::types` became `pub`. `ModuleFile.items`
  became private (it was `pub`); callers reach types through `types()`.
- `analyzer::extract_group_name` became `pub` (with a doc comment it did not have before).
- The test-only helper `duplicate_names` moved into `tests/unified_emit.rs` as a local function.
  It was `#[cfg(test)] pub(crate)` in the source file; it is not public API.
- The seven emit tests and one analyzer test moved verbatim to `tests/unified_emit.rs` and
  `tests/unified_analyzer.rs`. The test count did not drop.
- A Rust visibility detail: enum variant fields always share the enum's visibility. A `pub`
  qualifier on a variant field is a compile error (E0449). Making `TypeBody` a `pub enum` is
  enough; its `Struct`/`Alias` fields are then public.
- New public API means a version bump: `foundation_openapi` 0.0.3 → 0.0.4, and its only consumer
  `foundation_codegentools` requirement 0.0.3 → 0.0.4.

## Dead ends

- Regenerating `digitalocean`/`hetzner` without the selection wasted a full generation pass and
  needed a `git clean`. The per-crate command in `lib.rs` is the source of truth.
