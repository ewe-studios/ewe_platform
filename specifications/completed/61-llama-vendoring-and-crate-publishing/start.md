# Start Here — 61: llama.cpp vendoring and crate publishing

Read `requirements.md` first. Work the phases in order; each ends in a commit.

**Status (2026-11-20): completed.** Phases 1 to 5 are done. The consumer tarball
builds with no environment, the CI guard is in, and llama.cpp is at `b12563`.
The normal release flow publishes the fixed versions after the merge.

## Phase 1 — restore the vendoring

1. `git mv tools/llama.cpp infrastructure/llama-bindings/llama.cpp`
2. `build.rs`: default `LLAMA_DIR` to `CARGO_MANIFEST_DIR/llama.cpp`, keeping the
   env override.
3. `rg -n "tools/llama.cpp|LLAMA_DIR|TOOLS_DIR" --hidden` and fix every hit —
   scripts, Dockerfiles, CI, the `llama-server` copy step.
4. Full rebuild, then `cargo publish --dry-run -p infrastructure_llama_bindings`.
   **The dry run is the acceptance test**, not the build.
5. Record the tarball size from `cargo package --list` in `requirements.md` R3.
6. Commit + push.

## Phase 2 — bump and cascade

Version bumps land as one commit before any publish.

- `foundation_repl` → 0.2.1
- `foundation_core` → 0.0.5, then cascade every dependent's `version =` spec

Check **both** dependency spellings when cascading:

```bash
rg -n 'foundation_core' --glob 'Cargo.toml'
for f in $(git ls-files '*Cargo.toml'); do
  awk -v F="$f" '/^\[(dependencies|dev-dependencies|build-dependencies)\.foundation_core\]/{b=1;next} /^\[/{b=0} b && /^version/{print F": "$0}' "$f"
done
```

Then re-resolve deliberately so a stale lockfile cannot hide a bad manifest:

```bash
cargo update --workspace --dry-run   # or: cargo metadata >/dev/null
```

Commit + push.

## Phase 3 — publish, in this order

Dry-run each before the real thing. Never `--no-verify`.

1. `foundation_repl` 0.2.1
2. `foundation_core` 0.0.5
3. `cargo yank -p infrastructure_llama_cpp --version 0.0.1`
4. `infrastructure_llama_bindings` 0.0.2, then `infrastructure_llama_cpp` 0.0.2
5. `foundation_ai` — confirm every path dep is on crates.io first
6. The remaining dependents

Verify each landed before starting the next:

```bash
curl -s -A check https://index.crates.io/fo/un/foundation_repl | tail -1
```

## Phase 4 — make the tarball build for a consumer

The published 0.0.5 fails with:

```
build.rs:343: get TOOLS_DIR environment: NotPresent
```

1. `infrastructure/llama-bindings/build.rs`:
   - `LLAMA_DIR` unset → `CARGO_MANIFEST_DIR/llama.cpp`.
   - Remove the `TOOLS_DIR` `expect`; `_tools_directory` is unused.
   - `EMSDK_DIR` is optional; add `upstream/emscripten` only when it is set.
2. Package, extract outside the repository, unset the build env, and check:

   ```bash
   cargo package -p infrastructure_llama_bindings --allow-dirty
   rm -rf /tmp/pkg && mkdir -p /tmp/pkg
   tar xzf target/package/infrastructure_llama_bindings-*.crate -C /tmp/pkg
   cd /tmp/pkg/infrastructure_llama_bindings-*
   env -u LLAMA_DIR -u TOOLS_DIR -u EMSDK_DIR cargo check --lib
   ```

3. Add the same isolated build to `scripts/ci/package_workspace.py` (R5).
4. Commit and push. Let release-plz bump and publish. Record the fixed version
   in `requirements.md`.

## Phase 5 — update llama.cpp (done)

1. `git -C tools/llama.cpp fetch --tags && git -C tools/llama.cpp checkout b12563`.
2. `cargo check -p infrastructure_llama_bindings`; migrate the wrapper to the
   new API (see R6).
3. `cargo test -p infrastructure_llama_cpp` and `cargo check -p foundation_ai`.
4. Package and run the isolated build again. CMake may need more files; the
   include list now takes `**/*.in`, `**/*.cmake` and `LICENSE`.

## Done when

- The extracted tarball builds with `LLAMA_DIR`, `TOOLS_DIR` and `EMSDK_DIR`
  unset. **Done.**
- `infrastructure_llama_cpp 0.0.1` is yanked, and a fixed version of both
  crates is live. **Release pending (T5).**
- The CI check extracts and builds one tarball. **Done.**
- `tools/llama.cpp` is at the latest release tag and the wrapper compiles.
  **Done at `b12563`.**
- Anything learned goes in `learnings.md` here.
