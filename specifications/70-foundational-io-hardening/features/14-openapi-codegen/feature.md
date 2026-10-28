---
feature: "OpenAPI generator: typed booleans and arrays in queries, typed D1 read replication, inline tests, genapi flags"
status: "pending"
priority: "medium"
depends_on: []
branch: "fix/io-14-openapi-codegen"
crates: ["foundation_openapi", "foundation_codegentools", "foundation_deployment_docker", "foundation_deployment_cloudflare"]
backlog: "docs/bug-backlog.md §8 (3 entries); D1 read_replication untyped (recorded on PR #158's branch)"
---

# Feature 14: OpenAPI generator

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** OpenAPI Specification (OpenAPI); continuous integration (CI);
pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. `CLAUDE.md`
rule 5: fix the generator and regenerate; never hand-edit generated code.

## T0. Check that every entry is still open (table in the PR body)

## E1. Boolean query parameters are generated as `Option<String>`

- **Example:** Docker `ContainerStatsArgs.stream` / `one_shot`.
- **Fix:** the generator emits `Option<bool>` for `type: boolean` query parameters and
  serialises `true`/`false`. Array query parameters (`type: array`) become `Vec<T>` serialised
  as repeated keys (Docker `image_build` `t` needs this: several tags). Regenerate every
  provider crate that uses the generator; list them in the PR body.
- **Tests** (`foundation_openapi/tests/`): `boolean_query_param_is_bool`;
  `array_query_param_repeats_key` (golden output of a small spec); Docker:
  `image_build_sends_two_tags` against the mock daemon.

## E2. D1 `read_replication` update bodies are untyped

- **Where:** `foundation_deployment_cloudflare/src/generated/workers/mod.rs`:
  `D1DatabaseUpdateRequestBody.read_replication` and
  `D1DatabaseUpdatePartialRequestBody.read_replication` are `HashMap<String, serde_json::Value>`;
  create and details use `D1ReadReplicationDetails`.
- **Fix:** find why the generator does not resolve the same schema for the update bodies (a
  `$ref` inside `allOf`/`oneOf`, or an inline object); fix it; regenerate.
- **Test:** `d1_update_body_uses_typed_read_replication` (compile-level: build an update body
  with `D1ReadReplicationDetails { mode: Auto }` and serialise it to the expected JSON).

## E3. Inline tests in `unified/emit.rs` and `unified/analyzer.rs` (rule 7)

- Move to `tests/`; counts must not drop.

## E4. genapi ignores its flags in two places

- Its post-step `cargo fix` always runs in the dev profile: pass `--profile` through.
- `generate hetzner` ignores the crate's declared operation selection: honour it.
- **Tests:** a unit test of the command builder for `--profile`; a test that `generate` with a
  selection file emits only the selected operations (golden list).

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_openapi foundation_codegentools foundation_deployment_docker foundation_deployment_cloudflare foundation_deployment_hetzner foundation_deployment_digitalocean; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
git diff --stat origin/master -- '*/generated/*' | tail -1   # regenerated files only; explain the size in the PR body
```

## Done when

- [ ] T0 table; entries fixed; backlog entries deleted.
- [ ] Generated code changed only by regeneration (the PR body names the generator command).
- [ ] CI green.
