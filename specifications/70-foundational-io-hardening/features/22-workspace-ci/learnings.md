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

