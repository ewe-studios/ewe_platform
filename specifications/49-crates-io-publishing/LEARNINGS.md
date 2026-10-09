# Learnings — crates.io Publishing

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
