# Learnings — crates.io Publishing

## 2026-11-09: two release failures in run 42516594075

_Written in ASD-STE100 Simplified Technical English (STE)._

### A versioned dev-dependency can make a publish cycle (#442)

- Cargo keeps a dev-dependency in the published manifest when it has a `version`. It
  removes a dev-dependency that has only a `path`.
- `foundation_nostd` had `[dev-dependencies.foundation_core]` with `path` and
  `version = "0.0.19"`. `foundation_core` depends on `foundation_nostd`. So
  `cargo package -p foundation_nostd` had to find `foundation_core 0.0.19` on crates.io,
  and that version cannot publish before `foundation_nostd`.
- Root cause of the late detection: `scripts/ci/package_workspace.py` packages against a
  `[patch.crates-io]` table that maps every workspace crate to its path. The patch
  resolves the dependent locally, so the dry run passed.
- Fix: remove the `version`. A test-only dependency on a crate that depends back on you
  must be path-only. `package_workspace.py` now runs `dev_dependency_cycles` before it
  packages and prints each cycle.
- Check: with the fix, `cargo package -p foundation_nostd --no-verify` passes against the
  real crates.io index, and the packaged `Cargo.toml` has no `foundation_core`.
- A scan of the workspace found no other cycle.

### release-plz loads old commits, so its cargo must read old manifests (#443)

- release-plz copies the repository to a temporary directory. It runs `cargo package` at
  the commit of each crate's last release, to compare the packages.
- Commits before `df86d7e6` (21 October 2026) have the nightly-only `codegen-backend`
  profile key in the root `Cargo.toml`. Stable cargo 1.98 stops with "feature
  `codegen-backend` is required".
- So "keep the workspace loadable on stable" is not enough for the release-PR job. That job
  also needs a cargo that loads every past release commit.
- Fix: the release-PR job uses the pinned nightly from `rust-toolchain.toml` and sets
  `CARGO_UNSTABLE_CODEGEN_BACKEND=true`. It publishes nothing. The publish job stays on
  stable, because it packages only the current commit.
- The job can move back to stable when every crate has a release after `df86d7e6`.

### crates.io state is not the same as "all crates publish"

- On 2026-11-09, 44 of 104 publishable crates were not on crates.io yet, and 38 were at an
  older version. The failed publish job stopped them. Read the crates.io API (with a
  `User-Agent` header) against `cargo metadata --no-deps` to check the real state.

## Crate Name Availability (2026-06-16)

- All 38 crate names confirmed available on crates.io
- Initial scan flagged 4 names as "TAKEN" (`foundation_core`, `foundation_macros`, `foundation_nostd`, `foundation_wasm`) — but direct verification via WebFetch confirmed they are all available (likely yanked previously or stale cache from the agent)
- The crates.io API (`https://crates.io/api/v1/crates/<name>`) returned 404 for all 4, confirming availability

## Metadata Requirements

- 4 crates were missing `description` fields: `foundation_auth`, `foundation_cedar`, `foundation_logging`, `foundation_netio`
- Descriptions were added during feature 00 — all 38 crates now have descriptions
- No crates have `readme` fields yet — this is tracked in feature 01
- `categories` field is optional but recommended for crates.io discoverability

## Infrastructure Crates

- `infrastructure_llama_bindings` and `infrastructure_llama_cpp` are FFI bindings that require C compilation (llama.cpp)
- These may have longer build times and platform-specific issues during `cargo publish`
- User confirmed both should be published to crates.io

## State Re-assessment (2026-07-22, taken over by agentic-reliability work)

The spec was written for **38** crates; the workspace now has **57** publishable
crates (backends/* + infrastructure/*, excluding `foundation_platform` — another
agent owns it — and `infrastructure_docker` which is `publish = false`). New
since the spec: foundation_connectrpc(+_codegen), foundation_cronjobs,
foundation_iogate, foundation_keychain, foundation_proxy, foundation_repl,
foundation_sshkit, foundation_vectors, foundation_wireguard, foundation_wasmtime,
foundation_auth_ui, foundation_buildtools, and the split
foundation_deployment_{cloudflare,digitalocean,docker,hetzner,huggingface,platform}.

**Skip list (per owner):** everything under `/crates/*` (already excluded from
the workspace), `foundation_platform`, `infrastructure_docker`.

### Published vs local (checked via crates.io API, User-Agent required or 403)

- **Already published (20):** foundation_arrow, foundation_codegen,
  foundation_codegentools, foundation_compact, foundation_conditional,
  foundation_config, foundation_core, foundation_db, foundation_errstacks,
  foundation_jsonschema, foundation_logging, foundation_macros, foundation_netio,
  foundation_nostd, foundation_openapi, foundation_theme, foundation_ui_traits,
  foundation_wasm, infrastructure_llama_bindings, infrastructure_llama_cpp.
- **NOT yet published (~36):** foundation_ai, foundation_auth(+_ui),
  foundation_browser, foundation_buildtools, foundation_cedar,
  foundation_connectrpc(+_codegen), foundation_cronjobs, foundation_deployment(+6
  provider crates), foundation_html, foundation_http, foundation_iogate,
  foundation_keychain, foundation_nativeapis, foundation_packager,
  foundation_proxy, foundation_repl, foundation_runtimes, foundation_shell,
  foundation_signals, foundation_sshkit, foundation_testbed, foundation_testing,
  foundation_toolings, foundation_ui_components, foundation_vectors,
  foundation_wasmtime, foundation_wasm_ui, foundation_wireguard.

### Version bumps required (17 of the 20 published crates changed since publish)

crates.io rejects re-publishing an existing version, so any already-published
crate that changed needs a bump BEFORE it can be re-published, and every
dependent's dep-spec version must cascade to match.

| crate | published | commits since | needs bump |
|---|---|---|---|
| foundation_netio | 0.0.1 | 170 | yes |
| foundation_core | 0.0.3 | 65 | yes |
| foundation_db | 0.0.1 | 35 | yes |
| foundation_macros | 0.0.4 | 34 | yes |
| foundation_openapi | 0.0.1 | 27 | yes |
| foundation_wasm | 0.0.3 | 24 | yes |
| infrastructure_llama_cpp | 0.0.1 | 13 | yes |
| foundation_codegentools | 0.1.0 | 12 | yes |
| infrastructure_llama_bindings | 0.0.1 | 7 | yes |
| foundation_compact | 0.2.0 | 6 | yes |
| foundation_nostd | 0.0.4 | 6 | yes |
| foundation_ui_traits | 0.0.1 | 3 | yes |
| foundation_jsonschema | 0.0.1 | 2 | yes |
| foundation_arrow / foundation_codegen / foundation_theme | — | 1 each | yes |
| foundation_conditional / foundation_config / foundation_errstacks / foundation_logging | — | 0 | no (unchanged) |

### Key facts for whoever executes the publish

- Internal deps already use `path` + `version` (correct for crates.io).
- crates.io token IS configured (`~/.cargo/credentials.toml` + `CARGO_REGISTRY_TOKEN`).
- No `cargo-release` / `cargo-workspaces` installed; version-bump cascade is
  manual (or install a tool).
- **Publishing is irreversible** — a version can only be yanked, never replaced.
  FFI crates (llama_bindings bundles llama.cpp C++) and feature-gated crates
  (cuda/metal/wasm) are the highest dry-run risk; validate each with
  `cargo publish --dry-run` first.
