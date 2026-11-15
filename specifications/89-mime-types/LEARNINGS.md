# Learnings: Spec 89, Feature 01, MIME Types

_Written 2026-11-16 in ASD-STE100 Simplified Technical English (STE)._

## What landed

| Part | State |
|---|---|
| `foundation_macros::include_json!` and `include_yaml!` | done, with tests |
| `foundation_nostd` `phf` re-export | done |
| `foundation_netio::shared::mime` | done, with tests and the wasm check |
| `foundation_netio/data/mime.json` and the generator | done |
| The two consumers (`foundation_http`, `foundation_browser`) | done |
| The agreement test with `new_mime_guess` | done |
| Version bumps | done: `foundation_nostd` 0.0.10, `foundation_macros` 0.0.22, `foundation_netio` 0.0.15 |

## The duplicate-key build proof

The duplicate-key case is a compile-time error, so it is not a test in `tests/`.
A temporary probe crate under `/tmp/mime-macro-probe` has one binary for each
bad input. Each binary uses a fixture in
`backends/foundation_macros/tests/fixtures/`.

The command for the JSON duplicate key:

```bash
cd /tmp/mime-macro-probe
env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND -u CARGO_UNSTABLE_CODEGEN_BACKEND \
  cargo build --bin dup_json
```

The result line:

```text
error: include_json! finds a duplicate key "js" in /Users/darkvoid/Dev/ewe_platform/.claude/worktrees/spec-89-mime-types/backends/foundation_macros/tests/fixtures/duplicate_key.json at line 3 column 6
```

The other inputs give these messages:

| Binary | Message |
|---|---|
| `dup_yaml` | `error: include_yaml! finds a duplicate key "js" in .../duplicate_key.yaml` |
| `array` | `error: include_json! needs a JSON object of strings in .../array.json at line 2 column 2` |
| `non_string` | `error: include_json! needs a string value for "js", found a number at line 3 column 1` |
| `nested` | `error: include_json! needs a string value for "js", found an object at line 5 column 1` |
| `nested_yaml` | `error: include_yaml! needs a string value for "js", found a mapping` |
| `sequence_key` | `error: include_yaml! needs a scalar key in .../sequence_key.yaml` |
| `missing` | `error: include_json! cannot read .../no_such_file.json (relative to /private/tmp/mime-macro-probe)` |
| `two_args` | `error: include_json! takes exactly one string literal path` |

The environment variables are removed because this shell sets
`CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift`, which stable cargo does not
accept. The probe crate builds outside the workspace.

## Root causes and dead ends

### The macro emits `compile_error!` with no semicolon

The first version emitted `compile_error!(...);`. The macro expands in
expression position, so the compiler also reported "macro expansion ignores
`;` and any tokens following". The extra message hid the real one. The fix
emits the token with no semicolon.

### `phf_codegen` writes `::phf` by default

`phf_codegen::Map::build()` writes a bare `::phf::Map`. A caller of the macro
does not depend on `phf` directly, so the literal would not resolve. The macro
sets `phf_path("::foundation_nostd::phf")`, and `foundation_nostd` re-exports
the crate. `phf_codegen` also panics on a duplicate key. The macro checks for a
duplicate first, through a `MapAccess` visitor, so the user sees a compile
error and not a panic.

### `serde_json` keeps the last pair of a repeated key

A parse to `serde_json::Value` loses a repeated key. The macro parses through
`MapAccess` and keeps a set of seen keys. `serde_yaml_ng` does the same, so the
YAML macro uses the same check.

### `serde_yaml_ng` implements the deserializer trait on the value

`serde_yaml_ng::Deserializer` implements `serde::Deserializer` on the value, not
on `&mut`. The YAML call consumes the deserializer. The JSON call uses `&mut`,
because it must call `end()` to refuse text after the object.

### A first search missed two dependency forms

The version-bump search first matched only `foundation_x = { ..., version = "…" }`
lines. It missed:

- a table form such as `[dependencies.foundation_macros]` with a separate
  `version = "0.0.21"` line;
- a target table such as
  `[target.'cfg(not(target_family = "wasm"))'.dependencies.foundation_macros]`.

A blanket replace also changed a `[dependencies.foundation_core]` version by
mistake. The safe method tracks the current section header in `awk` and
replaces only a version line whose section names the crate. Run it again after
the bump to prove that no old requirement remains.

## Measured facts that changed

| Fact | The plan said | The measurement says |
|---|---|---|
| Media types in mime-db 1.54.0 | 2,601 | 2,522 |
| Extensions from the rule | 1,246 | 1,239 |
| Ambiguous extensions | 56 | 48 |

The download is the tag `v1.54.0` and the npm package `mime-db@1.54.0`, and both
give 2,522. The plan counted a different revision, or it used a different file.
`requirements.md` and `progress.md` now hold the measured numbers. Issue #591
records the error.

## Findings that stay open

| Issue | Severity | Finding |
|---|---|---|
| #591 | low | The plan's mime-db counts were wrong. Fixed in this branch. |
| #592 | medium | `foundation_platform/src/responder.rs` keeps a second extension map. It is outside this feature's owned paths. |
| #593 | medium | Rule R7 maps `mp4` to `application/mp4`, because both `application/mp4` and `video/mp4` are IANA and the rule takes the alphabetically first type. |

## Other notes

- The #578 regression test (a request for a `.mjs` file that reads the
  `Content-Type` line) is in commit `27ed28a81` on branch
  `spec/88-executors`. It is not on `origin/master`, so this branch cannot run
  it. The new table gives `mjs` the type `text/javascript`, and
  `tests/mime_tests.rs` proves it.
- The `ico` row is now `image/vnd.microsoft.icon`, the IANA type. The old
  hand-written map used `image/x-icon`.
- The `js` row is now `text/javascript`, the IANA type. The old map used
  `application/javascript`.
- `mime_guess` resolves `mjs` to `application/javascript`, so the agreement
  test leaves `mjs` out. That difference is the reason for the shared table.
- Extensions in mime-db can hold an underscore (`x_b`, `x_t`, `fe_launch`).
  The data test allows a lower-case letter, a digit, `-` and `_`, and it
  refuses a dot.
