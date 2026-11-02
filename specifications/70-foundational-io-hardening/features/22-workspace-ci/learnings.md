# Feature 22: learnings

## E1. Per-worktree target directories

- `scripts/ci/ci.py` already spread a package environment over every step in
  `run_package`. Setting `CARGO_TARGET_DIR` there is the one place that covers
  every step and every worktree.
- The worktree name is the basename of the repository root (`ROOT.name`). A
  worktree checkout has its own directory name, so the target directory stays
  separate.
- Do not use `RUSTFLAGS=-Cmetadata=<id>`. That rebuilds all dependencies.
