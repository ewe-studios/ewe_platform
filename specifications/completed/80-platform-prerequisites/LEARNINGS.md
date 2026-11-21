# LEARNINGS — Platform Prerequisites

## Why the seams left spec 83 (2026-11-14)

Spec 83 bundled four missions. The Env bundle, the transport facts and the migration groups
change shared crates that the proxy also uses. Keeping them in the agent RPC spec made the proxy
spec depend on the agent RPC spec, which is the coupling the design forbids.

The features now have one owner. The agent RPC server (83), the proxy (82), the auth library
(81) and mutual TLS (87) all depend on this spec.

## Why the `Env` bundle left this specification again (2026-11-15)

The owner reviewed six conflicts on 2026-11-15. One of them was the `Env` design here: it
described a second clock, an `IdSource` with two meanings and a bundle that did not match the
`Ctx` that spec 76 had already landed. A consumer that read both designs could not tell which
one was true.

The owner direction: "Ctx should be the main means of all of such injection... lets create a
small spec (featureless) that adds the bits into foundation_context then others get it by
default."

So the bundle is [spec 84](../84-context-injection/requirements.md) now. It is featureless,
it extends `Ctx`, and it owns the one `Timer`, `EntropySource` and `IdSource`. This spec keeps
only what is about the transport and the database.

The lesson: a bundle of injected effects belongs next to the type that carries it, not in a
specification about platforms. The two designs drifted while they had different owners.

## Do not repeat spec 76 or spec 84

Spec 76 already built `foundation_context::Ctx` with the clock, config and a seeded RNG, and
moved about 600 sites. Spec 84 adds the `Env` entry.

This spec adds the remaining delta only:

- connectrpc request extensions, and the deadline clock from `Ctx`.
- foundation_http TLS peer facts.
- Migration groups and `FleetRunner`.

A second context type is a defect. A second clock is a defect. Verify the baseline before
writing code.

## Moved design files

- `features/01-migration-groups/migrations.md` — the migration-group design.

The Env design and the deterministic-testing document are in
`specifications/84-context-injection/`.

## A public type keeps one shape on every target (2026-11-15)

`MigrationSource::Directory(PathBuf)` was a `#[cfg(not(target_family = "wasm"))]` variant, so
the public enum changed shape per target. The owner asked for every instance of that pattern to
be checked across the split specifications (spec 77 section 6.0, the part wrappers).

The variant now exists on every target. A `PathBuf` is portable; only reading a directory is
not. `MigrationRunner::plan` reports the new `MigrationProblem::NoFileSystem { group }` when a
wasm host is given a `Directory` group. The readers stay gated, because they are capabilities:
`MigrationLayout::scan`, `from_dir` and `add_from_dir` are native only.

The rule: a `cfg` goes on a private inner type, a method, a module or a dependency. It never
changes the variants of a public enum.

## A Cargo cycle hides behind an optional dependency (2026-11-21)

`migrations.md` gave `foundation_db` a dependency on `foundation_nativeapis` to name
`AsyncVfsFileSystem`. That dependency cannot exist. `foundation_nativeapis` declares
`foundation_db` as an **optional** dependency for `vfs-d1`, `vfs-r2` and `vfs-fjall`. Cargo
rejects the package cycle even when no feature turns the optional edge on, and even when the
edge is target-gated.

The fix is a small crate, `foundation_vfs`, that holds `async_traits.rs`, `error.rs` and
`types.rs`. `foundation_nativeapis` re-exports them, so no public path changes.
`foundation_db` depends on `foundation_vfs`. The tests still use `MemoryFs` from
`foundation_nativeapis` as a dev-dependency; a dev-only cycle is allowed.

The lesson: before a design adds an edge between two large crates, read both `Cargo.toml`
files for optional reverse edges. Cargo checks the package graph, not the enabled features.

## Other findings from task T25 (2026-11-21)

- **Turso `execute` rejects a statement that returns rows.** A test migration that used
  `SELECT 1` failed with "unexpected row during execution". Migrations are data definition
  language (DDL) or data manipulation language (DML) statements; the statement splitter sends
  each statement through `execute_batch_statements`.
- **D1 batch errors lost their text.** `D1Store::execute_batch_statements` reported only
  "one or more statements failed". The migration runner needs "UNIQUE constraint failed" to
  tell a primary-key conflict from a real SQL error, so the message now includes the API error
  messages.
- **trybuild builds a scratch crate in `target/tests/trybuild`.** Its `CARGO_MANIFEST_DIR` is
  not the crate under test, and an environment variable set in the test did not reach the
  proc macro. The macro now tries the manifest directory first and then walks up to the
  workspace root. The test file names the data directory from the workspace root.
- **`proc-macro-crate` reports `FoundCrate::Itself` for an integration test** of the same
  package, where `crate` is the test target, not the library. The macro compares
  `CARGO_CRATE_NAME` with the package name and emits `::foundation_db` for the test target.
- **The local D1 harness works.** `wrangler dev --local` on port 8789 with
  `CF_INTEGRATION_TEST=1` runs the migration tests against Cloudflare D1 (transaction rollback
  and idempotent apply).
## 2026-11-21: transport facts

- `build_ctx` built a fresh `Extensions` map instead of using the request's.
  The request owns `Option<Extensions>`; move a clone into
  `RequestContext::for_dispatch` or the handler never sees the value.
- The rustls server handshake is lazy. `acceptor.accept` returns before the
  handshake, so the peer chain is empty at accept time. Capture it after the
  first successful protocol peek, when the handshake has completed.
- The optional `ssl-openssl` and `ssl-native-tls` features are not in the CI
  runs, so they rotted. A feature that CI never builds is not supported.
  Issue #630 records both.

