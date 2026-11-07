---
feature: "Verify OpenAPI boolean query parameters generate Option<bool>, then close #317"
status: "pending"
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

- [ ] Run the three `rg` checks in section 2 and record the lines.
- [ ] Confirm a test covers the boolean query mapping. If none exists, add
      `a_boolean_query_parameter_generates_option_bool` in
      `backends/foundation_openapi/tests/` (registered in its test module): build a small
      endpoint with one boolean query parameter, run the generator, assert
      `Option<bool>` and the `"true"`/`"false"` serialiser.
- [ ] Run the verify commands.
- [ ] Close #317 in the PR body with the evidence.

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
