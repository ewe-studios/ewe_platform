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

### T5 — release the fixed versions

Done in the tree. The live 0.0.5 and 0.0.6 still carry the consumer defect.
Release-plz bumps and publishes through the normal release PR after the merge.
The release PR records the live versions.

## Status

The implementation is complete. The spec is completed and moved to
`specifications/completed/`. The release follows the repository flow.
