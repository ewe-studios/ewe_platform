# Feature 22: learnings

## E1. Per-worktree target directories

- `scripts/ci/ci.py` already spread a package environment over every step in
  `run_package`. Setting `CARGO_TARGET_DIR` there is the one place that covers
  every step and every worktree.
- The worktree name is the basename of the repository root (`ROOT.name`). A
  worktree checkout has its own directory name, so the target directory stays
  separate.
- Do not use `RUSTFLAGS=-Cmetadata=<id>`. That rebuilds all dependencies.

## E2. Delete `crates/ewe_deployables`

- The decision check is `rg -n "use ewe_deployables|ewe_deployables::" --glob
  '*.rs'` outside the crate. It returns zero hits. No `Cargo.toml` depends on
  it. The only references are specification documents and the backlog entry.
- The per-provider deployment crates (`foundation_deployment_cloudflare`,
  `foundation_deployment_docker`) cover the old crate, so deletion is correct.
- The `.agents/AGENTS.md` doc note for E1 is left uncommitted in the `.agents`
  submodule. That submodule shares its working tree with rules 11 and 13 that
  the main session is still drafting, so this pass does not commit there.

## E5. Gate the `Bindgen` command binding

- The warning came from binding `args` in the match arm and using it only in
  the `#[cfg(feature = "wasm-bindgen-test")]` branch. A `run_bindgen_command`
  helper, one per cfg arm, removes the warning: the `wasm-bindgen-test` arm
  uses `args`, the other arm takes `_args`.
- cargo nightly 1.100.0 surfaces a new `unused_dependencies` manifest lint.
  `foundation_testbed` declares `deno_v8` only to enable its V8 backend for
  `deno_core` (feature unification), so the lint flags it. This is not the E5
  warning; it is recorded in the backlog.

## E6. A separate mise environment for the wasm tools

- mise environment files overlay the base `mise.toml`; they do not replace it.
  Pinning only deno and wasm-bindgen in `mise.wasm.toml` is not enough, because
  the base `[tools]` list is still merged in. The isolation comes from
  `[settings] auto_install = false` plus an explicit `mise install deno
  wasm-bindgen` in the task body.
- Verified: `MISE_ENV=wasm mise config get settings.auto_install` returns
  `false`; the two wasm tasks appear only under `MISE_ENV=wasm`; the base
  `mise tasks` no longer lists them.

