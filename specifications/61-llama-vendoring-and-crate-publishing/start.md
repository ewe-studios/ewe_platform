# Start Here — 61: llama.cpp vendoring and crate publishing

Read `requirements.md` first. Work the phases in order; each ends in a commit.

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

## Done when

- `cargo publish --dry-run` passes for every crate in the chain.
- `infrastructure_llama_cpp 0.0.1` is yanked and 0.0.2 is live.
- Every crate listed in R4 is published at its new version.
- Anything learned goes in `LEARNINGS.md` here, following spec 49's file.
