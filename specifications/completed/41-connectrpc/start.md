---
workspace_name: "ewe_platform"
spec_directory: "specifications/41-connectrpc"
this_file: "specifications/41-connectrpc/start.md"
feature_name: "overview"
created: 2026-07-03
updated: 2026-07-03
---

# Start: foundation_connectrpc — ConnectRPC for the EWE Platform

## Workflow

1. Read `plan.md` (goal, key decisions, phasing, resolution record).
2. Read the decision docs **in order 00 → 14** — they are the single source of truth.
   Every doc's `## Decided Details` section is normative decided behaviour (stable labels
   like P6/R1/RS6 are cited across docs). There are **no open questions**: anything titled
   "Open Questions" or "Review-Gap Coverage" would be a regression.
3. Work the features in `features/` in numeric order within a phase, honoring each
   feature's `depends_on`. Each feature has its own `start.md`; `feature.md` is the work
   unit — scope, acceptance criteria, pointers — never a substitute for the decisions.
4. Repo rules that bind here: rust-clean-code skill before writing Rust; finish each
   feature 100% (no partial scaffolding); tests in `tests/`; `#[valtron_test]` for pool
   tests (never `#[test]` + `#[serial]`); tracing macros not eprintln; async-canonical
   (sync wraps async via valtron off-pool only); no silent fallback defaults.

## Phase map (feature numbers)

| Phase | Features | Delivers |
|---|---|---|
| 1a — valtron core        | 01–03           | parking futures, `Pipe<T>`/FramePipe, async macros (supersedes spec-50's features) |
| 1b — netio/http enablers | 04–11           | IncrementalDecoder, h1 part-iterators + trailers, lowercase headers, pushable body, Extensions/ConnectionContext, AsRawFd, reactor parking, graceful drain |
| 1c — connectrpc crate    | 12–28           | errors, codecs, compression/buffers, envelope, Ctx, seam, interceptors, Connect + gRPC-Web on h1, router, h1 transport, client, auth, codegen (proto + code-first), Phase-1 conformance |
| 2 — HTTP/2 + gRPC        | 29–32, 40–42    | owned http2 module (substrate → both multiplexers + ALPN/h2c×3 → tuning), full gRPC, shared reactor + io_uring readiness backend |
| 3 — HTTP/3               | 33–35, 43       | quinn-proto QUIC backend, http3 module, transport integration; io_uring completion mode |
| 4 — WebSocket (last)     | 36–39           | resumable decoder, server task, Depends read model, WebSocketTransport (bidi on h1) |

Deferred (not features): iroh P2P (Decision 01 — build (a) on demand, never (b)
speculatively); per-worker uring rings (re-evaluated inside feature 43 per D14 OQ#14.1).

## Supersession note

Decision 00 absorbed `specifications/50-valtron-async-readiness`; features **01–03 here
supersede** spec-50's `features/01-waker-queue-bridge`, `03-async-valtron-macros`, and
(as explicitly superseded design) `02-readiness-source-seam` — the ReadinessSource/OnceLock
abstraction is NOT built; the nativeapis reactor + `RegisteredFd: EventReadiness` is the
seam (Decision 00 Level 2 note, feature 10).

## Verification spine

- Feature-level acceptance criteria first; then the conformance harness (feature 28 for
  Phase 1, extended by 31/35) is the arbiter for wire behaviour.
- Decision docs win over feature files on any discrepancy; fix the feature file when found.

_Created: 2026-07-03_
