# Spec 61 progress

_Last updated: 2026-11-20. Written in ASD-STE100 Simplified Technical English (STE)._

## Done

| Item | State |
|---|---|
| llama.cpp inside the package | Tracked symlink; the tarball lists 1216 `llama.cpp` files. 5.6 MiB compressed |
| Yank | `infrastructure_llama_cpp 0.0.1` is yanked |
| `build.rs` defaults | `LLAMA_DIR` falls back to `CARGO_MANIFEST_DIR/llama.cpp`; `TOOLS_DIR` is gone; `EMSDK_DIR` is optional |
| Package contents | `**/*.in`, `**/*.cmake` and `LICENSE` added for CMake |
| Consumer build | The extracted tarball builds in `/tmp` with `LLAMA_DIR`, `TOOLS_DIR` and `EMSDK_DIR` unset |
| CI guard | `scripts/ci/package_workspace.py` extracts `infrastructure_llama_bindings` and runs `cargo check --lib` with a clean env |
| llama.cpp update | `b12032` → `b12563` (commit `9fbe30c2a`) |
| API migration | State, model, vocab and grammar calls moved to the new names (R6) |
| Tests | 71 doc tests and 15 integration tests pass |
| Consumers | `cargo check -p foundation_ai` passes |

## Open

### T5 — release the fixed versions

The live 0.0.5 and 0.0.6 carry the consumer defect. Release-plz bumps and
publishes after the merge. Do not bump by hand. Record the fixed versions here
when they are live.

## Next

1. Commit and push the fix.
2. Merge to master so release-plz publishes.
3. Confirm the new versions on crates.io and the isolated build against the
   published tarball.
4. Mark the spec completed and move it to `specifications/completed/`.
