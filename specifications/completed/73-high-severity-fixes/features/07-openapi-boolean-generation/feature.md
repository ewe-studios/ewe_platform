---
feature: "Verify OpenAPI boolean query parameters generate Option<bool>, then close #317"
status: "completed"
priority: "high"
depends_on: []
crates: ["foundation_openapi"]
issues: [317]
model: "haiku"
branch: "spec73/f07-openapi-boolean-generation"
---

# Feature 07: OpenAPI boolean generation — verification

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** OpenAPI Specification (OpenAPI); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#317](https://github.com/ewe-studios/ewe_platform/issues/317) with evidence. The
defect is already fixed on master.

## 2. Measured state (2026-11-07)

The issue names `ContainerStatsArgs.stream` and `one_shot`. The generated file
`backends/foundation_deployment_docker/src/generated/stats/mod.rs:179-186` declares both as
`Option<bool>`.

The generator does the same:
- `foundation_openapi/src/unified/generator.rs:2119-2123` emits `Option<bool>` when
  `ep.boolean_query_params` contains the name;
- `generator.rs:2693-2697` serialises it as `"true"` or `"false"`;
- `foundation_openapi/src/endpoint.rs:320` `extract_boolean_query_params` fills the set;
- `type_resolver.rs:146` maps `boolean` to `bool`.

This is a verification feature, not an implementation feature.

## 3. Tasks

- [x] Run the `rg` checks in section 2 and record the lines. The generated sample is
      `foundation_deployment_docker/src/generated/stats/mod.rs:183,185`.
- [x] Confirm a test covers the boolean query mapping. `boolean_query_param_is_bool` in
      `backends/foundation_openapi/tests/query_params.rs` already covers the emitted type and
      serialiser. The agent added `boolean_query_param_is_recorded_on_the_endpoint`, which
      pins the extraction step that feeds the generator.
- [x] Run the verify commands: fmt clean, clippy `-D warnings` clean, 3 tests pass.
- [x] Close #317 in the PR body with the evidence.

## 6. Result (2026-11-07)

Verified. The commit is `1879b9e04`, merged into `spec73/high-severity-fixes` as `0aef12e2a`.
The crate has no `tests/mod.rs`; Cargo discovers `tests/query_params.rs`.

The brief asked for an assertion that `?flag=true` parses to `Some(true)`. The crate is a
client-side generator and has no query-string parser, so that assertion cannot exist here.
The proof is the emitted type (`Option<bool>`) and the emitted serialiser
(`map(|b| if b { "true" } else { "false" })`).

## 4. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_openapi
cargo clippy --profile uat -p foundation_openapi --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_openapi
```

## 5. Done when

- The generated code and the generator are confirmed, with line numbers in the PR.
- A test pins the boolean mapping.
- #317 is closed with the evidence.
