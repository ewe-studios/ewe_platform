---
feature: "Every crate reports errors through foundation_errstacks (ErrorTrace over a derive_more enum); no thiserror"
status: "done"
lane: "after both lanes merge (touches crates of both lanes)"
model: "sonnet"
branch: "fix/io-67-errstacks"
crates: ["foundation_wireguard", "foundation_proxy", "foundation_html", "foundation_arrow", "foundation_sshkit", "foundation_toolings", "foundation_codegen", "foundation_nativeapis", "foundation_keychain", "crates/watchers", "crates/domain", "crates/channels", "crates/watch_utils"]
catalogue_rows: [54, 55]
---

# Feature 67: one error standard (`foundation_errstacks`)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

**Owner rule (2026-11-02):** `foundation_errstacks` is the standard for errors. A crate's
fallible API returns `Result<T, ErrorTrace<CrateError>>`, where `CrateError` is a `derive_more`
enum (`Display`, `Error`, `From`), as in `foundation_connectrpc` (`ConnectResult<T>`),
`foundation_nativeapis` VFS (`VfsResult<T>`), `foundation_deployment_platform`
(`DockerResult<T>`) and `foundation_testbed`. `thiserror` is not used.

Starts after both lane PRs merge (crates of both lanes). Branch from `master` then.

## Problem (found in feature 54; re-audited on `spec70/io-hardening-final`, 2026-11-05)

1. These crates declare `foundation_errstacks` but have no `ErrorTrace` use in `src`:
   `foundation_wireguard` (`WgResult<T> = Result<T, WgError>`), `foundation_proxy`,
   `foundation_arrow`, `foundation_sshkit`, `foundation_toolings`.
   `foundation_nativeapis` and `foundation_keychain` already use it; their remaining work is
   the `thiserror` conversion below.
2. These crates declare `thiserror`: `crates/watch_utils` (no use), `foundation_codegen`
   (6 uses), `foundation_nativeapis` (3), `foundation_keychain` (1), `crates/domain` (2),
   `crates/channels` (2), `crates/watchers` (1).
3. `foundation_html` declares neither `foundation_errstacks` nor `thiserror`. Its errors are
   plain enums with plain aliases (`ElementError`/`ElementResult`,
   `ParsingTagError`/`ParsingResult`, `IslandError`, `MarkupStrError`). Convert it: add
   `foundation_errstacks` to the manifest and return `ErrorTrace<…>`.

`foundation_shell` is out of scope. It is an empty placeholder crate: `src/lib.rs` holds only
`add(left, right)`, so it has no error surface. It stays in `ci/packages.toml`. Its backlog
entry says it needs either an implementation (spec 59 moves that work to
`foundation_shell_repl`) or removal.

Out of scope: `backends/foundation_boringtun` is a published fork of Cloudflare's boringtun
(BSD-3-Clause) with upstream authors in its manifest. It keeps `thiserror` and the upstream
shape. The CI guard below skips it by name, with this reason in the code comment.

Measured commands:

```bash
rg -l foundation_errstacks backends/*/Cargo.toml crates/*/Cargo.toml
rg -l '^thiserror' backends/*/Cargo.toml crates/*/Cargo.toml
rg -c 'ErrorTrace' backends/<crate>/src
```

## Design (one commit per crate)

1. The crate's error enum stays (or becomes) a `derive_more` enum; replace `thiserror` derives
   with `derive_more` (`#[display(...)]`, `#[error(source)]`/`#[from]`).
2. Add `pub type <Crate>Result<T> = Result<T, ErrorTrace<<Crate>Error>>;` and return it from the
   crate's public fallible functions. Convert at the boundary with the errstacks helpers the
   other crates use (`IntoErrorTrace`/`.change_context(...)`, `.attach(...)` — read
   `foundation_errstacks/src/result_ext.rs` and copy the pattern from `foundation_connectrpc`).
   Attach useful context where an error crosses a layer (the peer, the path, the key id).
3. Update every caller in the workspace. Where a caller matched on the old enum, match on
   `trace.current_context()` (or the errstacks accessor the other crates use).
4. Remove `thiserror` from each manifest; remove it from the workspace dependencies when no crate
   uses it.
5. Add a CI guard: a check (in `scripts/ci/ci.py` lint step or a small test) that fails when a
   crate manifest names `thiserror`, or names `foundation_errstacks` without using it.

## Tests

- Per crate: one test that an error crossing a layer carries its attached context
  (`format!("{trace:?}")` contains it) and that `current_context()` is the crate's enum variant.
- Existing tests pass with the new types.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in <each crate changed>; do python3 scripts/ci/ci.py run $c --profile dev || exit 1; done
rg -l '^thiserror' backends/*/Cargo.toml crates/*/Cargo.toml   # no output
cargo check --workspace --all-targets --profile dev
```

## Done when

- [x] Every listed crate returns `ErrorTrace<…>`; no `thiserror` left; the CI guard exists.
