---
feature: "Transport Prerequisites"
description: "Two small library fixes the hosts need: connectrpc keeps request extensions through dispatch and takes its deadline clock from Ctx; foundation_http reports the TLS peer certificates after the handshake."
status: "completed"
priority: "high"
depends_on: ["specifications/84-context-injection"]
part_of: "specifications/83-agent-rpc-serving-system/agent-rpc-architecture.md"
estimated_effort: "small"
created: 2026-10-27
last_updated: 2026-11-21
author: "Main Agent"
tasks:
  completed: 2
  uncompleted: 0
  total: 2
  completion_percentage: 100
---

# Transport Prerequisites

_Split out of feature 07 on 2026-10-27 (owner direction: each feature is focused on one
thing). Written in ASD-STE100 Simplified Technical English (STE). Code and
identifiers stay as they are._

The design files of this feature are in this directory. The shared architecture, glossary and
owner decisions are in [`agent-rpc-architecture.md`](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md). Task
IDs keep their old numbers, so references such as "task T3" stay valid. The
[glossary](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md#2-glossary) explains each term.

## 1. What this feature does

Two small fixes in shared crates. `foundation_connectrpc` keeps request extensions through dispatch (the `build_ctx` fix), so client auth can read connection facts, and computes deadlines from the `Ctx` clock. `foundation_http` puts the peer certificates in `ConnectionContext::tls` after the TLS handshake, so mTLS auth can read them.

Part of the agent-rpc proof of concept.

### Baseline (2026-11-21)

| Area | Measured state | Remaining delta |
|---|---|---|
| Request extensions | `SimpleIncomingRequest.extensions` holds the extension map, but `build_ctx` passes `Extensions::new()` to `RequestContext::for_dispatch` | Move the request's map in; add a dispatch test |
| Deadline clock | `build_ctx` computes `app_ctx.clock().now() + d`; `DeadlineTimer::sleep_until` takes a `Ctx` | None |
| TLS peer facts | `ConnectionContext.tls` and `TlsInfo` exist; the accept loop builds the context with `tls: None` before the handshake | Capture the peer certificates at the handshake and fill `tls` |
| `CallHooks::with_clock` | Not present; the timer takes the `Ctx` instead | None; keep the current timer API |

## 2. What to read

| Design file | Sections |
|---|---|
| [mtls.md](../../../87-mutual-tls/mtls.md) | section 3.4 (the host must report the peer certificate) |
| [auth.md](../../../81-foundation-auth/features/00-foundation-auth/auth.md) | section 8.2 (the interceptor needs the `build_ctx` fix) |
| [84/requirements.md](../../../84-context-injection/requirements.md) | section 3 (the `Env` bundle; the clock is `ctx.clock()`) |
| [hosts.md](../../../83-agent-rpc-serving-system/features/04-native-host-and-dst/hosts.md) | section 4.3 (listeners and TLS) and the `foundation_connectrpc` row of section 5.4 |
| [agent-rpc-architecture.md](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md) | architecture (section 3), owner decisions (section 11) |
| [agent-rpc-architecture.md](../../../83-agent-rpc-serving-system/agent-rpc-architecture.md) | section 13: the workflow and the rules to check before each push |

## 3. Tasks

| Task | What | Main files | Done when |
|---|---|---|---|
| T3 | connectrpc: keep request extensions through dispatch (`build_ctx` fix); `CallHooks::with_clock` | `foundation_connectrpc/src/shared/router/dispatch.rs` | dispatch tests pass; deadlines follow the `Ctx` clock |
| T4 | `ConnectionContext::tls` holds the peer certificates after the handshake | `foundation_http` | `tls_peer_info_tests` pass |

## 4. Done when

The dispatch tests pass and deadlines follow the `Ctx` clock. `tls_peer_info_tests` pass.

## 5. Landed (2026-11-21)

- **T3.** `build_ctx` moves a clone of `SimpleIncomingRequest.extensions` into
  `RequestContext::for_dispatch`. A value in that map now reaches the handler.
  New test: `request_extensions_reach_the_handler` in
  `foundation_connectrpc/tests/router_dispatch_tests.rs` (11 tests pass).
- **T4.** `TlsPeerFacts` reports the peer chain on the rustls, openssl and
  native-tls server streams. `RawStream::tls_info` reads it. After the first
  successful protocol peek, `ProtocolDetectHandler` rebuilds the
  `ConnectionContext` with `tls: Some(..)`.
  New test: `tls_peer_info_tests.rs` runs a mutual-TLS server and client, and
  asserts the handler sees the client certificate.
- The optional openssl and native-tls feature sets do not build on the current
  tree, so their `TlsPeerFacts` impls are unverified. Issue #630 records both.

## 6. Pull requests

One PR (part of PR group C).
