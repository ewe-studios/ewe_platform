# Progress - 00a Foundation DB

_Last updated: 2026-04-12_

**Status:** ✅ Complete — 32 / 32 tasks (100%)

Unified storage backend with Turso sync backend, Cloudflare D1/R2,
in-memory, and JSON file fallbacks. Valtron-only async, zero tokio.

## Done

- [x] All storage backends compile
- [x] `StorageProvider` dispatches uniformly (KeyValue, Query, RateLimiter, Blob)
- [x] In-memory backend with zeroizing
- [x] Turso backend with migrations
- [x] libsql backend
- [x] JSON file backend with atomic writes
- [x] Encryption integration for sensitive columns
- [x] D1 backend (KV, Query, RateLimiter, Blob via HTTP)
- [x] R2 backend (Blob via HTTP, native binary storage)
- [x] Cleanup operations + integration tests
- [x] `cargo test --package foundation_db` — 70 tests passing
- [x] `cargo clippy --package foundation_db -- -D warnings` — zero warnings
- [x] `foundation_auth` wired through `foundation_db::StorageProvider`
- [x] Per-backend credential-store wrappers collapsed into one
  `CredentialStorage` — backend is chosen when the provider is built
- [x] Local auth tests exercise a real SQLite file via the Turso provider
  (tempdir + `CredentialStorage::turso`) instead of the in-memory shim
- [x] `cargo test --package foundation_auth` — 16 / 16 passing
- [x] `cargo clippy --package foundation_auth -- -D warnings` — zero warnings

## Collateral Fix

`StorageProvider::new` now calls `init_schema()` automatically for Turso
and libsql backends, so consumers receive a ready-to-use provider without
a separate setup step. Previously callers had to call `init_schema()` on
the backend directly, which wasn't even reachable through `StorageProvider`.

## Simplification

Removed `TursoCredentialStore` and `MemoryCredentialStore`. Both were
identical — each just drained Valtron streams from `StorageProvider`.
One generic `CredentialStorage` now covers every backend
`foundation_db` supports, with constructors `::new(provider)`,
`::turso(url)`, and `::memory()`.
