# Issue 1: Deprecated `cargo.buildScripts.enable` breaks rust-analyzer

## Symptoms

- Zed shows no type hints, no inline diagnostics, no go-to-definition for Rust code
- `Zed.log` contains repeated errors:
  ```
  cargo/buildScripts/enable: unexpected field
  cargo/runBuildScripts: unexpected field
  ```
- rust-analyzer reports `server cancelled the request` for diagnostics — it rejects the config and stops processing

## Root Cause

The project-level `.zed/settings.json` uses the **deprecated nested form** for enabling build script execution:

```json
// .zed/settings.json — BROKEN
"lsp": {
  "rust-analyzer": {
    "initialization_options": {
      "cargo": {
        "allFeatures": false,
        "loadOutDirsFromCheck": true,
        "buildScripts": {       // ← this nested form was removed
          "enable": true
        }
      }
    }
  }
}
```

rust-analyzer's config schema was flattened. The correct top-level field is `cargo.runBuildScripts` (boolean), and the nested `buildScripts.enable` is rejected as an unknown field. When rust-analyzer receives unrecognized config keys, it logs `unexpected field` warnings and cancels downstream requests — meaning **no type inference, no diagnostics, no completions**.

Zed also internally generates a `cargo/runBuildScripts: unexpected field` warning from its own translation layer, compounding the issue.

## Fix Applied

Replace the nested `buildScripts.enable` with the flat `runBuildScripts` field:

```json
"cargo": {
  "allFeatures": false,
  "loadOutDirsFromCheck": true,
  "runBuildScripts": true
}
```

Alternatively, `runBuildScripts` can be omitted entirely — it defaults to `true` when `procMacro.enable` is also `true`, which it is in this project.

**File changed:** `.zed/settings.json` (project-level, not `~/.config/zed/settings.json`)

## How to verify

After the fix, restart Zed (or run `zed: restart language servers` from the command palette). Check:
1. No more `unexpected field` lines in `~/.local/share/zed/logs/Zed.log`
2. Inlay hints appear on function return types and parameters
3. Hover shows type documentation
4. Go-to-definition works

## Reference

- rust-analyzer config docs: <https://rust-analyzer.github.io/manual.html#configuration>
- Zed rust-analyzer docs: <https://zed.dev/docs/languages/rust>
