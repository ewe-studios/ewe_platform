# Issue 2: rust-analyzer / rustc version mismatch

## Symptoms

- Type hints are missing or incomplete even after config fields are fixed
- rust-analyzer produces spurious errors that `cargo check` does not
- Go-to-definition skips generic types or associated types
- Hover shows `unknown` for types that `cargo check` resolves fine

## Root Cause

There is a **~4 month version gap** between the rust-analyzer binary and the rustc toolchain it analyzes:

| Component | Version | Date | Source |
|---|---|---|---|
| rust-analyzer | `0.3.2753` | Jan 12, 2026 | `~/.local/bin/rust-analyzer` (standalone) |
| rustc | `1.97.0-nightly` (67bcaa9c4) | May 1, 2026 | rustup nightly toolchain |

rust-analyzer is tightly coupled to rustc's internal representation of types, traits, and HIR. A version mismatch means:
- rust-analyzer doesn't understand new compiler internals the code relies on
- Type inference fails silently for constructs introduced after rust-analyzer's date
- Diagnostics are stale or wrong

### Why this happened

The project uses `rust-toolchain.toml` with `profile = "minimal"`, which does **not** include the `rust-analyzer` component:

```toml
[toolchain]
channel = "nightly"
profile = "minimal"
```

Meanwhile, the global Zed settings (`~/.config/zed/settings.json`, symlinked from `dotarch/config/zed/settings.json`) pin a hardcoded path:

```json
"lsp": {
  "rust-analyzer": {
    "binary": {
      "path": "rust-analyzer",   // resolves to ~/.local/bin/rust-analyzer
    }
  }
}
```

That standalone binary at `~/.local/bin/rust-analyzer` was installed manually at some point and has never been updated since January 2026. Meanwhile, `rustup component list` confirms `rust-analyzer-x86_64-unknown-linux-gnu` **is available** for the nightly toolchain — it just isn't installed.

## Fix Applied

1. **Install rust-analyzer via rustup** so it matches the active toolchain:
   ```bash
   rustup component add rust-analyzer --toolchain nightly-x86_64-unknown-linux-gnu
   ```

2. **Remove the hardcoded binary path** from global Zed settings so Zed auto-discovers the rustup-managed binary:
   ```json
   // ~/.config/zed/settings.json (global)
   // REMOVE this block entirely:
   "lsp": {
     "rust-analyzer": {
       "binary": {
         "path": "rust-analyzer",
         "arguments": []
       }
     }
   }
   ```

   Without an explicit `binary.path`, Zed uses `rustup run nightly rust-analyzer` to find the correct version for each project's toolchain.

3. **Update `rust-toolchain.toml` profile** from `minimal` to `default`:
   ```toml
   # rust-toolchain.toml
   profile = "default"   # was "minimal"
   ```

   The `default` profile includes `rustfmt` and `clippy` (already explicitly listed as components, but now guaranteed by profile) and makes the toolchain self-documenting for contributors. The explicit `components` list is updated to add `rust-analyzer`:
   ```toml
   components = ["rustfmt", "rustc-dev", "rust-src", "rust-analyzer"]
   ```

4. **Install rust-analyzer component** for the current nightly toolchain (one-time for existing installs):
   ```bash
   rustup component add rust-analyzer --toolchain nightly-x86_64-unknown-linux-gnu
   ```

5. **Remove the stale standalone binary** (optional but recommended to avoid future confusion):
   ```bash
   rm ~/.local/bin/rust-analyzer
   ```

## How to verify

After the fix:
1. `rustup which rust-analyzer` should resolve to `~/.rustup/toolchains/nightly-*/bin/rust-analyzer`
2. `rust-analyzer --version` should show a version dated May 2026 or later (matching rustc's May 1 date)
3. In Zed, run `zed: restart language servers` and check `Zed.log` — the rust-analyzer version in the startup line should match the nightly toolchain
4. Type hints and go-to-definition should work for all code, including nightly-only features

## Preventing recurrence

- Always install rust-analyzer as a rustup component, never as a standalone binary
- Keep `rust-toolchain.toml` profile at `default` (changed from `minimal`), and ensure `components` includes `rust-analyzer`:
  ```toml
  [toolchain]
  channel = "nightly"
  profile = "default"
  components = ["rustfmt", "rustc-dev", "rust-src", "rust-analyzer"]
  ```
- Run `rustup update` periodically to keep all components in sync
