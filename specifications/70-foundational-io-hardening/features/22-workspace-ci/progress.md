# Feature 22: progress

Branch: `fix/io-20-vfs-process-ipc`.

## Done

- **E1.** `scripts/ci/ci.py` now picks a target directory per worktree. The
  default is `<repo>/target`. When `CARGO_TARGET_DIR` is set, the script uses
  `$CARGO_TARGET_DIR/<checkout-name>`. `.agents/AGENTS.md` documents this rule
  where worktrees are described. The backlog entry "Shared target dir
  clobbering" is deleted in the same commit.
- **E2.** `crates/ewe_deployables` is deleted. No code uses it, and the
  per-provider deployment crates cover it. The stale comment in the workspace
  `Cargo.toml` is removed. The backlog entry "`crates/ewe_deployables` doesn't
  build" is deleted in the same commit.

## In progress

- None.

## Next

- E5: gate `Command::Bindgen(args)` in `foundation_testbed`.
- E6: split the wasm mise tools into their own mise environment.
- E7: fix the `foundation_keychain` wasm warnings.
- E8: fix the `foundation_deployment_cloudflare` wasm warning.
- E4 and E3 stay for a later pass.
