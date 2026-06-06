# LEARNINGS

## Past Discoveries and Mistakes

### 2026-06-06: Spec Review — Async/Sync Pattern Corrections

**Issue 1: Sync/async direction was backwards in Feature 14 (Cedar).**
The spec showed sync implementations bridged to async via `spawn_blocking`, but the
correct pattern is async-first with valtron sync wrappers. Async contains the I/O logic;
sync bridges to async via valtron `from_future` + `collect_one`.

**Issue 2: Tokio was incorrectly referenced.**
Tokio has no role in the valtron bridging pattern. Both native and wasm sync wrappers
use valtron as the execution engine. Tokio references were replaced with valtron patterns.

**Issue 3: QueryStream/AsyncQueryStream parity mismatch.**
`foundation_db`'s `QueryStore` (sync) returns a valtron StreamIterator while
`AsyncQueryStore` (async) returns `Vec<T>`. This defeats streaming for async.
Feature 00 was created to fix this — `AsyncQueryStream` wraps valtron's
StreamIterator as `futures_core::Stream` for parity.

**Issue 4: foundation_nativeapis vs foundation_db roles unclear.**
Clarified: foundation_db = database storage (Turso, libsql, D1).
foundation_nativeapis = filesystem operations. CredentialStorage primarily uses
foundation_db; foundation_nativeapis only for file-based caching.

**Issue 5: WASM git HTTP fallback was the wrong default.**
gix supports WASM via gix-protocol with custom HTTP transport. The design now uses
a pluggable `PolicyFetcher` trait with `GitPolicyFetcher` (gix-based) as primary
and `HttpPolicyFetcher` as fallback.

**Issue 6: Handler types needed clarification.**
foundation_http has two handler traits: `ServeWriter` (sync, native) and `WebServe`
(async, wasm). Features 09 and 12 were updated to describe both and explain how
ServeWriter bridges to async via valtron.

### 2026-06-06: Git Storage Capability Split

Git operations belong in `foundation_nativeapis` as a generic `PolicyFetcher` trait.
`foundation_cedar` consumes this trait — it knows Cedar paths and parsing, not git.

