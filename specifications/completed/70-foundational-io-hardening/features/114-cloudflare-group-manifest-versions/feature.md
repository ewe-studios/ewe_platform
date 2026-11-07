---
feature: "Generated Cloudflare group manifests read their siblings' versions"
status: "done"
lane: "now"
model: "sonnet"
branch: "spec70/io-hardening-final"
crates: ["foundation_openapi", "foundation_codegentools"]
catalogue_rows: []  # docs/bug-backlog.md §16
---

# Feature 114: generated group manifests carry current versions

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05)

The job `foundation_deployment_cloudflare` fails on every run. Run 42029393923, job
127252605990:

1. 13:07:24 UTC: the `build` step starts and exits 0 at 13:12:59.
2. 13:11:30 UTC: the build prints
   `warning: foundation_deployment_cloudflare@0.2.0: regenerated cloudflare's src/generated/`.
3. 13:13:00 UTC: the `clippy` step fails in `cargo metadata` with exit 101:

    ```
    error: failed to select a version for the requirement `foundation_netio = "^0.0.4"`
    candidate versions found which didn't match: 0.0.6
    location searched: /__w/ewe_platform/ewe_platform/backends/foundation_netio
    required by package `foundation_cloudflare_access v0.1.0 (...)`
    ```

**Root cause:** `UnifiedGenerator::write_group_crate` in
`backends/foundation_openapi/src/unified/generator.rs` writes a manifest for every group
crate. It hard-codes `foundation_netio`'s requirement as `version = "0.0.4"`. The crate is
0.0.6. The committed manifests say 0.0.6, so the build script sees a stale output, rewrites
all 42 group manifests with 0.0.4, and the next `cargo metadata` cannot resolve the
requirement.

The same function also hard-codes `foundation_deployment = "0.0.3"`,
`foundation_macros = "0.0.7"` and `foundation_cloudflare_types = "0.1.0"`. These are
correct today, and they carry the same latent bug.

The build step passes and the clippy step fails because each `ci.py` step calls
`cargo_metadata()` again. The first call sees the committed manifests; the second sees the
rewritten ones.

## Design

1. Add `unified::group_crate_manifest(siblings, crate_name, safe) -> Result<String,
   GenError>`. It renders the same manifest, and reads every path dependency's version from
   that crate's own `Cargo.toml` (`<siblings>/<dependency>/Cargo.toml`). The generator can
   no longer drift from the crate it points at.
2. `siblings` is the shipped workspace root, never the directory being written.
   `Codegen::check` renders the group crates into a scratch tree, and that tree holds no
   sibling crates: reading the versions from next to the output failed the build with
   `No such file or directory`. The generator therefore takes the version source as its own
   input (`UnifiedGenerator::with_sibling_crates`), and `Codegen` passes the shipped
   `backends/` root even when the group crates go to a scratch directory.
3. Add `GenError::ManifestVersion { path, dependency }` for a sibling manifest with no
   `[package] version`. Read with `get`, not with indexing, so a missing table returns the
   error and does not panic.
4. `write_group_crate` writes the result of `group_crate_manifest`.
5. Bump `foundation_openapi` to 0.0.5, because it gains a public function and a `GenError`
   variant. `foundation_codegentools` requires 0.0.5.
6. Do not change the manifest's content for the versions that are correct today. The
   committed manifests do not need an edit; they already match the fixed generator.

## Tests

`backends/foundation_openapi/tests/group_crate_manifest.rs`:

- `each_path_dependency_requires_its_siblings_version`: a temporary tree with the four
  sibling crates at 9.x versions. Every rendered requirement carries the sibling's version.
  This test fails on the old code, which writes 0.0.4.
- `a_scratch_render_reads_the_shipped_workspace`: the output tree holds no sibling crates,
  and the render still names their versions. This is the shape `Codegen::check` uses, and
  it is the case that failed the Cloudflare build.
- `a_sibling_without_a_version_names_its_manifest`: the error is `ManifestVersion` and it
  names the manifest.
- `committed_group_manifests_require_current_sibling_versions`: walks
  `backends/foundation_cloudflare_*` and checks that every requirement equals the sibling's
  real version. This test fails if a committed manifest drifts again.

`backends/foundation_codegentools/tests/cloudflare_types.rs`:

- `the_full_cloudflare_check_is_current`: runs the same `Codegen::check` the Cloudflare
  build script runs, so the scratch render is exercised by a test and not only by a build.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_openapi --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_openapi
cargo check --profile uat -p foundation_codegentools
# In the Linux CI image, prove the build script no longer regenerates:
cargo build --profile uat -p foundation_deployment_cloudflare
git status --porcelain -- backends/
```

## Done when

- [x] The generator reads every sibling version from its crate.
- [x] The tests pass, including the full `Codegen::check` of the Cloudflare declaration.
- [x] `foundation_openapi` is 0.0.5 and `foundation_codegentools` requires 0.0.5.
- [x] The build script regenerates nothing (`git status` is clean after the build, and the
      build exits 0 in the Linux CI image).
- [x] The `foundation_deployment_cloudflare` job is green: run 42040473182 success on
      `e01e83d80` (PR #179).
- [x] Backlog §16 is deleted.
