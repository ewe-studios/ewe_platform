# Spec 80 Progress: Platform Prerequisites

_Last updated: 2026-11-21._
_Last updated: 2026-11-15._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — transport prerequisites | ⏳ 0/2, design complete | Moved from spec 83 feature 09 |
| 01 — migration groups | ✅ 1/1, task T25 landed | Moved from spec 83 feature 10 |

## Feature 01 landed (2026-11-21)

Task T25 is implemented in `foundation_db`, `foundation_macros` and the new
`foundation_vfs` crate.

| Area | What landed |
|---|---|
| Types | `MigrationSet`, `MigrationSource` (`Static`, `Embedded`, `Directory(PathBuf)`), `EmbeddedMigration`, `PlannedMigration`, `MigrationConfig`, `RunSummary`, `MigrationError`, `MigrationProblem` |
| Runner | `from_static`, `from_dir`, `add_from_static`, `add_from_dir`, `with_config`, `groups`, async `plan`, async `run_async`, `record_checksums` |
| Rules | Every plan rule of migrations.md section 4.4 and every applied rule of section 4.5, collected in one error |
| Transactions | One batch per migration plus its record row; the runner retries after a primary-key conflict |
| Record table | `_migrations (group_name, number, name, checksum, applied_at)` and the upgrade of the old `id` table, with the 023 repair |
| Layouts | `MigrationLayout`, `MigrationLayoutConfig`, `scan`, `runner_for`, `databases`, `embed_migration_layout!` |
| Macros | `embed_migrations!` and `embed_migration_layout!` with build-time checks and a trybuild case |
| Fleet | `FleetRunner`, `FleetPlan`, `FleetConfig`, `DatabaseResolver`, `FleetReport`, `FleetOutcome` |
| Groups | `DB_CORE` (numbers 1 to 28, 023 included). `AUTH_KEYS` and `AUTH_TRUST` are not added: `foundation_auth` has no SQL files yet. |

The 023 file `src/core/schema/sql/023_create_vectors.sql` is restored and is
part of `DB_CORE`, so the upgrade repair has the SQL that it records.

### Layer change: the VFS traits moved to `foundation_vfs`

`foundation_nativeapis` has an optional dependency on `foundation_db` for its
Cloudflare delta stores. Cargo rejects a cycle even when that optional edge is
off, so `foundation_db` cannot depend on `foundation_nativeapis`. The async VFS
traits and the shared VFS types now live in the new crate `foundation_vfs`.
`foundation_nativeapis` re-exports them, so every old path such as
`foundation_nativeapis::vfs::AsyncVfsFileSystem` still works. `foundation_db`
depends on `foundation_vfs`, and reads directories through it on every target.
The earlier `NoFileSystem` plan is not needed: `plan` always receives a file
system, and `EmptyFs` serves a runner whose groups are all in code or embedded.

| 00 — transport prerequisites | ✅ 2/2, completed 2026-11-21 | Moved from spec 83 feature 09 |
| 01 — migration groups | ⏳ 0/1, design complete | Moved from spec 83 feature 10 |

## What left this specification

The `Env` and side-effect bundle used to be feature 00 here. The owner moved it to
[spec 84](../84-context-injection/requirements.md) on 2026-11-15, because the bundle must
agree with the landed `Ctx` of spec 76, and the two designs had diverged. Spec 84 lands first.

| Old feature | New home |
|---|---|
| 00 env and side effects (`feature.md`, `start.md`) | Deleted; spec 84 replaces them |
| `00-env-and-side-effects/env.md` | `specifications/84-context-injection/env.md` |
| `00-env-and-side-effects/deterministic-simulation-testing.md` | `specifications/84-context-injection/deterministic-simulation-testing.md` |
| 01 transport prerequisites | Feature 00 of this specification |
| 02 migration groups | Feature 01 of this specification |

## Baseline from spec 76

Spec 76 is in progress and has landed `foundation_context::Ctx`, `RpcCtx`, the injected clock
and RNG, and the migration of about 600 `dyn Clock` sites. Each feature in this spec starts with
a baseline task that verifies the 76 state on master and records the remaining delta.

## Why this specification exists

The agent RPC server (83), the proxy (82), the auth library (81) and mutual TLS (87) need the
same transport facts and the same migration groups. Keeping them inside 83 made the proxy
depend on the agent RPC spec. They now have one owner.

## Next steps

1. Run the baseline task of feature 00 against spec 76 and spec 84.
2. Point the consumers at the peer facts once feature 00 lands: spec 87 (`MtlsAuth`), spec 86
   (`CallCredentials`), spec 82 (mTLS enrollment).
3. Add `AUTH_KEYS` and `AUTH_TRUST` to `foundation_auth` when that crate has its SQL files.
2. Implement feature 00 and feature 01. They are independent, so they may run in parallel.
3. Point the consumers at the peer facts once feature 00 lands: spec 87 (`MtlsAuth`), spec 86
   (`CallCredentials`), spec 82 (mTLS enrollment).

## Open questions

- See each feature's open questions. The migration-group questions are in
  `features/01-migration-groups/migrations.md`.

## Documentation correction (2026-11-15, revised 2026-11-21)

- **`MigrationSource` (feature 01, `migrations.md`).** `Directory(PathBuf)` was a
  `cfg(not(wasm))` variant, so the public enum changed shape per target. The variant now exists
  on every target. Issue #556 records it, and it is closed.
- **Directory reading (task T25, 2026-11-21).** The implementation reads a directory through
  `foundation_vfs::AsyncVfsFileSystem` on every target, as `migrations.md` section 4.2.1 says.
  `plan` therefore has no `NoFileSystem` problem: a host always passes a file system, and
  `EmptyFs` covers a runner whose groups are all in code or embedded.
## Documentation correction (2026-11-15)

- **`MigrationSource` (feature 01, `migrations.md`).** `Directory(PathBuf)` was a
  `cfg(not(wasm))` variant, so the public enum changed shape per target. The variant now exists
  on every target; `plan` reports the new `MigrationProblem::NoFileSystem { group }` on wasm;
  the directory readers stay native only.

No code changed. Issue #556 records it, and it is closed.

## 2026-11-21: feature 00 landed

- T3: `build_ctx` keeps the request extension map. The new test
  `request_extensions_reach_the_handler` proves a handler reads it.
- T4: `TlsPeerFacts` and `RawStream::tls_info` expose the peer chain. The
  protocol detector rebuilds the connection context after the handshake. The
  new `tls_peer_info_tests` runs a real mutual-TLS handshake and checks the
  client certificate in `ConnectionContext::tls`.
- Results: 11 dispatch tests pass; the peer-info test passes; clippy and fmt
  are clean for `foundation_netio`, `foundation_http` and
  `foundation_connectrpc`.
- Issue #630: the `ssl-openssl` and `ssl-native-tls` feature sets do not build,
  so their `TlsPeerFacts` impls are unverified. CI never builds them.
- Feature 01 (migration groups) is next.
